use z_engine_protocol::Message;

use super::*;

fn png() -> MediaSource {
    MediaSource::Base64 {
        media_type: "image/png".into(),
        data: "AAAA".into(),
    }
}

fn map(messages: Vec<Message>, breakpoints: Vec<usize>, cache: bool) -> Vec<Value> {
    let mut request = ModelRequest::new("m", messages);
    request.cache_breakpoints = breakpoints;
    conversation(&request, cache)
}

fn tool_round() -> Message {
    Message::new(
        Role::User,
        vec![
            ContentBlock::text("keep going"),
            ContentBlock::tool_result(CallId::from("c1"), "file body", false),
            ContentBlock::ToolResult {
                tool_use_id: CallId::from("c2"),
                content: vec![ToolResultPart::Image { source: png() }],
                is_error: false,
            },
        ],
    )
}

#[test]
fn user_text_stays_a_string_and_media_become_parts() {
    let plain = map(vec![Message::user_text("hi")], vec![], false);
    assert_eq!(plain, [json!({"role": "user", "content": "hi"})]);
    let rich = Message::new(
        Role::User,
        vec![
            ContentBlock::text("look"),
            ContentBlock::Image { source: png() },
            ContentBlock::Document {
                source: MediaSource::Base64 {
                    media_type: "application/pdf".into(),
                    data: "JVBE".into(),
                },
                title: None,
            },
        ],
    );
    let content = &map(vec![rich], vec![], false)[0]["content"];
    assert_eq!(content[0], json!({"type": "text", "text": "look"}));
    assert_eq!(content[1]["image_url"]["url"], "data:image/png;base64,AAAA");
    assert_eq!(
        content[2],
        json!({"type": "file", "file": {"filename": "document.pdf", "file_data": "data:application/pdf;base64,JVBE"}})
    );
}

#[test]
fn tool_results_come_first_and_images_follow_as_a_user_message() {
    let out = map(vec![tool_round()], vec![], false);
    assert_eq!(out.len(), 3);
    assert_eq!(
        out[0],
        json!({"role": "tool", "tool_call_id": "c1", "content": "file body"})
    );
    assert_eq!(out[1]["tool_call_id"], "c2");
    assert_eq!(out[1]["content"], EMPTY_TOOL_OUTPUT);
    let user = &out[2]["content"];
    assert_eq!(user[0]["text"], "Images returned by tool call c2:");
    assert_eq!(user[1]["type"], "image_url");
    assert_eq!(user[2], json!({"type": "text", "text": "keep going"}));
}

#[test]
fn assistant_joins_text_and_serializes_tool_calls() {
    let assistant = Message::new(
        Role::Assistant,
        vec![
            ContentBlock::Thinking {
                text: "hidden".into(),
                signature: Some("sig".into()),
            },
            ContentBlock::text("Reading."),
            ContentBlock::ToolUse {
                id: CallId::from("c1"),
                name: "Read".into(),
                input: json!({"file_path": "a.rs"}),
            },
        ],
    );
    let tool_only = Message::new(
        Role::Assistant,
        vec![ContentBlock::ToolUse {
            id: CallId::from("c2"),
            name: "Bash".into(),
            input: json!({}),
        }],
    );
    let reasoning_only = Message::new(
        Role::Assistant,
        vec![ContentBlock::RedactedThinking { data: "x".into() }],
    );
    let out = map(vec![assistant, tool_only, reasoning_only], vec![], false);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0]["content"], "Reading.");
    assert_eq!(
        out[0]["tool_calls"],
        json!([{"id": "c1", "type": "function", "function": {"name": "Read", "arguments": "{\"file_path\":\"a.rs\"}"}}])
    );
    assert_eq!(out[1]["content"], Value::Null);
    assert_eq!(out[1]["tool_calls"][0]["function"]["arguments"], "{}");
}

#[test]
fn breakpoints_mark_the_last_text_part_of_the_message() {
    let out = map(
        vec![Message::user_text("a"), tool_round()],
        vec![0, 1, 9],
        true,
    );
    let ephemeral = json!({"type": "ephemeral"});
    assert_eq!(
        out[0]["content"],
        json!([{"type": "text", "text": "a", "cache_control": ephemeral}])
    );
    let last_user = out.last().unwrap();
    assert_eq!(last_user["content"][2]["cache_control"], ephemeral);
    assert!(last_user["content"][0].get("cache_control").is_none());
    let tool_last = map(
        vec![Message::new(
            Role::User,
            vec![ContentBlock::tool_result(CallId::from("c1"), "ok", false)],
        )],
        vec![0],
        true,
    );
    assert_eq!(
        tool_last[0]["content"],
        json!([{"type": "text", "text": "ok", "cache_control": ephemeral}])
    );
    let disabled = map(vec![Message::user_text("a")], vec![0], false);
    assert_eq!(disabled[0]["content"], "a");
}
