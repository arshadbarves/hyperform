//! Throwaway project directories, optionally initialized as git repos.

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug)]
pub struct FixtureRepo {
    dir: tempfile::TempDir,
}

impl FixtureRepo {
    /// An empty directory (not a git repo).
    pub fn empty() -> Self {
        Self {
            dir: tempfile::tempdir().expect("create temp dir"),
        }
    }

    /// A git repo with `files` committed on `main`.
    pub fn git(files: &[(&str, &str)]) -> Self {
        let repo = Self::empty();
        for (path, contents) in files {
            repo.write(path, contents);
        }
        repo.run_git(&["init", "-q", "-b", "main"]);
        repo.run_git(&["config", "user.email", "test@example.com"]);
        repo.run_git(&["config", "user.name", "Test"]);
        repo.run_git(&["config", "commit.gpgsign", "false"]);
        repo.run_git(&["add", "-A"]);
        repo.run_git(&["commit", "-q", "-m", "initial", "--allow-empty"]);
        repo
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn join(&self, relative: &str) -> PathBuf {
        self.dir.path().join(relative)
    }

    pub fn write(&self, relative: &str, contents: &str) {
        let path = self.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent dirs");
        }
        std::fs::write(path, contents).expect("write fixture file");
    }

    pub fn read(&self, relative: &str) -> String {
        std::fs::read_to_string(self.join(relative)).expect("read fixture file")
    }

    pub fn exists(&self, relative: &str) -> bool {
        self.join(relative).exists()
    }

    /// Run git in the repo and return stdout; panics on failure.
    pub fn run_git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(self.path())
            .output()
            .expect("spawn git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }
}
