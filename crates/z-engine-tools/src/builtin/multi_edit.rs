//! `MultiEdit`: several replacements in one file, applied in order in
//! memory and written once. All-or-nothing: one failing edit applies none.

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_policy::Action;
use z_engine_prompts::tools as prompts;
use z_engine_protocol::Preview;

use super::edit::edit_spec;
use crate::context::ToolCtx;
use crate::edit::{EditSpec, describe, edit_file, preview_edits};
use crate::error::ToolError;
use crate::input::{Fields, path_field};
use crate::names;
use crate::output::ToolOutput;
use crate::schema;
use crate::tool::Tool;

#[derive(Debug, Default)]
pub struct MultiEditTool;

fn specs(input: &Value) -> Result<Vec<EditSpec>, ToolError> {
    let fields = Fields::new(input)?;
    let Some(Value::Array(edits)) = fields.value("edits") else {
        return Err(ToolError::invalid("`edits` must be an array of edits"));
    };
    if edits.is_empty() {
        return Err(ToolError::invalid("`edits` must contain at least one edit"));
    }
    edits
        .iter()
        .enumerate()
        .map(|(index, edit)| {
            Fields::new(edit)
                .and_then(|fields| edit_spec(&fields))
                .map_err(|e| match e {
                    ToolError::InvalidInput(reason) => {
                        ToolError::invalid(format!("edit {}: {reason}", index + 1))
                    }
                    other => other,
                })
        })
        .collect()
}

#[async_trait]
impl Tool for MultiEditTool {
    fn name(&self) -> &str {
        names::MULTI_EDIT
    }

    fn description(&self) -> String {
        prompts::MULTI_EDIT.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({
                "file_path": {"type": "string", "description": "The file to modify: an absolute path, or a path relative to the project root."},
                "edits": {
                    "type": "array",
                    "minItems": 1,
                    "description": "Replacements applied in order, each to the result of the previous one.",
                    "items": schema::object(
                        json!({
                            "old_string": {"type": "string", "description": "The exact text to replace."},
                            "new_string": {"type": "string", "description": "The replacement text."},
                            "replace_all": {"type": "boolean", "default": false, "description": "Replace every occurrence (default false)."}
                        }),
                        &["old_string", "new_string"],
                    )
                }
            }),
            &["file_path", "edits"],
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
        let count = input
            .get("edits")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        match path_field(input, "file_path", ctx) {
            Some(path) => format!("Edit {} ({count} edits)", ctx.display(&path)),
            None => "MultiEdit".to_string(),
        }
    }

    async fn preview(&self, input: &Value, ctx: &ToolCtx) -> Option<Preview> {
        let path = path_field(input, "file_path", ctx)?;
        preview_edits(ctx, &path, specs(input).ok()?).await
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let path = ctx.resolve(Fields::new(&input)?.non_empty("file_path")?);
        let edits = specs(&input)?;
        let total = edits.len();
        let display = ctx.display(&path);
        let edit = edit_file(ctx, &path, edits, |failure| {
            format!(
                "Edit {} of {total} failed, so no edits were applied to {display}: {}",
                failure.index + 1,
                failure.message
            )
        })
        .await?;
        Ok(describe(ctx, &path, &edit, true))
    }
}
