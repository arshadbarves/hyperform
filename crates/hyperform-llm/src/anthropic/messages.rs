//! Conversation -> Anthropic `messages`: block mapping, same-role merging,
//! tool results first in user turns, and message cache breakpoints.

use hyperform_protocol::{CallId, ContentBlock, MediaSource, Role, ToolResultPart};
use serde_json::{Value, json};

use crate::responses::is_responses_signature;
use crate::types::ModelRequest;
use crate::wire::ephemeral;

/// Map the conversation. At most `breakpoint_budget` message breakpoints
/// are kept (the latest ones); reasoning blocks are only replayed when
/// `thinking` is enabled for this request, and never another API's
/// encrypted reasoning.
pub(super) fn build(
    request: &ModelRequest,
    breakpoint_budget: usize,
    thinking: bool,
) -> Vec<Value> {
    let mapped: Vec<Vec<Value>> = request
        .messages
        .iter()
        .map(|message| blocks(&message.content, thinking))
        .collect();
    let marks = select_breakpoints(&request.cache_breakpoints, &mapped, breakpoint_budget);
    let mut merged: Vec<(Role, Vec<Value>)> = Vec::new();
    for (index, (message, mut blocks)) in request.messages.iter().zip(mapped).enumerate() {
        if blocks.is_empty() {
            continue;
        }
        if marks.contains(&index) {
            mark_last_cacheable(&mut blocks);
        }
        match merged.last_mut() {
            Some((role, existing)) if *role == message.role => existing.extend(blocks),
            _ => merged.push((message.role, blocks)),
        }
    }
    merged
        .into_iter()
        .map(|(role, mut blocks)| {
            let role = match role {
                Role::User => {
                    tool_results_first(&mut blocks);
                    "user"
                }
                Role::Assistant => "assistant",
            };
            json!({"role": role, "content": blocks})
        })
        .collect()
}

fn blocks(content: &[ContentBlock], thinking: bool) -> Vec<Value> {
    content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } if !text.trim().is_empty() => {
                Some(json!({"type": "text", "text": text}))
            }
            ContentBlock::Image { source } => {
                Some(json!({"type": "image", "source": source_json(source)}))
            }
            ContentBlock::Document { source, title } => {
                let mut document = json!({"type": "document", "source": source_json(source)});
                if let Some(title) = title {
                    document["title"] = json!(title);
                }
                Some(document)
            }
            ContentBlock::Thinking {
                text,
                signature: Some(signature),
            } if thinking && !is_responses_signature(signature) => {
                Some(json!({"type": "thinking", "thinking": text, "signature": signature}))
            }
            ContentBlock::RedactedThinking { data } if thinking => {
                Some(json!({"type": "redacted_thinking", "data": data}))
            }
            ContentBlock::ToolUse { id, name, input } => {
                let input = if input.is_object() {
                    input.clone()
                } else {
                    json!({})
                };
                Some(json!({"type": "tool_use", "id": id.as_str(), "name": name, "input": input}))
            }
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => Some(tool_result(tool_use_id, content, *is_error)),
            // Empty text and reasoning that cannot be replayed.
            _ => None,
        })
        .collect()
}

fn tool_result(id: &CallId, content: &[ToolResultPart], is_error: bool) -> Value {
    let parts: Vec<Value> = content
        .iter()
        .filter_map(|part| match part {
            ToolResultPart::Text { text } if !text.trim().is_empty() => {
                Some(json!({"type": "text", "text": text}))
            }
            ToolResultPart::Text { .. } => None,
            ToolResultPart::Image { source } => {
                Some(json!({"type": "image", "source": source_json(source)}))
            }
        })
        .collect();
    let mut block = json!({"type": "tool_result", "tool_use_id": id.as_str()});
    if !parts.is_empty() {
        block["content"] = Value::Array(parts);
    }
    if is_error {
        block["is_error"] = json!(true);
    }
    block
}

fn source_json(source: &MediaSource) -> Value {
    match source {
        MediaSource::Base64 { media_type, data } => {
            json!({"type": "base64", "media_type": media_type, "data": data})
        }
        MediaSource::Url { url } => json!({"type": "url", "url": url}),
    }
}

/// Requested breakpoints that land on a cacheable block, latest kept.
fn select_breakpoints(requested: &[usize], mapped: &[Vec<Value>], budget: usize) -> Vec<usize> {
    let mut eligible: Vec<usize> = requested
        .iter()
        .copied()
        .filter(|&index| {
            mapped
                .get(index)
                .is_some_and(|blocks| blocks.iter().any(is_cacheable))
        })
        .collect();
    eligible.sort_unstable();
    eligible.dedup();
    let dropped = eligible.len().saturating_sub(budget);
    eligible.split_off(dropped)
}

/// Thinking blocks cannot carry `cache_control`.
fn is_cacheable(block: &Value) -> bool {
    !matches!(
        block["type"].as_str(),
        Some("thinking" | "redacted_thinking")
    )
}

fn mark_last_cacheable(blocks: &mut [Value]) {
    if let Some(block) = blocks.iter_mut().rev().find(|block| is_cacheable(block)) {
        block["cache_control"] = ephemeral();
    }
}

/// Tool results must open the user turn that answers a tool call.
fn tool_results_first(blocks: &mut [Value]) {
    blocks.sort_by_key(|block| block["type"] != "tool_result");
}

#[cfg(test)]
mod tests;
