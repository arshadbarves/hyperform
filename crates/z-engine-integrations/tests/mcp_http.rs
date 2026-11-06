//! MCP over streamable HTTP against an axum fake: session and version
//! headers, JSON and SSE replies, server requests inside a stream, errors,
//! timeouts, cancellation, list changes, capability gating, and a manager
//! reconnect after the session expired.

mod support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use support::mcp_http::FakeHttpMcp;
use support::{eventually, text_of};
use tokio_util::sync::CancellationToken;
use z_engine_integrations::{
    IntegrationError, McpClient, McpListKind, McpManager, McpServerSpec, McpServerState,
};

fn spec(server: &FakeHttpMcp) -> McpServerSpec {
    McpServerSpec::http("remote", server.url.clone()).with_timeout(Duration::from_secs(5))
}

async fn call(
    client: &McpClient,
    tool: &str,
) -> Result<z_engine_integrations::CallToolResult, IntegrationError> {
    client
        .call_tool(tool, json!({"text": "hi"}), &CancellationToken::new())
        .await
}

fn cancel_notices(server: &FakeHttpMcp) -> Vec<Value> {
    server
        .requests()
        .into_iter()
        .filter(|r| r.body["method"] == "notifications/cancelled")
        .map(|r| r.body["params"]["requestId"].clone())
        .collect()
}

#[tokio::test]
async fn handshake_headers_sse_pages_and_server_requests() {
    let server = FakeHttpMcp::start().await;
    let client = McpClient::connect(&spec(&server), None).await.unwrap();
    let tools = client.list_tools().await.unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "echo",
            "sse_echo",
            "fail",
            "malformed",
            "drop",
            "slow",
            "notify",
            "expire"
        ]
    );

    let answered = eventually(|| {
        server
            .requests()
            .iter()
            .any(|r| r.body["id"] == "srv-ping" && r.body["result"] == json!({}))
    })
    .await;
    assert!(answered, "the ping inside the SSE stream was answered");
    let requests = server.requests();
    let posts: Vec<_> = requests
        .iter()
        .filter(|r| r.http_method == "POST")
        .collect();
    assert_eq!(posts[0].body["method"], "initialize");
    assert!(posts[0].session.is_none());
    for later in &posts[1..] {
        assert_eq!(later.session.as_deref(), Some("session-1"));
        assert_eq!(later.version.as_deref(), Some("2025-06-18"));
        assert!(
            later
                .accept
                .as_deref()
                .unwrap_or("")
                .contains("text/event-stream")
        );
    }
    assert_eq!(server.rpc_methods()[1], "notifications/initialized");
    assert!(eventually(|| server.requests().iter().any(|r| r.http_method == "GET")).await);

    client.shutdown().await;
    let deleted = server
        .requests()
        .into_iter()
        .find(|r| r.http_method == "DELETE");
    assert_eq!(
        deleted.and_then(|r| r.session).as_deref(),
        Some("session-1")
    );
}

#[tokio::test]
async fn json_and_sse_results_errors_and_disconnects() {
    let server = FakeHttpMcp::start().await;
    let client = McpClient::connect(&spec(&server), None).await.unwrap();
    assert_eq!(text_of(&call(&client, "echo").await.unwrap()), "echo: hi");
    assert_eq!(
        text_of(&call(&client, "sse_echo").await.unwrap()),
        "echo: hi"
    );
    let failed = call(&client, "fail").await.unwrap();
    assert!(failed.is_error);
    let malformed = call(&client, "malformed").await;
    assert!(
        matches!(malformed, Err(IntegrationError::Protocol(_))),
        "{malformed:?}"
    );
    let dropped = call(&client, "drop").await;
    assert!(
        matches!(dropped, Err(IntegrationError::Disconnected(_))),
        "{dropped:?}"
    );
    assert_eq!(
        text_of(&call(&client, "echo").await.unwrap()),
        "echo: hi",
        "still usable"
    );
}

