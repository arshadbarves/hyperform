//! Workspace fingerprint: a 16-hex digest that changes whenever a file's
//! content changes (seen through size and mtime) and stays stable
//! otherwise. Used to tell whether verification evidence is still fresh.

use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};

use crate::HostError;
use crate::blocking::run_blocking;
use crate::digest::short_hex;
use crate::git::{git, git_available, head_sha, parse_porcelain_z, repo_root};
use crate::search::walker;

/// Files stat-ed for one fingerprint (changed files in a repository, all
/// files otherwise).
const MAX_FILES: usize = 20_000;

/// Git work trees hash HEAD, the porcelain status, and the size and mtime
/// of every changed or untracked file; other directories hash the path,
/// size and mtime of up to 20,000 gitignore-filtered files.
pub async fn workspace_fingerprint(root: &Path) -> Result<String, HostError> {
    if git_available() {
        if let Some(top) = repo_root(root).await {
            return git_fingerprint(root, top).await;
        }
    }
    let root = root.to_path_buf();
    run_blocking(move || tree_fingerprint(&root)).await
}

async fn git_fingerprint(root: &Path, top: PathBuf) -> Result<String, HostError> {
    let head = head_sha(root)
        .await?
        .unwrap_or_else(|| "unborn".to_string());
    let status = git(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )
    .await?;
    run_blocking(move || {
        let mut hasher = Sha256::new();
        hasher.update(b"git\0");
        hasher.update(head.as_bytes());
        hasher.update(b"\0");
        hasher.update(status.as_bytes());
        let entries = parse_porcelain_z(&status);
        for entry in entries.iter().take(MAX_FILES) {
            let meta = std::fs::symlink_metadata(top.join(&entry.path)).ok();
            stamp(&mut hasher, &entry.path, meta.as_ref());
        }
        if entries.len() > MAX_FILES {
            hasher.update(b"truncated\n");
        }
        Ok(short_hex(hasher))
    })
    .await
}

fn tree_fingerprint(root: &Path) -> Result<String, HostError> {
    if !root.is_dir() {
        return Err(HostError::NotFound(format!("directory {}", root.display())));
    }
    let mut hasher = Sha256::new();
    hasher.update(b"tree\0");
    let mut seen = 0usize;
    for entry in walker(root).build().filter_map(Result::ok) {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        if seen == MAX_FILES {
            hasher.update(b"truncated\n");
            break;
        }
        seen += 1;
        let rel = entry.path().strip_prefix(root).unwrap_or(entry.path());
        stamp(
            &mut hasher,
            &rel.to_string_lossy(),
            entry.metadata().ok().as_ref(),
        );
    }
    Ok(short_hex(hasher))
}

fn stamp(hasher: &mut Sha256, path: &str, meta: Option<&Metadata>) {
    let line = match meta {
        Some(meta) => {
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            format!("{path}\0{}\0{mtime}\n", meta.len())
        }
        None => format!("{path}\0missing\n"),
    };
    hasher.update(line.as_bytes());
}
