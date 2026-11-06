//! `JobOutput`: new output and state of a background shell or agent.

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_policy::Action;
use z_engine_prompts::tools as prompts;
use z_engine_protocol::{JobId, JobKind, JobStatus};

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::{Fields, str_field};
use crate::names;
use crate::output::ToolOutput;
use crate::schema;
use crate::text::{trim_output, truncate_output};
use crate::tool::Tool;

#[derive(Debug, Default)]
pub struct JobOutputTool;

fn status_label(status: JobStatus) -> &'static str {
    match status {
        JobStatus::Running => "running",
        JobStatus::Completed => "completed",
        JobStatus::Failed => "failed",
        JobStatus::Killed => "killed",
    }
}

#[async_trait]
impl Tool for JobOutputTool {
    fn name(&self) -> &str {
        names::JOB_OUTPUT
    }

    fn description(&self) -> String {
        prompts::JOB_OUTPUT.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({
                "job_id": {"type": "string", "description": "The id of the background job."},
                "filter": {"type": "string", "description": "Regular expression; only matching output lines are returned."},
                "wait_ms": {"type": "integer", "minimum": 0, "maximum": 600000, "description": "Wait up to this many milliseconds for the job to finish before returning."}
            }),
            &["job_id"],
        )
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn action(&self, _input: &Value, _ctx: &ToolCtx) -> Action {
        Action::Other { read_only: true }
    }

    fn title(&self, input: &Value, _ctx: &ToolCtx) -> String {
        format!("Output of {}", str_field(input, "job_id").unwrap_or("job"))
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let fields = Fields::new(&input)?;
        let job = JobId::from(fields.non_empty("job_id")?.trim());
        let filter = fields.optional_text("filter")?.map(str::to_string);
        if let Some(pattern) = &filter {
            regex::Regex::new(pattern)
                .map_err(|e| ToolError::invalid(format!("`filter` is not a valid regex: {e}")))?;
        }
        let wait = fields
            .u64("wait_ms")?
            .filter(|&ms| ms > 0)
            .map(|ms| Duration::from_millis(ms.min(ctx.limits.bash_max_timeout_ms)));
        let jobs = ctx.ports.jobs()?;
        let read = ctx
            .until_cancelled(jobs.output(ctx, &job, filter, wait))
            .await?
            .map_err(ToolError::failed)?;
        let kind = match read.kind {
            JobKind::Shell => "shell",
            JobKind::Agent => "agent",
        };
        let mut state = format!("Job {job} ({kind}) status: {}", status_label(read.status));
        if let Some(code) = read.exit_code {
            state.push_str(&format!(", exit code {code}"));
        }
        let output = read.output.trim_end_matches('\n');
        let body = if read.output.trim().is_empty() {
            "(no new output)".to_string()
        } else if read.kind == JobKind::Shell {
            let subject = format!("background job {job}");
            trim_output(ctx, "job", names::JOB_OUTPUT, &subject, output).await
        } else {
            truncate_output(ctx, "job", output)
        };
        let failed = matches!(read.status, JobStatus::Failed);
        Ok(ToolOutput::text(format!("{state}.\n\n{body}"), state).with_error(failed))
    }
}
