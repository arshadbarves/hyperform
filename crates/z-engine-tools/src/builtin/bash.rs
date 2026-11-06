//! `Bash`: runs a command in the agent's persistent working directory with
//! live output, a deadline, cancellation, and truncated results; or starts
//! it as a background job.

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_host::{
    DEFAULT_MAX_OUTPUT_BYTES, HostError, RunOutput, RunSpec, is_sandbox_denial, run,
};
use z_engine_policy::{Action, shell};
use z_engine_prompts::tools as prompts;
use z_engine_protocol::Preview;

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::{Fields, str_field};
use crate::names;
use crate::output::ToolOutput;
use crate::schema;
use crate::text::trim_output;
use crate::tool::Tool;

const TITLE_CHARS: usize = 80;

#[derive(Debug, Default)]
pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        names::BASH
    }

    fn description(&self) -> String {
        prompts::BASH.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({
                "command": {"type": "string", "description": "The command to run."},
                "description": {"type": "string", "description": "What the command does in 5-10 words, e.g. \"Run the parser unit tests\"."},
                "timeout": {"type": "integer", "minimum": 1, "maximum": 600000, "description": "Timeout in milliseconds (default 120000, max 600000)."},
                "run_in_background": {"type": "boolean", "description": "Start the command as a background job and return its job id at once."}
            }),
            &["command"],
        )
    }

    fn is_read_only(&self, input: &Value) -> bool {
        str_field(input, "command").is_some_and(shell::is_read_only)
    }

    fn action(&self, input: &Value, _ctx: &ToolCtx) -> Action {
        Action::Execute {
            command: str_field(input, "command").unwrap_or_default().to_string(),
        }
    }

    fn title(&self, input: &Value, _ctx: &ToolCtx) -> String {
        let label = str_field(input, "description")
            .filter(|text| !text.trim().is_empty())
            .or_else(|| str_field(input, "command"))
            .unwrap_or("Bash");
        let first = label.trim().lines().next().unwrap_or_default();
        if first.chars().count() > TITLE_CHARS || label.trim().lines().nth(1).is_some() {
            format!("{}...", first.chars().take(TITLE_CHARS).collect::<String>())
        } else {
            first.to_string()
        }
    }

    async fn preview(&self, input: &Value, _ctx: &ToolCtx) -> Option<Preview> {
        Some(Preview::Command {
            command: str_field(input, "command")?.to_string(),
            description: str_field(input, "description").map(str::to_string),
        })
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let fields = Fields::new(&input)?;
        let command = fields.non_empty("command")?;
        let description = fields.optional_text("description")?;
        if fields.bool("run_in_background")?.unwrap_or(false) {
            return background(ctx, command, description).await;
        }
        let timeout_ms = fields
            .u64("timeout")?
            .unwrap_or(ctx.limits.bash_default_timeout_ms)
            .clamp(1, ctx.limits.bash_max_timeout_ms);
        let mut notes = Vec::new();
        let out = match execute(ctx, command, ctx.current_cwd(), timeout_ms).await {
            Err(HostError::NotFound(_)) if ctx.current_cwd() != ctx.root => {
                notes.push(format!(
                    "The working directory {} no longer exists; the command ran in the project root.",
                    ctx.current_cwd().display()
                ));
                ctx.set_cwd(ctx.root.clone());
                execute(ctx, command, ctx.root.clone(), timeout_ms).await
            }
            other => other,
        }
        .map_err(|error| match error {
            HostError::SandboxUnavailable(reason) => ToolError::failed(format!(
                "The command was not run: the sandbox cannot start ({reason})."
            )),
            other => ToolError::from(other),
        })?;
        if out.cancelled {
            return Err(ToolError::Cancelled);
        }
        if let Some(dir) = out.final_cwd.clone() {
            if ctx.is_allowed(&dir) {
                ctx.set_cwd(dir);
            } else {
                notes.push(format!(
                    "Shell cwd was reset to {}: {} is outside the allowed directories.",
                    ctx.root.display(),
                    dir.display()
                ));
                ctx.set_cwd(ctx.root.clone());
            }
        }
        Ok(report(ctx, command, &out, timeout_ms, notes).await)
    }
}

