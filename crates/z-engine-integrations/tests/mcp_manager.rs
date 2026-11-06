//! The MCP manager over stdio servers: concurrent start with mixed
//! outcomes, status, the merged catalog, change events, reconnect after a
//! crash, restart and shutdown.

mod support;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde_json::json;
use support::{eventually, fake_mcp, text_of};
use tokio_util::sync::CancellationToken;
use z_engine_integrations::{
    IntegrationError, McpChange, McpChangeKind, McpManager, McpServerSpec, McpServerState,
    tool_name,
};

fn specs() -> Vec<McpServerSpec> {
    let mut disabled = fake_mcp("off", &[]);
    disabled.enabled = false;
    vec![
        fake_mcp("fake", &[]),
        McpServerSpec::stdio("broken", "zengine-no-such-mcp-server", Vec::new()),
        disabled,
        fake_mcp("fake", &["--tools-only"]),
    ]
}

async fn started() -> (McpManager, Arc<Mutex<Vec<McpChange>>>) {
    let changes = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&changes);
    let manager = McpManager::new();
    manager
        .start(
            specs(),
            Some(Arc::new(move |change| sink.lock().unwrap().push(change))),
        )
        .await;
    (manager, changes)
}

#[tokio::test]
async fn start_reports_ready_failed_and_disabled_servers() {
    let (manager, changes) = started().await;
    let status = manager.status();
    assert_eq!(status.len(), 3, "the duplicate name is ignored");
    let fake = &status[0];
    assert_eq!(
        (fake.name.as_str(), &fake.state),
        ("fake", &McpServerState::Ready)
    );
    assert_eq!(
        (fake.tool_count, fake.resource_count, fake.prompt_count),
        (9, 1, 1)
    );
    assert!(fake.error.is_none());
    let McpServerState::Failed(reason) = &status[1].state else {
        panic!("expected a failed server: {:?}", status[1]);
    };
    assert!(reason.contains("not found on PATH"), "{reason}");
    assert_eq!(status[1].error.as_ref(), Some(reason));
    assert_eq!(status[2].state, McpServerState::Disabled);
    let events = changes.lock().unwrap();
    assert!(
        events
            .iter()
            .any(|c| c.server == "fake" && c.kind == McpChangeKind::Status)
    );
    assert!(!events.iter().any(|c| c.server == "off"));
}

#[tokio::test]
async fn catalog_calls_and_lookups() {
    let (manager, _) = started().await;
    let tools = manager.tools().await;
    assert_eq!(tools.len(), 9);
    assert!(tools.iter().all(|(server, _)| server == "fake"));
    let qualified = tool_name("fake", "echo");
    assert_eq!(
        manager.resolve_tool(&qualified),
        Some(("fake".into(), "echo".into()))
    );
    assert_eq!(manager.resolve_tool("mcp__fake__nope"), None);
    let cancel = CancellationToken::new();
    let echoed = manager
        .call_tool("fake", "echo", json!({"text": "hi"}), &cancel)
        .await
        .unwrap();
    assert_eq!(text_of(&echoed), "echo: hi");
    let disabled = manager.call_tool("off", "echo", json!({}), &cancel).await;
    assert!(
        matches!(disabled, Err(IntegrationError::Unsupported(_))),
        "{disabled:?}"
    );
    let unknown = manager
        .call_tool("nobody", "echo", json!({}), &cancel)
        .await;
    assert!(
        matches!(unknown, Err(IntegrationError::NotFound(_))),
        "{unknown:?}"
    );

    let resources = manager.list_resources(None).await.unwrap();
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].0, "fake");
    let contents = manager
        .read_resource("fake", &resources[0].1.uri)
        .await
        .unwrap();
    assert_eq!(contents[0].text.as_deref(), Some("# Fake readme"));
    let prompts = manager.prompts().await.unwrap();
    assert_eq!(prompts.len(), 1);
    let args = BTreeMap::from([("file".to_string(), "a.rs".to_string())]);
    let messages = manager.get_prompt("fake", "review", args).await.unwrap();
    assert_eq!(messages[0].text, "Please review a.rs");
}

#[tokio::test]
async fn list_changes_are_forwarded_and_refresh_the_catalog() {
    let (manager, changes) = started().await;
    manager
        .call_tool("fake", "notify", json!({}), &CancellationToken::new())
        .await
        .unwrap();
    let forwarded = eventually(|| {
        changes
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.server == "fake" && c.kind == McpChangeKind::Tools)
    })
    .await;
    assert!(forwarded);
    assert_eq!(manager.tools().await.len(), 9, "the refreshed list");
}

#[tokio::test]
async fn a_crashed_server_fails_the_call_then_reconnects_once() {
    let (manager, changes) = started().await;
    let cancel = CancellationToken::new();
    let crashed = manager.call_tool("fake", "crash", json!({}), &cancel).await;
    assert!(
        matches!(crashed, Err(IntegrationError::Disconnected(_))),
        "{crashed:?}"
    );
    let failed = &manager.status()[0];
    assert!(
        matches!(&failed.state, McpServerState::Failed(_)),
        "{failed:?}"
    );
    let crash_logged = || {
        manager.status()[0]
            .stderr_tail
            .iter()
            .any(|line| line.contains("crashing on purpose"))
    };
    assert!(eventually(crash_logged).await);
    assert!(
        manager.tools().await.is_empty(),
        "a failed server offers no tools"
    );
    assert_eq!(
        manager.tools_or_last_known().await.len(),
        9,
        "but its last listed tools stay known, so a call can reconnect it"
    );

    let echoed = manager
        .call_tool("fake", "echo", json!({"text": "back"}), &cancel)
        .await
        .unwrap();
    assert_eq!(text_of(&echoed), "echo: back");
    assert_eq!(manager.status()[0].state, McpServerState::Ready);
    assert_eq!(manager.tools().await.len(), 9);
    let statuses = changes
        .lock()
        .unwrap()
        .iter()
        .filter(|c| c.server == "fake" && c.kind == McpChangeKind::Status)
        .count();
    assert!(
        statuses >= 4,
        "connecting/ready, failed, connecting/ready: {statuses}"
    );
}

#[tokio::test]
async fn restart_and_shutdown() {
    let (manager, _) = started().await;
    manager.restart("fake").await.unwrap();
    assert_eq!(manager.status()[0].state, McpServerState::Ready);
    let broken = manager.restart("broken").await;
    assert!(
        matches!(broken, Err(IntegrationError::Spawn { .. })),
        "{broken:?}"
    );
    let off = manager.restart("off").await;
    assert!(
        matches!(off, Err(IntegrationError::Unsupported(_))),
        "{off:?}"
    );
    manager.shutdown_all().await;
    assert!(manager.status().is_empty());
    assert!(manager.tools().await.is_empty());
    let gone = manager
        .call_tool("fake", "echo", json!({}), &CancellationToken::new())
        .await;
    assert!(matches!(gone, Err(IntegrationError::NotFound(_))));
}
