//! The native Anthropic adapter end to end over HTTP.

mod support;

use serde_json::json;
use tokio_util::sync::CancellationToken;
use z_engine_llm::{
    LlmError, ModelRequest, ProviderConfig, ProviderKind, StopReason, SystemBlock, ThinkingConfig,
    ToolSpec, build_client, collect,
};
use z_engine_protocol::{ContentBlock, Effort, Message, Usage};

use support::server::{MockServer, Reply};
use support::stream::{config, drain};

const THINKING_TOOL: &str = include_str!("fixtures/sse/anthropic_thinking_tool.sse");
const OVERLOADED: &str = include_str!("fixtures/sse/anthropic_overloaded.sse");

fn anthropic(url: &str) -> ProviderConfig {
    ProviderConfig {
        kind: Some(ProviderKind::Anthropic),
        ..config(&format!("{url}/v1/"))
    }
}

fn request() -> ModelRequest {
    let read = ToolSpec {
        name: "Read".into(),
        description: "Read a file".into(),
        input_schema: json!({"type": "object", "properties": {"file_path": {"type": "string"}}}),
    };
    let mut request = ModelRequest::new(
        "claude-sonnet-4-5",
        vec![Message::user_text("read main.rs")],
    )
    .with_system(vec![SystemBlock::cached("You are a coding agent.")])
    .with_tools(vec![read])
    .with_max_tokens(32_000);
    request.thinking = Some(ThinkingConfig {
        effort: Effort::Medium,
        budget_tokens: None,
    });
    request.temperature = Some(0.2);
    request.cache_tools = true;
    request.cache_breakpoints = vec![0];
    request
}

#[tokio::test]
async fn thinking_tool_turn_streams_end_to_end() {
    let server = MockServer::start(vec![Reply::Sse(THINKING_TOOL.into())]).await;
    let client = build_client(&anthropic(&server.url)).unwrap();
    let turn = collect(client.stream(request(), CancellationToken::new()))
        .await
        .unwrap();
    assert_eq!(turn.stop, Some(StopReason::ToolUse));
    assert_eq!(
        turn.usage,
        Usage {
            input_tokens: 10,
            output_tokens: 85,
            cache_read_tokens: 1_800,
            cache_write_tokens: 200,
            reasoning_tokens: 0,
        }
    );
    assert!(matches!(
        &turn.content[0],
        ContentBlock::Thinking { text, signature: Some(_) }
            if text == "The user wants main.rs. I should read it."
    ));
    assert_eq!(turn.text(), "Let me read it.");
    assert!(matches!(
        &turn.content[2],
        ContentBlock::ToolUse { id, name, input }
            if id.as_str() == "toolu_01READ" && name == "Read" && *input == json!({"file_path": "src/main.rs"})
    ));
}

#[tokio::test]
async fn request_uses_the_messages_endpoint_headers_and_body() {
    let server = MockServer::start(vec![Reply::Sse(THINKING_TOOL.into())]).await;
    let client = build_client(&anthropic(&server.url)).unwrap();
    drain(client.stream(request(), CancellationToken::new())).await;
    let sent = &server.requests()[0];
    assert_eq!(sent.path, "/v1/messages");
    assert_eq!(sent.headers["x-api-key"], "test-key");
    assert_eq!(sent.headers["anthropic-version"], "2023-06-01");
    assert_eq!(
        sent.headers["anthropic-beta"],
        "interleaved-thinking-2025-05-14"
    );
    assert!(!sent.headers.contains_key("authorization"));
    let body = &sent.body;
    assert_eq!(body["stream"], true);
    assert_eq!(body["max_tokens"], 32_000);
    assert_eq!(
        body["thinking"],
        json!({"type": "enabled", "budget_tokens": 8_192})
    );
    assert!(body.get("temperature").is_none());
    assert_eq!(body["tool_choice"], json!({"type": "auto"}));
    let ephemeral = json!({"type": "ephemeral"});
    assert_eq!(body["system"][0]["cache_control"], ephemeral);
    assert_eq!(body["tools"][0]["cache_control"], ephemeral);
    assert_eq!(
        body["messages"][0]["content"][0]["cache_control"],
        ephemeral
    );
}

#[tokio::test]
async fn overloaded_error_event_surfaces_typed() {
    let server = MockServer::start(vec![Reply::Sse(OVERLOADED.into())]).await;
    let client = build_client(&anthropic(&server.url)).unwrap();
    let items = drain(client.stream(request(), CancellationToken::new())).await;
    let error = items.last().cloned().unwrap().unwrap_err();
    assert_eq!(
        error,
        LlmError::Overloaded {
            attempts: 1,
            detail: "Overloaded".into()
        }
    );
    assert!(error.is_retryable());
}

#[tokio::test]
async fn http_errors_keep_status_and_body() {
    let body = r#"{"type":"error","error":{"type":"invalid_request_error","message":"prompt is too long: 210000 tokens > 200000 maximum"}}"#;
    let server = MockServer::start(vec![Reply::status(400, &[], body)]).await;
    let client = build_client(&anthropic(&server.url)).unwrap();
    let items = drain(client.stream(request(), CancellationToken::new())).await;
    assert_eq!(items.len(), 1);
    let error = items[0].clone().unwrap_err();
    assert_eq!(
        error,
        LlmError::Http {
            status: 400,
            body: body.into()
        }
    );
    assert!(error.is_context_overflow());
}
