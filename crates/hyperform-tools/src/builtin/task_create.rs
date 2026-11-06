//! `TaskCreate` (`work_plans`): adds a task to the attached work plan,
//! starting a plan when none is attached.

use async_trait::async_trait;
use hyperform_policy::Action;
use hyperform_prompts::tools as prompts;
use serde_json::{Value, json};

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::{str_field, whole};
use crate::names;
use crate::output::ToolOutput;
use crate::ports::NewTask;
use crate::schema;
use crate::tool::Tool;

#[derive(Debug, Default)]
pub struct TaskCreateTool;

/// Acceptance criteria, shared with `TaskUpdate`.
pub(crate) fn acceptance_schema() -> Value {
    json!({
        "type": "array",
        "description": "Acceptance criteria; check methods are met only by a passing Verify run of that kind on the current code.",
        "items": schema::object(
            json!({
                "text": {"type": "string", "description": "What must hold, e.g. \"Retry tests pass\"."},
                "method": {"type": "string", "enum": ["test", "build", "lint", "typecheck", "format", "custom", "inspection", "user"]}
            }),
            &["text", "method"],
        )
    })
}

/// `kind`, `priority` and `scope`, shared with `TaskUpdate`.
pub(crate) fn task_fields() -> [(&'static str, Value); 3] {
    [
        (
            "kind",
            json!({"type": "string", "enum": ["research", "design", "implement", "verify", "integrate", "experiment"]}),
        ),
        (
            "priority",
            json!({"type": "string", "enum": ["low", "normal", "high"]}),
        ),
        (
            "scope",
            json!({"type": "array", "items": {"type": "string"}, "description": "Files or globs (relative to the project root) the task changes."}),
        ),
    ]
}

#[async_trait]
impl Tool for TaskCreateTool {
    fn name(&self) -> &str {
        names::TASK_CREATE
    }

    fn description(&self) -> String {
        prompts::TASK_CREATE.to_string()
    }

    fn input_schema(&self) -> Value {
        let mut properties = json!({
            "subject": {"type": "string", "minLength": 1, "description": "Short imperative title."},
            "description": {"type": "string", "description": "What done looks like and what another agent would need."},
            "activeForm": {"type": "string", "description": "Present continuous, shown while the task runs."},
            "parentId": {"type": "string", "description": "Makes this a subtask of that task."},
            "blockedBy": {"type": "array", "items": {"type": "string"}, "description": "Task ids that must be completed first."},
            "acceptance": acceptance_schema(),
            "requirements": {"type": "array", "items": {"type": "string"}, "description": "Ids of plan requirements this task serves."}
        });
        for (key, value) in task_fields() {
            properties[key] = value;
        }
        schema::object(properties, &["subject", "description"])
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
        match str_field(input, "subject").map(str::trim) {
            Some(subject) if !subject.is_empty() => format!("Create task: {subject}"),
            _ => "Create task".to_string(),
        }
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let task: NewTask = whole(&input)?;
        if task.subject.trim().is_empty() {
            return Err(ToolError::invalid("`subject` must not be empty"));
        }
        let work = ctx.ports.work()?;
        let reply = work
            .create_task(ctx, task)
            .await
            .map_err(ToolError::failed)?;
        Ok(ToolOutput::text(reply.text, reply.summary))
    }
}
