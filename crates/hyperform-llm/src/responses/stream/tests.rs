use hyperform_protocol::ContentBlock;
use serde_json::json;

use super::*;
use crate::accumulate::{AssistantTurn, ResponseAccumulator};
use crate::transport::parse_all;

const REASONING_TOOL: &str =
    include_str!("../../../tests/fixtures/sse/responses_reasoning_tool.sse");
const INCOMPLETE: &str = include_str!("../../../tests/fixtures/sse/responses_incomplete.sse");

fn events(body: &str) -> Vec<StreamItem> {
    parse_all(ResponsesStreamParser::default(), body)
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

fn chunk(value: Value) -> String {
    format!("data: {value}\n\n")
}

fn completed() -> String {
    chunk(json!({"type": "response.completed", "response": {"status": "completed"}}))
}

fn error_of(body: &str) -> LlmError {
    events(body).pop().unwrap().unwrap_err()
}

#[test]
fn items_are_tracked_by_output_index_despite_rotating_ids() {
    assert_eq!(
        ok(REASONING_TOOL),
        [
            ModelEvent::ThinkingDelta("Need to read ".into()),
            ModelEvent::ThinkingDelta("main.rs.".into()),
            ModelEvent::ThinkingDelta("\n\n".into()),
            ModelEvent::ThinkingDelta("Then answer.".into()),
            ModelEvent::ThinkingSignature(reasoning::signature("gAAAAAenc")),
            ModelEvent::TextDelta("Reading ".into()),
            ModelEvent::TextDelta("it.".into()),
            ModelEvent::ToolUseStart {
                index: 2,
                id: "call_read_1".into(),
                name: "Read".into(),
            },
            ModelEvent::ToolUseDelta {
                index: 2,
                partial_json: "{\"file_path\":".into(),
            },
            ModelEvent::ToolUseDelta {
                index: 2,
                partial_json: "\"src/main.rs\"}".into(),
            },
            ModelEvent::ToolUseEnd { index: 2 },
            ModelEvent::Usage(Usage {
                input_tokens: 200,
                output_tokens: 80,
                cache_read_tokens: 1_000,
                cache_write_tokens: 0,
                reasoning_tokens: 40,
            }),
            ModelEvent::Stop(StopReason::ToolUse),
        ]
    );
}

#[test]
fn a_reasoning_tool_turn_accumulates_with_its_signature() {
    let turn = turn(REASONING_TOOL);
    assert_eq!(turn.stop, Some(StopReason::ToolUse));
    assert_eq!(
        turn.content[0],
        ContentBlock::Thinking {
            text: "Need to read main.rs.\n\nThen answer.".into(),
            signature: Some(reasoning::signature("gAAAAAenc")),
        }
    );
    assert_eq!(turn.text(), "Reading it.");
    assert!(matches!(
        &turn.content[2],
        ContentBlock::ToolUse { id, name, input }
            if id.as_str() == "call_read_1" && name == "Read" && *input == json!({"file_path": "src/main.rs"})
    ));
    assert!(turn.malformed.is_empty());
}

#[test]
fn incomplete_responses_stop_at_max_tokens() {
    let events = ok(INCOMPLETE);
    assert_eq!(
        events[1..],
        [
            ModelEvent::Usage(Usage {
                input_tokens: 30,
                output_tokens: 16,
                reasoning_tokens: 12,
                ..Usage::default()
            }),
            ModelEvent::Stop(StopReason::MaxTokens),
        ]
    );
    let filtered = chunk(json!({
        "type": "response.incomplete",
        "response": {"incomplete_details": {"reason": "content_filter"}},
    }));
    assert_eq!(ok(&filtered), [ModelEvent::Stop(StopReason::Refusal)]);
}

#[test]
fn text_turns_end_and_lifecycle_noise_is_skipped() {
    let body = [
        "data: not json\n\n".to_string(),
        ": keep-alive\n\n".to_string(),
        chunk(json!({"type": "response.in_progress", "response": {}})),
        chunk(json!({"type": "response.output_text.delta", "output_index": 0, "delta": "hi"})),
        chunk(json!({"type": "response.output_text.delta", "output_index": 0, "delta": ""})),
        completed(),
        "data: [DONE]\n\n".to_string(),
    ]
    .concat();
    assert_eq!(
        ok(&body),
        [
            ModelEvent::TextDelta("hi".into()),
            ModelEvent::Stop(StopReason::EndTurn),
        ]
    );
}

#[test]
fn calls_and_summaries_seen_only_when_done_still_arrive() {
    let call = json!({
        "type": "function_call", "id": "fc_x", "call_id": "call_9",
        "name": "Bash", "arguments": "{\"command\":\"ls\"}",
    });
    let reasoning = json!({
        "type": "reasoning", "id": "rs_x", "encrypted_content": "blob",
        "summary": [{"type": "summary_text", "text": "Look first."}],
    });
    let body = [
        chunk(json!({"type": "response.output_item.done", "output_index": 0, "item": reasoning})),
        chunk(json!({"type": "response.output_item.done", "output_index": 1, "item": call})),
        completed(),
    ]
    .concat();
    let events = ok(&body);
    assert_eq!(
        events,
        [
            ModelEvent::ThinkingDelta("Look first.".into()),
            ModelEvent::ThinkingSignature(reasoning::signature("blob")),
            ModelEvent::ToolUseStart {
                index: 1,
                id: "call_9".into(),
                name: "Bash".into(),
            },
            ModelEvent::ToolUseDelta {
                index: 1,
                partial_json: "{\"command\":\"ls\"}".into(),
            },
            ModelEvent::ToolUseEnd { index: 1 },
            ModelEvent::Stop(StopReason::ToolUse),
        ]
    );
}

#[test]
fn an_open_call_is_closed_at_completion() {
    let added = json!({
        "type": "response.output_item.added", "output_index": 0,
        "item": {"type": "function_call", "call_id": "c1", "name": "Read", "arguments": ""},
    });
    let body = [chunk(added), completed()].concat();
    let events = ok(&body);
    assert_eq!(
        events[1..],
        [
            ModelEvent::ToolUseEnd { index: 0 },
            ModelEvent::Stop(StopReason::ToolUse),
        ]
    );
}

#[test]
fn errors_are_typed() {
    let rate = chunk(json!({"type": "error", "code": "rate_limit_exceeded", "message": "slow"}));
    assert_eq!(
        error_of(&rate),
        LlmError::RateLimited {
            attempts: 1,
            detail: "rate_limit_exceeded: slow".into()
        }
    );
    let failed = chunk(json!({
        "type": "response.failed",
        "response": {"error": {"code": "server_error", "message": "boom"}},
    }));
    let error = error_of(&failed);
    assert!(error.is_retryable(), "{error:?}");
    let overflow = chunk(json!({
        "type": "error",
        "error": {"code": "context_length_exceeded", "message": "too long"},
    }));
    let error = error_of(&overflow);
    assert!(
        error.is_context_overflow() && !error.is_retryable(),
        "{error:?}"
    );
}

#[test]
fn a_body_without_completion_is_truncated() {
    let body = chunk(json!({"type": "response.output_text.delta", "delta": "par"}));
    let items = events(&body);
    assert_eq!(items[0], Ok(ModelEvent::TextDelta("par".into())));
    assert!(matches!(items[1], Err(LlmError::Stream(_))));
}
