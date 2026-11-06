use serde_json::json;
use z_engine_protocol::ContentBlock;

use super::*;
use crate::accumulate::ResponseAccumulator;
use crate::transport::parse_all;

const TEXT: &str = include_str!("../../../tests/fixtures/sse/anthropic_text.sse");
const THINKING_TOOL: &str = include_str!("../../../tests/fixtures/sse/anthropic_thinking_tool.sse");
const OVERLOADED: &str = include_str!("../../../tests/fixtures/sse/anthropic_overloaded.sse");

fn events(body: &str) -> Vec<StreamItem> {
    parse_all(MessagesStreamParser::default(), body)
}

fn ok(body: &str) -> Vec<ModelEvent> {
    events(body)
        .into_iter()
        .map(|item| item.expect("unexpected stream error"))
        .collect()
}

fn event(name: &str, data: serde_json::Value) -> String {
    format!("event: {name}\ndata: {data}\n\n")
}

#[test]
fn text_fixture_emits_deltas_usage_and_stop() {
    assert_eq!(
        ok(TEXT),
        [
            ModelEvent::TextDelta("Hello".into()),
            ModelEvent::TextDelta(", world".into()),
            ModelEvent::Usage(Usage {
                input_tokens: 25,
                output_tokens: 12,
                ..Usage::default()
            }),
            ModelEvent::Stop(StopReason::EndTurn),
        ]
    );
}

#[test]
fn thinking_signature_and_tool_use_stream_in_order() {
    let events = ok(THINKING_TOOL);
    assert_eq!(
        events,
        [
            ModelEvent::ThinkingDelta("The user wants main.rs.".into()),
            ModelEvent::ThinkingDelta(" I should read it.".into()),
            ModelEvent::ThinkingSignature(
                "EqQBCgIYAhIM1gbcDa9GJwZA2b3hGgxBdjrkzLoky3dl1pkiMOYds".into()
            ),
            ModelEvent::TextDelta("Let me read it.".into()),
            ModelEvent::ToolUseStart {
                index: 2,
                id: "toolu_01READ".into(),
                name: "Read".into()
            },
            ModelEvent::ToolUseDelta {
                index: 2,
                partial_json: "{\"file_path\":".into()
            },
            ModelEvent::ToolUseDelta {
                index: 2,
                partial_json: " \"src/main.rs\"}".into()
            },
            ModelEvent::ToolUseEnd { index: 2 },
            ModelEvent::Usage(Usage {
                input_tokens: 10,
                output_tokens: 85,
                cache_read_tokens: 1_800,
                cache_write_tokens: 200,
                reasoning_tokens: 0,
            }),
            ModelEvent::Stop(StopReason::ToolUse),
        ]
    );
    let mut acc = ResponseAccumulator::default();
    for event in &events {
        acc.absorb(event);
    }
    let turn = acc.finish();
    assert!(matches!(
        &turn.content[0],
        ContentBlock::Thinking { signature: Some(signature), .. } if signature.starts_with("EqQB")
    ));
    assert!(matches!(
        &turn.content[2],
        ContentBlock::ToolUse { input, .. } if *input == json!({"file_path": "src/main.rs"})
    ));
}

#[test]
fn overloaded_error_event_is_typed() {
    assert_eq!(
        events(OVERLOADED).last(),
        Some(&Err(LlmError::Overloaded {
            attempts: 1,
            detail: "Overloaded".into()
        }))
    );
}

#[test]
fn error_types_map_to_errors() {
    let error = |kind: &str| {
        let body = event(
            "error",
            json!({"type": "error", "error": {"type": kind, "message": "m"}}),
        );
        events(&body).pop().unwrap().unwrap_err()
    };
    assert!(matches!(
        error("rate_limit_error"),
        LlmError::RateLimited { attempts: 1, .. }
    ));
    assert_eq!(
        error("invalid_request_error"),
        LlmError::Http {
            status: 400,
            body: "m".into()
        }
    );
    assert_eq!(
        error("api_error"),
        LlmError::Http {
            status: 500,
            body: "m".into()
        }
    );
    let overflow = json!({"type": "error", "error": {"type": "invalid_request_error", "message": "prompt is too long: 210000 tokens > 200000 maximum"}});
    let unnamed = format!("data: {overflow}\n\n");
    let error = events(&unnamed).pop().unwrap().unwrap_err();
    assert!(error.is_context_overflow());
}

#[test]
fn redacted_thinking_blocks_are_forwarded() {
    let body = [
        event("content_block_start", json!({"type": "content_block_start", "index": 0, "content_block": {"type": "redacted_thinking", "data": "EmwKAhgB"}})),
        event("content_block_stop", json!({"type": "content_block_stop", "index": 0})),
        event("message_delta", json!({"type": "message_delta", "delta": {"stop_reason": "refusal"}, "usage": {"output_tokens": 3}})),
        event("message_stop", json!({"type": "message_stop"})),
    ]
    .concat();
    let events = ok(&body);
    assert_eq!(events[0], ModelEvent::RedactedThinking("EmwKAhgB".into()));
    assert_eq!(events.last(), Some(&ModelEvent::Stop(StopReason::Refusal)));
}

#[test]
fn stream_endings_are_classified() {
    let start = event(
        "content_block_start",
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": "t", "name": "Bash", "input": {}}}),
    );
    let truncated = events(&start);
    assert_eq!(
        truncated.last(),
        Some(&Err(LlmError::Stream("stream ended unexpectedly".into())))
    );
    let delta = event(
        "message_delta",
        json!({"type": "message_delta", "delta": {"stop_reason": "max_tokens"}, "usage": {"output_tokens": 9}}),
    );
    let events = ok(&format!("{start}{delta}"));
    assert_eq!(
        &events[1..],
        [
            ModelEvent::ToolUseEnd { index: 0 },
            ModelEvent::Usage(Usage {
                output_tokens: 9,
                ..Usage::default()
            }),
            ModelEvent::Stop(StopReason::MaxTokens),
        ]
    );
}

#[test]
fn stop_reasons_map() {
    assert_eq!(stop_reason("end_turn"), StopReason::EndTurn);
    assert_eq!(stop_reason("tool_use"), StopReason::ToolUse);
    assert_eq!(stop_reason("max_tokens"), StopReason::MaxTokens);
    assert_eq!(stop_reason("stop_sequence"), StopReason::StopSequence);
    assert_eq!(stop_reason("refusal"), StopReason::Refusal);
    assert_eq!(
        stop_reason("pause_turn"),
        StopReason::Other("pause_turn".into())
    );
}
