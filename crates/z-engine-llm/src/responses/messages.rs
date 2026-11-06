//! Conversation -> Responses `input` items.
//!
//! Tool results become `function_call_output` items ahead of the rest of
//! their user turn; their images follow in a user message, since outputs
//! carry text only. Encrypted reasoning is replayed only for the current
//! tool loop (assistant turns after the last user input): an older turn
//! may come from another model, whose reasoning this one cannot read.

use serde_json::{Value, json};
use z_engine_protocol::{CallId, ContentBlock, MediaSource, Message, Role, ToolResultPart};

use super::reasoning;
use crate::turns::last_user_input;
use crate::wire::{EMPTY_TOOL_OUTPUT, document_filename, media_url, tool_images_note};

pub(super) fn input(messages: &[Message]) -> Vec<Value> {
    let replay_from = last_user_input(messages).map_or(0, |index| index + 1);
    let mut out = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        match message.role {
            Role::User => push_user(&mut out, &message.content),
            Role::Assistant => push_assistant(&mut out, &message.content, index >= replay_from),
        }
    }
    out
}

fn push_user(out: &mut Vec<Value>, blocks: &[ContentBlock]) {
    let mut tool_images: Vec<(&CallId, Vec<Value>)> = Vec::new();
    let mut parts = Vec::new();
    for block in blocks {
        match block {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                ..
            } => {
                let (text, images) = split_tool_result(content);
                let output = if text.is_empty() {
                    EMPTY_TOOL_OUTPUT.to_string()
                } else {
                    text
                };
                out.push(json!({
                    "type": "function_call_output",
                    "call_id": tool_use_id.as_str(),
                    "output": output,
                }));
                if !images.is_empty() {
                    tool_images.push((tool_use_id, images));
                }
            }
            ContentBlock::Text { text } if !text.is_empty() => parts.push(input_text(text)),
            ContentBlock::Image { source } => parts.push(input_image(source)),
            ContentBlock::Document { source, title } => {
                parts.push(input_file(source, title.as_deref()));
            }
            _ => {}
        }
    }
    let mut content = Vec::new();
    for (id, images) in tool_images {
        content.push(input_text(&tool_images_note(id)));
        content.extend(images);
    }
    content.extend(parts);
    if !content.is_empty() {
        out.push(json!({"role": "user", "content": content}));
    }
}

fn push_assistant(out: &mut Vec<Value>, blocks: &[ContentBlock], replay_reasoning: bool) {
    for block in blocks {
        match block {
            ContentBlock::Thinking {
                text,
                signature: Some(signature),
            } if replay_reasoning => {
                if let Some(encrypted) = reasoning::encrypted(signature) {
                    out.push(reasoning_item(text, encrypted));
                }
            }
            ContentBlock::Text { text } if !text.is_empty() => out.push(json!({
                "role": "assistant",
                "content": [{"type": "output_text", "text": text}],
            })),
            ContentBlock::ToolUse { id, name, input } => out.push(json!({
                "type": "function_call",
                "call_id": id.as_str(),
                "name": name,
                "arguments": input.to_string(),
            })),
            _ => {}
        }
    }
}

fn reasoning_item(summary: &str, encrypted: &str) -> Value {
    let summary = if summary.is_empty() {
        json!([])
    } else {
        json!([{"type": "summary_text", "text": summary}])
    };
    json!({"type": "reasoning", "summary": summary, "encrypted_content": encrypted})
}

fn split_tool_result(content: &[ToolResultPart]) -> (String, Vec<Value>) {
    let mut texts = Vec::new();
    let mut images = Vec::new();
    for part in content {
        match part {
            ToolResultPart::Text { text } if !text.is_empty() => texts.push(text.as_str()),
            ToolResultPart::Text { .. } => {}
            ToolResultPart::Image { source } => images.push(input_image(source)),
        }
    }
    (texts.join("\n"), images)
}

fn input_text(text: &str) -> Value {
    json!({"type": "input_text", "text": text})
}

fn input_image(source: &MediaSource) -> Value {
    json!({"type": "input_image", "image_url": media_url(source)})
}

fn input_file(source: &MediaSource, title: Option<&str>) -> Value {
    match source {
        MediaSource::Url { url } => json!({"type": "input_file", "file_url": url}),
        MediaSource::Base64 { .. } => json!({
            "type": "input_file",
            "filename": document_filename(source, title),
            "file_data": media_url(source),
        }),
    }
}

#[cfg(test)]
mod tests;
