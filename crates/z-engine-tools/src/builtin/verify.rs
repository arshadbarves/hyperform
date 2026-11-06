//! `Verify`: lists the project's checks, or runs one and reports its record
//! as verification evidence.

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_policy::Action;
use z_engine_prompts::tools as prompts;
use z_engine_protocol::CheckRecord;

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::{Fields, str_field};
use crate::names;
use crate::output::ToolOutput;
use crate::ports::CheckSummary;
use crate::schema;
use crate::text::truncate_output;
use crate::tool::Tool;

#[derive(Debug, Default)]
pub struct VerifyTool;

fn runs(input: &Value) -> bool {
    str_field(input, "action").is_some_and(|action| action.trim() == "run")
}

#[async_trait]
impl Tool for VerifyTool {
    fn name(&self) -> &str {
        names::VERIFY
    }

    fn description(&self) -> String {
        prompts::VERIFY.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({
                "action": {"type": "string", "enum": ["list", "run"], "description": "list the checks, or run one."},
                "check": {"type": "string", "description": "The id of the check to run (required for run)."}
            }),
            &["action"],
        )
    }

    fn is_read_only(&self, input: &Value) -> bool {
        !runs(input)
    }

    fn action(&self, input: &Value, ctx: &ToolCtx) -> Action {
        if !runs(input) {
            return Action::Other { read_only: true };
        }
        let id = str_field(input, "check").unwrap_or_default().trim();
        let command = ctx.ports.checks.as_ref().and_then(|checks| {
            checks
                .list(ctx)
                .into_iter()
                .find(|check| check.id == id)
                .map(|check| check.command)
        });
        match command {
            Some(command) => Action::Execute { command },
            None => Action::Other { read_only: false },
        }
    }

    fn title(&self, input: &Value, _ctx: &ToolCtx) -> String {
        if runs(input) {
            format!(
                "Run check {}",
                str_field(input, "check").unwrap_or("?").trim()
            )
        } else {
            "List checks".to_string()
        }
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let fields = Fields::new(&input)?;
        let checks = ctx.ports.checks()?;
        let known = checks.list(ctx);
        match fields.non_empty("action")?.trim() {
            "list" => Ok(list(&known)),
            "run" => {
                let id = fields
                    .optional_text("check")?
                    .ok_or_else(|| {
                        ToolError::invalid(
                            "`check` is required to run a check; list them with action \"list\"",
                        )
                    })?
                    .trim();
                if !known.iter().any(|check| check.id == id) {
                    let ids: Vec<&str> = known.iter().map(|check| check.id.as_str()).collect();
                    return Err(ToolError::invalid(format!(
                        "unknown check {id:?}; available checks: {}",
                        if ids.is_empty() {
                            "none".to_string()
                        } else {
                            ids.join(", ")
                        }
                    )));
                }
                let record = ctx
                    .until_cancelled(checks.run(ctx, id))
                    .await?
                    .map_err(ToolError::failed)?;
                Ok(report(ctx, &record))
            }
            other => Err(ToolError::invalid(format!(
                "`action` must be list or run, not {other:?}"
            ))),
        }
    }
}

fn list(checks: &[CheckSummary]) -> ToolOutput {
    if checks.is_empty() {
        return ToolOutput::text(
            "No checks were discovered or configured for this project. Run project commands with Bash instead.",
            "No checks",
        );
    }
    let mut text = "| id | kind | label | command |\n|---|---|---|---|\n".to_string();
    for check in checks {
        text.push_str(&format!(
            "| {} | {} | {} | `{}` |\n",
            check.id,
            check.kind.label(),
            check.label,
            check.command
        ));
    }
    ToolOutput::text(text, format!("{} checks", checks.len()))
}

fn report(ctx: &ToolCtx, record: &CheckRecord) -> ToolOutput {
    let verdict = match (record.passed, record.timed_out) {
        (_, true) => "TIMED OUT",
        (true, false) => "PASS",
        (false, false) => "FAIL",
    };
    let exit = record.exit_code.map_or_else(
        || "no exit code".to_string(),
        |code| format!("exit code {code}"),
    );
    let mut text = format!(
        "{verdict}: {} ({}) `{}` ({exit}, {:.1} s)\n",
        record.label,
        record.kind.label(),
        record.command,
        record.duration_ms as f64 / 1000.0
    );
    if let Some(tests) = record.tests {
        text.push_str(&format!(
            "Tests: {} passed, {} failed, {} skipped\n",
            tests.passed, tests.failed, tests.skipped
        ));
    }
    if let Some(artifact) = &record.artifact {
        text.push_str(&format!("Full output: {artifact}\n"));
    }
    if !record.output_tail.trim().is_empty() {
        text.push_str("\nOutput (last lines):\n");
        text.push_str(&truncate_output(
            ctx,
            "check",
            record.output_tail.trim_end(),
        ));
        text.push('\n');
    }
    ToolOutput::text(text, format!("{verdict} {}", record.label))
        .with_error(!record.passed)
        .ran_command()
}
