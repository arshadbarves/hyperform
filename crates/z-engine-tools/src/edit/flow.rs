//! The locked read-check-apply-write sequence of `Edit` and `MultiEdit`,
//! and the same computation without writing for approval previews.

use std::path::Path;

use z_engine_protocol::Preview;

use super::access::{ensure_fresh, load_text, save};
use super::engine::{Applied, EditFailure, EditSpec, apply_edits};
use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::text::{cap_preview, creation_diff, unified_diff};

/// A completed edit: the content before (`None` for a new file) and after.
#[derive(Debug)]
pub(crate) struct FileEdit {
    pub(crate) before: Option<String>,
    pub(crate) applied: Applied,
}

/// Applies `edits` to `path` under its lock and writes the result. Existing
/// files must have been read and be unchanged since.
pub(crate) async fn edit_file(
    ctx: &ToolCtx,
    path: &Path,
    edits: Vec<EditSpec>,
    describe: impl Fn(EditFailure) -> String,
) -> Result<FileEdit, ToolError> {
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ipynb"))
    {
        return Err(ToolError::invalid(format!(
            "{} is a Jupyter notebook; edit its cells with NotebookEdit",
            ctx.display(path)
        )));
    }
    let _guard = ctx.locks.lock(path).await;
    let before = load_text(ctx, path).await?;
    if before.is_some() {
        ensure_fresh(ctx, path)?;
    }
    let applied = compute(before.clone(), edits)
        .await?
        .map_err(|failure| ToolError::failed(describe(failure)))?;
    ctx.check_cancelled()?;
    save(ctx, path, &applied.content).await?;
    Ok(FileEdit { before, applied })
}

/// The approval diff for `edits`, or `None` when they would not apply.
pub(crate) async fn preview_edits(
    ctx: &ToolCtx,
    path: &Path,
    edits: Vec<EditSpec>,
) -> Option<Preview> {
    let before = load_text(ctx, path).await.ok()?;
    let applied = compute(before.clone(), edits).await.ok()?.ok()?;
    let display = ctx.display(path);
    let diff = match &before {
        Some(old) => unified_diff(old, &applied.content, &display),
        None => creation_diff(&applied.content, &display),
    };
    Some(Preview::Diff {
        path: display,
        diff: cap_preview(diff),
    })
}

/// Runs the matching off the async executor: the fuzzy rung is CPU-bound.
async fn compute(
    before: Option<String>,
    edits: Vec<EditSpec>,
) -> Result<Result<Applied, EditFailure>, ToolError> {
    tokio::task::spawn_blocking(move || apply_edits(before.as_deref(), &edits))
        .await
        .map_err(|e| ToolError::failed(format!("the edit computation failed: {e}")))
}