#[tokio::test]
async fn timeouts_and_cancellation_post_a_cancel_notice() {
    let server = FakeHttpMcp::start().await;
    let short = spec(&server).with_timeout(Duration::from_millis(500));
    let client = McpClient::connect(&short, None).await.unwrap();
    let timed_out = call(&client, "slow").await;
    assert!(
        matches!(timed_out, Err(IntegrationError::Timeout { .. })),
        "{timed_out:?}"
    );
    assert!(eventually(|| cancel_notices(&server).len() == 1).await);

    let patient = McpClient::connect(&spec(&server), None).await.unwrap();
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        trigger.cancel();
    });
    let started = std::time::Instant::now();
    let cancelled = patient.call_tool("slow", json!({}), &cancel).await;
    assert!(
        matches!(cancelled, Err(IntegrationError::Cancelled)),
        "{cancelled:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(eventually(|| cancel_notices(&server).len() == 2).await);
}

#[tokio::test]
async fn list_changes_in_a_stream_reach_the_callback() {
    let server = FakeHttpMcp::start().await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    let callback = Arc::new(move |kind: McpListKind| sink.lock().unwrap().push(kind));
    let client = McpClient::connect(&spec(&server), Some(callback))
        .await
        .unwrap();
    assert_eq!(text_of(&call(&client, "notify").await.unwrap()), "notified");
    assert!(eventually(|| *seen.lock().unwrap() == vec![McpListKind::Tools]).await);
    assert!(client.is_changed(McpListKind::Tools));
}

#[tokio::test]
async fn undeclared_capabilities_are_not_requested_over_http() {
    let server = FakeHttpMcp::start().await;
    let client = McpClient::connect(&spec(&server), None).await.unwrap();
    assert!(client.list_resources().await.unwrap().is_empty());
    assert!(client.list_prompts().await.unwrap().is_empty());
    let read = client.read_resource("x://y").await;
    assert!(
        matches!(read, Err(IntegrationError::Unsupported(_))),
        "{read:?}"
    );
    assert!(
        !server
            .rpc_methods()
            .iter()
            .any(|m| m.starts_with("resources/") || m.starts_with("prompts/"))
    );
}

#[tokio::test]
async fn unreachable_servers_are_not_sent_and_http_errors_are_typed() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let closed = McpServerSpec::http("gone", format!("http://127.0.0.1:{port}/mcp"));
    let error = McpClient::connect(&closed, None).await.unwrap_err();
    assert!(matches!(error, IntegrationError::NotSent(_)), "{error:?}");

    let server = FakeHttpMcp::start().await;
    let client = McpClient::connect(&spec(&server), None).await.unwrap();
    let rejected = call(&client, "http_500").await;
    assert!(
        matches!(rejected, Err(IntegrationError::Http { status: 500, ref body }) if body == "internal boom"),
        "{rejected:?}"
    );
    let invalid = McpServerSpec::http("bad", "ftp://example.com/mcp");
    assert!(matches!(
        McpClient::connect(&invalid, None).await,
        Err(IntegrationError::Unsupported(_))
    ));
}

#[tokio::test]
async fn the_manager_reconnects_when_the_session_expired() {
    let server = FakeHttpMcp::start().await;
    let manager = McpManager::new();
    manager.start(vec![spec(&server)], None).await;
    assert_eq!(manager.status()[0].state, McpServerState::Ready);
    let cancel = CancellationToken::new();
    manager
        .call_tool("remote", "expire", json!({}), &cancel)
        .await
        .unwrap();
    let echoed = manager
        .call_tool("remote", "echo", json!({"text": "again"}), &cancel)
        .await
        .unwrap();
    assert_eq!(text_of(&echoed), "echo: again");
    let initializes = server
        .rpc_methods()
        .iter()
        .filter(|m| *m == "initialize")
        .count();
    assert_eq!(initializes, 2, "one reconnect with a new session");
    let last = server
        .requests()
        .into_iter()
        .rev()
        .find(|r| r.http_method == "POST")
        .unwrap();
    assert_eq!(last.session.as_deref(), Some("session-2"));
    manager.shutdown_all().await;
}
