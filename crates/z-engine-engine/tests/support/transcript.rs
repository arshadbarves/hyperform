//! Reading and checking transcripts and requests in tests.

use std::collections::HashSet;

use z_engine_llm::ModelRequest;
use z_engine_protocol::{CallId, ContentBlock, Message, Role, ToolResultPart};

/// Every text block of the last user message, joined.
pub fn last_user_text(request: &ModelRequest) -> String {
    let message = request
        .messages
        .iter()
        .rev()
        .find(|message| message.role == Role::User)
        .expect("a user message");
    all_text(message)
}

/// Text blocks and tool-result text of a message.
pub fn all_text(message: &Message) -> String {
    let mut out = Vec::new();
    for block in &message.content {
        match block {
            ContentBlock::Text { text } => out.push(text.clone()),
            ContentBlock::ToolResult { content, .. } => out.push(part_text(content)),
            _ => {}
        }
    }
    out.join("\n")
}

/// `(tool_use_id, is_error, text)` of every result in a message.
pub fn results(message: &Message) -> Vec<(CallId, bool, String)> {
    message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => Some((tool_use_id.clone(), *is_error, part_text(content))),
            _ => None,
        })
        .collect()
}

fn part_text(content: &[ToolResultPart]) -> String {
    content
        .iter()
        .filter_map(|part| match part {
            ToolResultPart::Text { text } => Some(text.as_str()),
            ToolResultPart::Image { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A request any provider accepts: it opens with a user message, roles
/// alternate, and every tool use is answered.
pub fn assert_valid_request(request: &ModelRequest) {
    let roles: Vec<Role> = request.messages.iter().map(|m| m.role).collect();
    assert_eq!(roles.first(), Some(&Role::User), "{roles:?}");
    for pair in roles.windows(2) {
        assert_ne!(pair[0], pair[1], "roles must alternate: {roles:?}");
    }
    assert_valid_transcript(&request.messages);
}

/// Every `tool_use` is answered by exactly one `tool_result` in the next
/// message, and no result is duplicated.
pub fn assert_valid_transcript(messages: &[Message]) {
    for (index, message) in messages.iter().enumerate() {
        let uses: Vec<&CallId> = message.tool_uses().map(|(id, _, _)| id).collect();
        if uses.is_empty() {
            continue;
        }
        let next = messages
            .get(index + 1)
            .unwrap_or_else(|| panic!("tool uses without results at message {index}"));
        let answered: Vec<CallId> = results(next).into_iter().map(|(id, _, _)| id).collect();
        let unique: HashSet<&CallId> = answered.iter().collect();
        assert_eq!(
            unique.len(),
            answered.len(),
            "duplicate results: {answered:?}"
        );
        for id in uses {
            assert!(answered.contains(id), "tool use {id} has no result");
        }
    }
}
