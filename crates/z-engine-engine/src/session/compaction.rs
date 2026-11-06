//! Manual compaction (`Compact` or `/compact <instructions>`) of the main
//! agent's working set, run by the actor as an activity.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use z_engine_context::estimate_messages;
use z_engine_protocol::{AgentId, CompactionTrigger, NoticeLevel};

use crate::run::{CompactJob, MainSink, TranscriptSink, summarize};
use crate::session::SessionCore;

pub(crate) async fn compact_now(
    core: &Arc<SessionCore>,
    instructions: Option<String>,
    cancel: &CancellationToken,
) {
    let sink = MainSink::new(Arc::clone(core), None);
    let working = sink.working();
    let tokens_before = core.with_state(|state| match state.context_tokens {
        0 => estimate_messages(&state.working),
        tokens => tokens,
    });
    let agent = AgentId::main();
    let job = CompactJob {
        core,
        sink: &sink,
        agent: &agent,
        trigger: CompactionTrigger::Manual,
        instructions: instructions.as_deref(),
        cancel,
    };
    match summarize(&job, &working, tokens_before).await {
        Ok(Some(compacted)) => {
            let tokens = estimate_messages(&compacted);
            core.with_state(|state| state.context_tokens = tokens);
        }
        Ok(None) => core.events.notice(
            NoticeLevel::Info,
            "The conversation is too short to compact.",
        ),
        Err(error) => core
            .events
            .notice(NoticeLevel::Warn, format!("Compaction failed: {error}")),
    }
}
