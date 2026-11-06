//! Catalogs of the fake stdio MCP server: paginated tools, resources,
//! prompts, capability gating, list-change notifications, and protocol
//! version negotiation.

mod support;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use support::{eventually, fake_mcp};
use tokio_util::sync::CancellationToken;
use z_engine_integrations::{IntegrationError, McpClient, McpListKind};
use z_engine_protocol::Role;

async fn methods_seen(client: &McpClient) -> Vec<String> {
    let stats = client
        .call_tool("stats", json!({}), &CancellationToken::new())
        .await
        .unwrap()
        .structured
        .unwrap();
    stats["methods"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

#[tokio::test]
async fn tools_follow_the_cursor_across_pages() {
    let client = McpClient::connect(&fake_mcp("fake", &[]), None)
        .await
        .unwrap();
    let tools = client.list_tools().await.unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "echo",
            "fail",
            "slow",
            "crash",
            "notify",
            "malformed",
            "noisy",
            "media",
            "stats"
        ]
    );
    assert_eq!(tools[0].annotations.read_only_hint, Some(true));
    assert_eq!(tools[0].description.as_deref(), Some("Echoes text"));
    assert_eq!(tools[0].input_schema["type"], "object");
    let listings = methods_seen(&client)
        .await
        .iter()
        .filter(|m| *m == "tools/list")
        .count();
    assert_eq!(listings, 2);
}

#[tokio::test]
async fn resources_and_prompts_are_listed_read_and_rendered() {
    let client = McpClient::connect(&fake_mcp("fake", &[]), None)
        .await
        .unwrap();
    let resources = client.list_resources().await.unwrap();
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].mime_type.as_deref(), Some("text/markdown"));
    let contents = client.read_resource(&resources[0].uri).await.unwrap();
    assert_eq!(contents[0].text.as_deref(), Some("# Fake readme"));
    let prompts = client.list_prompts().await.unwrap();
    assert_eq!(prompts[0].name, "review");
    assert!(prompts[0].arguments[0].required);
    let args = BTreeMap::from([("file".to_string(), "src/lib.rs".to_string())]);
    let messages = client.get_prompt("review", &args).await.unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].role, Role::User);
    assert_eq!(messages[0].text, "Please review src/lib.rs");
}

#[tokio::test]
async fn undeclared_capabilities_are_never_requested() {
    let client = McpClient::connect(&fake_mcp("fake", &["--tools-only"]), None)
        .await
        .unwrap();
    assert!(!client.capabilities().resources && !client.capabilities().prompts);
    assert!(client.list_resources().await.unwrap().is_empty());
    assert!(client.list_prompts().await.unwrap().is_empty());
    let read = client.read_resource("fake://readme").await;
    assert!(
        matches!(read, Err(IntegrationError::Unsupported(_))),
        "{read:?}"
    );
    let prompt = client.get_prompt("review", &BTreeMap::new()).await;
    assert!(
        matches!(prompt, Err(IntegrationError::Unsupported(_))),
        "{prompt:?}"
    );
    let methods = methods_seen(&client).await;
    assert!(
        !methods
            .iter()
            .any(|m| m.starts_with("resources/") || m.starts_with("prompts/")),
        "{methods:?}"
    );
}

#[tokio::test]
async fn list_changes_set_the_flag_and_reach_the_callback() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    let callback = Arc::new(move |kind: McpListKind| sink.lock().unwrap().push(kind));
    let client = McpClient::connect(&fake_mcp("fake", &[]), Some(callback))
        .await
        .unwrap();
    assert!(!client.is_changed(McpListKind::Tools));
    client
        .call_tool("notify", json!({}), &CancellationToken::new())
        .await
        .unwrap();
    assert!(eventually(|| !seen.lock().unwrap().is_empty()).await);
    assert_eq!(*seen.lock().unwrap(), vec![McpListKind::Tools]);
    assert!(client.is_changed(McpListKind::Tools));
    client.list_tools().await.unwrap();
    assert!(
        !client.is_changed(McpListKind::Tools),
        "listing clears the flag"
    );
    client
        .call_tool("notify", json!({}), &CancellationToken::new())
        .await
        .unwrap();
    assert!(client.take_changed(McpListKind::Tools));
    assert!(!client.take_changed(McpListKind::Tools));
}

#[tokio::test]
async fn unsupported_protocol_versions_are_refused() {
    let spec = fake_mcp("old", &["--protocol=1999-01-01"]);
    let error = McpClient::connect(&spec, None).await.unwrap_err();
    assert!(
        matches!(error, IntegrationError::Unsupported(_)),
        "{error:?}"
    );
    assert!(error.to_string().contains("1999-01-01"));
    let older = McpClient::connect(&fake_mcp("older", &["--protocol=2025-03-26"]), None)
        .await
        .unwrap();
    assert_eq!(older.protocol_version(), "2025-03-26");
}
