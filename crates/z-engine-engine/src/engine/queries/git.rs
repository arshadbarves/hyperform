//! Git-scope review panel, the sidebar's project summary and the worktree
//! panel: working-tree changes versus `HEAD` with line counts, one file's
//! diff, the branch with a changed-file count, and a linked worktree for a
//! new chat. Paths are relative to the repository root.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use z_engine_host::{
    create_worktree, current_branch, diff_head_file, git, repo_root, status_porcelain,
};

use crate::engine::Engine;
use crate::error::EngineError;

/// Untracked files larger than this report no line count.
const MAX_COUNTED_BYTES: u64 = 2 * 1024 * 1024;
const WORKTREES_DIR: &str = ".z-engine/worktrees";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitChangedFile {
    pub path: String,
    /// `added`, `modified` or `deleted`.
    pub status: String,
    pub added: u32,
    pub deleted: u32,
}

/// What the sidebar shows beside a project's name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoSummary {
    /// Top-level directory: git-scope paths are relative to it.
    pub root: String,
    /// `None` on a detached HEAD.
    pub branch: Option<String>,
    /// Changed, staged and untracked files.
    pub changed: u32,
}

impl Engine {
    /// Branch and changed-file count of the repository containing
    /// `project_root`; `None` when it is not inside a git repository.
    pub async fn git_summary(
        &self,
        project_root: &Path,
    ) -> Result<Option<RepoSummary>, EngineError> {
        let Some(repo) = repo_root(project_root).await else {
            return Ok(None);
        };
        let branch = current_branch(&repo).await?;
        let changed = u32::try_from(status_porcelain(&repo).await?.len()).unwrap_or(u32::MAX);
        Ok(Some(RepoSummary {
            root: repo.to_string_lossy().into_owned(),
            branch,
            changed,
        }))
    }

    /// Changed, staged and untracked files of the repository containing
    /// `project_root`.
    pub async fn git_changed_files(
        &self,
        project_root: &Path,
    ) -> Result<Vec<GitChangedFile>, EngineError> {
        let repo = repository(project_root).await?;
        let numstat = numstat(&repo).await;
        let mut files = Vec::new();
        for entry in status_porcelain(&repo).await? {
            let letter = if entry.index != ' ' {
                entry.index
            } else {
                entry.worktree
            };
            let status = match letter {
                '?' | 'A' => "added",
                'D' => "deleted",
                _ => "modified",
            };
            let (added, deleted) = match numstat.get(&entry.path) {
                Some(counts) => *counts,
                None if status == "added" => (count_lines(&repo.join(&entry.path)), 0),
                None => (0, 0),
            };
            files.push(GitChangedFile {
                path: entry.path,
                status: status.to_string(),
                added,
                deleted,
            });
        }
        Ok(files)
    }

    /// Unified diff of one repository-relative file versus `HEAD`
    /// (untracked files as wholly added).
    pub async fn git_file_diff(
        &self,
        project_root: &Path,
        path: &str,
    ) -> Result<String, EngineError> {
        if Path::new(path).is_absolute() {
            return Err(EngineError::Invalid(
                "the path must be relative to the repository".to_string(),
            ));
        }
        let repo = repository(project_root).await?;
        Ok(diff_head_file(&repo, Path::new(path)).await?)
    }

    /// Adds `.z-engine/worktrees/<slug>` on the new branch `zengine/<slug>`
    /// and returns its canonical path.
    pub async fn create_worktree(
        &self,
        project_root: &Path,
        name: &str,
    ) -> Result<PathBuf, EngineError> {
        let slug = slug(name).ok_or_else(|| {
            EngineError::Invalid("the worktree name needs letters or digits".to_string())
        })?;
        let repo = repository(project_root).await?;
        let path = project_root.join(WORKTREES_DIR).join(&slug);
        let branch = format!("zengine/{slug}");
        Ok(create_worktree(&repo, &path, &branch, None).await?.path)
    }
}

async fn repository(project_root: &Path) -> Result<PathBuf, EngineError> {
    repo_root(project_root).await.ok_or_else(|| {
        EngineError::Invalid(format!(
            "{} is not inside a git repository",
            project_root.display()
        ))
    })
}

/// Lowercase letters, digits and `-`; `None` when nothing is left.
fn slug(name: &str) -> Option<String> {
    let slug: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_whitespace() { '-' } else { c })
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    let slug = slug.trim_matches('-').to_string();
    slug.chars()
        .any(|c| c.is_ascii_alphanumeric())
        .then_some(slug)
}