async fn execute(
    ctx: &ToolCtx,
    command: &str,
    cwd: PathBuf,
    timeout_ms: u64,
) -> Result<RunOutput, HostError> {
    let spec = RunSpec {
        command: command.to_string(),
        cwd,
        timeout: Duration::from_millis(timeout_ms),
        shell: ctx.shell.effective_spec()?,
        env: ctx.shell.env.clone(),
        track_cwd: true,
        max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
        stdin: None,
    };
    run(spec, ctx.cancel.clone(), ctx.progress.clone()).await
}

async fn report(
    ctx: &ToolCtx,
    command: &str,
    out: &RunOutput,
    timeout_ms: u64,
    mut notes: Vec<String>,
) -> ToolOutput {
    if let Some(hint) = sandbox_hint(ctx.shell.sandbox.is_some(), out) {
        notes.push(hint.to_string());
    }
    let combined = out.combined.trim_end_matches('\n');
    let mut text = trim_output(ctx, "bash", names::BASH, command, combined).await;
    if text.trim().is_empty() {
        text = "(no output)".to_string();
    }
    if out.truncated {
        text.push_str(
            "\n(The output exceeded the capture limit, so part of its middle was dropped.)",
        );
    }
    let (status, summary) = match (out.timed_out, out.exit_code) {
        (true, _) => (
            format!("Command timed out after {timeout_ms} ms and was killed."),
            format!("Timed out after {:.0} s", timeout_ms as f64 / 1000.0),
        ),
        (false, Some(0)) => (String::new(), "Exit code 0".to_string()),
        (false, Some(code)) => (format!("Exit code {code}"), format!("Exit code {code}")),
        (false, None) => (
            "The command was terminated by a signal.".to_string(),
            "Terminated by a signal".to_string(),
        ),
    };
    for line in notes
        .iter()
        .chain(std::iter::once(&status))
        .filter(|l| !l.is_empty())
    {
        text.push_str("\n\n");
        text.push_str(line);
    }
    let failed = out.timed_out || out.exit_code != Some(0);
    ToolOutput::text(text, summary)
        .with_error(failed)
        .ran_command()
}

/// Explains a failure that looks like the sandbox refusing a write.
fn sandbox_hint(sandboxed: bool, out: &RunOutput) -> Option<&'static str> {
    let failed = out.exit_code.is_some_and(|code| code != 0);
    (sandboxed && failed && is_sandbox_denial(&out.stderr)).then_some(
        "The command runs in a sandbox and appears to have tried to write outside the allowed directories (or to reach the network). Keep writes inside the project, or ask the user to re-run it without the sandbox.",
    )
}

async fn background(
    ctx: &ToolCtx,
    command: &str,
    description: Option<&str>,
) -> Result<ToolOutput, ToolError> {
    let jobs = ctx.ports.jobs()?;
    let job = jobs
        .spawn_shell(ctx, command.to_string(), description.map(str::to_string))
        .await
        .map_err(|e| ToolError::failed(format!("could not start the background job: {e}")))?;
    Ok(ToolOutput::text(
        format!(
            "Started in the background as job {job}. Read its output with JobOutput (job_id \"{job}\") and stop it with JobKill when it is no longer needed. You are notified when it finishes."
        ),
        format!("Running in background ({job})"),
    )
    .ran_command())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failed_with(stderr: &str) -> RunOutput {
        RunOutput {
            exit_code: Some(1),
            stderr: stderr.to_string(),
            ..RunOutput::default()
        }
    }

    #[test]
    fn sandbox_hint_needs_a_sandboxed_failure_with_a_denial() {
        let denied = failed_with("touch: /x: Operation not permitted\n");
        assert!(sandbox_hint(true, &denied).is_some());
        assert!(sandbox_hint(false, &denied).is_none());
        assert!(sandbox_hint(true, &failed_with("no such file\n")).is_none());
        let passed = RunOutput {
            exit_code: Some(0),
            ..denied
        };
        assert!(sandbox_hint(true, &passed).is_none());
    }
}
