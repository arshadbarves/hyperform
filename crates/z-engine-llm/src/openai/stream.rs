//! Chat Completions chunk stream -> normalized [`ModelEvent`]s.
//!
//! The finish reason usually arrives before the final usage chunk, so the
//! stop is held until `[DONE]` (or the end of the body) and emitted after
//! the cumulative usage.

use std::collections::BTreeMap;

use serde_json::Value;
use z_engine_protocol::{CallId, Usage};

use crate::error::LlmError;
use crate::sse::SseEvent;
use crate::transport::{StreamItem, StreamParser};
use crate::types::{ModelEvent, StopReason};

#[derive(Debug, Default)]
struct ToolCall {
    id: Option<String>,
    name: String,
    /// Argument fragments received before the name was known.
    pending: String,
    started: bool,
    ended: bool,
}

#[derive(Debug, Default)]
pub(crate) struct ChatStreamParser {
    tools: BTreeMap<usize, ToolCall>,
    finish: Option<String>,
    usage: Option<Usage>,
}

impl StreamParser for ChatStreamParser {
    fn push(&mut self, event: &SseEvent) -> Vec<StreamItem> {
        if event.is_done() {
            return self.complete();
        }
        let chunk: Value = match serde_json::from_str(&event.data) {
            Ok(chunk) => chunk,
            Err(error) => {
                tracing::warn!(%error, len = event.data.len(), "skipping unparseable stream chunk");
                return Vec::new();
            }
        };
        if let Some(error) = chunk.get("error").filter(|error| !error.is_null()) {
            return vec![Err(chunk_error(error))];
        }
        let usage = chunk
            .get("usage")
            .filter(|usage| !usage.is_null())
            .or_else(|| chunk.pointer("/x_groq/usage"));
        if let Some(usage) = usage.and_then(parse_usage) {
            self.usage = Some(usage);
        }
        let mut out = Vec::new();
        let choice = chunk
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first());
        if let Some(choice) = choice {
            // `message` covers gateways that answer without streaming.
            if let Some(delta) = choice.get("delta").or_else(|| choice.get("message")) {
                self.delta(delta, &mut out);
            }
            if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
                self.finish = Some(reason.to_string());
                self.end_tools(&mut out);
            }
        }
        out
    }

    fn finish(&mut self) -> Vec<StreamItem> {
        if self.finish.is_some() {
            self.complete()
        } else {
            vec![Err(LlmError::Stream("stream ended unexpectedly".into()))]
        }
    }
}

impl ChatStreamParser {
    fn delta(&mut self, delta: &Value, out: &mut Vec<StreamItem>) {
        let reasoning = delta
            .get("reasoning_content")
            .and_then(Value::as_str)
            .or_else(|| delta.get("reasoning").and_then(Value::as_str));
        if let Some(text) = reasoning.filter(|text| !text.is_empty()) {
            out.push(Ok(ModelEvent::ThinkingDelta(text.to_string())));
        }
        if let Some(text) = delta
            .get("content")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
        {
            out.push(Ok(ModelEvent::TextDelta(text.to_string())));
        }
        if let Some(calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for (position, call) in calls.iter().enumerate() {
                self.tool_delta(position, call, out);
            }
        }
    }

    fn tool_delta(&mut self, position: usize, call: &Value, out: &mut Vec<StreamItem>) {
        let index = call
            .get("index")
            .and_then(Value::as_u64)
            .and_then(|index| usize::try_from(index).ok())
            .unwrap_or(position);
        let tool = self.tools.entry(index).or_default();
        if tool.id.is_none() {
            tool.id = call
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .map(str::to_string);
        }
        let function = call.get("function");
        let arguments = function
            .and_then(|function| function.get("arguments"))
            .map(argument_text)
            .unwrap_or_default();
        if tool.started {
            if !arguments.is_empty() {
                out.push(Ok(ModelEvent::ToolUseDelta {
                    index,
                    partial_json: arguments,
                }));
            }
            return;
        }
        tool.pending.push_str(&arguments);
        if let Some(name) = function
            .and_then(|function| function.get("name"))
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
        {
            tool.name = name.to_string();
            tool.started = true;
            let id = tool.id.clone().unwrap_or_else(|| CallId::new().0);
            out.push(Ok(ModelEvent::ToolUseStart {
                index,
                id,
                name: tool.name.clone(),
            }));
            if !tool.pending.is_empty() {
                out.push(Ok(ModelEvent::ToolUseDelta {
                    index,
                    partial_json: std::mem::take(&mut tool.pending),
                }));
            }
        }
    }

