//! Conversation messages -> Chat Completions `messages` entries.
//!
//! Tool results become `role: tool` messages that must directly follow the
//! assistant message carrying the matching `tool_calls`, so they are emitted
//! before any other content of the same user message.

use hyperform_protocol::{CallId, ContentBlock, MediaSource, Role, ToolResultPart};
use serde_json::{Value, json};

use crate::types::ModelRequest;
use crate::wire::{EMPTY_TOOL_OUTPUT, document_filename, ephemeral, media_url, tool_images_note};

pub(super) fn conversation(request: &ModelRequest, cache_control: bool) -> Vec<Value> {
    let mut out = Vec::new();
    for (index, message) in request.messages.iter().enumerate() {
        let start = out.len();
        match message.role {
            Role::User => push_user(&mut out, &message.content),
            Role::Assistant => push_assistant(&mut out, &message.content),
        }
        let breakpoint = cache_control && request.cache_breakpoints.contains(&index);
        if breakpoint && out.len() > start {
            if let Some(last) = out.last_mut() {
                mark_breakpoint(last);
            }
        }
    }
    out
}

fn push_user(out: &mut Vec<Value>, blocks: &[ContentBlock]) {
    let mut tool_images = Vec::new();
    let mut parts = Vec::new();
    for block in blocks {
        match block {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                ..
            } => {
                let (text, images) = split_tool_result(content);
                let text = if text.is_empty() {
                    EMPTY_TOOL_OUTPUT.to_string()
                } else {
                    text
                };
                out.push(
                    json!({"role": "tool", "tool_call_id": tool_use_id.as_str(), "content": text}),
                );
                if !images.is_empty() {
                    tool_images.push((tool_use_id, images));
                }
            }
            ContentBlock::Text { text } if !text.is_empty() => parts.push(text_part(text)),
            ContentBlock::Image { source } => parts.push(image_part(source)),
            ContentBlock::Document { source, title } => {
                parts.push(document_part(source, title.as_deref()));
            }
            // Reasoning and tool calls have no place in a user turn here.
            _ => {}
        }
    }
    let mut content = Vec::new();
    for (id, images) in tool_images {
        content.push(image_note(id));
        content.extend(images);
    }
    content.extend(parts);
    if !content.is_empty() {
        out.push(json!({"role": "user", "content": user_content(content)}));
    }
}

fn push_assistant(out: &mut Vec<Value>, blocks: &[ContentBlock]) {
    let texts: Vec<&str> = blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } if !text.is_empty() => Some(text.as_str()),
            _ => None,
        })
        .collect();
    let tool_calls: Vec<Value> = blocks
        .iter()
        .filter_map(|block| match block {
            ContentBlock::ToolUse { id, name, input } => Some(json!({
                "id": id.as_str(),
                "type": "function",
                "function": {"name": name, "arguments": input.to_string()},
            })),
            _ => None,
        })
        .collect();
    if texts.is_empty() && tool_calls.is_empty() {
        return;
    }
    let content = if texts.is_empty() {
        Value::Null
    } else {
        Value::String(texts.join("\n\n"))
    };
    let mut message = json!({"role": "assistant", "content": content});
    if !tool_calls.is_empty() {
        message["tool_calls"] = Value::Array(tool_calls);
    }
    out.push(message);
}

fn split_tool_result(content: &[ToolResultPart]) -> (String, Vec<Value>) {
    let mut texts = Vec::new();
    let mut images = Vec::new();
    for part in content {
        match part {
            ToolResultPart::Text { text } if !text.is_empty() => texts.push(text.as_str()),
            ToolResultPart::Text { .. } => {}
            ToolResultPart::Image { source } => images.push(image_part(source)),
        }
    }
    (texts.join("\n"), images)
}

/// Plain text stays a string (some local servers accept nothing else);
/// anything multimodal becomes a content-part array.
fn user_content(parts: Vec<Value>) -> Value {
    if parts.iter().all(|part| part["type"] == "text") {
        let texts: Vec<&str> = parts
            .iter()
            .filter_map(|part| part["text"].as_str())
            .collect();
        return Value::String(texts.join("\n\n"));
    }
    Value::Array(parts)
}

/// Put `cache_control` on the message's last text part.
fn mark_breakpoint(message: &mut Value) {
    let content = &mut message["content"];
    if let Value::String(text) = content {
        let text = std::mem::take(text);
        *content = json!([text_part(&text)]);
    }
    let last_text = content
        .as_array_mut()
        .and_then(|parts| parts.iter_mut().rev().find(|part| part["type"] == "text"));
    if let Some(part) = last_text {
        part["cache_control"] = ephemeral();
    }
}

fn text_part(text: &str) -> Value {
    json!({"type": "text", "text": text})
}

fn image_note(id: &CallId) -> Value {
    text_part(&tool_images_note(id))
}

fn image_part(source: &MediaSource) -> Value {
    json!({"type": "image_url", "image_url": {"url": media_url(source)}})
}

fn document_part(source: &MediaSource, title: Option<&str>) -> Value {
    let filename = document_filename(source, title);
    json!({"type": "file", "file": {"filename": filename, "file_data": media_url(source)}})
}

#[cfg(test)]
mod tests;
