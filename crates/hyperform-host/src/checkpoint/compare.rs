//! Comparing a snapshot with the working tree as it is now, without taking
//! a new snapshot: the review panel's "changes since this session began".
//! Files created after the latest snapshot are not in the shadow index, so
//! they are found as untracked (honouring the shadow's excludes).

use std::collections::BTreeMap;
use std::path::{Component, Path};

use super::restore::{ChangeKind, PathChange};
use super::shadow::ShadowRepo;
use crate::HostError;

const LITERAL: &[(&str, &str)] = &[("GIT_LITERAL_PATHSPECS", "1")];

impl ShadowRepo {
    /// Paths that differ between snapshot `from` and the working tree,
    /// sorted.
    pub async fn diff_worktree(&self, from: &str) -> Result<Vec<PathChange>, HostError> {
        let from = self.resolve_commit(from).await?;
        let raw = self
            .cmd([
                "diff",
                "--name-status",
                "-z",
                "--no-renames",
                "--no-ext-diff",
                from.as_str(),
                "--",
            ])
            .run()
            .await?;
        let mut changes = BTreeMap::new();
        let mut fields = raw.split('\0').filter(|field| !field.is_empty());
        while let (Some(status), Some(path)) = (fields.next(), fields.next()) {
            let kind = match status.chars().next() {
                Some('A') => ChangeKind::Added,
                Some('D') => ChangeKind::Deleted,
                _ => ChangeKind::Modified,
            };
            changes.insert(path.to_string(), kind);
        }
        for path in self.untracked().await? {
            // Removed from the index by a later snapshot, then re-created.
            let kind = match changes.get(&path) {
                Some(ChangeKind::Deleted) => ChangeKind::Modified,
                _ if self.in_commit(&from, &path).await? => ChangeKind::Modified,
                _ => ChangeKind::Added,
            };
            changes.insert(path, kind);
        }
        Ok(changes
            .into_iter()
            .map(|(path, kind)| PathChange { path, kind })
            .collect())
    }

    /// Unified diff of one project-relative path from snapshot `from` to
    /// the working tree; empty when it is unchanged.
    pub async fn diff_worktree_file(&self, from: &str, path: &str) -> Result<String, HostError> {
        check_relative(path)?;
        let from = self.resolve_commit(from).await?;
        let indexed = self
            .cmd(["ls-files", "--error-unmatch", "--", path])
            .envs(LITERAL)
            .output()
            .await?
            .success();
        if indexed || self.in_commit(&from, path).await? {
            return self
                .cmd([
                    "diff",
                    "--no-color",
                    "--no-ext-diff",
                    from.as_str(),
                    "--",
                    path,
                ])
                .envs(LITERAL)
                .run()
                .await;
        }
        if !self.work_tree().join(path).is_file() {
            return Ok(String::new());
        }
        let out = self
            .cmd([
                "diff",
                "--no-color",
                "--no-ext-diff",
                "--no-index",
                "--",
                "/dev/null",
                path,
            ])
            .output()
            .await?;
        // `--no-index` exits 1 when the inputs differ.
        match out.code {
            Some(0 | 1) => Ok(out.stdout),
            _ => Err(out.into_error()),
        }
    }

    async fn untracked(&self) -> Result<Vec<String>, HostError> {
        let raw = self
            .cmd(["ls-files", "--others", "--exclude-standard", "-z"])
            .run()
            .await?;
        Ok(raw
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(str::to_string)
            .collect())
    }

    async fn in_commit(&self, commit: &str, path: &str) -> Result<bool, HostError> {
        let spec = format!("{commit}:{path}");
        Ok(self
            .cmd(["cat-file", "-e", spec.as_str()])
            .output()
            .await?
            .success())
    }
}

fn check_relative(path: &str) -> Result<(), HostError> {
    let relative = Path::new(path);
    let plain = !path.is_empty()
        && relative
            .components()
            .all(|part| matches!(part, Component::Normal(_)));
    if plain {
        Ok(())
    } else {
        Err(HostError::Blocked(format!(
            "{path} is not a path inside the project"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git_available;

    async fn fixture() -> (tempfile::TempDir, tempfile::TempDir, ShadowRepo, String) {
        let project = tempfile::tempdir().unwrap();
        let store = tempfile::tempdir().unwrap();
        std::fs::write(project.path().join("keep.txt"), "same\n").unwrap();
        std::fs::write(project.path().join("edit.txt"), "one\n").unwrap();
        std::fs::write(project.path().join("gone.txt"), "bye\n").unwrap();
        let repo = ShadowRepo::open(store.path(), project.path())
            .await
            .unwrap();
        let first = repo.snapshot("first").await.unwrap();
        (project, store, repo, first)
    }

    #[tokio::test]
    async fn worktree_changes_include_files_created_after_the_last_snapshot() {
        if !git_available() {
            return;
        }
        let (project, _store, repo, first) = fixture().await;
        let root = project.path();
        std::fs::write(root.join("edit.txt"), "two\n").unwrap();
        std::fs::remove_file(root.join("gone.txt")).unwrap();
        std::fs::write(root.join("snap.txt"), "in a later snapshot\n").unwrap();
        repo.snapshot("second").await.unwrap();
        std::fs::create_dir(root.join("dir")).unwrap();
        std::fs::write(root.join("dir/new.txt"), "fresh\n").unwrap();
        let changes = repo.diff_worktree(&first).await.unwrap();
        let change = |path: &str, kind| PathChange {
            path: path.to_string(),
            kind,
        };
        assert_eq!(
            changes,
            vec![
                change("dir/new.txt", ChangeKind::Added),
                change("edit.txt", ChangeKind::Modified),
                change("gone.txt", ChangeKind::Deleted),
                change("snap.txt", ChangeKind::Added),
            ]
        );
    }

    #[tokio::test]
    async fn file_diffs_cover_tracked_new_and_unchanged_files() {
        if !git_available() {
            return;
        }
        let (project, _store, repo, first) = fixture().await;
        std::fs::write(project.path().join("edit.txt"), "two\n").unwrap();
        std::fs::write(project.path().join("new.txt"), "fresh\n").unwrap();
        let edited = repo.diff_worktree_file(&first, "edit.txt").await.unwrap();
        assert!(
            edited.contains("-one") && edited.contains("+two"),
            "{edited}"
        );
        let created = repo.diff_worktree_file(&first, "new.txt").await.unwrap();
        assert!(created.contains("+fresh"), "{created}");
        assert_eq!(
            repo.diff_worktree_file(&first, "keep.txt").await.unwrap(),
            ""
        );
        assert_eq!(
            repo.diff_worktree_file(&first, "nope.txt").await.unwrap(),
            ""
        );
        for escape in ["../x", "/etc/passwd", ""] {
            assert!(matches!(
                repo.diff_worktree_file(&first, escape).await,
                Err(HostError::Blocked(_))
            ));
        }
        assert!(matches!(
            repo.diff_worktree(&"0".repeat(40)).await,
            Err(HostError::NotFound(_))
        ));
    }
}
