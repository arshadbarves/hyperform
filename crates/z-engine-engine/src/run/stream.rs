//! Streams one response: deltas become GUI events as they arrive, retries
//! become `Retrying`, and the accumulator assembles the final message.

use z_engine_llm::{AssistantTurn, LlmError, ModelEvent, ModelRequest, ResponseAccumulator};
use z_engine_protocol::{Event, MessageId};

use super::spec::RunContext;

/// How a response stream ended.
#[derive(Debug)]
pub(crate) enum StreamEnd {
    Complete(AssistantTurn),
    /// Cancelled mid-stream; carries what arrived before the cancel.
    Cancelled(AssistantTurn),
    Failed(LlmError, AssistantTurn),
}

pub(crate) async fn stream_response(
    ctx: &RunContext,
    request: ModelRequest,
    message_id: &MessageId,
) -> StreamEnd {
    let agent_id = ctx.spec.agent_id.clone();
    let events = &ctx.core.events;
    events.emit(Event::AssistantStarted {
        agent_id: agent_id.clone(),
        message_id: message_id.clone(),
    });
    let mut stream = ctx.core.client().stream(request, ctx.cancel.child_token());
    let mut acc = ResponseAccumulator::default();
    loop {
        let item = tokio::select! {
            biased;
            () = ctx.cancel.cancelled() => return StreamEnd::Cancelled(acc.finish()),
            item = stream.recv() => item,
        };
        let event = match item {
            None => return StreamEnd::Complete(acc.finish()),
            Some(Ok(event)) => event,
            Some(Err(LlmError::Cancelled)) => return StreamEnd::Cancelled(acc.finish()),
            Some(Err(error)) => return StreamEnd::Failed(error, acc.finish()),
        };
        match &event {
            ModelEvent::TextDelta(text) => events.emit(Event::TextDelta {
                agent_id: agent_id.clone(),
                message_id: message_id.clone(),
                text: text.clone(),
            }),
            ModelEvent::ThinkingDelta(text) => events.emit(Event::ThinkingDelta {
                agent_id: agent_id.clone(),
                message_id: message_id.clone(),
                text: text.clone(),
            }),
            ModelEvent::Retrying {
                attempt,
                delay_ms,
                reason,
            } => events.emit(Event::Retrying {
                attempt: *attempt,
                delay_ms: *delay_ms,
                reason: reason.clone(),
            }),
            _ => {}
        }
        acc.absorb(&event);
    }
}
