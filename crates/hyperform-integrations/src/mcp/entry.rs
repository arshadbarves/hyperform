//! One configured server inside the manager: its state (Connecting ->
//! Ready | Failed, or Disabled), the cached catalog, serialized
//! (re)connection, and the rule that a request is retried on a fresh
//! connection only when it was never sent.

use std::fmt;
use std::sync::{Arc, Mutex};

use futures::future::BoxFuture;
use tokio_util::sync::CancellationToken;

use super::client::McpClient;
use super::notifications::ListChangedCallback;
use super::spec::McpServerSpec;
use super::status::{McpChange, McpChangeCallback, McpChangeKind, McpServerState, McpServerStatus};
use super::types::{McpListKind, McpToolInfo};
use crate::error::IntegrationError;
use crate::process::StderrLog;
use crate::sync::lock;

struct EntryState {
    state: McpServerState,
    client: Option<Arc<McpClient>>,
    tools: Vec<McpToolInfo>,
    resource_count: usize,
    prompt_count: usize,
    /// Bumped by every (re)connect and shutdown; stale watchers compare it.
    generation: u64,
}

pub(super) struct ServerEntry {
    pub(super) spec: McpServerSpec,
    stderr: StderrLog,
    state: Mutex<EntryState>,
    connect_lock: tokio::sync::Mutex<()>,
    on_change: Option<McpChangeCallback>,
}

impl fmt::Debug for ServerEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerEntry")
            .field("name", &self.spec.name)
            .field("state", &lock(&self.state).state)
            .finish_non_exhaustive()
    }
}

struct Catalog {
    tools: Vec<McpToolInfo>,
    resource_count: usize,
    prompt_count: usize,
}

impl ServerEntry {
    pub(super) fn new(spec: McpServerSpec, on_change: Option<McpChangeCallback>) -> Arc<Self> {
        let state = if spec.enabled {
            McpServerState::Connecting
        } else {
            McpServerState::Disabled
        };
        Arc::new(Self {
            spec,
            stderr: StderrLog::default(),
            state: Mutex::new(EntryState {
                state,
                client: None,
                tools: Vec::new(),
                resource_count: 0,
                prompt_count: 0,
                generation: 0,
            }),
            connect_lock: tokio::sync::Mutex::new(()),
            on_change,
        })
    }

    fn emit(&self, kind: McpChangeKind) {
        if let Some(callback) = &self.on_change {
            callback(McpChange {
                server: self.spec.name.clone(),
                kind,
            });
        }
    }

    fn snapshot(&self) -> (McpServerState, Option<Arc<McpClient>>) {
        let state = lock(&self.state);
        (state.state.clone(), state.client.clone())
    }

    fn disabled(&self) -> IntegrationError {
        IntegrationError::Unsupported(format!("MCP server `{}` is disabled", self.spec.name))
    }

    /// Connects, replacing (and shutting down) any previous client.
    pub(super) async fn connect(self: &Arc<Self>) -> Result<Arc<McpClient>, IntegrationError> {
        let _serial = self.connect_lock.lock().await;
        self.connect_locked().await
    }

    async fn connect_locked(self: &Arc<Self>) -> Result<Arc<McpClient>, IntegrationError> {
        if !self.spec.enabled {
            return Err(self.disabled());
        }
        let stale = {
            let mut state = lock(&self.state);
            state.generation += 1;
            state.state = McpServerState::Connecting;
            state.client.take()
        };
        self.emit(McpChangeKind::Status);
        if let Some(stale) = stale {
            stale.shutdown().await;
        }
        match self.open().await {
            Ok((client, catalog)) => {
                let generation = {
                    let mut state = lock(&self.state);
                    state.state = McpServerState::Ready;
                    state.client = Some(Arc::clone(&client));
                    state.tools = catalog.tools;
                    state.resource_count = catalog.resource_count;
                    state.prompt_count = catalog.prompt_count;
                    state.generation
                };
                self.watch(client.closed_token(), generation);
                self.emit(McpChangeKind::Status);
                Ok(client)
            }
            Err(error) => {
                lock(&self.state).state = McpServerState::Failed(error.to_string());
                self.emit(McpChangeKind::Status);
                Err(error)
            }
        }
    }

    async fn open(self: &Arc<Self>) -> Result<(Arc<McpClient>, Catalog), IntegrationError> {
        let weak = Arc::downgrade(self);
        let on_list_changed: ListChangedCallback = Arc::new(move |kind: McpListKind| {
            if let Some(entry) = weak.upgrade() {
                entry.emit(kind.into());
            }
        });
        let client =
            McpClient::connect_with_log(&self.spec, Some(on_list_changed), self.stderr.clone());
        let client = Arc::new(client.await?);
        let tools = match client.list_tools().await {
            Ok(tools) => tools,
            Err(error) => {
                client.shutdown().await;
                return Err(error);
            }
        };
        let resource_count = count(&self.spec.name, "resources", client.list_resources().await);
        let prompt_count = count(&self.spec.name, "prompts", client.list_prompts().await);
        let catalog = Catalog {
            tools,
            resource_count,
            prompt_count,
        };
        Ok((client, catalog))
    }

    /// Marks the server failed when this connection closes on its own.
    fn watch(self: &Arc<Self>, closed: CancellationToken, generation: u64) {
        let weak = Arc::downgrade(self);
        tokio::spawn(async move {
            closed.cancelled().await;
            let Some(entry) = weak.upgrade() else { return };
            let reason = {
                let mut state = lock(&entry.state);
                if state.generation != generation || state.state != McpServerState::Ready {
                    return;
                }
                let reason = state
                    .client
                    .as_ref()
                    .and_then(|client| client.close_reason())
                    .unwrap_or_else(|| "the connection closed".into());
                state.state = McpServerState::Failed(format!("connection lost: {reason}"));
                reason
            };
            tracing::warn!(server = %entry.spec.name, %reason, "MCP server connection lost");
            entry.emit(McpChangeKind::Status);
        });
    }

