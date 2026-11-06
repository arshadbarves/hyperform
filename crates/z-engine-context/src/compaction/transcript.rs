//! Plain-text transcript of messages for the compaction summarizer.

use std::collections::HashMap;

use serde_json::Value;
use z_engine_protocol::{CallId, ContentBlock, Message, Role, ToolResultPart};

/// One entry per block, separated by blank lines: `User: ...` and
/// `Assistant: ...` for text, `Tool call <name> <compact json>` for calls
/// and `Tool result (<name>): ...` / `Tool error (<name>): ...` for results.
/// Thinking is omitted; media appear as `[image]` or `[document]`. Result
/// text, and every string inside a tool input, is cut to
/// `max_chars_per_result` characters, keeping its head and tail.
pub fn render_for_summary(messages: &[Message], max_chars_per_result: usize) -> String {
    let names: HashMap<&CallId, &str> = messages
        .iter()
        .flat_map(Message::tool_uses)
        .map(|(id, name, _)| (id, name))
        .collect();
    let mut entries = Vec::new();
    for message in messages {
        let speaker = match message.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
        };
        for block in &message.content {
            let entry = match block {
                ContentBlock::Text { text } if text.trim().is_empty() => continue,
                ContentBlock::Text { text } => format!("{speaker}: {}", text.trim()),
                ContentBlock::Image { .. } => format!("{speaker}: [image]"),
                ContentBlock::Document {
                    title: Some(title), ..
                } => format!("{speaker}: [document: {title}]"),
                ContentBlock::Document { title: None, .. } => format!("{speaker}: [document]"),
                ContentBlock::Thinking { .. } | ContentBlock::RedactedThinking { .. } => continue,
                ContentBlock::ToolUse { name, input, .. } => format!(
                    "Tool call {name} {}",
                    clip_strings(input, max_chars_per_result)
                ),
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    let label = if *is_error {
                        "Tool error"
                    } else {
                        "Tool result"
                    };
                    let name = names
                        .get(tool_use_id)
                        .map(|name| format!(" ({name})"))
                        .unwrap_or_default();
                    format!(
                        "{label}{name}: {}",
                        truncate(&result_body(content), max_chars_per_result)
                    )
                }
            };
            entries.push(entry);
        }
    }
    entries.join("\n\n")
}

fn result_body(parts: &[ToolResultPart]) -> String {
    parts
        .iter()
        .map(|part| match part {
            ToolResultPart::Text { text } => text.as_str(),
            ToolResultPart::Image { .. } => "[image]",
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn clip_strings(value: &Value, max_chars: usize) -> Value {
    match value {
        Value::String(text) => Value::String(truncate(text, max_chars)),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| clip_strings(item, max_chars))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, item)| (key.clone(), clip_strings(item, max_chars)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// `text` when it fits, else its first and last `max_chars / 2`
/// characters around a marker naming how many were cut.
pub(super) fn truncate(text: &str, max_chars: usize) -> String {
    let total = text.chars().count();
    if total <= max_chars {
        return text.to_string();
    }
    let tail = max_chars / 2;
    let head_end = byte_offset(text, max_chars - tail);
    let tail_start = byte_offset(text, total - tail);
    format!(
        "{}\n[… {} chars truncated …]\n{}",
        &text[..head_end],
        total - max_chars,
        &text[tail_start..]
    )
}

fn byte_offset(text: &str, chars: usize) -> usize {
    text.char_indices()
        .nth(chars)
        .map_or(text.len(), |(offset, _)| offset)
}

#[cfg(test)]
#[path = "transcript_tests.rs"]
mod tests;
