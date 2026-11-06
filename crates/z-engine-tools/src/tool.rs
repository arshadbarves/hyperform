//! The contract every tool implements, built-in or MCP. The engine gates
//! each call on `action` through the policy, previews it for approval, then
//! runs `call`; tools never decide permissions themselves.

use async_trait::async_trait;
use serde_json::Value;
use z_engine_policy::Action;
use z_engine_protocol::Preview;

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::output::ToolOutput;

#[async_trait]
pub trait Tool: Send + Sync {
    /// Model-facing name (Claude Code-compatible, e.g. `Read`).
    fn name(&self) -> &str;

    /// Model-facing description, from `z_engine_prompts::tools` plus any
    /// dynamic parts (such as the agent catalog).
    fn description(&self) -> String;

    /// JSON Schema object for the input.
    fn input_schema(&self) -> Value;

    /// Whether this input only observes state. Plan mode and read-only
    /// agents rely on it; it is not a permission decision.
    fn is_read_only(&self, input: &Value) -> bool;

    /// Whether this call may run alongside neighbouring safe calls.
    fn is_concurrency_safe(&self, input: &Value) -> bool {
        self.is_read_only(input)
    }

    /// The effect the policy decides on. Must be total: malformed input
    /// yields a conservative action and `call` reports the problem.
    fn action(&self, input: &Value, ctx: &ToolCtx) -> Action;

    /// Short card title, e.g. "Edit src/main.rs".
    fn title(&self, input: &Value, ctx: &ToolCtx) -> String;

    /// Rich detail for the approval card (a diff, a command).
    async fn preview(&self, _input: &Value, _ctx: &ToolCtx) -> Option<Preview> {
        None
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError>;
}
