//! Responses API event stream -> normalized [`ModelEvent`]s.
//!
//! Output items are keyed by `output_index`, because GitHub Copilot rotates
//! item ids on every event. Usage and the stop arrive together with
//! `response.completed` or `response.incomplete`.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use z_engine_protocol::{CallId, Usage};

use super::reasoning;
use crate::error::LlmError;
use crate::sse::SseEvent;
use crate::transport::{StreamItem, StreamParser};
use crate::types::{ModelEvent, StopReason};

#[derive(Debug, Default)]
pub(crate) struct ResponsesStreamParser {
    /// Open function calls by output index; `true` once arguments arrived.
    tools: BTreeMap<usize, bool>,
    /// Reasoning items whose summary was streamed as deltas.
    summarized: BTreeSet<usize>,
    saw_tools: bool,
}

impl StreamParser for ResponsesStreamParser {
    fn push(&mut self, event: &SseEvent) -> Vec<StreamItem> {
        if event.is_done() {
            return Vec::new();
        }
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
        let text = || text_field(&data, "delta");
        match kind {
            "response.output_text.delta" => text()
                .map(ModelEvent::TextDelta)
                .into_iter()
                .map(Ok)
                .collect(),
            "response.reasoning_summary_text.delta" => {
                self.summarized.extend(output_index(&data));
                text()
                    .map(ModelEvent::ThinkingDelta)
                    .into_iter()
                    .map(Ok)
                    .collect()
            }
            "response.reasoning_summary_part.added" => {
                let summary_index = data.get("summary_index").and_then(Value::as_u64);
                if summary_index.is_some_and(|index| index > 0) {
                    vec![Ok(ModelEvent::ThinkingDelta("\n\n".into()))]
                } else {
                    Vec::new()
                }
            }
            "response.output_item.added" => self.item_added(&data),
            "response.function_call_arguments.delta" => self.arguments(&data),
            "response.output_item.done" => self.item_done(&data),
            "response.completed" | "response.incomplete" => self.complete(&data),
            "response.failed" => {
                let error = data.pointer("/response/error").unwrap_or(&data);
                vec![Err(stream_error(error))]
            }
            "error" => {
                let error = data.get("error").filter(|error| error.is_object());
                vec![Err(stream_error(error.unwrap_or(&data)))]
            }
            // Lifecycle, content-part and `.done` echoes carry nothing new.
            _ => Vec::new(),
        }
    }

    fn finish(&mut self) -> Vec<StreamItem> {
        vec![Err(LlmError::Stream("stream ended unexpectedly".into()))]
    }
}

impl ResponsesStreamParser {
    fn item_added(&mut self, data: &Value) -> Vec<StreamItem> {
        let (Some(index), Some(item)) = (output_index(data), data.get("item")) else {
            return Vec::new();
        };
        if item_type(item) != Some("function_call") {
            return Vec::new();
        }
        self.start_tool(index, item)
    }

    fn start_tool(&mut self, index: usize, item: &Value) -> Vec<StreamItem> {
        self.saw_tools = true;
        let id = text_field(item, "call_id")
            .or_else(|| text_field(item, "id"))
            .unwrap_or_else(|| CallId::new().0);
        let mut out = vec![Ok(ModelEvent::ToolUseStart {
            index,
            id,
            name: text_field(item, "name").unwrap_or_default(),
        })];
        let arguments = text_field(item, "arguments");
        self.tools.insert(index, arguments.is_some());
        out.extend(arguments.map(|partial_json| {
            Ok(ModelEvent::ToolUseDelta {
                index,
                partial_json,
            })
        }));
        out
    }

    fn arguments(&mut self, data: &Value) -> Vec<StreamItem> {
        let (Some(index), Some(partial_json)) = (output_index(data), text_field(data, "delta"))
        else {
            return Vec::new();
        };
        match self.tools.get_mut(&index) {
            Some(seen) => {
                *seen = true;
                vec![Ok(ModelEvent::ToolUseDelta {
                    index,
                    partial_json,
                })]
            }
            None => {
                tracing::warn!(index, "dropping arguments for an unknown function call");
                Vec::new()
            }
        }
    }

