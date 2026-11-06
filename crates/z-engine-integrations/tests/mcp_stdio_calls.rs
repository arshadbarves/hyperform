//! Tool calls against the fake stdio MCP server: success, content kinds,
//! tool-reported errors, malformed results, noise, crashes, timeouts and
//! cancellation. No failure may come back as an empty success.

mod support;

use std::time::Duration;

use serde_json::{Value, json};
use support::{eventually, fake_mcp, text_of};
use tokio_util::sync::CancellationToken;
use z_engine_integrations::{
    IntegrationError, McpClient, McpContent, McpServerSpec, content_to_parts,
};
use z_engine_protocol::ToolResultPart;

async fn connect(spec: &McpServerSpec) -> McpClient {
    McpClient::connect(spec, None)
        .await
        .expect("connect to the fake server")
}

async fn call(
    client: &McpClient,
    tool: &str,
    args: Value,
) -> Result<z_engine_integrations::CallToolResult, IntegrationError> {
    client
        .call_tool(tool, args, &CancellationToken::new())
        .await
}

async fn stats(client: &McpClient) -> Value {
    call(client, "stats", json!({}))
        .await
        .unwrap()
        .structured
        .unwrap()
}

/// Cancellation notices the server received, polled because the notice is
/// sent in the background and may trail the caller's next request.
async fn cancelled_ids(client: &McpClient, expected: usize) -> Vec<Value> {
    for _ in 0..100 {
        let seen = stats(client).await["cancelled"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if seen.len() >= expected {
            return seen;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Vec::new()
}

#[tokio::test]
async fn handshake_and_successful_calls() {
    let client = connect(&fake_mcp("fake", &[])).await;
    assert_eq!(client.protocol_version(), "2025-06-18");
    assert_eq!(client.server_info().name, "zengine-fake-mcp");
    assert_eq!(client.instructions(), Some("Fake server for tests."));
    assert!(client.capabilities().tools && client.capabilities().resources);
    let echoed = call(&client, "echo", json!({"text": "héllo 🦀"}))
        .await
        .unwrap();
    assert!(!echoed.is_error);
    assert_eq!(text_of(&echoed), "echo: héllo 🦀");
    let null_args = call(&client, "echo", Value::Null).await.unwrap();
    assert_eq!(text_of(&null_args), "echo: ");
    let bad_args = call(&client, "echo", json!([1])).await;
    assert!(matches!(bad_args, Err(IntegrationError::Protocol(_))));

    let media = call(&client, "media", json!({})).await.unwrap();
    assert_eq!(media.structured, Some(json!({"ok": true})));
    assert!(
        matches!(&media.content[1], McpContent::Image { mime_type, .. } if mime_type == "image/png")
    );
    let parts = content_to_parts(&media.content);
    assert_eq!(parts.len(), 4);
    assert!(matches!(parts[1], ToolResultPart::Image { .. }));
    assert!(
        matches!(&parts[2], ToolResultPart::Text { text } if text == "[resource link] readme: fake://readme")
    );
    assert!(
        matches!(&parts[3], ToolResultPart::Text { text } if text == "[resource fake://notes]\nnotes")
    );

    let seen = stats(&client).await;
    let methods: Vec<&str> = seen["methods"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(&methods[..2], ["initialize", "notifications/initialized"]);
    client.shutdown().await;
    assert!(client.is_closed());
}

#[tokio::test]
async fn tool_errors_are_results_but_broken_replies_are_errors() {
    let client = connect(&fake_mcp("fake", &[])).await;
    let failed = call(&client, "fail", json!({})).await.unwrap();
    assert!(failed.is_error);
    assert_eq!(text_of(&failed), "the tool failed on purpose");
    let malformed = call(&client, "malformed", json!({})).await;
    assert!(
        matches!(malformed, Err(IntegrationError::Protocol(_))),
        "{malformed:?}"
    );
    let unknown = call(&client, "nope", json!({})).await;
    assert!(
        matches!(unknown, Err(IntegrationError::Rpc { code: -32602, .. })),
        "{unknown:?}"
    );
    let noisy = call(&client, "noisy", json!({})).await.unwrap();
    assert_eq!(text_of(&noisy), "after the noise");
}

#[tokio::test]
async fn a_crash_is_a_disconnect_not_an_empty_success() {
    let client = connect(&fake_mcp("fake", &[])).await;
    let crashed = call(&client, "crash", json!({})).await;
    assert!(
        matches!(crashed, Err(IntegrationError::Disconnected(_))),
        "{crashed:?}"
    );
    assert!(client.is_closed());
    let after = call(&client, "echo", json!({"text": "x"})).await;
    assert!(
        matches!(after, Err(IntegrationError::NotSent(_))),
        "{after:?}"
    );
    assert!(
        eventually(|| client
            .stderr_tail()
            .iter()
            .any(|l| l.contains("crashing on purpose")))
        .await,
        "{:?}",
        client.stderr_tail()
    );
}

#[tokio::test]
async fn timeouts_announce_the_cancellation_and_keep_the_connection() {
    // Generous enough for a cold process start, short against the tool.
    let spec = fake_mcp("fake", &[]).with_timeout(Duration::from_secs(2));
    let client = connect(&spec).await;
    let slow = call(&client, "slow", json!({"ms": 20_000})).await;
    assert!(
        matches!(slow, Err(IntegrationError::Timeout { ref method, .. }) if method == "tools/call"),
        "{slow:?}"
    );
    let echoed = call(&client, "echo", json!({"text": "still here"}))
        .await
        .unwrap();
    assert_eq!(text_of(&echoed), "echo: still here");
    assert_eq!(cancelled_ids(&client, 1).await.len(), 1);
}

#[tokio::test]
async fn cancellation_returns_promptly_and_notifies_the_server() {
    let client = connect(&fake_mcp("fake", &[])).await;
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        trigger.cancel();
    });
    let started = std::time::Instant::now();
    let slow = client.call_tool("slow", json!({"ms": 5000}), &cancel).await;
    assert!(matches!(slow, Err(IntegrationError::Cancelled)), "{slow:?}");
    assert!(started.elapsed() < Duration::from_secs(3));
    let cancelled = cancelled_ids(&client, 1).await;
    assert_eq!(cancelled.len(), 1);
    assert!(cancelled[0].is_number());
}

#[tokio::test]
async fn missing_commands_fail_to_spawn() {
    let spec = McpServerSpec::stdio("ghost", "zengine-no-such-mcp-server", Vec::new());
    let error = McpClient::connect(&spec, None).await.unwrap_err();
    assert!(matches!(error, IntegrationError::Spawn { .. }), "{error:?}");
    assert!(error.to_string().contains("not found on PATH"));
}
