use super::*;

fn png() -> MediaSource {
    MediaSource::Base64 {
        media_type: "image/png".into(),
        data: "AAAA".into(),
    }
}

fn user(blocks: Vec<ContentBlock>) -> Message {
    Message::new(Role::User, blocks)
}

fn assistant(blocks: Vec<ContentBlock>) -> Message {
    Message::new(Role::Assistant, blocks)
}

fn reasoned_call(id: &str, summary: &str, blob: &str) -> Message {
    assistant(vec![
        ContentBlock::Thinking {
            text: summary.into(),
            signature: Some(reasoning::signature(blob)),
        },
        ContentBlock::text("Reading."),
        ContentBlock::ToolUse {
            id: CallId::from(id),
            name: "Read".into(),
            input: json!({"file_path": "a.rs"}),
        },
    ])
}

fn result(id: &str) -> Message {
    user(vec![ContentBlock::tool_result(
        CallId::from(id),
        "body",
        false,
    )])
}

fn types(items: &[Value]) -> Vec<&str> {
    items
        .iter()
        .map(|item| item["type"].as_str().or(item["role"].as_str()).unwrap())
        .collect()
}

#[test]
fn assistant_turns_become_messages_calls_and_reasoning() {
    let items = input(&[
        Message::user_text("task"),
        reasoned_call("c1", "plan", "b1"),
    ]);
    assert_eq!(
        items,
        [
            json!({"role": "user", "content": [{"type": "input_text", "text": "task"}]}),
            json!({
                "type": "reasoning",
                "summary": [{"type": "summary_text", "text": "plan"}],
                "encrypted_content": "b1",
            }),
            json!({"role": "assistant", "content": [{"type": "output_text", "text": "Reading."}]}),
            json!({
                "type": "function_call",
                "call_id": "c1",
                "name": "Read",
                "arguments": "{\"file_path\":\"a.rs\"}",
            }),
        ]
    );
}

#[test]
fn reasoning_replays_only_after_the_last_user_input() {
    let reminder = ContentBlock::text("<system-reminder>\nsteer\n</system-reminder>");
    let messages = vec![
        Message::user_text("first"),
        reasoned_call("c1", "old", "b1"),
        result("c1"),
        Message::assistant_text("done"),
        Message::user_text("second"),
        reasoned_call("c2", "", "b2"),
        user(vec![
            ContentBlock::tool_result(CallId::from("c2"), "body", false),
            reminder,
        ]),
        reasoned_call("c3", "new", "b3"),
    ];
    let items = input(&messages);
    let blobs: Vec<&str> = items
        .iter()
        .filter_map(|item| item["encrypted_content"].as_str())
        .collect();
    assert_eq!(blobs, ["b2", "b3"]);
    let empty = items
        .iter()
        .find(|item| item["encrypted_content"] == "b2")
        .unwrap();
    assert_eq!(empty["summary"], json!([]));
}

#[test]
fn foreign_and_unsigned_reasoning_is_never_sent() {
    let turn = assistant(vec![
        ContentBlock::Thinking {
            text: "claude".into(),
            signature: Some("EqQBCkYIBRgCKkA".into()),
        },
        ContentBlock::Thinking {
            text: "chat".into(),
            signature: None,
        },
        ContentBlock::RedactedThinking {
            data: "opaque".into(),
        },
        ContentBlock::text("ok"),
    ]);
    let items = input(&[Message::user_text("hi"), turn]);
    assert_eq!(types(&items), ["user", "assistant"]);
}

#[test]
fn tool_results_lead_and_their_images_follow_as_user_content() {
    let round = user(vec![
        ContentBlock::text("keep going"),
        ContentBlock::tool_result(CallId::from("c1"), "", false),
        ContentBlock::ToolResult {
            tool_use_id: CallId::from("c2"),
            content: vec![
                ToolResultPart::Text {
                    text: "shot".into(),
                },
                ToolResultPart::Image { source: png() },
            ],
            is_error: false,
        },
    ]);
    let items = input(&[round]);
    assert_eq!(
        items[0],
        json!({"type": "function_call_output", "call_id": "c1", "output": EMPTY_TOOL_OUTPUT})
    );
    assert_eq!(items[1]["output"], "shot");
    assert_eq!(
        items[2]["content"],
        json!([
            {"type": "input_text", "text": "Images returned by tool call c2:"},
            {"type": "input_image", "image_url": "data:image/png;base64,AAAA"},
            {"type": "input_text", "text": "keep going"},
        ])
    );
}

#[test]
fn user_media_map_to_input_parts() {
    let pdf = MediaSource::Base64 {
        media_type: "application/pdf".into(),
        data: "JVBE".into(),
    };
    let remote = MediaSource::Url {
        url: "https://example.com/spec.pdf".into(),
    };
    let items = input(&[user(vec![
        ContentBlock::Image { source: png() },
        ContentBlock::Document {
            source: pdf,
            title: Some("spec.pdf".into()),
        },
        ContentBlock::Document {
            source: remote,
            title: None,
        },
        ContentBlock::text(""),
    ])]);
    assert_eq!(
        items[0]["content"],
        json!([
            {"type": "input_image", "image_url": "data:image/png;base64,AAAA"},
            {"type": "input_file", "filename": "spec.pdf", "file_data": "data:application/pdf;base64,JVBE"},
            {"type": "input_file", "file_url": "https://example.com/spec.pdf"},
        ])
    );
}
