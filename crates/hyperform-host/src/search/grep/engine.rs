//! Entry points: pick ripgrep when it is on `PATH`, else the builtin
//! engine, and render either result the same way.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tokio_util::sync::CancellationToken;

use super::format::render;
use super::plan::Plan;
use super::query::{GrepEngine, GrepQuery, GrepResult};
use super::{builtin, ripgrep};
use crate::HostError;
use crate::blocking::run_blocking;

static RIPGREP: OnceLock<Option<PathBuf>> = OnceLock::new();

fn ripgrep_path() -> Option<&'static Path> {
    RIPGREP.get_or_init(|| which::which("rg").ok()).as_deref()
}

/// Whether `rg` was found on `PATH` (checked once per process).
pub fn rg_available() -> bool {
    ripgrep_path().is_some()
}

/// Searches under `root` with ripgrep when available; a ripgrep process
/// failure falls back to the builtin engine.
pub async fn grep(
    root: &Path,
    query: &GrepQuery,
    cancel: CancellationToken,
) -> Result<GrepResult, HostError> {
    if rg_available() {
        match grep_with_engine(root, query, cancel.clone(), GrepEngine::Ripgrep).await {
            Err(HostError::Process(reason)) => {
                tracing::warn!(%reason, "ripgrep failed; using the builtin engine");
            }
            other => return other,
        }
    }
    grep_with_engine(root, query, cancel, GrepEngine::Builtin).await
}

/// [`grep`] with a forced engine (tests compare both engines).
#[doc(hidden)]
pub async fn grep_with_engine(
    root: &Path,
    query: &GrepQuery,
    cancel: CancellationToken,
    engine: GrepEngine,
) -> Result<GrepResult, HostError> {
    let plan = Plan::new(root, query)?;
    if cancel.is_cancelled() {
        return Err(HostError::Cancelled);
    }
    let collected = match engine {
        GrepEngine::Ripgrep => {
            let rg = ripgrep_path()
                .ok_or_else(|| HostError::NotFound("ripgrep (rg) on PATH".to_string()))?;
            ripgrep::search(&plan, rg, &cancel).await?
        }
        GrepEngine::Builtin => run_blocking(move || builtin::search(&plan, &cancel)).await?,
    };
    Ok(render(collected, query, engine))
}
