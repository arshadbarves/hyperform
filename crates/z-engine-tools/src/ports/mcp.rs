//! Connected MCP servers: tool calls and resources.

use async_trait::async_trait;
use serde_json::Value;
use z_engine_protocol::ToolResultPart;

use crate::context::ToolCtx;

#[async_trait]
pub trait McpPort: Send + Sync {
    /// The result content and whether the server flagged it as an error.
    async fn call_tool(
        &self,
        ctx: &ToolCtx,
        server: &str,
        tool: &str,
        input: Value,
    ) -> Result<(Vec<ToolResultPart>, bool), String>;

    /// A formatted listing; `None` lists every connected server.
    async fn list_resources(&self, ctx: &ToolCtx, server: Option<&str>) -> Result<String, String>;

    async fn read_resource(
        &self,
        ctx: &ToolCtx,
        server: &str,
        uri: &str,
    ) -> Result<Vec<ToolResultPart>, String>;

    /// Makes deferred MCP tools (model-facing names) callable by the
    /// calling agent from its next request on; returns a report for the
    /// model.
    async fn load_tools(&self, _ctx: &ToolCtx, _names: &[String]) -> Result<String, String> {
        Err("MCP tools are not deferred in this session; call them directly".to_string())
    }
}
