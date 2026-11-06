//! Fixtures shared by the integration tests.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

/// Runs `git` for fixture setup, panicking on failure.
pub fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("spawn git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Writes `files` (relative path, content) under `root`.
pub fn write_files(root: &Path, files: &[(&str, &str)]) {
    for (rel, content) in files {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
}

/// A repository on `main` with one commit containing `files`; hooks and
/// signing are disabled so the user's global config cannot interfere.
pub fn repo_with(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    write_files(dir.path(), files);
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "initial"]);
    dir
}

pub fn init_repo(root: &Path) {
    git(root, &["init", "-q", "-b", "main"]);
    for (key, value) in [
        ("user.name", "Test"),
        ("user.email", "test@example.com"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", ".git/no-hooks"),
    ] {
        git(root, &["config", key, value]);
    }
}

/// Sets a file's mtime to `offset_secs` after a fixed epoch-relative base.
pub fn set_mtime(path: &Path, offset_secs: u64) {
    let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(base + Duration::from_secs(offset_secs))
        .unwrap();
}

pub fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap()
}

/// Whether `pid` is gone within a few seconds (unix `kill -0`).
#[cfg(unix)]
pub async fn eventually_dead(pid: &str) -> bool {
    for _ in 0..60 {
        let alive = Command::new("kill")
            .args(["-0", pid])
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        if !alive {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

/// Waits until `path` exists and has content, returning it trimmed.
pub async fn wait_for_file(path: &Path) -> String {
    for _ in 0..100 {
        if let Ok(text) = std::fs::read_to_string(path) {
            if !text.trim().is_empty() {
                return text.trim().to_string();
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("{} never appeared", path.display());
}
