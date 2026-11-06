//! The GitHub Copilot client over HTTP: `/models` routing, the three APIs,
//! Copilot's headers, and the fallbacks when the list or the token fails.

mod support;

use std::sync::Arc;
use std::time::{Duration, Instant};

use hyperform_llm::{
    LlmError, ModelClient, ModelEvent, ModelRequest, StopReason, SystemBlock, ThinkingConfig,
    ToolSpec, build_copilot_client, collect,
};
use hyperform_protocol::{
    CallId, ContentBlock, Effort, MediaSource, Message, Role, ToolResultPart,
};
use serde_json::json;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use support::server::{MockServer, Reply};
use support::stream::{config, drain, next};

const MODELS: &str = include_str!("fixtures/copilot_models.json");
const MESSAGES: &str = include_str!("fixtures/sse/anthropic_thinking_tool.sse");
const RESPONSES: &str = include_str!("fixtures/sse/responses_reasoning_tool.sse");
const CHAT: &str = include_str!("fixtures/sse/tool_call_trailing_usage.sse");
const SESSION: &str = "01K5ZQ8X1B2C3D4E5F6G7H8J9K";
const STARTED: &str =
    "data: {\"type\":\"response.output_text.delta\",\"output_index\":0,\"delta\":\"started\"}\n\n";
const PROMPT: Duration = Duration::from_secs(1);

fn models() -> Reply {
    Reply::status(200, &[("content-type", "application/json")], MODELS)
}

fn copilot(url: &str) -> Arc<dyn ModelClient> {
    build_copilot_client(&config(url)).unwrap()
}

fn request(model: &str, messages: Vec<Message>) -> ModelRequest {
    let read = ToolSpec {
        name: "Read".into(),
        description: "Read a file".into(),
        input_schema: json!({"type": "object", "properties": {"file_path": {"type": "string"}}}),
    };
    let mut request = ModelRequest::new(model, messages)
        .with_system(vec![SystemBlock::new("You are a coding agent.")])
        .with_tools(vec![read])
        .with_max_tokens(8_000);
    request.thinking = Some(ThinkingConfig {
        effort: Effort::Medium,
        budget_tokens: None,
    });
    request.session_key = Some(SESSION.into());
    request
}

fn ask(model: &str) -> ModelRequest {
    request(model, vec![Message::user_text("read main.rs")])
}

fn calls(server: &MockServer) -> Vec<(String, String)> {
    server
        .requests()
        .into_iter()
        .map(|sent| (sent.method, sent.path))
        .collect()
}

fn call(method: &str, path: &str) -> (String, String) {
    (method.to_string(), path.to_string())
}

#[tokio::test]
async fn each_model_goes_to_the_api_its_listing_names() {
    let replies = vec![
        models(),
        Reply::Sse(MESSAGES.into()),
        Reply::Sse(RESPONSES.into()),
        Reply::Sse(CHAT.into()),
    ];
    let server = MockServer::start(replies).await;
    let client = copilot(&server.url);
    assert_eq!(client.provider(), "github-copilot");
    for model in ["claude-sonnet-4.5", "gpt-5", "gpt-4.1"] {
        let turn = collect(client.stream(ask(model), CancellationToken::new()))
            .await
            .unwrap();
        assert_eq!(turn.stop, Some(StopReason::ToolUse), "{model}");
    }
    assert_eq!(
        calls(&server),
        [
            call("GET", "/models"),
            call("POST", "/v1/messages"),
            call("POST", "/responses"),
            call("POST", "/chat/completions"),
        ]
    );
    let sent = server.requests();
    for recorded in &sent {
        let headers = &recorded.headers;
        assert_eq!(headers["authorization"], "Bearer test-key");
        assert!(
            headers["user-agent"]
                .to_str()
                .unwrap()
                .starts_with("opencode/")
        );
        assert_eq!(headers["x-github-api-version"], "2026-06-01");
        assert_eq!(headers["openai-intent"], "conversation-edits");
        assert!(!headers.contains_key("x-api-key"));
    }
    for recorded in &sent[1..] {
        assert_eq!(recorded.headers["x-initiator"], "user");
        assert_eq!(recorded.headers["x-interaction-id"], SESSION);
        assert!(!recorded.headers.contains_key("copilot-vision-request"));
    }
    let (messages, responses, chat) = (&sent[1], &sent[2], &sent[3]);
    assert_eq!(messages.headers["anthropic-version"], "2023-06-01");
    assert_eq!(
        messages.headers["anthropic-beta"],
        "interleaved-thinking-2025-05-14"
    );
    assert_eq!(messages.body["max_tokens"], 8_000);
    assert_eq!(messages.body["thinking"]["type"], "enabled");
    assert!(!responses.headers.contains_key("anthropic-beta"));
    assert_eq!(responses.body["store"], false);
    assert_eq!(
        responses.body["include"],
        json!(["reasoning.encrypted_content"])
    );
    assert_eq!(
        responses.body["reasoning"],
        json!({"effort": "medium", "summary": "auto"})
    );
    assert_eq!(responses.body["instructions"], "You are a coding agent.");
    assert!(responses.body.get("max_output_tokens").is_none());
    assert_eq!(chat.body["model"], "gpt-4.1");
    assert!(chat.body.get("max_tokens").is_none());
    assert!(chat.body.get("reasoning_effort").is_none());
    assert_eq!(chat.body["tools"][0]["function"]["name"], "Read");
}

