//! A streamed OpenAI-compatible tool call over HTTP, end to end.

mod support;

use serde_json::json;
use tokio_util::sync::CancellationToken;
use z_engine_llm::{
    ModelEvent, ModelRequest, ProviderConfig, StopReason, SystemBlock, ToolSpec, build_client,
    collect,
};
use z_engine_protocol::{ContentBlock, Message, Usage};

use support::server::{MockServer, Reply};
use support::stream::{config, drain};

const TOOL_CALL: &str = include_str!("fixtures/sse/tool_call_trailing_usage.sse");

fn request() -> ModelRequest {
    let read = ToolSpec {
        name: "Read".into(),
        description: "Read a file".into(),
        input_schema: json!({
            "type": "object",
            "properties": {"file_path": {"type": "string"}},
            "required": ["file_path"],
        }),
    };
    ModelRequest::new("openai/gpt-test", vec![Message::user_text("show main.rs")])
        .with_system(vec![SystemBlock::cached("You are a coding agent.")])
        .with_tools(vec![read])
}

#[tokio::test]
async fn streamed_tool_call_arrives_in_order() {
    let server = MockServer::start(vec![Reply::Sse(TOOL_CALL.into())]).await;
    let client = build_client(&config(&format!("{}/v1/", server.url))).unwrap();
    let events: Vec<ModelEvent> = drain(client.stream(request(), CancellationToken::new()))
        .await
        .into_iter()
        .map(|item| item.expect("stream error"))
        .collect();
    assert_eq!(events[0], ModelEvent::TextDelta("Reading.".into()));
    assert_eq!(
        events[1],
        ModelEvent::ToolUseStart {
            index: 0,
            id: "call_read_1".into(),
            name: "Read".into()
        }
    );
    let arguments: String = events
        .iter()
        .filter_map(|event| match event {
            ModelEvent::ToolUseDelta { partial_json, .. } => Some(partial_json.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(arguments, r#"{"file_path": "src/main.rs"}"#);
    assert_eq!(
        &events[events.len() - 3..],
        [
            ModelEvent::ToolUseEnd { index: 0 },
            ModelEvent::Usage(Usage {
                input_tokens: 64,
                output_tokens: 24,
                cache_read_tokens: 256,
                ..Usage::default()
            }),
            ModelEvent::Stop(StopReason::ToolUse),
        ]
    );
}

#[tokio::test]
async fn request_carries_auth_and_the_mapped_body() {
    let server = MockServer::start(vec![Reply::Sse(TOOL_CALL.into())]).await;
    let client = build_client(&config(&format!("{}/v1", server.url))).unwrap();
    assert_eq!(client.provider(), "custom");
    let turn = collect(client.stream(request(), CancellationToken::new()))
        .await
        .unwrap();
    let call = turn.content.iter().find_map(|block| match block {
        ContentBlock::ToolUse { id, name, input } => Some((id.as_str(), name.as_str(), input)),
        _ => None,
    });
    assert_eq!(
        call,
        Some(("call_read_1", "Read", &json!({"file_path": "src/main.rs"})))
    );

    let recorded = server.requests();
    assert_eq!(recorded.len(), 1);
    let sent = &recorded[0];
    assert_eq!(sent.path, "/v1/chat/completions");
    assert_eq!(sent.headers["authorization"], "Bearer test-key");
    assert_eq!(sent.headers["content-type"], "application/json");
    assert_eq!(sent.body["model"], "openai/gpt-test");
    assert_eq!(sent.body["stream"], true);
    assert_eq!(sent.body["stream_options"], json!({"include_usage": true}));
    assert_eq!(
        sent.body["messages"],
        json!([
            {"role": "system", "content": "You are a coding agent."},
            {"role": "user", "content": "show main.rs"}
        ])
    );
    assert_eq!(sent.body["tools"][0]["function"]["name"], "Read");
    assert_eq!(sent.body["tool_choice"], "auto");
    assert_eq!(sent.body["max_tokens"], 4_096);
}

#[tokio::test]
async fn gateway_answering_without_streaming_still_yields_the_call() {
    let completion = json!({
        "choices": [{
            "message": {"role": "assistant", "content": null, "tool_calls": [{
                "id": "call_json", "type": "function",
                "function": {"name": "Read", "arguments": "{\"file_path\":\"a.rs\"}"}
            }]},
            "finish_reason": "tool_calls"
        }],
        "usage": {"prompt_tokens": 11, "completion_tokens": 5}
    });
    let reply = Reply::status(
        200,
        &[("content-type", "application/json")],
        &completion.to_string(),
    );
    let server = MockServer::start(vec![reply]).await;
    let client = build_client(&config(&server.url)).unwrap();
    let turn = collect(client.stream(request(), CancellationToken::new()))
        .await
        .unwrap();
    assert_eq!(turn.stop, Some(StopReason::ToolUse));
    assert_eq!(turn.usage.input_tokens, 11);
    assert!(matches!(
        &turn.content[0],
        ContentBlock::ToolUse { id, input, .. }
            if id.as_str() == "call_json" && *input == json!({"file_path": "a.rs"})
    ));
}

#[tokio::test]
async fn keyless_local_servers_get_no_authorization_header() {
    let server = MockServer::start(vec![Reply::Sse(TOOL_CALL.into())]).await;
    let config = ProviderConfig {
        api_key: None,
        ..config(&server.url)
    };
    let client = build_client(&config).unwrap();
    drain(client.stream(request(), CancellationToken::new())).await;
    assert!(!server.requests()[0].headers.contains_key("authorization"));
}
