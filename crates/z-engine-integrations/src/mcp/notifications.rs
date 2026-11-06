//! Notifications an MCP server sends on its own: list changes set a flag
//! and reach the optional callback; log messages go to tracing.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::Value;

use super::types::McpListKind;
use crate::jsonrpc::{NotificationHandler, RpcNotification};

/// Called on the connection's reading task: keep it quick and non-blocking.
pub type ListChangedCallback = Arc<dyn Fn(McpListKind) + Send + Sync>;

/// Set when the server announces a change; cleared by the next listing.
#[derive(Debug, Default)]
pub(crate) struct ChangeFlags {
    tools: AtomicBool,
    resources: AtomicBool,
    prompts: AtomicBool,
}

impl ChangeFlags {
    pub(crate) fn flag(&self, kind: McpListKind) -> &AtomicBool {
        match kind {
            McpListKind::Tools => &self.tools,
            McpListKind::Resources => &self.resources,
            McpListKind::Prompts => &self.prompts,
        }
    }
}

pub(crate) fn notification_handler(
    server: &str,
    changes: Arc<ChangeFlags>,
    callback: Option<ListChangedCallback>,
) -> NotificationHandler {
    let server = server.to_string();
    Arc::new(move |notification: RpcNotification| {
        let kind = match notification.method.as_str() {
            "notifications/tools/list_changed" => McpListKind::Tools,
            "notifications/resources/list_changed" => McpListKind::Resources,
            "notifications/prompts/list_changed" => McpListKind::Prompts,
            "notifications/message" => {
                log_message(&server, notification.params.as_ref());
                return;
            }
            other => {
                tracing::trace!(%server, method = other, "MCP notification ignored");
                return;
            }
        };
        changes.flag(kind).store(true, Ordering::SeqCst);
        if let Some(callback) = &callback {
            callback(kind);
        }
    })
}

fn log_message(server: &str, params: Option<&Value>) {
    let level = params
        .and_then(|p| p.get("level"))
        .and_then(Value::as_str)
        .unwrap_or("info");
    let data = params
        .and_then(|p| p.get("data"))
        .cloned()
        .unwrap_or(Value::Null);
    match level {
        "error" | "critical" | "alert" | "emergency" => {
            tracing::warn!(%server, %level, %data, "MCP server log");
        }
        _ => tracing::debug!(%server, %level, %data, "MCP server log"),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::json;

    use super::*;

    #[test]
    fn list_changes_set_flags_and_reach_the_callback() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        let changes = Arc::new(ChangeFlags::default());
        let handler = notification_handler(
            "srv",
            Arc::clone(&changes),
            Some(Arc::new(move |kind| sink.lock().unwrap().push(kind))),
        );
        for method in [
            "notifications/prompts/list_changed",
            "notifications/message",
            "notifications/progress",
        ] {
            handler(RpcNotification {
                method: method.into(),
                params: Some(json!({"level": "error", "data": "x"})),
            });
        }
        assert_eq!(*seen.lock().unwrap(), vec![McpListKind::Prompts]);
        assert!(changes.flag(McpListKind::Prompts).load(Ordering::SeqCst));
        assert!(!changes.flag(McpListKind::Tools).load(Ordering::SeqCst));
    }
}
