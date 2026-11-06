//! `JobKill`: stops a background shell or agent.

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_policy::Action;
use z_engine_prompts::tools as prompts;
use z_engine_protocol::JobId;

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::{Fields, str_field};
use crate::names;
use crate::output::ToolOutput;
use crate::schema;
use crate::tool::Tool;

#[derive(Debug, Default)]
pub struct JobKillTool;

#[async_trait]
impl Tool for JobKillTool {
    fn name(&self) -> &str {
        names::JOB_KILL
    }

    fn description(&self) -> String {
        prompts::JOB_KILL.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({
                "job_id": {"type": "string", "description": "The id of the background job to stop."}
            }),
            &["job_id"],
        )
    }

    /// Stopping the agent's own job changes no files.
    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn action(&self, _input: &Value, _ctx: &ToolCtx) -> Action {
        Action::Other { read_only: true }
    }

    fn title(&self, input: &Value, _ctx: &ToolCtx) -> String {
        format!("Stop {}", str_field(input, "job_id").unwrap_or("job"))
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let job = JobId::from(Fields::new(&input)?.non_empty("job_id")?.trim());
        let jobs = ctx.ports.jobs()?;
        jobs.kill(ctx, &job).await.map_err(ToolError::failed)?;
        Ok(ToolOutput::text(
            format!("Job {job} was stopped. Output it produced can still be read with JobOutput."),
            format!("Stopped {job}"),
        ))
    }
}
