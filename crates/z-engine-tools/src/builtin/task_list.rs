//! `TaskList` (`work_plans`): the attached plan's tasks by state.

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
pub struct TaskListTool;

#[async_trait]
impl Tool for TaskListTool {
    fn name(&self) -> &str {
        names::TASK_LIST
    }

    fn description(&self) -> String {
        prompts::TASK_LIST.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(json!({}), &[])
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn action(&self, _input: &Value, _ctx: &ToolCtx) -> Action {
        Action::Other { read_only: true }
    }

    fn title(&self, _input: &Value, _ctx: &ToolCtx) -> String {
        "List tasks".to_string()
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        Fields::new(&input)?;
        let work = ctx.ports.work()?;
        let reply = work.list_tasks(ctx).await.map_err(ToolError::failed)?;
        Ok(ToolOutput::text(reply.text, reply.summary))
    }
}
