//! Side requests (titles, summaries, page extraction): drained without live
//! deltas, stopped by cancellation, and always accounted, even on failure.

use tokio_util::sync::CancellationToken;
use z_engine_llm::{AssistantTurn, LlmError, ModelRequest, ResponseAccumulator};
use z_engine_protocol::AgentId;

use super::usage;
use crate::error::EngineError;
use crate::session::SessionCore;

pub(crate) async fn side_request(
    core: &SessionCore,
    agent: &AgentId,
    request: ModelRequest,
    cancel: &CancellationToken,
) -> Result<AssistantTurn, EngineError> {
    let model = request.model.clone();
    let mut stream = core.client().stream(request, cancel.child_token());
    let mut acc = ResponseAccumulator::default();
    let failure = loop {
        let item = tokio::select! {
            biased;
            () = cancel.cancelled() => break Some(LlmError::Cancelled),
            item = stream.recv() => item,
        };
        match item {
            None => break None,
            Some(Ok(event)) => acc.absorb(&event),
            Some(Err(error)) => break Some(error),
        }
    };
    let turn = acc.finish();
    usage::record_side(core, agent, &model, turn.usage);
    match failure {
        None => Ok(turn),
        Some(error) => Err(error.into()),
    }
}
