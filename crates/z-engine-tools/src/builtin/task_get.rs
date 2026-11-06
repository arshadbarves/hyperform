//! `TaskGet` (`work_plans`): one task's packet.

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
use crate::tool::Tool;

#[derive(Debug, Default)]
pub struct TaskGetTool;

#[async_trait]
impl Tool for TaskGetTool {
    fn name(&self) -> &str {
        names::TASK_GET
    }

    fn description(&self) -> String {
        prompts::TASK_GET.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({"taskId": {"type": "string", "minLength": 1}}),
            &["taskId"],
        )
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn action(&self, _input: &Value, _ctx: &ToolCtx) -> Action {
        Action::Other { read_only: true }
    }

    fn title(&self, input: &Value, _ctx: &ToolCtx) -> String {
        match str_field(input, "taskId").map(str::trim) {
            Some(id) if !id.is_empty() => format!("Read task {id}"),
            _ => "Read task".to_string(),
        }
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let task_id = Fields::new(&input)?.non_empty("taskId")?.trim().to_string();
        let work = ctx.ports.work()?;
        let reply = work
            .get_task(ctx, &task_id)
            .await
            .map_err(ToolError::failed)?;
        Ok(ToolOutput::text(reply.text, reply.summary))
    }
}
