//! Anthropic Messages event stream -> normalized [`ModelEvent`]s.
//!
//! Usage accumulates from `message_start` and `message_delta` (both carry
//! running totals) and is emitted once, right before the stop.

use std::collections::BTreeSet;

use serde_json::Value;
use z_engine_protocol::Usage;

use crate::error::LlmError;
use crate::sse::SseEvent;
use crate::transport::{StreamItem, StreamParser};
use crate::types::{ModelEvent, StopReason};

#[derive(Debug, Default)]
pub(crate) struct MessagesStreamParser {
    /// Indices of `tool_use` blocks not yet stopped.
    open_tools: BTreeSet<usize>,
    usage: Usage,
    stop: Option<StopReason>,
}

impl StreamParser for MessagesStreamParser {
    fn push(&mut self, event: &SseEvent) -> Vec<StreamItem> {
        let data: Value = match serde_json::from_str(&event.data) {
            Ok(data) => data,
            Err(error) => {
                tracing::warn!(%error, len = event.data.len(), "skipping unparseable stream event");
                return Vec::new();
            }
        };
        let kind = data
            .get("type")
            .and_then(Value::as_str)
            .or(event.event.as_deref())
            .unwrap_or_default();
        match kind {
            "message_start" => {
                if let Some(usage) = data.pointer("/message/usage") {
                    self.update_usage(usage);
                }
                Vec::new()
            }
            "content_block_start" => self.block_start(&data),
            "content_block_delta" => block_delta(&data),
            "content_block_stop" => match index(&data) {
                Some(index) if self.open_tools.remove(&index) => {
                    vec![Ok(ModelEvent::ToolUseEnd { index })]
                }
                _ => Vec::new(),
            },
            "message_delta" => {
                if let Some(reason) = data.pointer("/delta/stop_reason").and_then(Value::as_str) {
                    self.stop = Some(stop_reason(reason));
                }
                if let Some(usage) = data.get("usage") {
                    self.update_usage(usage);
                }
                Vec::new()
            }
            "message_stop" => self.complete(StopReason::EndTurn),
            "error" => vec![Err(stream_error(data.get("error").unwrap_or(&data)))],
            // `ping` and event types added after this adapter was written.
            _ => Vec::new(),
        }
    }

    fn finish(&mut self) -> Vec<StreamItem> {
        if self.stop.is_some() {
            self.complete(StopReason::EndTurn)
        } else {
            vec![Err(LlmError::Stream("stream ended unexpectedly".into()))]
        }
    }
}

impl MessagesStreamParser {
    fn block_start(&mut self, data: &Value) -> Vec<StreamItem> {
        let (Some(index), Some(block)) = (index(data), data.get("content_block")) else {
            return Vec::new();
        };
        let text = |field: &str| {
            block
                .get(field)
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
                .map(str::to_string)
        };
        let mut out = Vec::new();
        match block.get("type").and_then(Value::as_str) {
            Some("text") => out.extend(text("text").map(ModelEvent::TextDelta)),
            Some("thinking") => {
                out.extend(text("thinking").map(ModelEvent::ThinkingDelta));
                out.extend(text("signature").map(ModelEvent::ThinkingSignature));
            }
            Some("redacted_thinking") => {
                out.push(ModelEvent::RedactedThinking(
                    text("data").unwrap_or_default(),
                ));
            }
            Some("tool_use") => {
                self.open_tools.insert(index);
                out.push(ModelEvent::ToolUseStart {
                    index,
                    id: text("id").unwrap_or_default(),
                    name: text("name").unwrap_or_default(),
                });
            }
            _ => {}
        }
        out.into_iter().map(Ok).collect()
    }

    fn update_usage(&mut self, usage: &Value) {
        let field = |name: &str| usage.get(name).and_then(Value::as_u64);
        if let Some(tokens) = field("input_tokens") {
            self.usage.input_tokens = tokens;
        }
        if let Some(tokens) = field("cache_creation_input_tokens") {
            self.usage.cache_write_tokens = tokens;
        }
        if let Some(tokens) = field("cache_read_input_tokens") {
            self.usage.cache_read_tokens = tokens;
        }
        if let Some(tokens) = field("output_tokens") {
            self.usage.output_tokens = tokens;
        }
    }

    /// Close any open tool, then emit the cumulative usage and the stop.
    fn complete(&mut self, default: StopReason) -> Vec<StreamItem> {
        let mut out: Vec<StreamItem> = std::mem::take(&mut self.open_tools)
            .into_iter()
            .map(|index| Ok(ModelEvent::ToolUseEnd { index }))
            .collect();
        if !self.usage.is_empty() {
            out.push(Ok(ModelEvent::Usage(self.usage)));
        }
        out.push(Ok(ModelEvent::Stop(self.stop.take().unwrap_or(default))));
        out
    }
}

fn block_delta(data: &Value) -> Vec<StreamItem> {
    let (Some(index), Some(delta)) = (index(data), data.get("delta")) else {
        return Vec::new();
    };
    let field = |name: &str| {
        delta
            .get(name)
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    };
    let event = match delta.get("type").and_then(Value::as_str) {
        Some("text_delta") => field("text").map(ModelEvent::TextDelta),
        Some("thinking_delta") => field("thinking").map(ModelEvent::ThinkingDelta),
        Some("signature_delta") => field("signature").map(ModelEvent::ThinkingSignature),
        Some("input_json_delta") => {
            field("partial_json").map(|partial_json| ModelEvent::ToolUseDelta {
                index,
                partial_json,
            })
        }
        _ => None,
    };
    event.into_iter().map(Ok).collect()
}

fn index(data: &Value) -> Option<usize> {
    data.get("index")
        .and_then(Value::as_u64)
        .and_then(|index| usize::try_from(index).ok())
}

fn stop_reason(reason: &str) -> StopReason {
    match reason {
        "end_turn" => StopReason::EndTurn,
        "tool_use" => StopReason::ToolUse,
        "max_tokens" => StopReason::MaxTokens,
        "stop_sequence" => StopReason::StopSequence,
        "refusal" => StopReason::Refusal,
        other => StopReason::Other(other.to_string()),
    }
}

fn stream_error(error: &Value) -> LlmError {
    let detail = error
        .get("message")
        .and_then(Value::as_str)
        .map_or_else(|| error.to_string(), str::to_string);
    let status = match error.get("type").and_then(Value::as_str) {
        Some("overloaded_error") => {
            return LlmError::Overloaded {
                attempts: 1,
                detail,
            };
        }
        Some("rate_limit_error") => {
            return LlmError::RateLimited {
                attempts: 1,
                detail,
            };
        }
        Some("invalid_request_error") => 400,
        Some("authentication_error") => 401,
        Some("permission_error") => 403,
        Some("not_found_error") => 404,
        Some("request_too_large") => 413,
        _ => 500,
    };
    LlmError::Http {
        status,
        body: detail,
    }
}

#[cfg(test)]
mod tests;