    /// The current client, or one new connection attempt when the server
    /// failed or its connection closed. The flag says a reconnect happened.
    async fn usable_client(self: &Arc<Self>) -> Result<(Arc<McpClient>, bool), IntegrationError> {
        match self.snapshot() {
            (McpServerState::Disabled, _) => Err(self.disabled()),
            (McpServerState::Ready, Some(client)) if !client.is_closed() => Ok((client, false)),
            (_, stale) => Ok((self.reconnect(stale.as_ref()).await?, true)),
        }
    }

    async fn reconnect(
        self: &Arc<Self>,
        stale: Option<&Arc<McpClient>>,
    ) -> Result<Arc<McpClient>, IntegrationError> {
        let _serial = self.connect_lock.lock().await;
        // Another caller may have reconnected while this one waited.
        if let (McpServerState::Ready, Some(current)) = self.snapshot() {
            let replaced = stale.is_none_or(|stale| !Arc::ptr_eq(stale, &current));
            if replaced && !current.is_closed() {
                return Ok(current);
            }
        }
        self.connect_locked().await
    }

    /// Runs `op` on a usable client. A request that was never sent is
    /// retried once on a fresh connection; a connection lost mid-request is
    /// reported (the outcome is unknown), never retried.
    pub(super) async fn run<T>(
        self: &Arc<Self>,
        op: impl Fn(Arc<McpClient>) -> BoxFuture<'static, Result<T, IntegrationError>>,
    ) -> Result<T, IntegrationError> {
        let (client, reconnected) = self.usable_client().await?;
        match op(Arc::clone(&client)).await {
            Err(IntegrationError::NotSent(reason)) if !reconnected => {
                tracing::info!(server = %self.spec.name, %reason, "request not sent; reconnecting once");
                let client = self.reconnect(Some(&client)).await?;
                op(client).await
            }
            Err(error @ IntegrationError::Disconnected(_)) => {
                self.mark_failed(&client, &error);
                Err(error)
            }
            other => other,
        }
    }

    fn mark_failed(&self, client: &Arc<McpClient>, error: &IntegrationError) {
        {
            let mut state = lock(&self.state);
            let current = state
                .client
                .as_ref()
                .is_some_and(|c| Arc::ptr_eq(c, client));
            if !current || state.state != McpServerState::Ready {
                return;
            }
            state.state = McpServerState::Failed(error.to_string());
        }
        self.emit(McpChangeKind::Status);
    }

    /// Tools of a ready server, refreshed first when it announced a change.
    pub(super) async fn tools(&self) -> Vec<McpToolInfo> {
        let Some(client) = self.ready_client() else {
            return Vec::new();
        };
        if client.is_changed(McpListKind::Tools) {
            match client.list_tools().await {
                Ok(tools) => {
                    let mut state = lock(&self.state);
                    if state
                        .client
                        .as_ref()
                        .is_some_and(|c| Arc::ptr_eq(c, &client))
                    {
                        state.tools = tools;
                    }
                }
                Err(error) => tracing::warn!(
                    server = %self.spec.name, %error,
                    "could not refresh MCP tools; keeping the previous list"
                ),
            }
        }
        lock(&self.state).tools.clone()
    }

    pub(super) fn cached_tools(&self) -> Vec<McpToolInfo> {
        lock(&self.state).tools.clone()
    }

    /// [`ServerEntry::tools`], except that a server whose connection was
    /// lost (or is being re-established) keeps the tools it last listed:
    /// calling one reconnects it. Servers that never connected offer none.
    pub(super) async fn tools_or_last_known(&self) -> Vec<McpToolInfo> {
        match self.snapshot().0 {
            McpServerState::Ready => self.tools().await,
            McpServerState::Disabled => Vec::new(),
            McpServerState::Connecting | McpServerState::Failed(_) => self.cached_tools(),
        }
    }

    pub(super) fn ready_client(&self) -> Option<Arc<McpClient>> {
        match self.snapshot() {
            (McpServerState::Ready, Some(client)) if !client.is_closed() => Some(client),
            _ => None,
        }
    }

    pub(super) fn status(&self) -> McpServerStatus {
        let state = lock(&self.state);
        let mut current = state.state.clone();
        if current == McpServerState::Ready
            && let Some(reason) = state.client.as_ref().and_then(|c| c.close_reason())
        {
            current = McpServerState::Failed(format!("connection lost: {reason}"));
        }
        McpServerStatus {
            name: self.spec.name.clone(),
            error: match &current {
                McpServerState::Failed(reason) => Some(reason.clone()),
                _ => None,
            },
            state: current,
            tool_count: state.tools.len(),
            resource_count: state.resource_count,
            prompt_count: state.prompt_count,
            stderr_tail: self.stderr.tail(),
        }
    }

    /// Closes the connection (waiting for an in-flight connect first).
    pub(super) async fn shutdown(&self) {
        let _serial = self.connect_lock.lock().await;
        let client = {
            let mut state = lock(&self.state);
            state.generation += 1;
            state.client.take()
        };
        if let Some(client) = client {
            client.shutdown().await;
        }
    }
}

fn count<T>(server: &str, what: &str, listed: Result<Vec<T>, IntegrationError>) -> usize {
    match listed {
        Ok(items) => items.len(),
        Err(error) => {
            tracing::warn!(%server, %error, "could not list MCP {what}");
            0
        }
    }
}
