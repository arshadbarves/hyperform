//! The only OS and network adapter: files, processes, background shells,
//! the command sandbox, search, git, checkpoints, fetch. Every other v2 crate reaches the machine
//! through these small, typed APIs.

mod blocking;
mod digest;

pub mod checkpoint;
pub mod download;
pub mod error;
pub mod fingerprint;
pub mod fs;
pub mod git;
pub mod media;
pub mod net;
pub mod process;
pub mod sandbox;
pub mod search;
pub mod web;

pub use checkpoint::{ChangeKind, PathChange, RestoreReport, ShadowRepo, project_key};
pub use download::{
    DownloadSpec, Downloader, downloaded_bytes, file_matches, part_path, remove_download_dir,
    sha256_file,
};
pub use error::HostError;
pub use fingerprint::workspace_fingerprint;
pub use fs::{
    FileKind, FileTracker, Freshness, PathLocks, TextFile, atomic_write, atomic_write_sync,
    expand_tilde, is_within, normalize, read_text, relative_display, resolve, sniff,
};
pub use git::{
    ApplyOutcome, GitSummary, StatusEntry, WorktreeHandle, apply_patch, changed_files, commit_all,
    create_worktree, current_branch, diff, diff_head_file, diff_range, diffstat_range, git,
    git_available, git_with_env, head_sha, is_repo, prune_worktrees, recent_commits,
    remove_worktree, repo_root, status_porcelain, summary,
};
pub use media::{DEFAULT_MAX_IMAGE_BYTES, pdf_text, read_image, read_pdf_base64};
pub use net::free_loopback_port;
pub use process::{
    BackgroundShells, BackgroundSpec, DEFAULT_MAX_OUTPUT_BYTES, DEFAULT_RUN_TIMEOUT, EnvPolicy,
    JobEvent, JobEventSink, JobRead, JobSnapshot, OutputSink, RunOutput, RunSpec, ShellKind,
    ShellSpec, kill_tree, resolve_shell, run,
};
pub use sandbox::{
    SandboxBackend, SandboxProfile, TOOL_CACHES, is_sandbox_denial, sandbox_shell, tool_caches,
};
pub use search::{
    FileIndex, GlobResult, GrepEngine, GrepMode, GrepQuery, GrepResult, glob, grep,
    grep_with_engine, rg_available,
};
pub use web::{
    DEFAULT_FETCH_MAX_BYTES, FetchOptions, FetchedPage, SearchBackend, SearchHit, USER_AGENT,
    WebClient,
};
