//! Read-before-edit enforcement and the write path shared by the editing
//! tools. Callers hold the path's lock from the freshness check through the
//! write, so concurrent edits of one file serialize.

use std::path::Path;

use z_engine_host::{Freshness, atomic_write, read_text};

use crate::context::ToolCtx;
use crate::error::ToolError;

/// Largest file the editing tools load.
const MAX_EDIT_BYTES: usize = 16 * 1024 * 1024;

/// The file's current text, or `None` when it does not exist. Non-UTF-8
/// and oversized files are refused: writing them back would corrupt them.
pub(crate) async fn load_text(ctx: &ToolCtx, path: &Path) -> Result<Option<String>, ToolError> {
    let file = match read_text(path, MAX_EDIT_BYTES).await {
        Ok(file) => file,
        Err(e) if e.is_not_found() => return Ok(None),
        Err(e) => {
            return Err(ToolError::failed(format!(
                "cannot read {}: {e}",
                ctx.display(path)
            )));
        }
    };
    if file.truncated {
        return Err(ToolError::failed(format!(
            "{} is {} bytes, too large to edit here; use Bash for bulk changes",
            ctx.display(path),
            file.len
        )));
    }
    if file.lossy {
        return Err(ToolError::failed(format!(
            "{} is not valid UTF-8 text, so it cannot be edited as text",
            ctx.display(path)
        )));
    }
    Ok(Some(file.content))
}

/// Refuses files the model has not read, or that changed since it did.
pub(crate) fn ensure_fresh(ctx: &ToolCtx, path: &Path) -> Result<(), ToolError> {
    match ctx.files.check_fresh(path) {
        Ok(()) => Ok(()),
        Err(Freshness::NotRead) => Err(ToolError::failed(format!(
            "{} has not been read yet. Read it first, then retry.",
            ctx.display(path)
        ))),
        Err(Freshness::Modified) => Err(ToolError::failed(format!(
            "{} has been modified since it was last read, either by the user or by another process. Read it again, then retry.",
            ctx.display(path)
        ))),
    }
}

/// Atomically replaces `path` and refreshes its read stamp.
pub(crate) async fn save(ctx: &ToolCtx, path: &Path, content: &str) -> Result<(), ToolError> {
    atomic_write(path, content.as_bytes())
        .await
        .map_err(|e| ToolError::failed(format!("could not write {}: {e}", ctx.display(path))))?;
    if let Err(e) = ctx.files.record_write(path) {
        // The write happened; a missing stamp only forces a re-read later.
        tracing::warn!(path = %path.display(), error = %e, "could not record the write");
    }
    Ok(())
}
