//! Read-only repository queries: identity, status, diffs, history, and the
//! summary injected into prompts.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use super::cli::{GitCmd, git, git_available};
use super::status::{StatusEntry, parse_porcelain_z};
use crate::HostError;
use crate::fs::{is_within, resolve};

const SUMMARY_STATUS_LINES: usize = 20;
const SUMMARY_COMMITS: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSummary {
    /// `None` on a detached HEAD.
    pub branch: Option<String>,
    /// `None` before the first commit.
    pub head: Option<String>,
    pub dirty_files: usize,
    /// The first 20 `git status --short` lines.
    pub status_short: String,
    /// `git log --oneline` of the latest commits.
    pub recent_commits: Vec<String>,
}

/// Whether `path` is inside a git work tree.
pub async fn is_repo(path: &Path) -> bool {
    match git(&dir_of(path), &["rev-parse", "--is-inside-work-tree"]).await {
        Ok(out) => out.trim() == "true",
        Err(_) => false,
    }
}

/// Top-level directory of the work tree containing `path`.
pub async fn repo_root(path: &Path) -> Option<PathBuf> {
    let out = git(&dir_of(path), &["rev-parse", "--show-toplevel"])
        .await
        .ok()?;
    let top = out.trim_end_matches(['\n', '\r']);
    (!top.is_empty()).then(|| PathBuf::from(top))
}

/// The commit HEAD points at; `None` before the first commit.
pub async fn head_sha(root: &Path) -> Result<Option<String>, HostError> {
    let out = GitCmd::new(root, ["rev-parse", "--verify", "--quiet", "HEAD^{commit}"])
        .output()
        .await?;
    match out.code {
        Some(0) => Ok(Some(out.stdout.trim().to_string())),
        Some(1) if out.stderr.trim().is_empty() => Ok(None),
        _ => Err(out.into_error()),
    }
}

/// The checked-out branch; `None` on a detached HEAD.
pub async fn current_branch(root: &Path) -> Result<Option<String>, HostError> {
    let out = GitCmd::new(root, ["symbolic-ref", "--quiet", "--short", "HEAD"])
        .output()
        .await?;
    match out.code {
        Some(0) => Ok(Some(out.stdout.trim().to_string())),
        Some(1) => Ok(None),
        _ => Err(out.into_error()),
    }
}

/// Every changed, staged, and untracked (individual) file.
pub async fn status_porcelain(root: &Path) -> Result<Vec<StatusEntry>, HostError> {
    let raw = git(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )
    .await?;
    Ok(parse_porcelain_z(&raw))
}

/// Paths from [`status_porcelain`] (new names for renames), sorted.
pub async fn changed_files(root: &Path) -> Result<Vec<String>, HostError> {
    let mut paths: Vec<String> = status_porcelain(root)
        .await?
        .into_iter()
        .map(|entry| entry.path)
        .collect();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

/// `git diff` of the work tree (or the index with `staged`), optionally
/// limited to one path.
pub async fn diff(root: &Path, path: Option<&Path>, staged: bool) -> Result<String, HostError> {
    let mut tail: Vec<&OsStr> = Vec::new();
    if staged {
        tail.push(OsStr::new("--cached"));
    }
    if let Some(path) = path {
        tail.extend([OsStr::new("--"), path.as_os_str()]);
    }
    diff_cmd(root, &tail).run().await
}

/// Work tree versus HEAD for one file; untracked files (and every file
/// before the first commit) are shown as wholly added.
pub async fn diff_head_file(root: &Path, path: &Path) -> Result<String, HostError> {
    let absolute = resolve(root, path);
    if !is_within(root, &absolute) {
        return Err(HostError::Blocked(format!(
            "{} is outside the repository",
            path.display()
        )));
    }
    let rel = absolute.strip_prefix(root).unwrap_or(&absolute);
    let tracked = GitCmd::new(
        root,
        [
            OsStr::new("ls-files"),
            OsStr::new("--error-unmatch"),
            OsStr::new("--"),
            rel.as_os_str(),
        ],
    )
    .output()
    .await?
    .success();
    if tracked && head_sha(root).await?.is_some() {
        let tail = [OsStr::new("HEAD"), OsStr::new("--"), rel.as_os_str()];
        return diff_cmd(root, &tail).run().await;
    }
    if !absolute.is_file() {
        return Ok(String::new());
    }
    let tail = [
        OsStr::new("--no-index"),
        OsStr::new("--"),
        OsStr::new("/dev/null"),
        rel.as_os_str(),
    ];
    let out = diff_cmd(root, &tail).output().await?;
    // `--no-index` exits 1 when the inputs differ.
    match out.code {
        Some(0 | 1) => Ok(out.stdout),
        _ => Err(out.into_error()),
    }
}

/// `git log --oneline` of the last `n` commits; empty before the first.
pub async fn recent_commits(root: &Path, n: usize) -> Result<Vec<String>, HostError> {
    if head_sha(root).await?.is_none() {
        return Ok(Vec::new());
    }
    let count = format!("-n{n}");
    let out = git(
        root,
        &["log", "--oneline", "--no-decorate", "--no-color", &count],
    )
    .await?;
    Ok(out.lines().map(str::to_string).collect())
}

/// Branch, HEAD, dirty files and recent history for prompts. Best effort:
/// `None` outside a repository or when git is unavailable.
pub async fn summary(root: &Path) -> Option<GitSummary> {
    if !git_available() || !is_repo(root).await {
        return None;
    }
    let entries = status_porcelain(root).await.ok()?;
    let status_short = entries
        .iter()
        .take(SUMMARY_STATUS_LINES)
        .map(StatusEntry::short_line)
        .collect::<Vec<_>>()
        .join("\n");
    Some(GitSummary {
        branch: current_branch(root).await.ok().flatten(),
        head: head_sha(root).await.ok().flatten(),
        dirty_files: entries.len(),
        status_short,
        recent_commits: recent_commits(root, SUMMARY_COMMITS)
            .await
            .unwrap_or_default(),
    })
}

fn diff_cmd<'a>(root: &'a Path, tail: &[&OsStr]) -> GitCmd<'a> {
    let head = [
        OsStr::new("diff"),
        OsStr::new("--no-color"),
        OsStr::new("--no-ext-diff"),
    ];
    GitCmd::new(root, head.iter().chain(tail))
}

/// Git needs a directory to run in; for a file use its parent.
fn dir_of(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()
            .map_or_else(|| path.to_path_buf(), Path::to_path_buf)
    }
}
