//! Microcompaction: replace the text of old, large tool results with a
//! short marker. Blocks are edited in place, so every tool_use keeps its
//! tool_result and the transcript stays valid for any provider.

use z_engine_protocol::{CallId, ContentBlock, Message, ToolResultPart};

/// Replacement texts must start with this marker; results that already do
/// are never planned again.
pub const CLEARED_PREFIX: &str = "[cleared";

/// One tool_result block whose text should be cleared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClearTarget {
    /// Index into the message list.
    pub message: usize,
    /// Index into that message's content.
    pub block: usize,
    pub call_id: CallId,
    /// Characters of result text (text parts joined by newlines).
    pub chars: usize,
}

/// Tool results older than the newest `keep_recent` results whose text is
/// longer than `min_chars` and not already cleared, oldest first.
pub fn plan_microcompact(
    messages: &[Message],
    keep_recent: usize,
    min_chars: usize,
) -> Vec<ClearTarget> {
    older_results(messages, keep_recent)
        .into_iter()
        .filter(|target| target.chars > min_chars)
        .collect()
}

/// Tool results older than the newest `keep_recent` results and not
/// already cleared, oldest first.
pub(super) fn older_results(messages: &[Message], keep_recent: usize) -> Vec<ClearTarget> {
    let results: Vec<(usize, usize, &CallId, &[ToolResultPart])> = messages
        .iter()
        .enumerate()
        .flat_map(|(message, msg)| {
            msg.content
                .iter()
                .enumerate()
                .filter_map(move |(block, content)| match content {
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        ..
                    } => Some((message, block, tool_use_id, content.as_slice())),
                    _ => None,
                })
        })
        .collect();
    let older = results.len().saturating_sub(keep_recent);
    results[..older]
        .iter()
        .filter_map(|&(message, block, call_id, parts)| {
            let text = result_text(parts);
            (!text.trim_start().starts_with(CLEARED_PREFIX)).then(|| ClearTarget {
                message,
                block,
                call_id: call_id.clone(),
                chars: text.chars().count(),
            })
        })
        .collect()
}

/// Replaces each target's text parts with `replacement(target, original)`;
/// image parts and the error flag are kept. The engine spills `original`
/// to an artifact and names it in the replacement. Targets that no longer
/// point at a tool_result with the planned call id are skipped; returns the
/// number of blocks replaced.
pub fn apply_microcompact(
    messages: &mut [Message],
    targets: &[ClearTarget],
    replacement: impl Fn(&ClearTarget, &str) -> String,
) -> usize {
    let mut applied = 0;
    for target in targets {
        let block = messages
            .get_mut(target.message)
            .and_then(|message| message.content.get_mut(target.block));
        let Some(ContentBlock::ToolResult {
            tool_use_id,
            content,
            ..
        }) = block
        else {
            continue;
        };
        if *tool_use_id != target.call_id {
            continue;
        }
        let text = replacement(target, &result_text(content));
        let images = std::mem::take(content)
            .into_iter()
            .filter(|part| matches!(part, ToolResultPart::Image { .. }));
        *content = std::iter::once(ToolResultPart::Text { text })
            .chain(images)
            .collect();
        applied += 1;
    }
    applied
}

/// Text parts of a tool result joined by newlines.
pub(crate) fn result_text(parts: &[ToolResultPart]) -> String {
    parts
        .iter()
        .filter_map(|part| match part {
            ToolResultPart::Text { text } => Some(text.as_str()),
            ToolResultPart::Image { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
#[path = "micro_tests.rs"]
mod tests;
