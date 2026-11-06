//! Git: the CLI runner, repository queries, worktrees, and patch
//! application.

mod cli;
mod patch;
mod repo;
mod status;
mod worktree;

pub use cli::{git, git_available, git_with_env};
pub use patch::{ApplyOutcome, apply_patch};
pub use repo::{
    GitSummary, changed_files, current_branch, diff, diff_head_file, head_sha, is_repo,
    recent_commits, repo_root, status_porcelain, summary,
};
pub use status::StatusEntry;
pub use worktree::{
    WorktreeHandle, commit_all, create_worktree, diff_range, diffstat_range, prune_worktrees,
    remove_worktree,
};

pub(crate) use cli::GitCmd;
pub(crate) use status::parse_porcelain_z;
