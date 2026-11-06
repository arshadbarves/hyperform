//! MCP server tools as [`Tool`]s: named `mcp__{server}__{tool}`, gated as
//! MCP actions, and called through the MCP port with the original names.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Map, Value, json};
use z_engine_policy::Action;

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::output::ToolOutput;
use crate::text::truncate_parts;
use crate::tool::Tool;

/// Provider limit on tool names.
pub const MAX_TOOL_NAME_CHARS: usize = 64;
const SUMMARY_CHARS: usize = 80;

/// `mcp__{server}__{tool}` with characters outside `[A-Za-z0-9_-]` replaced
/// by `_`, cut to 64 characters.
pub fn mcp_tool_name(server: &str, tool: &str) -> String {
    let sanitize = |part: &str| -> String {
        part.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    };
    format!("mcp__{}__{}", sanitize(server), sanitize(tool))
        .chars()
        .take(MAX_TOOL_NAME_CHARS)
        .collect()
}

/// A tool that forwards calls to `tool` on MCP `server`. It is read-only
/// (and so concurrency-safe) exactly when the server hints so.
pub fn mcp_tool(
    server: &str,
    tool: &str,
    description: &str,
    input_schema: Value,
    read_only_hint: bool,
) -> Arc<dyn Tool> {
    Arc::new(McpTool {
        name: mcp_tool_name(server, tool),
        server: server.to_string(),
        tool: tool.to_string(),
        description: description.trim().to_string(),
        schema: object_schema(input_schema),
        read_only: read_only_hint,
    })
}

#[derive(Debug)]
struct McpTool {
    name: String,
    server: String,
    tool: String,
    description: String,
    schema: Value,
    read_only: bool,
}

/// Providers require an object schema; servers sometimes omit `type`.
fn object_schema(schema: Value) -> Value {
    match schema {
        Value::Object(mut map) => {
            map.insert("type".into(), Value::from("object"));
            map.entry("properties")
                .or_insert_with(|| Value::Object(Map::new()));
            Value::Object(map)
        }
        _ => json!({"type": "object", "properties": {}}),
    }
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> String {
        if self.description.is_empty() {
            format!("Tool {} from the MCP server {}.", self.tool, self.server)
        } else {
            self.description.clone()
        }
    }

    fn input_schema(&self) -> Value {
        self.schema.clone()
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        self.read_only
    }

    fn action(&self, _input: &Value, _ctx: &ToolCtx) -> Action {
        Action::Mcp {
            server: self.server.clone(),
            tool: self.tool.clone(),
            read_only: self.read_only,
        }
    }

    fn title(&self, _input: &Value, _ctx: &ToolCtx) -> String {
        format!("{}: {}", self.server, self.tool)
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let mcp = ctx.ports.mcp()?;
        let (parts, is_error) = ctx
            .until_cancelled(mcp.call_tool(ctx, &self.server, &self.tool, input))
            .await?
            .map_err(|reason| {
                ToolError::failed(format!("{} on {} failed: {reason}", self.tool, self.server))
            })?;
        let output = ToolOutput::parts(truncate_parts(ctx, "mcp", parts), "").with_error(is_error);
        let first_line = output
            .text_content()
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();
        let summary = if first_line.is_empty() {
            format!("Called {}", self.tool)
        } else if first_line.chars().count() > SUMMARY_CHARS {
            format!(
                "{}...",
                first_line.chars().take(SUMMARY_CHARS).collect::<String>()
            )
        } else {
            first_line
        };
        Ok(ToolOutput { summary, ..output })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_sanitized_and_capped() {
        assert_eq!(
            mcp_tool_name("git hub", "create.issue"),
            "mcp__git_hub__create_issue"
        );
        assert_eq!(
            mcp_tool_name("a", &"x".repeat(100)).len(),
            MAX_TOOL_NAME_CHARS
        );
    }

    #[test]
    fn schemas_become_objects() {
        assert_eq!(
            object_schema(json!(null)),
            json!({"type": "object", "properties": {}})
        );
        let kept =
            object_schema(json!({"properties": {"q": {"type": "string"}}, "required": ["q"]}));
        assert_eq!(kept["type"], "object");
        assert_eq!(kept["required"], json!(["q"]));
    }
}