/// Line counts versus `HEAD`; empty before the first commit.
async fn numstat(repo: &Path) -> HashMap<String, (u32, u32)> {
    let args = ["diff", "HEAD", "--numstat", "--no-renames", "-z"];
    let Ok(raw) = git(repo, &args).await else {
        return HashMap::new();
    };
    raw.split('\0')
        .filter_map(|record| {
            let mut columns = record.trim_start_matches('\n').splitn(3, '\t');
            let added = columns.next()?.parse().unwrap_or(0);
            let deleted = columns.next()?.parse().unwrap_or(0);
            Some((columns.next()?.to_string(), (added, deleted)))
        })
        .collect()
}

fn count_lines(path: &Path) -> u32 {
    match std::fs::metadata(path) {
        Ok(meta) if meta.is_file() && meta.len() <= MAX_COUNTED_BYTES => std::fs::read(path)
            .map(|bytes| String::from_utf8_lossy(&bytes).lines().count() as u32)
            .unwrap_or(0),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use z_engine_host::git_available;

    use super::super::testing::{engine, git};
    use super::*;

    #[test]
    fn slugs_keep_letters_digits_and_dashes() {
        assert_eq!(slug("Fix Login Bug!").as_deref(), Some("fix-login-bug"));
        assert_eq!(slug("  --a1--  ").as_deref(), Some("a1"));
        assert_eq!(slug("../.."), None);
        assert_eq!(slug("---"), None);
    }

    #[tokio::test]
    async fn changed_files_diffs_and_worktrees() {
        if !git_available() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let root = dir.path().join("repo");
        std::fs::create_dir(&root).unwrap();
        git(&root, &["init", "--quiet"]);
        std::fs::write(root.join("a.txt"), "one\n").unwrap();
        std::fs::write(root.join("gone.txt"), "x\ny\n").unwrap();
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "--quiet", "-m", "init"]);
        std::fs::write(root.join("a.txt"), "two\nthree\n").unwrap();
        std::fs::remove_file(root.join("gone.txt")).unwrap();
        std::fs::write(root.join("new.txt"), "a\nb\nc\n").unwrap();

        let mut files = engine.git_changed_files(&root).await.unwrap();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let summary: Vec<(&str, &str, u32, u32)> = files
            .iter()
            .map(|f| (f.path.as_str(), f.status.as_str(), f.added, f.deleted))
            .collect();
        assert_eq!(
            summary,
            [
                ("a.txt", "modified", 2, 1),
                ("gone.txt", "deleted", 0, 2),
                ("new.txt", "added", 3, 0),
            ]
        );
        let diff = engine.git_file_diff(&root, "a.txt").await.unwrap();
        assert!(diff.contains("-one") && diff.contains("+three"), "{diff}");
        assert!(engine.git_file_diff(&root, "/etc/hosts").await.is_err());

        let worktree = engine.create_worktree(&root, "My Task").await.unwrap();
        assert!(worktree.ends_with(".z-engine/worktrees/my-task"));
        assert!(worktree.join("a.txt").is_file());
        assert!(engine.create_worktree(&root, "My Task").await.is_err());
        assert!(engine.create_worktree(&root, "!!").await.is_err());
        let outside = dir.path().join("plain");
        std::fs::create_dir(&outside).unwrap();
        assert!(engine.git_changed_files(&outside).await.is_err());
    }

    #[tokio::test]
    async fn summary_names_the_branch_and_counts_changed_files() {
        if !git_available() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let root = dir.path().join("repo");
        std::fs::create_dir(&root).unwrap();
        git(&root, &["init", "--quiet"]);
        git(&root, &["checkout", "--quiet", "-b", "feature"]);
        std::fs::write(root.join("a.txt"), "one\n").unwrap();
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "--quiet", "-m", "init"]);
        let clean = engine.git_summary(&root).await.unwrap().unwrap();
        assert_eq!(clean.branch.as_deref(), Some("feature"));
        assert_eq!(clean.changed, 0);
        assert_eq!(
            std::fs::canonicalize(&clean.root).unwrap(),
            std::fs::canonicalize(&root).unwrap()
        );

        std::fs::write(root.join("a.txt"), "two\n").unwrap();
        std::fs::write(root.join("b.txt"), "new\n").unwrap();
        let dirty = engine.git_summary(&root.join("a.txt")).await.unwrap();
        assert_eq!(dirty.map(|s| s.changed), Some(2));

        let plain = dir.path().join("plain");
        std::fs::create_dir(&plain).unwrap();
        assert_eq!(engine.git_summary(&plain).await.unwrap(), None);
    }
}
