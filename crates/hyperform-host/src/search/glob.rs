//! File-name globbing for the `Glob` tool: standard glob semantics (`*`
//! stays within one path segment, `**` crosses segments), gitignore-aware,
//! newest files first.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use globset::GlobBuilder;

use super::walk::walker;
use crate::HostError;
use crate::fs::resolve;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobResult {
    /// Absolute paths of matching files, most recently modified first.
    pub paths: Vec<PathBuf>,
    /// More files matched than `limit`.
    pub truncated: bool,
}

/// Files under `base` (default `root`; relative bases are taken against
/// `root`) whose path relative to `base` matches `pattern`. An absolute
/// pattern is matched against absolute paths. Blocking: walks the tree.
pub fn glob(
    root: &Path,
    pattern: &str,
    base: Option<&Path>,
    limit: usize,
) -> Result<GlobResult, HostError> {
    let pattern = pattern.trim();
    let pattern = pattern.strip_prefix("./").unwrap_or(pattern);
    if pattern.is_empty() {
        return Err(HostError::Invalid("glob pattern is empty".to_string()));
    }
    let matcher = GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map_err(|e| HostError::Invalid(format!("bad glob `{pattern}`: {e}")))?
        .compile_matcher();
    let absolute = Path::new(pattern).is_absolute();
    let base = base.map_or_else(|| resolve(root, "."), |b| resolve(root, b));
    if !base.is_dir() {
        return Err(HostError::NotFound(format!("directory {}", base.display())));
    }

    let mut found: Vec<(Option<SystemTime>, PathBuf)> = Vec::new();
    for entry in walker(&base).build() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                tracing::debug!(error = %e, "glob walk skipped an entry");
                continue;
            }
        };
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let candidate = if absolute {
            entry.path()
        } else {
            entry.path().strip_prefix(&base).unwrap_or(entry.path())
        };
        if matcher.is_match(candidate) {
            let modified = entry.metadata().ok().and_then(|m| m.modified().ok());
            found.push((modified, entry.into_path()));
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let truncated = found.len() > limit;
    found.truncate(limit);
    Ok(GlobResult {
        paths: found.into_iter().map(|(_, path)| path).collect(),
        truncated,
    })
}