    fn item_done(&mut self, data: &Value) -> Vec<StreamItem> {
        let (Some(index), Some(item)) = (output_index(data), data.get("item")) else {
            return Vec::new();
        };
        match item_type(item) {
            Some("function_call") => {
                let mut out = Vec::new();
                let seen = match self.tools.remove(&index) {
                    Some(seen) => seen,
                    None => {
                        out.extend(self.start_tool(index, item));
                        self.tools.remove(&index).unwrap_or(true)
                    }
                };
                if !seen {
                    out.extend(text_field(item, "arguments").map(|partial_json| {
                        Ok(ModelEvent::ToolUseDelta {
                            index,
                            partial_json,
                        })
                    }));
                }
                out.push(Ok(ModelEvent::ToolUseEnd { index }));
                out
            }
            Some("reasoning") => {
                let mut out = Vec::new();
                if !self.summarized.remove(&index) {
                    let summary = summary_text(item);
                    if !summary.is_empty() {
                        out.push(Ok(ModelEvent::ThinkingDelta(summary)));
                    }
                }
                if let Some(encrypted) = text_field(item, "encrypted_content") {
                    out.push(Ok(ModelEvent::ThinkingSignature(reasoning::signature(
                        &encrypted,
                    ))));
                }
                out
            }
            _ => Vec::new(),
        }
    }

    /// Close open calls, then emit the usage and the stop.
    fn complete(&mut self, data: &Value) -> Vec<StreamItem> {
        let mut out: Vec<StreamItem> = std::mem::take(&mut self.tools)
            .into_keys()
            .map(|index| Ok(ModelEvent::ToolUseEnd { index }))
            .collect();
        let response = data.get("response").unwrap_or(&Value::Null);
        if let Some(usage) = response.get("usage").and_then(parse_usage) {
            out.push(Ok(ModelEvent::Usage(usage)));
        }
        let stop = match response
            .pointer("/incomplete_details/reason")
            .and_then(Value::as_str)
        {
            Some("max_output_tokens") => StopReason::MaxTokens,
            Some("content_filter") => StopReason::Refusal,
            _ if self.saw_tools => StopReason::ToolUse,
            _ => StopReason::EndTurn,
        };
        out.push(Ok(ModelEvent::Stop(stop)));
        out
    }
}

fn output_index(data: &Value) -> Option<usize> {
    data.get("output_index")
        .and_then(Value::as_u64)
        .and_then(|index| usize::try_from(index).ok())
}

fn item_type(item: &Value) -> Option<&str> {
    item.get("type").and_then(Value::as_str)
}

fn text_field(value: &Value, name: &str) -> Option<String> {
    value
        .get(name)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn summary_text(item: &Value) -> String {
    let parts = item.get("summary").and_then(Value::as_array);
    let texts: Vec<&str> = parts
        .into_iter()
        .flatten()
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .filter(|text| !text.is_empty())
        .collect();
    texts.join("\n\n")
}

/// Responses usage -> protocol usage; `input_tokens` excludes cache reads.
fn parse_usage(usage: &Value) -> Option<Usage> {
    let count = |pointer: &str| usage.pointer(pointer).and_then(Value::as_u64).unwrap_or(0);
    let cache_read = count("/input_tokens_details/cached_tokens");
    let parsed = Usage {
        input_tokens: count("/input_tokens").saturating_sub(cache_read),
        output_tokens: count("/output_tokens"),
        cache_read_tokens: cache_read,
        cache_write_tokens: 0,
        reasoning_tokens: count("/output_tokens_details/reasoning_tokens"),
    };
    (!parsed.is_empty()).then_some(parsed)
}

fn stream_error(error: &Value) -> LlmError {
    let code = error
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .map_or_else(|| error.to_string(), str::to_string);
    let detail = if code.is_empty() {
        message
    } else {
        format!("{code}: {message}")
    };
    match code {
        "rate_limit_exceeded" => LlmError::RateLimited {
            attempts: 1,
            detail,
        },
        "server_is_overloaded" | "slow_down" => LlmError::Overloaded {
            attempts: 1,
            detail,
        },
        code if code.starts_with("invalid") || code == "context_length_exceeded" => {
            LlmError::Http {
                status: 400,
                body: detail,
            }
        }
        _ => LlmError::Http {
            status: 500,
            body: detail,
        },
    }
}

#[cfg(test)]
mod tests;
