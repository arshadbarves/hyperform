//! Applying an agent's patch to the user's tree. A clean `git apply` is
//! tried first; otherwise a 3-way merge runs against a throwaway index that
//! mirrors the working tree, so uncommitted user changes merge too and the
//! real index is never touched. When conflicts remain, every file the patch
//! touches is put back byte for byte, so a failed apply changes nothing.

use std::path::{Path, PathBuf};

use super::cli::{GitCmd, git};
use crate::HostError;
use crate::fs::atomic_write_sync;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyOutcome {
    pub applied: bool,
    /// Paths that could not be merged (empty when applied).
    pub conflicts: Vec<String>,
    pub message: String,
}

pub async fn apply_patch(repo_root: &Path, patch: &str) -> Result<ApplyOutcome, HostError> {
    if patch.trim().is_empty() {
        return Ok(outcome(true, Vec::new(), "empty patch; nothing to apply"));
    }
    let direct = GitCmd::new(repo_root, ["apply", "--whitespace=nowarn", "-"])
        .stdin(patch.as_bytes())
        .output()
        .await?;
    if direct.success() {
        return Ok(outcome(true, Vec::new(), "applied cleanly"));
    }
    let touched = match touched_paths(repo_root, patch).await {
        Ok(paths) => paths,
        // Not a parseable patch: nothing was changed.
        Err(HostError::Git { .. }) => {
            let conflicts = conflicted_paths(&direct.stderr);
            return Ok(outcome(false, conflicts, direct.stderr.trim()));
        }
        Err(e) => return Err(e),
    };
    let originals = capture(repo_root, &touched)?;
    let index = ScratchIndex::create(repo_root).await?;
    let mirror = [("GIT_INDEX_FILE", index.path.clone())];
    let mut sync_args = vec!["update-index", "--add", "--remove", "--"];
    sync_args.extend(touched.iter().map(String::as_str));
    GitCmd::new(repo_root, &sync_args)
        .envs(&mirror)
        .run()
        .await?;
    let merged = GitCmd::new(repo_root, ["apply", "--3way", "--whitespace=nowarn", "-"])
        .envs(&mirror)
        .stdin(patch.as_bytes())
        .output()
        .await?;
    if merged.success() {
        return Ok(outcome(true, Vec::new(), "applied with a 3-way merge"));
    }
    for (path, bytes) in &originals {
        restore(path, bytes.as_deref())?;
    }
    let mut conflicts = conflicted_paths(&merged.stderr);
    if conflicts.is_empty() {
        conflicts = conflicted_paths(&direct.stderr);
    }
    let message = [direct.stderr.trim(), merged.stderr.trim()]
        .into_iter()
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    Ok(outcome(false, conflicts, &message))
}

fn outcome(applied: bool, conflicts: Vec<String>, message: &str) -> ApplyOutcome {
    ApplyOutcome {
        applied,
        conflicts,
        message: message.to_string(),
    }
}

/// Paths a patch mentions, from `git apply --numstat -z` (both sides of a
/// rename).
async fn touched_paths(repo_root: &Path, patch: &str) -> Result<Vec<String>, HostError> {
    let listing = GitCmd::new(repo_root, ["apply", "--numstat", "-z", "-"])
        .stdin(patch.as_bytes())
        .run()
        .await?;
    let mut fields = listing.split('\0');
    let mut paths = Vec::new();
    while let Some(record) = fields.next() {
        let mut columns = record.splitn(3, '\t');
        let (Some(_), Some(_), Some(path)) = (columns.next(), columns.next(), columns.next())
        else {
            continue;
        };
        if path.is_empty() {
            paths.extend(fields.next().map(str::to_string));
            paths.extend(fields.next().map(str::to_string));
        } else {
            paths.push(path.to_string());
        }
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

/// `U path` lines of a 3-way apply, else `error: path: reason` lines.
fn conflicted_paths(stderr: &str) -> Vec<String> {
    let unmerged: Vec<String> = stderr
        .lines()
        .filter_map(|line| line.strip_prefix("U "))
        .map(|path| path.trim().to_string())
        .collect();
    if !unmerged.is_empty() {
        return unmerged;
    }
    let known = [
        "patch does not apply",
        "already exists",
        "does not exist",
        "does not match index",
    ];
    let mut failed: Vec<String> = stderr
        .lines()
        .filter_map(|line| line.strip_prefix("error: "))
        .filter_map(|rest| {
            let (path, reason) = rest.split_once(": ")?;
            known
                .iter()
                .any(|k| reason.contains(k))
                .then(|| path.trim().to_string())
        })
        .collect();
    failed.sort();
    failed.dedup();
    failed
}

/// A copy of the index that the 3-way merge may freely rewrite.
struct ScratchIndex {
    path: PathBuf,
}

impl ScratchIndex {
    async fn create(repo_root: &Path) -> Result<ScratchIndex, HostError> {
        let real = git(repo_root, &["rev-parse", "--git-path", "index"]).await?;
        let real = repo_root.join(real.trim());
        let path = std::env::temp_dir().join(format!("zengine-apply-{}.index", ulid::Ulid::new()));
        match std::fs::copy(&real, &path) {
            Ok(_) => Ok(ScratchIndex { path }),
            // No index yet: git starts the scratch index empty.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ScratchIndex { path }),
            Err(e) => Err(HostError::io(&real, e)),
        }
    }
}

impl Drop for ScratchIndex {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_file(&self.path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(path = %self.path.display(), error = %e, "could not remove scratch index");
            }
        }
    }
}

/// A touched file and its bytes before the apply (`None`: it did not exist).
type Original = (PathBuf, Option<Vec<u8>>);

fn capture(repo_root: &Path, touched: &[String]) -> Result<Vec<Original>, HostError> {
    touched
        .iter()
        .map(|rel| {
            let path = repo_root.join(rel);
            let bytes = match std::fs::read(&path) {
                Ok(bytes) => Some(bytes),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(HostError::io(&path, e)),
            };
            Ok((path, bytes))
        })
        .collect()
}

fn restore(path: &Path, bytes: Option<&[u8]>) -> Result<(), HostError> {
    match bytes {
        Some(bytes) => atomic_write_sync(path, bytes),
        None => match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(HostError::io(path, e)),
        },
    }
}
