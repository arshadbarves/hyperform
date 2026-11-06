//! The shape every provider accepts, enforced on the way out: roles
//! alternate, and each assistant `tool_use` is answered in the very next
//! (user) message. The working set normally has that shape already; this
//! covers what a lost write or a turn that failed before the model
//! answered can leave behind (two user messages in a row, a call without
//! its result, a result far from its call).

use std::collections::HashSet;

use z_engine_protocol::{CallId, ContentBlock, Message, Role};

/// Stands in for a result the log never recorded.
pub const MISSING_RESULT: &str = "interrupted: no result was recorded for this call";

pub fn well_formed(messages: Vec<Message>) -> Vec<Message> {
    let mut merged = merge_adjacent_roles(messages);
    if merged.first().is_some_and(|first| first.role == Role::User) {
        answer_exactly(&mut merged[0], &[]);
    }
    let mut index = 0;
    while index < merged.len() {
        if merged[index].role == Role::Assistant {
            let calls: Vec<CallId> = merged[index]
                .tool_uses()
                .map(|(id, ..)| id.clone())
                .collect();
            let answered = merged.get(index + 1).is_some_and(|m| m.role == Role::User);
            if !calls.is_empty() && !answered {
                merged.insert(index + 1, Message::new(Role::User, Vec::new()));
            }
            if let Some(next) = merged.get_mut(index + 1) {
                answer_exactly(next, &calls);
            }
        }
        index += 1;
    }
    merge_adjacent_roles(merged)
}

/// Neighbours of one role merge (content in order, the first message's
/// id); empty messages are dropped.
pub fn merge_adjacent_roles(messages: Vec<Message>) -> Vec<Message> {
    let mut merged: Vec<Message> = Vec::with_capacity(messages.len());
    for message in messages {
        if message.content.is_empty() {
            continue;
        }
        match merged.last_mut() {
            Some(previous) if previous.role == message.role => {
                previous.content.extend(message.content);
            }
            _ => merged.push(message),
        }
    }
    merged
}

/// Keeps one result per call in `calls` (results first, in call order,
/// missing ones filled in) and drops results answering anything else.
fn answer_exactly(message: &mut Message, calls: &[CallId]) {
    let expected: HashSet<&CallId> = calls.iter().collect();
    let mut seen: HashSet<CallId> = HashSet::new();
    let mut results = Vec::new();
    let mut rest = Vec::new();
    for block in std::mem::take(&mut message.content) {
        match block {
            ContentBlock::ToolResult {
                ref tool_use_id, ..
            } => {
                if expected.contains(tool_use_id) && seen.insert(tool_use_id.clone()) {
                    results.push(block);
                }
            }
            other => rest.push(other),
        }
    }
    if results.len() == calls.len() && rest.is_empty() {
        message.content = results;
        return;
    }
    let mut ordered: Vec<ContentBlock> = calls
        .iter()
        .map(|call| {
            let found = results.iter().position(
                |block| matches!(block, ContentBlock::ToolResult { tool_use_id, .. } if tool_use_id == call),
            );
            match found {
                Some(position) => results.swap_remove(position),
                None => ContentBlock::tool_result(call.clone(), MISSING_RESULT, true),
            }
        })
        .collect();
    ordered.extend(rest);
    message.content = ordered;
}

#[cfg(test)]
#[path = "wellformed_tests.rs"]
mod tests;
