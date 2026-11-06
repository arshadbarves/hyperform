//! Context pressure. Above half the window, old tool results are cleared
//! in the working set (originals spilled to artifacts); above
//! `context.compact_at_percent`, older history is summarized. After a
//! context-overflow error both run regardless of the estimate.

use z_engine_context::{CLEARED_PREFIX, apply_microcompact, plan_microcompact};
use z_engine_protocol::{CompactionTrigger, Message, NoticeLevel};

use super::compact::{CompactJob, summarize};
use super::meter::ContextMeter;
use super::sink::TranscriptSink;
use super::spec::RunContext;
use crate::decisions::seams::review_clears;
use crate::error::EngineError;
use crate::settings::models;

/// Tool results shorter than this are never cleared by size alone.
pub(crate) const MIN_CLEAR_CHARS: usize = 1_000;

/// The working set after relief. Only cancellation is an error; a failed
/// summary is reported and the run continues with what it has.
pub(crate) async fn relieve(
    ctx: &RunContext,
    sink: &dyn TranscriptSink,
    meter: &mut ContextMeter,
    working: Vec<Message>,
    forced: bool,
) -> Result<Vec<Message>, EngineError> {
    let settings = ctx.core.settings();
    let catalog = ctx.core.catalog();
    let limit = models::context_window(&settings.settings, catalog.as_deref(), &ctx.model());
    let mut working = working;
    if forced || meter.measure(&working).saturating_mul(2) > limit {
        let keep = settings.settings.context.keep_recent_tool_results as usize;
        working = microcompact(ctx, sink, meter, working, keep).await;
    }
    let threshold =
        limit.saturating_mul(u64::from(settings.settings.context.compact_at_percent)) / 100;
    let used = meter.measure(&working);
    if !forced && used <= threshold {
        return Ok(working);
    }
    let job = CompactJob {
        core: &ctx.core,
        sink,
        agent: &ctx.spec.agent_id,
        trigger: CompactionTrigger::Auto,
        instructions: None,
        cancel: &ctx.cancel,
    };
    match summarize(&job, &working, used).await {
        Ok(Some(compacted)) => {
            meter.reset();
            Ok(compacted)
        }
        Ok(None) => Ok(working),
        Err(error) if ctx.cancel.is_cancelled() => Err(error),
        Err(error) => {
            ctx.core.events.notice(
                NoticeLevel::Warn,
                format!("could not compact the conversation: {error}"),
            );
            Ok(working)
        }
    }
}

/// Decision uses (the pressure seam) may keep planned clears or add more.
async fn microcompact(
    ctx: &RunContext,
    sink: &dyn TranscriptSink,
    meter: &mut ContextMeter,
    working: Vec<Message>,
    keep_recent: usize,
) -> Vec<Message> {
    let planned = plan_microcompact(&working, keep_recent, MIN_CLEAR_CHARS);
    let targets = review_clears(ctx, &working, planned).await;
    if targets.is_empty() {
        return working;
    }
    let current = sink.working();
    if !current
        .iter()
        .map(|m| &m.id)
        .eq(working.iter().map(|m| &m.id))
    {
        // The working set changed while decision uses ran (the user
        // brought back the full history); the plan no longer fits it.
        return current;
    }
    let artifacts = ctx.core.shared.store.artifacts(&ctx.core.id);
    let mut cleared = working.clone();
    let applied = apply_microcompact(&mut cleared, &targets, |target, original| {
        let hint = format!("tool-result-{}", target.call_id);
        match artifacts.write_text(&hint, "txt", original) {
            Ok(path) => format!(
                "{CLEARED_PREFIX}: {} characters of older tool output were removed to free \
                 context; the full output is saved at {}]",
                target.chars,
                path.display()
            ),
            Err(error) => {
                tracing::warn!(%error, "could not spill a cleared tool result");
                format!(
                    "{CLEARED_PREFIX}: {} characters of older tool output were removed to free \
                     context]",
                    target.chars
                )
            }
        }
    });
    tracing::debug!(applied, "cleared old tool results");
    meter.rebase(&working, &cleared);
    sink.set_working(cleared.clone());
    cleared
}