    fn end_tools(&mut self, out: &mut Vec<StreamItem>) {
        for (index, tool) in &mut self.tools {
            if tool.started && !tool.ended {
                tool.ended = true;
                out.push(Ok(ModelEvent::ToolUseEnd { index: *index }));
            } else if !tool.started && !tool.ended {
                tool.ended = true;
                tracing::warn!(index, "dropping a tool call that never received a name");
            }
        }
    }

    /// Close open tools, then emit the cumulative usage and the stop.
    fn complete(&mut self) -> Vec<StreamItem> {
        let mut out = Vec::new();
        self.end_tools(&mut out);
        if let Some(usage) = self.usage.take() {
            out.push(Ok(ModelEvent::Usage(usage)));
        }
        let saw_tools = self.tools.values().any(|tool| tool.started);
        let stop = match self.finish.take() {
            Some(reason) => stop_reason(&reason, saw_tools),
            None if saw_tools => StopReason::ToolUse,
            None => StopReason::EndTurn,
        };
        out.push(Ok(ModelEvent::Stop(stop)));
        out
    }
}

fn stop_reason(reason: &str, saw_tools: bool) -> StopReason {
    match reason {
        // Some compatible servers report `stop` for tool-call turns.
        "stop" if saw_tools => StopReason::ToolUse,
        "stop" => StopReason::EndTurn,
        "tool_calls" | "function_call" => StopReason::ToolUse,
        "length" => StopReason::MaxTokens,
        "content_filter" => StopReason::Refusal,
        other => StopReason::Other(other.to_string()),
    }
}

fn argument_text(arguments: &Value) -> String {
    match arguments {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn chunk_error(error: &Value) -> LlmError {
    let code = error.get("code").and_then(|code| {
        code.as_u64()
            .or_else(|| code.as_str().and_then(|text| text.parse().ok()))
    });
    let status = code
        .and_then(|code| u16::try_from(code).ok())
        .filter(|code| (100..=599).contains(code))
        .unwrap_or(500);
    let body = error
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| error.as_str())
        .map_or_else(|| error.to_string(), str::to_string);
    LlmError::Http { status, body }
}

/// OpenAI usage -> protocol usage; `input_tokens` excludes cache traffic.
fn parse_usage(usage: &Value) -> Option<Usage> {
    let count = |value: Option<&Value>| value.and_then(Value::as_u64).unwrap_or(0);
    let details = usage.get("prompt_tokens_details");
    let cached = details
        .and_then(|details| details.get("cached_tokens"))
        .or_else(|| usage.get("prompt_cache_hit_tokens"));
    let cache_read = count(cached);
    let cache_write = count(details.and_then(|details| details.get("cache_write_tokens")));
    let reasoning = usage
        .get("completion_tokens_details")
        .and_then(|details| details.get("reasoning_tokens"));
    let parsed = Usage {
        input_tokens: count(usage.get("prompt_tokens"))
            .saturating_sub(cache_read)
            .saturating_sub(cache_write),
        output_tokens: count(usage.get("completion_tokens")),
        cache_read_tokens: cache_read,
        cache_write_tokens: cache_write,
        reasoning_tokens: count(reasoning),
    };
    (!parsed.is_empty()).then_some(parsed)
}

#[cfg(test)]
mod tests;
