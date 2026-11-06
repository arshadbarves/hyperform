//! Pairing v1 tool results with the calls of the assistant message before
//! them. v1 wrote one event per result, in completion order, often with
//! task reports or titles in between.

use z_engine_protocol::{CallId, ContentBlock};

use super::v1::{V1Event, V1ToolCall};

const MISSING_RESULT: &str =
    "No result was recorded for this tool call; the v1 turn was interrupted.";

/// The calls of one assistant message and the results seen so far.
#[derive(Debug)]
pub(super) struct Round {
    calls: Vec<CallId>,
    results: Vec<Option<String>>,
}

impl Round {
    pub(super) fn new(calls: &[V1ToolCall]) -> Self {
        Self {
            calls: calls
                .iter()
                .map(|call| CallId::from(call.id.as_str()))
                .collect(),
            results: vec![None; calls.len()],
        }
    }

    /// Fill the first unanswered call with this id; false when none matches.
    pub(super) fn answer(&mut self, call_id: &str, content: &str) -> bool {
        let slot = self
            .calls
            .iter()
            .zip(self.results.iter_mut())
            .find(|(call, result)| call.as_str() == call_id && result.is_none());
        match slot {
            Some((_, result)) => {
                *result = Some(content.to_string());
                true
            }
            None => false,
        }
    }

    pub(super) fn is_complete(&self) -> bool {
        self.results.iter().all(Option::is_some)
    }

    /// One result block per call, in call order. A call without a result
    /// gets an error result so providers accept the history.
    pub(super) fn into_blocks(self) -> Vec<ContentBlock> {
        self.calls
            .into_iter()
            .zip(self.results)
            .map(|(call_id, result)| match result {
                Some(text) => ContentBlock::tool_result(call_id, text, false),
                None => ContentBlock::tool_result(call_id, MISSING_RESULT, true),
            })
            .collect()
    }
}

/// The last user or assistant message, when it is an assistant message
/// whose tool calls did not all get results before the end of the file.
/// v1 replay truncated such a round, so the import drops it too.
pub(super) fn unfinished_tail(events: &[V1Event]) -> Option<usize> {
    let last = events.iter().rposition(|event| {
        matches!(
            event,
            V1Event::UserMsg { .. } | V1Event::AssistantMsg { .. }
        )
    })?;
    let V1Event::AssistantMsg { tool_calls, .. } = &events[last] else {
        return None;
    };
    let answered: Vec<&str> = events[last + 1..]
        .iter()
        .filter_map(|event| match event {
            V1Event::ToolResult { tool_call_id, .. } => Some(tool_call_id.as_str()),
            _ => None,
        })
        .collect();
    tool_calls
        .iter()
        .any(|call| !answered.contains(&call.id.as_str()))
        .then_some(last)
}
