//! `ListMcpResources`: resources exposed by connected MCP servers.

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_policy::Action;
use z_engine_prompts::tools as prompts;

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::{Fields, str_field};
use crate::names;
use crate::output::ToolOutput;
use crate::schema;
use crate::text::truncate_output;
use crate::tool::Tool;

/// The `server` of the policy action when every server is listed.
const ALL_SERVERS: &str = "*";

#[derive(Debug, Default)]
pub struct ListMcpResourcesTool;

#[async_trait]
impl Tool for ListMcpResourcesTool {
    fn name(&self) -> &str {
        names::LIST_MCP_RESOURCES
    }

    fn description(&self) -> String {
        prompts::LIST_MCP_RESOURCES.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({
                "server": {"type": "string", "description": "Only list this server's resources (default: every connected server)."}
            }),
            &[],
        )
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn action(&self, input: &Value, _ctx: &ToolCtx) -> Action {
        let server = str_field(input, "server")
            .map(str::trim)
            .filter(|server| !server.is_empty())
            .unwrap_or(ALL_SERVERS);
        Action::Mcp {
            server: server.to_string(),
            tool: "resources".to_string(),
            read_only: true,
        }
    }

    fn title(&self, input: &Value, _ctx: &ToolCtx) -> String {
        match str_field(input, "server")
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(server) => format!("List MCP resources on {server}"),
            None => "List MCP resources".to_string(),
        }
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let server = Fields::new(&input)?.optional_text("server")?.map(str::trim);
        let mcp = ctx.ports.mcp()?;
        let listing = ctx
            .until_cancelled(mcp.list_resources(ctx, server))
            .await?
            .map_err(ToolError::failed)?;
        if listing.trim().is_empty() {
            return Ok(ToolOutput::text("No resources found.", "No resources"));
        }
        Ok(ToolOutput::text(
            truncate_output(ctx, "mcp", &listing),
            "Listed MCP resources",
        ))
    }
}
