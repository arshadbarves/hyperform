//! Comparing snapshots and restoring one. Restore first snapshots the
//! present, then touches only the paths that differ: files changed or
//! removed since the target come back from it, files created since are
//! deleted. Identical paths and anything a snapshot could not store
//! (ignored or oversized at either end) are never touched.

use std::path::Path;

use super::shadow::ShadowRepo;
use crate::HostError;

const CHECKOUT_BATCH: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathChange {
    /// Relative to the project root, `/`-separated.
    pub path: String,
    pub kind: ChangeKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RestoreReport {
    /// Paths rewritten from the target snapshot, sorted.
    pub restored: Vec<String>,
    /// Paths created after the target and removed, sorted.
    pub deleted: Vec<String>,
    /// Differing paths left alone because a snapshot could not store them
    /// (too large or ignored when the target or the present was taken).
    pub skipped: Vec<String>,
}

impl ShadowRepo {
    /// Paths that differ from snapshot `from` to snapshot `to`, sorted.
    pub async fn changed_paths(&self, from: &str, to: &str) -> Result<Vec<PathChange>, HostError> {
        let raw = self
            .cmd([
                "diff",
                "--name-status",
                "-z",
                "--no-renames",
                "--no-ext-diff",
                from,
                to,
                "--",
            ])
            .run()
            .await?;
        let mut fields = raw.split('\0').filter(|field| !field.is_empty());
        let mut changes = Vec::new();
        while let (Some(status), Some(path)) = (fields.next(), fields.next()) {
            let kind = match status.chars().next() {
                Some('A') => ChangeKind::Added,
                Some('D') => ChangeKind::Deleted,
                _ => ChangeKind::Modified,
            };
            changes.push(PathChange {
                path: path.to_string(),
                kind,
            });
        }
        changes.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(changes)
    }

    /// Makes the tree match snapshot `target` for every stored path.
    pub async fn restore(&self, target: &str) -> Result<RestoreReport, HostError> {
        let _guard = self.exclusive().await;
        let target = self.resolve_commit(target).await?;
        let excluded_then = self.excluded_at(&target).await?;
        let (current, excluded_now) = self.snapshot_locked("before restore").await?;
        let mut report = RestoreReport::default();
        for PathChange { path, kind } in self.changed_paths(&target, &current).await? {
            // Absent from one side only because it could not be stored:
            // its real state there is unknown, so it is left alone.
            let unknown = match kind {
                ChangeKind::Added => excluded_then.binary_search(&path).is_ok(),
                ChangeKind::Deleted => excluded_now.covers(&path),
                ChangeKind::Modified => false,
            };
            match (unknown, kind) {
                (true, _) => report.skipped.push(path),
                (false, ChangeKind::Added) => report.deleted.push(path),
                (false, _) => report.restored.push(path),
            }
        }
        // Deletions first: a file may be replaced by a directory or back.
        for rel in &report.deleted {
            remove_created(self.work_tree(), rel)?;
        }
        for batch in report.restored.chunks(CHECKOUT_BATCH) {
            let mut args = vec!["checkout", target.as_str(), "--"];
            args.extend(batch.iter().map(String::as_str));
            self.cmd(&args)
                .envs(&[("GIT_LITERAL_PATHSPECS", "1")])
                .run()
                .await?;
        }
        Ok(report)
    }
}

/// Removes a file created after the target, then any directories that the
/// removal left empty (never the work tree itself).
fn remove_created(work_tree: &Path, rel: &str) -> Result<(), HostError> {
    let path = work_tree.join(rel);
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_dir() => return Ok(()),
        Ok(_) => std::fs::remove_file(&path).map_err(|e| HostError::io(&path, e))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(HostError::io(&path, e)),
    }
    let mut dir = path.parent();
    while let Some(current) = dir {
        if current == work_tree || !current.starts_with(work_tree) {
            break;
        }
        // A directory that still holds anything (e.g. ignored files) stays.
        if std::fs::remove_dir(current).is_err() {
            break;
        }
        dir = current.parent();
    }
    Ok(())
}
