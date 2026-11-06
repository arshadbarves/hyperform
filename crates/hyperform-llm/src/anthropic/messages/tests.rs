use hyperform_protocol::Message;
use serde_json::json;

use super::*;

fn map(
    messages: Vec<Message>,
    breakpoints: Vec<usize>,
    budget: usize,
    thinking: bool,
) -> Vec<Value> {
    let mut request = ModelRequest::new("claude-test", messages);
    request.cache_breakpoints = breakpoints;
    build(&request, budget, thinking)
}

fn user(blocks: Vec<ContentBlock>) -> Message {
    Message::new(Role::User, blocks)
}

fn assistant(blocks: Vec<ContentBlock>) -> Message {
    Message::new(Role::Assistant, blocks)
}

fn signed_thinking() -> ContentBlock {
    ContentBlock::Thinking {
        text: "plan".into(),
        signature: Some("sig".into()),
    }
}

#[test]
fn consecutive_same_role_messages_merge_and_empty_ones_vanish() {
    let out = map(
        vec![
            Message::user_text("a"),
            Message::user_text("b"),
            Message::assistant_text("   "),
            Message::user_text("c"),
            Message::assistant_text("x"),
            Message::assistant_text("y"),
        ],
        vec![],
        4,
        false,
    );
    assert_eq!(
        out,
        [
            json!({"role": "user", "content": [
                {"type": "text", "text": "a"}, {"type": "text", "text": "b"}, {"type": "text", "text": "c"}
            ]}),
            json!({"role": "assistant", "content": [
                {"type": "text", "text": "x"}, {"type": "text", "text": "y"}
            ]}),
        ]
    );
}

#[test]
fn tool_results_open_the_user_turn_even_after_merging() {
    let out = map(
        vec![
            user(vec![ContentBlock::text("note")]),
            user(vec![
                ContentBlock::text("steer"),
                ContentBlock::tool_result(CallId::from("t1"), "ok", false),
                ContentBlock::ToolResult {
                    tool_use_id: CallId::from("t2"),
                    content: vec![
                        ToolResultPart::Text {
                            text: "boom".into(),
                        },
                        ToolResultPart::Image {
                            source: MediaSource::Url {
                                url: "https://x/y.png".into(),
                            },
                        },
                    ],
                    is_error: true,
                },
            ]),
        ],
        vec![],
        4,
        false,
    );
    let content = out[0]["content"].as_array().unwrap();
    let types: Vec<&str> = content
        .iter()
        .map(|block| block["type"].as_str().unwrap())
        .collect();
    assert_eq!(types, ["tool_result", "tool_result", "text", "text"]);
    assert_eq!(
        content[0],
        json!({"type": "tool_result", "tool_use_id": "t1", "content": [{"type": "text", "text": "ok"}]})
    );
    assert_eq!(content[1]["is_error"], true);
    assert_eq!(
        content[1]["content"][1]["source"],
        json!({"type": "url", "url": "https://x/y.png"})
    );
    assert_eq!(content[2]["text"], "note");
}

#[test]
fn reasoning_replays_only_when_signed_and_thinking_is_on() {
    let turn = assistant(vec![
        signed_thinking(),
        ContentBlock::Thinking {
            text: "unsigned".into(),
            signature: None,
        },
        ContentBlock::Thinking {
            text: "from the Responses API".into(),
            signature: Some(crate::responses::signature("blob")),
        },
        ContentBlock::RedactedThinking {
            data: "opaque".into(),
        },
        ContentBlock::ToolUse {
            id: CallId::from("t1"),
            name: "Read".into(),
            input: json!({"file_path": "a"}),
        },
    ]);
    let on = map(vec![turn.clone()], vec![], 4, true);
    assert_eq!(
        on[0]["content"],
        json!([
            {"type": "thinking", "thinking": "plan", "signature": "sig"},
            {"type": "redacted_thinking", "data": "opaque"},
            {"type": "tool_use", "id": "t1", "name": "Read", "input": {"file_path": "a"}}
        ])
    );
    let off = map(vec![turn], vec![], 4, false);
    assert_eq!(off[0]["content"].as_array().unwrap().len(), 1);
    assert_eq!(off[0]["content"][0]["type"], "tool_use");
}

#[test]
fn media_blocks_map_to_sources() {
    let out = map(
        vec![user(vec![
            ContentBlock::Image {
                source: MediaSource::Base64 {
                    media_type: "image/png".into(),
                    data: "AAAA".into(),
                },
            },
            ContentBlock::Document {
                source: MediaSource::Base64 {
                    media_type: "application/pdf".into(),
                    data: "JVBE".into(),
                },
                title: Some("spec.pdf".into()),
            },
        ])],
        vec![],
        4,
        false,
    );
    assert_eq!(
        out[0]["content"],
        json!([
            {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "AAAA"}},
            {"type": "document", "source": {"type": "base64", "media_type": "application/pdf", "data": "JVBE"}, "title": "spec.pdf"}
        ])
    );
}

#[test]
fn breakpoints_mark_the_last_cacheable_block_within_budget() {
    let messages = vec![
        Message::user_text("q1"),
        assistant(vec![ContentBlock::text("a1"), signed_thinking()]),
        Message::user_text("q2"),
        Message::user_text("q3"),
    ];
    let out = map(messages.clone(), vec![1, 2, 3, 3, 42], 4, true);
    assert_eq!(
        out[1]["content"][0]["cache_control"],
        json!({"type": "ephemeral"})
    );
    assert!(out[1]["content"][1].get("cache_control").is_none());
    let merged = out[2]["content"].as_array().unwrap();
    assert!(
        merged
            .iter()
            .all(|block| block.get("cache_control").is_some())
    );
    let limited = map(messages, vec![0, 1, 2, 3], 1, true);
    let marks: Vec<bool> = limited
        .iter()
        .flat_map(|message| message["content"].as_array().unwrap().clone())
        .map(|block| block.get("cache_control").is_some())
        .collect();
    assert_eq!(marks, [false, false, false, false, true]);
}
