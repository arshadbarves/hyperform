use serde_json::json;

use super::*;
use crate::accumulate::{AssistantTurn, ResponseAccumulator};
use crate::transport::parse_all;

const TEXT: &str = include_str!("../../../tests/fixtures/sse/text.sse");
const TOOL_SINGLE: &str = include_str!("../../../tests/fixtures/sse/tool_single.sse");
const TOOL_MULTI: &str = include_str!("../../../tests/fixtures/sse/tool_multi_interleaved.sse");
const USAGE_FINAL: &str = include_str!("../../../tests/fixtures/sse/usage_final.sse");
const KEEPALIVE: &str = include_str!("../../../tests/fixtures/sse/keepalive_comments.sse");
const MALFORMED: &str = include_str!("../../../tests/fixtures/sse/malformed_tool_json.sse");
const REASONING: &str = include_str!("../../../tests/fixtures/sse/reasoning_cached_usage.sse");
const TRAILING: &str = include_str!("../../../tests/fixtures/sse/tool_call_trailing_usage.sse");
const ERROR: &str = include_str!("../../../tests/fixtures/sse/error_midstream.sse");

fn events(body: &str) -> Vec<StreamItem> {
    parse_all(ChatStreamParser::default(), body)
}

fn ok(body: &str) -> Vec<ModelEvent> {
    events(body)
        .into_iter()
        .map(|item| item.expect("unexpected stream error"))
        .collect()
}

fn turn(body: &str) -> AssistantTurn {
    let mut acc = ResponseAccumulator::default();
    for event in ok(body) {
        acc.absorb(&event);
    }
    acc.finish()
}

fn chunk(value: serde_json::Value) -> String {
    format!("data: {value}\n\n")
}

fn usage(input: u64, output: u64) -> ModelEvent {
    ModelEvent::Usage(Usage {
        input_tokens: input,
        output_tokens: output,
        ..Usage::default()
    })
}

