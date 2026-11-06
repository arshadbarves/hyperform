//! Bringing a session's MCP servers in line with its settings without
//! blocking: servers connect in a background task, tools are registered
//! as each server becomes ready, list changes and status changes refresh
//! the catalog, and a notice summarizes each start.

use std::sync::{Arc, Weak};

use tokio::sync::mpsc;
use z_engine_integrations::{McpChange, McpChangeCallback, McpChangeKind, McpServerState};
use z_engine_protocol::NoticeLevel;
use z_engine_tools::mcp_tool_name;

use super::catalog::CatalogTool;
use super::hub::Server;
use super::specs::server_plans;
use crate::session::{SessionCore, rebuild_tools};
use crate::sync::lock;

/// Applies the MCP servers of the current settings: unchanged servers keep
/// running, changed ones restart, removed ones stop.
pub(crate) fn sync_servers(core: &Arc<SessionCore>) {
    let settings = core.settings();
    let home = core.shared.paths.home_dir.as_deref();
    let (plans, problems) = server_plans(&settings.settings.mcp, &core.root, home);
    for problem in problems {
        core.events.notice(NoticeLevel::Warn, problem);
    }
    let callback = change_callback(core);
    let reconciled = core.mcp.reconcile(plans);
    let weak = Arc::downgrade(core);
    tokio::spawn(async move {
        futures::future::join_all(reconciled.stopped.iter().map(|m| m.shutdown_all())).await;
        let starts = reconciled.started.iter().map(|server| {
            let spec = server.plan.spec.clone();
            server
                .manager
                .start(vec![spec], Some(Arc::clone(&callback)))
        });
        futures::future::join_all(starts).await;
        let Some(core) = live(&weak) else {
            let stops = reconciled.started.iter().map(|s| s.manager.shutdown_all());
            futures::future::join_all(stops).await;
            return;
        };
        refresh(&core).await;
        announce(&core, &reconciled.started);
    });
}

/// The session while it is open.
fn live(weak: &Weak<SessionCore>) -> Option<Arc<SessionCore>> {
    weak.upgrade().filter(|core| !core.cancel.is_cancelled())
}

/// Rebuilds the catalog from every ready server, and servers that lost
/// their connection (a call reconnects them), minus disabled tools; the
/// registry follows when the tools changed. Returns whether they did.
pub(super) async fn refresh(core: &SessionCore) -> bool {
    let mut tools: Vec<CatalogTool> = Vec::new();
    let mut prompts = Vec::new();
    for Server { plan, manager } in core.mcp.servers() {
        match manager.prompts().await {
            Ok(listed) => prompts.extend(listed),
            Err(error) => {
                tracing::debug!(server = %plan.spec.name, %error, "MCP prompts not listed")
            }
        }
        for (server, info) in manager.tools_or_last_known().await {
            if plan.disabled_tools.contains(&info.name) {
                continue;
            }
            let name = mcp_tool_name(&server, &info.name);
            if tools.iter().any(|tool| tool.name == name) {
                tracing::warn!(%name, "duplicate MCP tool name ignored");
                continue;
            }
            tools.push(CatalogTool { server, name, info });
        }
    }
    core.mcp.set_prompts(prompts);
    let changed = core.mcp.install(tools);
    if changed {
        rebuild_tools(core);
    }
    changed
}

/// Server changes go to one watcher task per session, which refreshes
/// the catalog after each burst of changes.
fn change_callback(core: &Arc<SessionCore>) -> McpChangeCallback {
    let mut slot = lock(&core.mcp.changes);
    let sender = match slot.as_ref() {
        Some(sender) => sender.clone(),
        None => {
            let (sender, receiver) = mpsc::unbounded_channel();
            tokio::spawn(watch(Arc::downgrade(core), core.cancel.clone(), receiver));
            *slot = Some(sender.clone());
            sender
        }
    };
    Arc::new(move |change: McpChange| {
        if sender.send(change).is_err() {
            tracing::debug!("MCP change after the session closed");
        }
    })
}

async fn watch(
    weak: Weak<SessionCore>,
    cancel: tokio_util::sync::CancellationToken,
    mut changes: mpsc::UnboundedReceiver<McpChange>,
) {
    loop {
        let first = tokio::select! {
            () = cancel.cancelled() => return,
            change = changes.recv() => match change {
                Some(change) => change,
                None => return,
            },
        };
        let mut burst = vec![first];
        while let Ok(more) = changes.try_recv() {
            burst.push(more);
        }
        let listed: Vec<String> = burst
            .iter()
            .filter(|change| change.kind == McpChangeKind::Tools)
            .map(|change| change.server.clone())
            .collect();
        if !burst.iter().any(|change| {
            matches!(
                change.kind,
                McpChangeKind::Tools | McpChangeKind::Status | McpChangeKind::Prompts
            )
        }) {
            continue;
        }
        let Some(core) = live(&weak) else { return };
        if refresh(&core).await && !listed.is_empty() {
            let count = core.mcp.catalog().tools.len();
            core.events.notice(
                NoticeLevel::Info,
                format!(
                    "MCP tools changed on {}; {count} MCP tool(s) are now available.",
                    listed.join(", ")
                ),
            );
        }
    }
}

/// One notice per start: which servers are ready and which failed.
fn announce(core: &SessionCore, started: &[Server]) {
    let mut ready = Vec::new();
    let mut failed = Vec::new();
    for status in started.iter().flat_map(|server| server.manager.status()) {
        match status.state {
            McpServerState::Ready => {
                ready.push(format!("{} ({} tools)", status.name, status.tool_count))
            }
            McpServerState::Failed(reason) => failed.push(format!("{}: {reason}", status.name)),
            McpServerState::Connecting | McpServerState::Disabled => {}
        }
    }
    let mut parts = Vec::new();
    if !ready.is_empty() {
        parts.push(format!("MCP servers ready: {}.", ready.join(", ")));
    }
    if !failed.is_empty() {
        parts.push(format!(
            "MCP servers failed to start: {}.",
            failed.join("; ")
        ));
    }
    if parts.is_empty() {
        return;
    }
    let level = if failed.is_empty() {
        NoticeLevel::Info
    } else {
        NoticeLevel::Warn
    };
    core.events.notice(level, parts.join(" "));
}
