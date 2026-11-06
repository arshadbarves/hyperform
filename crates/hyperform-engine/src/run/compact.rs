//! Summary compaction: `PreCompact` hooks, then the fast model summarizes
//! the older history and `[summary] + recent tail` becomes the working
//! set. Shared by automatic compaction and the manual `compact` command.
//! While a work plan is attached, the main agent's summary also yields
//! candidate discoveries and is followed by its task packet; a task-bound
//! helper's summary is followed by its task's packet.

use hyperform_context::{
    apply_summary, estimate_message, estimate_messages, plan_summary, render_for_summary,
    summary_message,
};
use hyperform_llm::{ModelRequest, SystemBlock};
use hyperform_prompts::auxiliary::COMPACT;
use hyperform_protocol::{
    AgentId, CompactionMarker, CompactionTrigger, ContentBlock, Message, now_ms,
};
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::side::side_request;
use super::sink::TranscriptSink;
use crate::error::EngineError;
use crate::hooks::{HookEvent, HookInput, run_hooks};
use crate::session::SessionCore;
use crate::settings::models::fast_model;
use crate::work::{CompactWork, helper_packet, split_discoveries};

/// Messages kept verbatim after the summary, when a split allows it.
const KEEP_RECENT_MESSAGES: usize = 4;
/// Characters of each tool result shown to the summarizer.
const MAX_CHARS_PER_RESULT: usize = 2_000;
const SUMMARY_MAX_TOKENS: u32 = 8_192;

/// Everything one compaction needs.
pub(crate) struct CompactJob<'a> {
    pub core: &'a SessionCore,
    pub sink: &'a dyn TranscriptSink,
    pub agent: &'a AgentId,
    pub trigger: CompactionTrigger,
    pub instructions: Option<&'a str>,
    pub cancel: &'a CancellationToken,
}

/// The new working set, or `None` when the history is too short to
/// summarize. The sink hears `compacting` once there is something to
/// summarize, before the summary request.
pub(crate) async fn summarize(
    job: &CompactJob<'_>,
    working: &[Message],
    tokens_before: u64,
) -> Result<Option<Vec<Message>>, EngineError> {
    let core = job.core;
    let instructions = job
        .instructions
        .map(str::trim)
        .filter(|text| !text.is_empty());
    let input = HookInput::new()
        .target(job.trigger.label())
        .with("trigger", json!(job.trigger.label()))
        .with(
            "custom_instructions",
            json!(instructions.unwrap_or_default()),
        );
    run_hooks(
        &core.hook_env(),
        &core.events,
        HookEvent::PreCompact,
        input,
        job.cancel,
    )
    .await;
    let Some(plan) = plan_summary(working, KEEP_RECENT_MESSAGES) else {
        return Ok(None);
    };
    job.sink.compacting(job.trigger);
    let mut prompt = render_for_summary(&working[..plan.split], MAX_CHARS_PER_RESULT);
    if let Some(focus) = instructions {
        prompt.push_str("\n\nFocus instructions from the user:\n");
        prompt.push_str(focus);
    }
    let work = CompactWork::begin(core, job.agent);
    let mut system = vec![SystemBlock::new(COMPACT)];
    system.extend(work.as_ref().map(CompactWork::system));
    let model = fast_model(&core.settings().settings, &core.main_model());
    let request = ModelRequest::new(model, vec![Message::user_text(prompt)])
        .with_system(system)
        .with_max_tokens(SUMMARY_MAX_TOKENS);
    let turn = side_request(core, job.agent, request, job.cancel).await?;
    let (text, discoveries) = match &work {
        Some(_) => split_discoveries(&turn.text()),
        None => (turn.text().trim().to_string(), Vec::new()),
    };
    if text.is_empty() {
        return Err(EngineError::Invalid(
            "the summarizer returned no text".into(),
        ));
    }
    let mut summary = summary_message(&text);
    let packet = match work {
        Some(work) => work.finish(core, &core.root, discoveries).await,
        None => helper_packet(core, job.agent).await,
    };
    if let Some(packet) = packet {
        summary.content.push(ContentBlock::text(packet));
    }
    let replaced = estimate_messages(&working[..plan.split]);
    let tokens_after = tokens_before
        .saturating_sub(replaced)
        .saturating_add(estimate_message(&summary));
    let marker = CompactionMarker {
        keep_from: working.get(plan.split).map(|message| message.id.clone()),
        summary: text.clone(),
        tokens_before,
        tokens_after,
        created_at: now_ms(),
    };
    let compacted = apply_summary(working, &plan, &summary);
    job.sink.compacted(marker, summary, compacted.clone())?;
    Ok(Some(compacted))
}
