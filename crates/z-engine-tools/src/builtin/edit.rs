//! `Edit`: one exact string replacement in a file the model has read, with
//! `replace_all` for every occurrence and a fallback ladder for a single
//! unambiguous near match.

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_policy::Action;
use z_engine_prompts::tools as prompts;
use z_engine_protocol::Preview;

use crate::context::ToolCtx;
use crate::edit::{EditSpec, describe, edit_file, preview_edits};
use crate::error::ToolError;
use crate::input::{Fields, path_field};
use crate::names;
use crate::output::ToolOutput;
use crate::schema;
use crate::tool::Tool;

#[derive(Debug, Default)]
pub struct EditTool;

/// The replacement described by an `Edit` (or `MultiEdit` entry) input.
pub(crate) fn edit_spec(fields: &Fields<'_>) -> Result<EditSpec, ToolError> {
    Ok(EditSpec {
        old: fields.required_str("old_string")?.to_string(),
        new: fields.required_str("new_string")?.to_string(),
        replace_all: fields.bool("replace_all")?.unwrap_or(false),
    })
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        names::EDIT
    }

    fn description(&self) -> String {
        prompts::EDIT.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({
                "file_path": {"type": "string", "description": "The file to modify: an absolute path, or a path relative to the project root."},
                "old_string": {"type": "string", "description": "The exact text to replace."},
                "new_string": {"type": "string", "description": "The replacement text (must differ from old_string)."},
                "replace_all": {"type": "boolean", "default": false, "description": "Replace every occurrence of old_string (default false)."}
            }),
            &["file_path", "old_string", "new_string"],
        )
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        false
    }

    fn action(&self, input: &Value, ctx: &ToolCtx) -> Action {
        Action::Write {
            paths: path_field(input, "file_path", ctx).into_iter().collect(),
        }
    }

    fn title(&self, input: &Value, ctx: &ToolCtx) -> String {
        match path_field(input, "file_path", ctx) {
            Some(path) => format!("Edit {}", ctx.display(&path)),
            None => "Edit".to_string(),
        }
    }

    async fn preview(&self, input: &Value, ctx: &ToolCtx) -> Option<Preview> {
        let path = path_field(input, "file_path", ctx)?;
        let spec = edit_spec(&Fields::new(input).ok()?).ok()?;
        preview_edits(ctx, &path, vec![spec]).await
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let fields = Fields::new(&input)?;
        let path = ctx.resolve(fields.non_empty("file_path")?);
        let spec = edit_spec(&fields)?;
        let display = ctx.display(&path);
        let edit = edit_file(ctx, &path, vec![spec], |failure| {
            format!("{display}: {}", failure.message)
        })
        .await?;
        Ok(describe(ctx, &path, &edit, false))
    }
}
