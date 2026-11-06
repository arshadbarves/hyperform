//! `McpPort`: tool calls with per-call cancellation, resources formatted
//! for the model, and loading deferred tools into the calling run.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use z_engine_integrations::McpManager;
use z_engine_protocol::ToolResultPart;
use z_engine_tools::{McpPort, ToolCtx};

use crate::mcp::{McpRunState, call_parts, resource_listing, resource_parts};
use crate::session::SessionCore;

#[derive(Debug)]
pub(crate) struct Mcp {
    core: Arc<SessionCore>,
    run: Arc<McpRunState>,
}

impl Mcp {
    pub(crate) fn new(core: Arc<SessionCore>, run: Arc<McpRunState>) -> Self {
        Self { core, run }
    }

    fn manager(&self, server: &str) -> Result<McpManager, String> {
        self.core
            .mcp
            .manager(server.trim())
            .ok_or_else(|| format!("no MCP server named `{server}` is configured"))
    }
}

#[async_trait]
impl McpPort for Mcp {
    async fn call_tool(
        &self,
        ctx: &ToolCtx,
        server: &str,
        tool: &str,
        input: Value,
    ) -> Result<(Vec<ToolResultPart>, bool), String> {
        let result = self
            .manager(server)?
            .call_tool(server, tool, input, &ctx.cancel)
            .await
            .map_err(|error| error.to_string())?;
        Ok((call_parts(&result), result.is_error))
    }

    async fn list_resources(&self, _ctx: &ToolCtx, server: Option<&str>) -> Result<String, String> {
        let mut resources = Vec::new();
        match server.map(str::trim).filter(|server| !server.is_empty()) {
            Some(server) => {
                resources = self
                    .manager(server)?
                    .list_resources(Some(server))
                    .await
                    .map_err(|error| error.to_string())?
            }
            None => {
                for manager in self.core.mcp.managers() {
                    match manager.list_resources(None).await {
                        Ok(listed) => resources.extend(listed),
                        Err(error) => tracing::debug!(%error, "MCP resources unavailable"),
                    }
                }
            }
        }
        Ok(resource_listing(&resources))
    }

    async fn read_resource(
        &self,
        _ctx: &ToolCtx,
        server: &str,
        uri: &str,
    ) -> Result<Vec<ToolResultPart>, String> {
        let contents = self
            .manager(server)?
            .read_resource(server.trim(), uri)
            .await
            .map_err(|error| error.to_string())?;
        Ok(resource_parts(&contents))
    }

    async fn load_tools(&self, _ctx: &ToolCtx, names: &[String]) -> Result<String, String> {
        let catalog = self.core.mcp.catalog();
        if !catalog.deferred() {
            return Err("MCP tools are not deferred in this session; call them directly".into());
        }
        let (known, unknown): (Vec<&String>, Vec<&String>) =
            names.iter().partition(|name| catalog.contains(name));
        if known.is_empty() {
            return Err(format!(
                "none of these is an available MCP tool: {}",
                join(&unknown)
            ));
        }
        self.run.load(known.iter().map(|name| (*name).clone()));
        let mut report = format!(
            "Loaded {}. They are callable from your next response on.",
            join(&known)
        );
        if !unknown.is_empty() {
            report.push_str(&format!(
                "\nNot found in the MCP tool list: {}.",
                join(&unknown)
            ));
        }
        Ok(report)
    }
}

fn join(names: &[&String]) -> String {
    names
        .iter()
        .map(|name| name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}
