//! `TodoWrite`: replaces the agent's todo list.

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_policy::Action;
use z_engine_prompts::tools as prompts;
use z_engine_protocol::{TodoItem, TodoStatus};

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::Fields;
use crate::names;
use crate::output::ToolOutput;
use crate::schema;
use crate::tool::Tool;

#[derive(Debug, Default)]
pub struct TodoWriteTool;

fn todos(input: &Value) -> Result<Vec<TodoItem>, ToolError> {
    let mut todos: Vec<TodoItem> = Fields::new(input)?.required("todos")?;
    for (index, todo) in todos.iter_mut().enumerate() {
        if todo.content.trim().is_empty() {
            return Err(ToolError::invalid(format!(
                "todo {}: `content` must not be empty",
                index + 1
            )));
        }
        if todo.active_form.trim().is_empty() {
            todo.active_form = todo.content.clone();
        }
    }
    Ok(todos)
}

#[async_trait]
impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        names::TODO_WRITE
    }

    fn description(&self) -> String {
        prompts::TODO_WRITE.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({
                "todos": {
                    "type": "array",
                    "description": "The complete, updated todo list.",
                    "items": schema::object(
                        json!({
                            "content": {"type": "string", "minLength": 1, "description": "The task in the imperative, e.g. \"Run the tests\"."},
                            "activeForm": {"type": "string", "minLength": 1, "description": "The task in the present continuous, e.g. \"Running the tests\"."},
                            "status": {"type": "string", "enum": ["pending", "in_progress", "completed"]}
                        }),
                        &["content", "activeForm", "status"],
                    )
                }
            }),
            &["todos"],
        )
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    /// One list per agent: concurrent updates would race.
    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        false
    }

    fn action(&self, _input: &Value, _ctx: &ToolCtx) -> Action {
        Action::Other { read_only: true }
    }

    fn title(&self, _input: &Value, _ctx: &ToolCtx) -> String {
        "Update todos".to_string()
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let todos = todos(&input)?;
        let count = |status| todos.iter().filter(|todo| todo.status == status).count();
        let (done, active, pending) = (
            count(TodoStatus::Completed),
            count(TodoStatus::InProgress),
            count(TodoStatus::Pending),
        );
        ctx.ports.interaction()?.update_todos(ctx, todos.clone());
        let mut text = format!(
            "Todo list updated: {done} completed, {active} in progress, {pending} pending."
        );
        if active > 1 {
            text.push_str(&format!(
                " {active} items are in_progress; keep exactly one in_progress at a time."
            ));
        } else if active == 0 && pending > 0 {
            text.push_str(" Mark the next item in_progress before you start it.");
        } else if active == 1 {
            text.push_str(
                " Keep exactly one item in_progress, and mark it completed as soon as it is done.",
            );
        }
        let summary = if todos.is_empty() {
            "Cleared todos".to_string()
        } else {
            format!("{done}/{} todos completed", todos.len())
        };
        Ok(ToolOutput::text(text, summary))
    }
}
