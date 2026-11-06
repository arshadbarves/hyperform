//! [`McpManager`]: every configured MCP server, connected concurrently,
//! with status for the GUI, a merged tool catalog, and requests that
//! reconnect once when a server's connection is already gone.

use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};

use futures::FutureExt;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::entry::ServerEntry;
use super::naming::tool_name;
use super::spec::McpServerSpec;
use super::status::{McpChangeCallback, McpServerStatus};
use super::types::{
    CallToolResult, McpPromptInfo, McpResourceInfo, McpToolInfo, PromptMessage, ResourceContents,
};
use crate::error::IntegrationError;
use crate::sync::lock;

/// Cheap to clone; clones share the servers.
#[derive(Debug, Clone, Default)]
pub struct McpManager {
    entries: Arc<Mutex<Vec<Arc<ServerEntry>>>>,
}

impl McpManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the current servers with `specs` and connects every enabled
    /// one concurrently; returns once each is Ready or Failed. Run it in a
    /// task to watch `Connecting` states through [`McpManager::status`].
    /// Duplicate names keep the first spec.
    pub async fn start(&self, specs: Vec<McpServerSpec>, on_change: Option<McpChangeCallback>) {
        self.shutdown_all().await;
        let mut names = HashSet::new();
        let entries: Vec<Arc<ServerEntry>> = specs
            .into_iter()
            .filter(|spec| {
                let fresh = names.insert(spec.name.clone());
                if !fresh {
                    tracing::warn!(server = %spec.name, "duplicate MCP server name ignored");
                }
                fresh
            })
            .map(|spec| ServerEntry::new(spec, on_change.clone()))
            .collect();
        lock(&self.entries).clone_from(&entries);
        let connects = entries
            .iter()
            .filter(|entry| entry.spec.enabled)
            .map(|entry| async move {
                if let Err(error) = entry.connect().await {
                    tracing::warn!(server = %entry.spec.name, %error, "MCP server failed to start");
                }
            });
        futures::future::join_all(connects).await;
    }

    fn entries(&self) -> Vec<Arc<ServerEntry>> {
        lock(&self.entries).clone()
    }

    fn entry(&self, server: &str) -> Result<Arc<ServerEntry>, IntegrationError> {
        self.entries()
            .into_iter()
            .find(|entry| entry.spec.name == server)
            .ok_or_else(|| IntegrationError::NotFound(format!("no MCP server named `{server}`")))
    }

    /// Every server in configuration order.
    pub fn status(&self) -> Vec<McpServerStatus> {
        self.entries().iter().map(|entry| entry.status()).collect()
    }

    /// `(server, tool)` for every tool of every ready server; servers that
    /// announced a change are listed again first (concurrently).
    pub async fn tools(&self) -> Vec<(String, McpToolInfo)> {
        self.list_tools(false).await
    }

    /// [`McpManager::tools`] plus the last listed tools of servers whose
    /// connection was lost, which reconnect when one is called.
    pub async fn tools_or_last_known(&self) -> Vec<(String, McpToolInfo)> {
        self.list_tools(true).await
    }

    async fn list_tools(&self, last_known: bool) -> Vec<(String, McpToolInfo)> {
        let entries = self.entries();
        let listed = futures::future::join_all(entries.iter().map(|entry| async move {
            if last_known {
                entry.tools_or_last_known().await
            } else {
                entry.tools().await
            }
        }))
        .await;
        entries
            .iter()
            .zip(listed)
            .flat_map(|(entry, tools)| {
                tools
                    .into_iter()
                    .map(|tool| (entry.spec.name.clone(), tool))
            })
            .collect()
    }

    /// Maps a model-facing `mcp__server__tool` name back to `(server, tool)`
    /// through the catalog, which [`split_tool_name`](super::split_tool_name)
    /// cannot do for sanitized or truncated names.
    pub fn resolve_tool(&self, qualified: &str) -> Option<(String, String)> {
        self.entries().iter().find_map(|entry| {
            entry
                .cached_tools()
                .into_iter()
                .find(|tool| tool_name(&entry.spec.name, &tool.name) == qualified)
                .map(|tool| (entry.spec.name.clone(), tool.name))
        })
    }

    /// Calls a tool with the server's timeout. Reconnects once when the
    /// server is down, and retries only a request that was never sent.
    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: Value,
        cancel: &CancellationToken,
    ) -> Result<CallToolResult, IntegrationError> {
        let (tool, cancel) = (tool.to_string(), cancel.clone());
        self.entry(server)?
            .run(move |client| {
                let (tool, arguments, cancel) = (tool.clone(), arguments.clone(), cancel.clone());
                async move { client.call_tool(&tool, arguments, &cancel).await }.boxed()
            })
            .await
    }

    /// Resources of one server, or of every ready server declaring them.
    pub async fn list_resources(
        &self,
        server: Option<&str>,
    ) -> Result<Vec<(String, McpResourceInfo)>, IntegrationError> {
        let mut all = Vec::new();
        for entry in self.targets(server)? {
            let resources = entry
                .run(|client| async move { client.list_resources().await }.boxed())
                .await?;
            all.extend(resources.into_iter().map(|r| (entry.spec.name.clone(), r)));
        }
        Ok(all)
    }

    pub async fn read_resource(
        &self,
        server: &str,
        uri: &str,
    ) -> Result<Vec<ResourceContents>, IntegrationError> {
        let uri = uri.to_string();
        self.entry(server)?
            .run(move |client| {
                let uri = uri.clone();
                async move { client.read_resource(&uri).await }.boxed()
            })
            .await
    }

    /// Prompts of every ready server declaring them.
    pub async fn prompts(&self) -> Result<Vec<(String, McpPromptInfo)>, IntegrationError> {
        let mut all = Vec::new();
        for entry in self.targets(None)? {
            let prompts = entry
                .run(|client| async move { client.list_prompts().await }.boxed())
                .await?;
            all.extend(prompts.into_iter().map(|p| (entry.spec.name.clone(), p)));
        }
        Ok(all)
    }

    pub async fn get_prompt(
        &self,
        server: &str,
        name: &str,
        arguments: BTreeMap<String, String>,
    ) -> Result<Vec<PromptMessage>, IntegrationError> {
        let name = name.to_string();
        self.entry(server)?
            .run(move |client| {
                let (name, arguments) = (name.clone(), arguments.clone());
                async move { client.get_prompt(&name, &arguments).await }.boxed()
            })
            .await
    }

    /// One named server, or every ready server (failed, disabled and
    /// connecting servers are skipped).
    fn targets(&self, server: Option<&str>) -> Result<Vec<Arc<ServerEntry>>, IntegrationError> {
        match server {
            Some(name) => Ok(vec![self.entry(name)?]),
            None => Ok(self
                .entries()
                .into_iter()
                .filter(|entry| entry.ready_client().is_some())
                .collect()),
        }
    }

    /// Reconnects one server (its tools and counts are listed again).
    pub async fn restart(&self, server: &str) -> Result<(), IntegrationError> {
        self.entry(server)?.connect().await.map(|_| ())
    }

    /// Shuts every server down and forgets them.
    pub async fn shutdown_all(&self) {
        let entries = std::mem::take(&mut *lock(&self.entries));
        futures::future::join_all(entries.iter().map(|entry| entry.shutdown())).await;
    }
}
