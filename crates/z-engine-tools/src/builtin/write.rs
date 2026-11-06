//! `Write`: creates a file or replaces one the model has read in full.
//! Existing files must be fresh; the write is atomic and serialized with
//! other edits of the same path.

use std::path::Path;

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_host::read_text;
use z_engine_policy::Action;
use z_engine_prompts::tools as prompts;
use z_engine_protocol::Preview;

use crate::context::ToolCtx;
use crate::edit::{ensure_fresh, save};
use crate::error::ToolError;
use crate::input::{Fields, path_field, str_field};
use crate::names;
use crate::output::ToolOutput;
use crate::schema;
use crate::text::{cap_preview, creation_diff, diff_stats, unified_diff};
use crate::tool::Tool;

/// Previous content loaded for the diff; beyond it the diff is partial.
const MAX_DIFF_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Default)]
pub struct WriteTool;

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        names::WRITE
    }

    fn description(&self) -> String {
        prompts::WRITE.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({
                "file_path": {"type": "string", "description": "The file to write: an absolute path, or a path relative to the project root."},
                "content": {"type": "string", "description": "The complete content of the file."}
            }),
            &["file_path", "content"],
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
            Some(path) => format!("Write {}", ctx.display(&path)),
            None => "Write".to_string(),
        }
    }

    async fn preview(&self, input: &Value, ctx: &ToolCtx) -> Option<Preview> {
        let path = path_field(input, "file_path", ctx)?;
        let content = str_field(input, "content")?;
        let display = ctx.display(&path);
        let diff = match previous(ctx, &path).await.ok()? {
            Some(old) => unified_diff(&old, content, &display),
            None => creation_diff(content, &display),
        };
        Some(Preview::Diff {
            path: display,
            diff: cap_preview(diff),
        })
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let fields = Fields::new(&input)?;
        let path = ctx.resolve(fields.non_empty("file_path")?);
        let content = fields.required_str("content")?;
        let display = ctx.display(&path);
        let _guard = ctx.locks.lock(&path).await;
        let before = previous(ctx, &path).await?;
        if before.is_some() {
            ensure_fresh(ctx, &path)?;
        }
        save(ctx, &path, content).await?;
        let lines = content.lines().count();
        let output = match before {
            None => ToolOutput::text(
                format!("Created {display} ({lines} lines)."),
                format!("Created {display}"),
            ),
            Some(old) => ToolOutput::text(
                format!(
                    "Wrote {lines} lines to {display}, replacing its previous content ({} lines changed).",
                    diff_stats(&old, content).label()
                ),
                format!("Wrote {lines} lines to {display}"),
            ),
        };
        Ok(output.wrote(path))
    }
}

/// The current content (lossy for non-UTF-8 files), `None` when absent.
async fn previous(ctx: &ToolCtx, path: &Path) -> Result<Option<String>, ToolError> {
    match read_text(path, MAX_DIFF_BYTES).await {
        Ok(file) => Ok(Some(file.content)),
        Err(e) if e.is_not_found() => Ok(None),
        Err(e) => Err(ToolError::failed(format!(
            "cannot write {}: {e}",
            ctx.display(path)
        ))),
    }
}
