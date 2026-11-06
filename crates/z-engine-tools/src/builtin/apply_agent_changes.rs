//! `ApplyAgentChanges`: merges a worktree agent's changes into the tree.

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_policy::Action;
use z_engine_prompts::tools as prompts;
use z_engine_protocol::AgentId;

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::{Fields, str_field};
use crate::names;
use crate::output::ToolOutput;
use crate::schema;
use crate::tool::Tool;

#[derive(Debug, Default)]
pub struct ApplyAgentChangesTool;

#[async_trait]
impl Tool for ApplyAgentChangesTool {
    fn name(&self) -> &str {
        names::APPLY_AGENT_CHANGES
    }

    fn description(&self) -> String {
        prompts::APPLY_AGENT_CHANGES.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({
                "agent_id": {"type": "string", "description": "The id of the finished worktree agent."}
            }),
            &["agent_id"],
        )
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        false
    }

    fn action(&self, _input: &Value, ctx: &ToolCtx) -> Action {
        Action::Write {
            paths: vec![ctx.root.clone()],
        }
    }

    fn title(&self, input: &Value, _ctx: &ToolCtx) -> String {
        format!(
            "Apply changes from {}",
            str_field(input, "agent_id").unwrap_or("agent")
        )
    }

    /// Not raced against cancellation: an abandoned merge could leave the
    /// tree half-applied, so the port decides how to stop.
    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let agent = AgentId::from(Fields::new(&input)?.non_empty("agent_id")?.trim());
        let agents = ctx.ports.agents()?;
        let summary = agents.apply_changes(ctx, &agent).await.map_err(|reason| {
            ToolError::failed(format!("could not apply {agent}'s changes: {reason}"))
        })?;
        Ok(ToolOutput::text(
            summary,
            format!("Applied changes from {agent}"),
        ))
    }
}
