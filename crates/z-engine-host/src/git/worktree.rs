//! Linked worktrees for isolated agents: create on a fresh branch, commit
//! everything, diff against the base, and remove. Never edits git config.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::cli::{GitCmd, git};
use crate::HostError;
use crate::fs::{atomic_write_sync, is_within};

/// Hooks may run test suites.
const COMMIT_TIMEOUT: Duration = Duration::from_secs(300);
const FALLBACK_NAME: &str = "user.name=Z Engine";
const FALLBACK_EMAIL: &str = "user.email=zengine@localhost";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeHandle {
    pub path: PathBuf,
    pub branch: String,
    /// The commit the branch started from.
    pub base_sha: String,
}

/// Adds a worktree at `path` on the new branch `branch`, starting from
/// `base` (default `HEAD`). A worktree inside the repository is added to
/// `info/exclude` so it never shows up as untracked.
pub async fn create_worktree(
    repo_root: &Path,
    path: &Path,
    branch: &str,
    base: Option<&str>,
) -> Result<WorktreeHandle, HostError> {
    let branch = branch.trim();
    let valid = !branch.is_empty()
        && GitCmd::new(repo_root, ["check-ref-format", "--branch", branch])
            .output()
            .await?
            .success();
    if !valid {
        return Err(HostError::Invalid(format!(
            "invalid branch name `{branch}`"
        )));
    }
    let exists = GitCmd::new(
        repo_root,
        [
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
    .output()
    .await?;
    if exists.success() {
        return Err(HostError::Invalid(format!(
            "branch `{branch}` already exists"
        )));
    }
    let base = base.unwrap_or("HEAD");
    let base_sha = git(
        repo_root,
        &["rev-parse", "--verify", &format!("{base}^{{commit}}")],
    )
    .await?
    .trim()
    .to_string();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| HostError::io(parent, e))?;
    }
    GitCmd::new(
        repo_root,
        [
            OsStr::new("worktree"),
            OsStr::new("add"),
            OsStr::new("--quiet"),
            OsStr::new("-b"),
            OsStr::new(branch),
            path.as_os_str(),
            OsStr::new(&base_sha),
        ],
    )
    .run()
    .await?;
    let path = std::fs::canonicalize(path).map_err(|e| HostError::io(path, e))?;
    exclude_if_nested(repo_root, &path).await?;
    Ok(WorktreeHandle {
        path,
        branch: branch.to_string(),
        base_sha,
    })
}

/// Stages everything and commits (hooks run as usual). `None` when there
/// was nothing to commit. A missing identity falls back to "Z Engine" for
/// this one command only.
pub async fn commit_all(worktree_path: &Path, message: &str) -> Result<Option<String>, HostError> {
    git(worktree_path, &["add", "-A"]).await?;
    let staged = GitCmd::new(worktree_path, ["diff", "--cached", "--quiet"])
        .output()
        .await?;
    match staged.code {
        Some(0) => return Ok(None),
        Some(1) => {}
        _ => return Err(staged.into_error()),
    }
    let mut args: Vec<&str> = Vec::new();
    if !config_set(worktree_path, "user.name").await? {
        args.extend(["-c", FALLBACK_NAME]);
    }
    if !config_set(worktree_path, "user.email").await? {
        args.extend(["-c", FALLBACK_EMAIL]);
    }
    args.extend(["commit", "--quiet", "-m", message]);
    GitCmd::new(worktree_path, &args)
        .timeout(COMMIT_TIMEOUT)
        .run()
        .await?;
    let head = git(worktree_path, &["rev-parse", "HEAD"]).await?;
    Ok(Some(head.trim().to_string()))
}

/// Binary-safe patch from `base` to `head`, applicable with `git apply`.
pub async fn diff_range(repo_root: &Path, base: &str, head: &str) -> Result<String, HostError> {
    git(
        repo_root,
        &[
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--binary",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            base,
            head,
            "--",
        ],
    )
    .await
}

/// Number of changed files and the `git diff --stat` text.
pub async fn diffstat_range(
    repo_root: &Path,
    base: &str,
    head: &str,
) -> Result<(u32, String), HostError> {
    let numstat = git(repo_root, &["diff", "--numstat", base, head, "--"]).await?;
    let files =
        u32::try_from(numstat.lines().filter(|l| !l.trim().is_empty()).count()).unwrap_or(u32::MAX);
    let stat = git(
        repo_root,
        &["diff", "--no-color", "--stat", base, head, "--"],
    )
    .await?;
    Ok((files, stat))
}

/// Removes the worktree (discarding its uncommitted changes) and, when
/// asked, deletes its branch.
pub async fn remove_worktree(
    repo_root: &Path,
    path: &Path,
    delete_branch: bool,
) -> Result<(), HostError> {
    let branch = if delete_branch {
        worktree_branch(repo_root, path).await?
    } else {
        None
    };
    GitCmd::new(
        repo_root,
        [
            OsStr::new("worktree"),
            OsStr::new("remove"),
            OsStr::new("--force"),
            path.as_os_str(),
        ],
    )
    .run()
    .await?;
    if let Some(branch) = branch {
        git(repo_root, &["branch", "-D", &branch]).await?;
    }
    Ok(())
}

/// Forgets worktrees whose directories are gone.
pub async fn prune_worktrees(repo_root: &Path) -> Result<(), HostError> {
    git(repo_root, &["worktree", "prune"]).await.map(|_| ())
}

async fn config_set(cwd: &Path, key: &str) -> Result<bool, HostError> {
    let out = GitCmd::new(cwd, ["config", "--get", key]).output().await?;
    match out.code {
        Some(0) => Ok(!out.stdout.trim().is_empty()),
        Some(1) => Ok(false),
        _ => Err(out.into_error()),
    }
}

async fn worktree_branch(repo_root: &Path, path: &Path) -> Result<Option<String>, HostError> {
    let listing = git(repo_root, &["worktree", "list", "--porcelain"]).await?;
    let wanted = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut current: Option<PathBuf> = None;
    for line in listing.lines() {
        if let Some(dir) = line.strip_prefix("worktree ") {
            let dir = PathBuf::from(dir);
            current = Some(std::fs::canonicalize(&dir).unwrap_or(dir));
        } else if let Some(reference) = line.strip_prefix("branch ") {
            if current.as_deref() == Some(wanted.as_path()) {
                let name = reference.strip_prefix("refs/heads/").unwrap_or(reference);
                return Ok(Some(name.to_string()));
            }
        }
    }
    Ok(None)
}

async fn exclude_if_nested(repo_root: &Path, worktree: &Path) -> Result<(), HostError> {
    if !is_within(repo_root, worktree) {
        return Ok(());
    }
    let root = std::fs::canonicalize(repo_root).map_err(|e| HostError::io(repo_root, e))?;
    let Ok(rel) = worktree.strip_prefix(&root) else {
        return Ok(());
    };
    let common = git(repo_root, &["rev-parse", "--git-common-dir"]).await?;
    let common = PathBuf::from(common.trim());
    let common = if common.is_absolute() {
        common
    } else {
        repo_root.join(common)
    };
    let exclude = common.join("info").join("exclude");
    let line = format!("/{}/", rel.to_string_lossy().replace('\\', "/"));
    let existing = match std::fs::read_to_string(&exclude) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(HostError::io(&exclude, e)),
    };
    if existing.lines().any(|l| l.trim() == line) {
        return Ok(());
    }
    let separator = if existing.is_empty() || existing.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    atomic_write_sync(
        &exclude,
        format!("{existing}{separator}{line}\n").as_bytes(),
    )
}
