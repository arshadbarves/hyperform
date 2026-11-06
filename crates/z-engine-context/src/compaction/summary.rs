//! Summary compaction: choose where the verbatim tail starts, then replace
//! everything before it with one summary message.

use std::collections::HashMap;

use z_engine_prompts::reminders::COMPACTION_SUMMARY;
use z_engine_protocol::{CallId, ContentBlock, Message, Role};

use crate::template::render_template;

/// Where summary compaction cuts the history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SummaryPlan {
    /// `messages[..split]` is summarized; `messages[split..]` stays verbatim.
    pub split: usize,
}

/// Fewer summarized messages than this is not worth a summary request.
const MIN_SUMMARIZED: usize = 2;

/// Where to cut the history so `messages[split..]` stays verbatim.
///
/// A split may land on the start of a real user turn (a user message
/// holding no tool results; tool-round messages also carry steering and
/// reminder text, so "not tool-results-only" is not enough) or on any
/// assistant message, which lets a single long agentic turn be compacted.
/// Either way no tool_use and its tool_result end up on opposite sides.
///
/// Picks the latest such split that keeps at least `keep_recent_messages`
/// messages verbatim; when none does, the latest split overall. `None`
/// when no split summarizes at least two messages.
pub fn plan_summary(messages: &[Message], keep_recent_messages: usize) -> Option<SummaryPlan> {
    let crossed = crossed_splits(messages);
    let mut splits = (MIN_SUMMARIZED..messages.len())
        .rev()
        .filter(|&split| !crossed[split] && is_split_point(&messages[split]));
    let latest = splits.next()?;
    let split = std::iter::once(latest)
        .chain(splits)
        .find(|&split| messages.len() - split >= keep_recent_messages)
        .unwrap_or(latest);
    Some(SummaryPlan { split })
}

/// A user message whose text is [`COMPACTION_SUMMARY`] around `summary`.
pub fn summary_message(summary: &str) -> Message {
    let text = render_template(COMPACTION_SUMMARY, &[("summary", summary.trim())]);
    Message::user_text(text.trim_end())
}

/// A message made by [`summary_message`].
pub fn is_summary(message: &Message) -> bool {
    let head = COMPACTION_SUMMARY
        .split("{{")
        .next()
        .unwrap_or_default()
        .trim();
    message.role == Role::User && !head.is_empty() && message.text().starts_with(head)
}

/// `summary` followed by the verbatim tail `messages[plan.split..]`.
/// `plan` must come from [`plan_summary`] on the same messages; a split past
/// the end keeps no tail.
pub fn apply_summary(messages: &[Message], plan: &SummaryPlan, summary: &Message) -> Vec<Message> {
    let tail = &messages[plan.split.min(messages.len())..];
    let mut out = Vec::with_capacity(tail.len() + 1);
    out.push(summary.clone());
    out.extend_from_slice(tail);
    out
}

/// An assistant message, or a user message that starts a real turn.
pub(super) fn is_split_point(message: &Message) -> bool {
    match message.role {
        Role::Assistant => true,
        Role::User => {
            !message.content.is_empty()
                && !message
                    .content
                    .iter()
                    .any(|block| matches!(block, ContentBlock::ToolResult { .. }))
        }
    }
}

/// `crossed[s]` is true when a tool_use and its tool_result sit on
/// opposite sides of split `s` (one of them before index `s`, the other at
/// or after it). Has `messages.len() + 1` entries.
pub(super) fn crossed_splits(messages: &[Message]) -> Vec<bool> {
    let mut uses: HashMap<&CallId, usize> = HashMap::new();
    for (index, message) in messages.iter().enumerate() {
        for (id, _, _) in message.tool_uses() {
            uses.entry(id).or_insert(index);
        }
    }
    // Difference array over split positions: a pair at (lo, hi) crosses
    // every split in lo + 1..=hi.
    let mut delta = vec![0i64; messages.len() + 2];
    for (index, message) in messages.iter().enumerate() {
        for block in &message.content {
            let ContentBlock::ToolResult { tool_use_id, .. } = block else {
                continue;
            };
            let Some(&use_index) = uses.get(tool_use_id) else {
                continue;
            };
            let (lo, hi) = (use_index.min(index), use_index.max(index));
            if lo < hi {
                delta[lo + 1] += 1;
                delta[hi + 1] -= 1;
            }
        }
    }
    let mut open = 0i64;
    delta[..=messages.len()]
        .iter()
        .map(|change| {
            open += change;
            open > 0
        })
        .collect()
}

#[cfg(test)]
#[path = "summary_tests.rs"]
mod tests;
