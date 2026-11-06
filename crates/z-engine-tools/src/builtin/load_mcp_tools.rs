//! `LoadMcpTools`: makes deferred MCP tools callable for the calling agent.

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_policy::Action;
use z_engine_prompts::tools as prompts;

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::Fields;
use crate::names;
use crate::output::ToolOutput;
use crate::schema;
use crate::tool::Tool;

#[derive(Debug, Default)]
pub struct LoadMcpToolsTool;

fn requested(input: &Value) -> Vec<String> {
    input
        .get("names")
        .and_then(Value::as_array)
        .map(|names| {
            names
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[async_trait]
impl Tool for LoadMcpToolsTool {
    fn name(&self) -> &str {
        names::LOAD_MCP_TOOLS
    }

    fn description(&self) -> String {
        prompts::LOAD_MCP_TOOLS.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({
                "names": {
                    "type": "array",
                    "items": {"type": "string"},
                    "minItems": 1,
                    "description": "Exact MCP tool names to load, e.g. mcp__github__create_issue."
                }
            }),
            &["names"],
        )
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn action(&self, _input: &Value, _ctx: &ToolCtx) -> Action {
        Action::Other { read_only: true }
    }

    fn title(&self, input: &Value, _ctx: &ToolCtx) -> String {
        match requested(input).len() {
            1 => "Load 1 MCP tool".to_string(),
            count => format!("Load {count} MCP tools"),
        }
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        Fields::new(&input)?;
        let names = requested(&input);
        if names.is_empty() {
            return Err(ToolError::invalid(
                "`names` must list at least one MCP tool name",
            ));
        }
        let mcp = ctx.ports.mcp()?;
        let report = mcp
            .load_tools(ctx, &names)
            .await
            .map_err(ToolError::failed)?;
        let summary = format!("Loaded {} MCP tool(s)", names.len());
        Ok(ToolOutput::text(report, summary))
    }
}