#[tokio::test]
async fn a_tool_round_replays_reasoning_as_an_agent_vision_request() {
    let replies = vec![
        models(),
        Reply::Sse(RESPONSES.into()),
        Reply::Sse(RESPONSES.into()),
    ];
    let server = MockServer::start(replies).await;
    let client = copilot(&server.url);
    let first = collect(client.stream(ask("gpt-5"), CancellationToken::new()))
        .await
        .unwrap();
    let screenshot = ContentBlock::ToolResult {
        tool_use_id: CallId::from("call_read_1"),
        content: vec![ToolResultPart::Image {
            source: MediaSource::Base64 {
                media_type: "image/png".into(),
                data: "AAAA".into(),
            },
        }],
        is_error: false,
    };
    let history = vec![
        Message::user_text("read main.rs"),
        Message::new(Role::Assistant, first.content),
        Message::new(Role::User, vec![screenshot]),
    ];
    drain(client.stream(request("gpt-5", history), CancellationToken::new())).await;
    let sent = server.requests();
    assert_eq!(sent.len(), 3, "the model list is fetched once");
    let round = &sent[2];
    assert_eq!(round.headers["x-initiator"], "agent");
    assert_eq!(round.headers["copilot-vision-request"], "true");
    let input = round.body["input"].as_array().unwrap();
    let reasoning = input
        .iter()
        .find(|item| item["type"] == "reasoning")
        .unwrap();
    assert_eq!(reasoning["encrypted_content"], "gAAAAAenc");
    assert!(input.iter().any(|item| {
        item["type"] == "function_call_output" && item["call_id"] == "call_read_1"
    }));
}

#[tokio::test]
async fn a_failed_model_list_routes_by_name() {
    let replies = vec![
        Reply::status(500, &[], "down"),
        Reply::Sse(RESPONSES.into()),
        Reply::Sse(CHAT.into()),
    ];
    let server = MockServer::start(replies).await;
    let client = copilot(&server.url);
    for model in ["gpt-5", "claude-sonnet-4.5"] {
        let turn = collect(client.stream(ask(model), CancellationToken::new())).await;
        assert!(turn.is_ok(), "{model}: {turn:?}");
    }
    assert_eq!(
        calls(&server),
        [
            call("GET", "/models"),
            call("POST", "/responses"),
            call("POST", "/chat/completions"),
        ]
    );
}

#[tokio::test]
async fn disabled_models_fail_without_a_request() {
    let server = MockServer::start(vec![models()]).await;
    let client = copilot(&server.url);
    let items = drain(client.stream(ask("o3-pro"), CancellationToken::new())).await;
    assert_eq!(items.len(), 1);
    assert!(
        matches!(&items[0], Err(LlmError::Config(message)) if message.contains("o3-pro")),
        "{items:?}"
    );
    assert_eq!(calls(&server), [call("GET", "/models")]);
}

#[tokio::test]
async fn a_refused_token_suggests_signing_in_again() {
    let replies = vec![models(), Reply::status(401, &[], "bad credentials")];
    let server = MockServer::start(replies).await;
    let client = copilot(&server.url);
    let items = drain(client.stream(ask("gpt-4.1"), CancellationToken::new())).await;
    assert!(
        matches!(
            items.last(),
            Some(Err(LlmError::Http { status: 401, body }))
                if body.contains("sign in to GitHub Copilot again") && body.contains("bad credentials")
        ),
        "{items:?}"
    );
}

#[tokio::test]
async fn cancelling_while_the_model_list_hangs_is_prompt() {
    let server = MockServer::start(vec![Reply::Hang]).await;
    let client = copilot(&server.url);
    let cancel = CancellationToken::new();
    let mut stream = client.stream(ask("gpt-5"), cancel.clone());
    let deadline = Instant::now() + Duration::from_secs(5);
    while server.requests().is_empty() {
        assert!(
            Instant::now() < deadline,
            "the model list was never requested"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let cancelled_at = Instant::now();
    cancel.cancel();
    assert_eq!(next(&mut stream).await, Some(Err(LlmError::Cancelled)));
    assert_eq!(next(&mut stream).await, None);
    assert!(cancelled_at.elapsed() < PROMPT);
}

#[tokio::test]
async fn cancelling_or_dropping_mid_stream_releases_the_connection() {
    for drop_it in [false, true] {
        let (closed, released) = oneshot::channel();
        let stall = Reply::Stall {
            prefix: STARTED.into(),
            closed,
        };
        let server = MockServer::start(vec![models(), stall]).await;
        let client = copilot(&server.url);
        let cancel = CancellationToken::new();
        let mut stream = client.stream(ask("gpt-5"), cancel.clone());
        assert_eq!(
            next(&mut stream).await,
            Some(Ok(ModelEvent::TextDelta("started".into())))
        );
        if drop_it {
            drop(stream);
        } else {
            cancel.cancel();
            assert_eq!(next(&mut stream).await, Some(Err(LlmError::Cancelled)));
            assert_eq!(next(&mut stream).await, None);
        }
        let outcome = tokio::time::timeout(Duration::from_secs(5), released).await;
        assert!(
            outcome.is_ok(),
            "the server kept streaming to a gone client"
        );
    }
}