fn tool_json(events: &[ModelEvent], wanted: usize) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            ModelEvent::ToolUseDelta {
                index,
                partial_json,
            } if *index == wanted => Some(partial_json.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn text_fixtures_emit_deltas_then_usage_then_stop() {
    assert_eq!(
        ok(TEXT),
        [
            ModelEvent::TextDelta("Hello".into()),
            ModelEvent::TextDelta(", world".into()),
            usage(4, 3),
            ModelEvent::Stop(StopReason::EndTurn),
        ]
    );
    assert_eq!(
        ok(USAGE_FINAL),
        [
            ModelEvent::TextDelta("Working on it.".into()),
            usage(120, 45),
            ModelEvent::Stop(StopReason::EndTurn),
        ]
    );
    assert_eq!(
        ok(KEEPALIVE),
        [
            ModelEvent::TextDelta("ok".into()),
            ModelEvent::Stop(StopReason::EndTurn)
        ]
    );
}

#[test]
fn single_tool_call_streams_start_fragments_and_end() {
    let events = ok(TOOL_SINGLE);
    assert_eq!(
        events[0],
        ModelEvent::ToolUseStart {
            index: 0,
            id: "call_abc".into(),
            name: "read_file".into()
        }
    );
    assert_eq!(tool_json(&events, 0), r#"{"path": "src/main.rs"}"#);
    assert_eq!(
        &events[events.len() - 3..],
        [
            ModelEvent::ToolUseEnd { index: 0 },
            usage(18, 14),
            ModelEvent::Stop(StopReason::ToolUse)
        ]
    );
}

#[test]
fn interleaved_tool_calls_keep_their_indexes() {
    let events = ok(TOOL_MULTI);
    let starts: Vec<(usize, &str, &str)> = events
        .iter()
        .filter_map(|event| match event {
            ModelEvent::ToolUseStart { index, id, name } => {
                Some((*index, id.as_str(), name.as_str()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(starts, [(0, "call_a", "read_file"), (1, "call_b", "bash")]);
    assert_eq!(tool_json(&events, 0), r#"{"path":"Cargo.toml"}"#);
    assert_eq!(tool_json(&events, 1), r#"{"command":"pwd"}"#);
    for index in [0, 1] {
        assert!(events.contains(&ModelEvent::ToolUseEnd { index }));
    }
}

#[test]
fn groq_usage_extension_is_read() {
    let body = [
        chunk(json!({"choices": [{"delta": {"content": "hi"}, "finish_reason": "stop"}]})),
        chunk(json!({"choices": [], "x_groq": {"usage": {"prompt_tokens": 7, "completion_tokens": 2}}})),
        "data: [DONE]\n\n".to_string(),
    ]
    .concat();
    assert_eq!(ok(&body)[1], usage(7, 2));
}

#[test]
fn reasoning_and_cache_details_are_normalized() {
    let events = ok(REASONING);
    assert_eq!(
        events[0],
        ModelEvent::ThinkingDelta("Weighing options.".into())
    );
    assert_eq!(events[1], ModelEvent::ThinkingDelta(" Picking one.".into()));
    assert_eq!(events[2], ModelEvent::TextDelta("Use a map.".into()));
    assert_eq!(
        events[3],
        ModelEvent::Usage(Usage {
            input_tokens: 50,
            output_tokens: 90,
            cache_read_tokens: 1_000,
            cache_write_tokens: 150,
            reasoning_tokens: 40,
        })
    );
    assert_eq!(events[4], ModelEvent::Stop(StopReason::EndTurn));
}

#[test]
fn trailing_usage_tool_call_assembles_into_a_turn() {
    let turn = turn(TRAILING);
    assert_eq!(turn.text(), "Reading.");
    assert_eq!(turn.stop, Some(StopReason::ToolUse));
    assert_eq!(turn.usage.input_tokens, 64);
    assert_eq!(turn.usage.cache_read_tokens, 256);
    let input = turn.content.iter().find_map(|block| match block {
        z_engine_protocol::ContentBlock::ToolUse { id, input, .. } => {
            Some((id.as_str(), input.clone()))
        }
        _ => None,
    });
    assert_eq!(
        input,
        Some(("call_read_1", json!({"file_path": "src/main.rs"})))
    );
}

#[test]
fn malformed_arguments_pass_through_to_the_accumulator() {
    let turn = turn(MALFORMED);
    assert_eq!(turn.malformed.len(), 1);
    assert_eq!(turn.malformed[0].id, "call_bad");
    assert_eq!(turn.malformed[0].raw, r#"{ "path": "x.txt""#);
}

#[test]
fn error_object_becomes_a_typed_http_error() {
    assert_eq!(
        events(ERROR),
        [Err(LlmError::Http {
            status: 502,
            body: "Upstream provider returned an error".into()
        })]
    );
    let named = chunk(json!({"error": {"code": "rate_limit", "message": "slow down"}}));
    assert!(matches!(
        &events(&named)[0],
        Err(LlmError::Http { status: 500, body }) if body == "slow down"
    ));
    let numeric_text = chunk(json!({"error": {"code": "429", "message": "busy"}}));
    assert!(matches!(
        &events(&numeric_text)[0],
        Err(LlmError::Http { status: 429, .. })
    ));
}

#[test]
fn arguments_before_the_name_are_buffered_and_ids_synthesized() {
    let body = [
        chunk(json!({"choices": [{"delta": {"tool_calls": [{"index": 0, "function": {"arguments": "{\"a\":"}}]}}]})),
        chunk(json!({"choices": [{"delta": {"tool_calls": [{"index": 0, "function": {"name": "Grep", "arguments": "1}"}}]}}]})),
        "data: [DONE]\n\n".to_string(),
    ]
    .concat();
    let events = ok(&body);
    match &events[0] {
        ModelEvent::ToolUseStart { index: 0, id, name } => {
            assert!(id.starts_with("call_"));
            assert_eq!(name, "Grep");
        }
        other => panic!("expected a tool start, got {other:?}"),
    }
    assert_eq!(
        events[1],
        ModelEvent::ToolUseDelta {
            index: 0,
            partial_json: "{\"a\":1}".into()
        }
    );
    assert_eq!(events[2], ModelEvent::ToolUseEnd { index: 0 });
    assert_eq!(events[3], ModelEvent::Stop(StopReason::ToolUse));
}

#[test]
fn stream_endings_are_classified() {
    let text = chunk(json!({"choices": [{"delta": {"content": "partial"}}]}));
    let truncated = events(&text);
    assert_eq!(
        truncated.last(),
        Some(&Err(LlmError::Stream("stream ended unexpectedly".into())))
    );
    let finished = format!(
        "{text}{}",
        chunk(json!({"choices": [{"delta": {}, "finish_reason": "length"}]}))
    );
    assert_eq!(
        ok(&finished).last(),
        Some(&ModelEvent::Stop(StopReason::MaxTokens))
    );
    let garbage = format!("data: {{not json\n\n{text}data: [DONE]\n\n");
    assert_eq!(
        ok(&garbage).last(),
        Some(&ModelEvent::Stop(StopReason::EndTurn))
    );
}

#[test]
fn non_streamed_message_bodies_are_understood() {
    let body = json!({
        "choices": [{
            "message": {"role": "assistant", "content": "done", "tool_calls": [
                {"id": "c9", "type": "function", "function": {"name": "Read", "arguments": {"file_path": "x"}}}
            ]},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 5, "completion_tokens": 2}
    });
    let events = ok(&chunk(body));
    assert_eq!(events[0], ModelEvent::TextDelta("done".into()));
    assert_eq!(
        events[2],
        ModelEvent::ToolUseDelta {
            index: 0,
            partial_json: "{\"file_path\":\"x\"}".into()
        }
    );
    assert_eq!(
        &events[3..],
        [
            ModelEvent::ToolUseEnd { index: 0 },
            usage(5, 2),
            ModelEvent::Stop(StopReason::ToolUse)
        ]
    );
}

#[test]
fn finish_reasons_map_to_stop_reasons() {
    assert_eq!(stop_reason("stop", false), StopReason::EndTurn);
    assert_eq!(stop_reason("tool_calls", false), StopReason::ToolUse);
    assert_eq!(stop_reason("function_call", false), StopReason::ToolUse);
    assert_eq!(stop_reason("length", true), StopReason::MaxTokens);
    assert_eq!(stop_reason("content_filter", false), StopReason::Refusal);
    assert_eq!(stop_reason("eos", false), StopReason::Other("eos".into()));
}
