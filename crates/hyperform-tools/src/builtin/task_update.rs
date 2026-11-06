//! `TaskUpdate` (`work_plans`): changes a task's status, fields,
//! dependencies or criteria; completing it passes the kernel's gate.

use async_trait::async_trait;
use hyperform_policy::Action;
use hyperform_prompts::tools as prompts;
use serde_json::{Value, json};

use super::task_create::{acceptance_schema, task_fields};
use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::{str_field, whole};
use crate::names;
use crate::output::ToolOutput;
use crate::ports::TaskChange;
use crate::schema;
use crate::tool::Tool;

#[derive(Debug, Default)]
pub struct TaskUpdateTool;

/// A helper's result contract (`ReportDraft`).
fn report_schema() -> Value {
    let list = |description: &str| json!({"type": "array", "items": {"type": "string"}, "description": description});
    json!({
        "type": "object",
        "description": "Your result as a dispatched helper, sent once before you end your run.",
        "properties": {
            "summary": {"type": "string", "description": "What you did and how it turned out."},
            "filesChanged": list("Files you changed (the engine reads the real list from the diff)."),
            "symbolsChanged": list("Functions, types or modules you changed."),
            "discoveries": list("Facts about the code others should know."),
            "decisions": list("Choices you made, each with its reason."),
            "experiments": list("What you tried and what it showed."),
            "testsRun": list("Checks you ran (the engine records the real runs)."),
            "blockers": list("What stopped you, if anything."),
            "followUps": list("Work you found that is outside this task."),
            "risks": list("What could still go wrong."),
            "confidence": {"type": "string", "enum": ["low", "medium", "high"]}
        },
        "required": ["summary"]
    })
}

#[async_trait]
impl Tool for TaskUpdateTool {
    fn name(&self) -> &str {
        names::TASK_UPDATE
    }

    fn description(&self) -> String {
        prompts::TASK_UPDATE.to_string()
    }

    fn input_schema(&self) -> Value {
        let ids = |description: &str| json!({"type": "array", "items": {"type": "string"}, "description": description});
        let mut properties = json!({
            "taskId": {"type": "string", "minLength": 1},
            "status": {"type": "string", "enum": ["pending", "in_progress", "completed", "blocked", "failed", "deleted"]},
            "reason": {"type": "string", "description": "Why the task is blocked or failed."},
            "subject": {"type": "string"},
            "description": {"type": "string"},
            "activeForm": {"type": "string"},
            "result": {"type": "string", "description": "One line on what the task produced, for its dependents."},
            "acceptance": acceptance_schema(),
            "addBlockedBy": ids("Task ids this task must wait for."),
            "removeBlockedBy": ids("Task ids this task no longer waits for."),
            "addBlocks": ids("Task ids that must wait for this task."),
            "metCriteria": ids("Ids of inspection criteria you confirmed (c1, c2, ...)."),
            "report": report_schema()
        });
        for (key, value) in task_fields() {
            properties[key] = value;
        }
        schema::object(properties, &["taskId"])
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        false
    }

    fn action(&self, _input: &Value, _ctx: &ToolCtx) -> Action {
        Action::Other { read_only: true }
    }

    fn title(&self, input: &Value, _ctx: &ToolCtx) -> String {
        let id = str_field(input, "taskId").unwrap_or_default().trim();
        match (id, str_field(input, "status")) {
            ("", _) => "Update task".to_string(),
            (id, Some(status)) => format!("Update task {id}: {status}"),
            (id, None) => format!("Update task {id}"),
        }
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let change: TaskChange = whole(&input)?;
        if change.task_id.trim().is_empty() {
            return Err(ToolError::invalid("`taskId` must not be empty"));
        }
        let work = ctx.ports.work()?;
        let reply = work
            .update_task(ctx, change)
            .await
            .map_err(ToolError::failed)?;
        Ok(ToolOutput::text(reply.text, reply.summary))
    }
}
