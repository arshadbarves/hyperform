//! `ReadMcpResource`: one resource from a connected MCP server.

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
use crate::text::truncate_parts;
use crate::tool::Tool;

#[derive(Debug, Default)]
pub struct ReadMcpResourceTool;

#[async_trait]
impl Tool for ReadMcpResourceTool {
    fn name(&self) -> &str {
        names::READ_MCP_RESOURCE
    }

    fn description(&self) -> String {
        prompts::READ_MCP_RESOURCE.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({
                "server": {"type": "string", "description": "The MCP server name."},
                "uri": {"type": "string", "description": "The resource URI, as listed by ListMcpResources."}
            }),
            &["server", "uri"],
        )
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn action(&self, input: &Value, _ctx: &ToolCtx) -> Action {
        Action::Mcp {
            server: str_field(input, "server")
                .unwrap_or_default()
                .trim()
                .to_string(),
            tool: "resources".to_string(),
            read_only: true,
        }
    }

    fn title(&self, input: &Value, _ctx: &ToolCtx) -> String {
        format!(
            "Read {} from {}",
            str_field(input, "uri").unwrap_or("resource"),
            str_field(input, "server").unwrap_or("MCP")
        )
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let fields = Fields::new(&input)?;
        let server = fields.non_empty("server")?.trim();
        let uri = fields.non_empty("uri")?.trim();
        let mcp = ctx.ports.mcp()?;
        let parts = ctx
            .until_cancelled(mcp.read_resource(ctx, server, uri))
            .await?
            .map_err(ToolError::failed)?;
        if parts.is_empty() {
            return Ok(ToolOutput::text(
                format!("{uri} on {server} is empty."),
                "Empty resource",
            ));
        }
        Ok(ToolOutput::parts(
            truncate_parts(ctx, "mcp", parts),
            format!("Read {uri}"),
        ))
    }
}
