//! A shadow git repository that snapshots the whole working tree, including
//! changes made by shell commands, without touching the user's repository,
//! index, or config. Every call sets `GIT_DIR`, `GIT_WORK_TREE` and
//! `GIT_INDEX_FILE` to the shadow; snapshots are chained on one ref so all
//! of them stay reachable. Each commit message records the files too large
//! to store, so a restore can leave them alone.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use super::exclude::{self, Exclusions};
use crate::HostError;
use crate::blocking::run_blocking;
use crate::digest::short_sha256;
use crate::git::GitCmd;

const REF: &str = "refs/zengine/checkpoints";
const EXCLUDED_TRAILER: &str = "Zengine-Excluded: ";
/// Hashing a large tree for the first snapshot can take a while.
const ADD_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_USER_IGNORED: usize = 20_000;
const IDENTITY: &[(&str, &str)] = &[
    ("GIT_AUTHOR_NAME", "Z Engine"),
    ("GIT_AUTHOR_EMAIL", "zengine@localhost"),
    ("GIT_COMMITTER_NAME", "Z Engine"),
    ("GIT_COMMITTER_EMAIL", "zengine@localhost"),
];
/// Byte-exact storage with no signing and no filesystem monitor.
const CONFIG: &[(&str, &str)] = &[
    ("core.autocrlf", "false"),
    ("core.fsmonitor", "false"),
    ("commit.gpgsign", "false"),
];
/// Disables end-of-line conversion and filters (e.g. LFS) in the shadow.
const ATTRIBUTES: &str = "* -text -filter -ident -eol\n";

/// The 16-hex key of a project: sha256 of its canonical root. Checkpoints
/// (`<key>.git`) and work plans (`plans/<key>/`) share it.
pub fn project_key(canonical_root: &Path) -> String {
    short_sha256(canonical_root.to_string_lossy().as_bytes())
}

/// Cheap to clone; clones share the snapshot lock.
#[derive(Debug, Clone)]
pub struct ShadowRepo {
    git_dir: PathBuf,
    work_tree: PathBuf,
    max_files: usize,
    lock: Arc<tokio::sync::Mutex<()>>,
}

impl ShadowRepo {
    /// Opens (creating when needed) the shadow repository for
    /// `project_root` at `checkpoints_dir/<16 hex of sha256(root)>.git`.
    /// Fails with `HostError::Blocked` for trees of more than 50,000 files.
    pub async fn open(
        checkpoints_dir: &Path,
        project_root: &Path,
    ) -> Result<ShadowRepo, HostError> {
        Self::open_with_file_limit(checkpoints_dir, project_root, exclude::MAX_CANDIDATES).await
    }

    /// [`ShadowRepo::open`] with a different file limit (tests).
    #[doc(hidden)]
    pub async fn open_with_file_limit(
        checkpoints_dir: &Path,
        project_root: &Path,
        max_files: usize,
    ) -> Result<ShadowRepo, HostError> {
        let work_tree =
            std::fs::canonicalize(project_root).map_err(|e| HostError::io(project_root, e))?;
        if !work_tree.is_dir() {
            return Err(HostError::Invalid(format!(
                "{} is not a directory",
                work_tree.display()
            )));
        }
        std::fs::create_dir_all(checkpoints_dir).map_err(|e| HostError::io(checkpoints_dir, e))?;
        let checkpoints_dir = std::fs::canonicalize(checkpoints_dir)
            .map_err(|e| HostError::io(checkpoints_dir, e))?;
        let key = project_key(&work_tree);
        let repo = ShadowRepo {
            git_dir: checkpoints_dir.join(format!("{key}.git")),
            work_tree,
            max_files,
            lock: Arc::default(),
        };
        if !repo.git_dir.join("HEAD").is_file() {
            repo.init().await?;
        }
        repo.refresh_excludes().await?;
        Ok(repo)
    }

    pub fn git_dir(&self) -> &Path {
        &self.git_dir
    }

    pub fn work_tree(&self) -> &Path {
        &self.work_tree
    }

    /// Records the current tree and returns its commit; an unchanged tree
    /// returns the previous snapshot's commit.
    pub async fn snapshot(&self, message: &str) -> Result<String, HostError> {
        let _guard = self.exclusive().await;
        Ok(self.snapshot_locked(message).await?.0)
    }

    /// Serializes snapshot and restore on this repository.
    pub(super) async fn exclusive(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.lock.lock().await
    }

    /// The snapshot commit and what it had to leave out.
    pub(super) async fn snapshot_locked(
        &self,
        message: &str,
    ) -> Result<(String, Exclusions), HostError> {
        let exclusions = self.refresh_excludes().await?;
        let add = self
            .cmd(["add", "-A", "--ignore-errors", "--", "."])
            .timeout(ADD_TIMEOUT)
            .output()
            .await?;
        if !add.success() {
            if add.stderr.contains("fatal:") {
                return Err(add.into_error());
            }
            tracing::warn!(stderr = %add.stderr.trim(), "checkpoint skipped unreadable files");
        }
        let tree = self.cmd(["write-tree"]).run().await?.trim().to_string();
        let parent = self.latest().await?;
        if let Some(parent) = &parent {
            let parent_tree = self
                .cmd(["rev-parse", &format!("{parent}^{{tree}}")])
                .run()
                .await?;
            if parent_tree.trim() == tree && self.excluded_at(parent).await? == exclusions.large {
                return Ok((parent.clone(), exclusions));
            }
        }
        let mut message = if message.trim().is_empty() {
            "checkpoint"
        } else {
            message
        }
        .to_string();
        if !exclusions.large.is_empty() {
            message.push_str("\n\n");
            for path in exclusions.large.iter().filter(|p| !p.contains('\n')) {
                message.push_str(&format!("{EXCLUDED_TRAILER}{path}\n"));
            }
        }
        let mut args = vec!["commit-tree", tree.as_str()];
        if let Some(parent) = &parent {
            args.extend(["-p", parent.as_str()]);
        }
        args.extend(["-m", message.as_str()]);
        let commit = self
            .cmd(&args)
            .envs(IDENTITY)
            .run()
            .await?
            .trim()
            .to_string();
        self.cmd(["update-ref", REF, &commit]).run().await?;
        Ok((commit, exclusions))
    }

    /// Files too large to store when `commit` was taken, sorted.
    pub(super) async fn excluded_at(&self, commit: &str) -> Result<Vec<String>, HostError> {
        let raw = self.cmd(["cat-file", "commit", commit]).run().await?;
        let message = raw.split_once("\n\n").map_or("", |(_, message)| message);
        let mut paths: Vec<String> = message
            .lines()
            .filter_map(|line| line.strip_prefix(EXCLUDED_TRAILER))
            .map(str::to_string)
            .collect();
        paths.sort();
        Ok(paths)
    }

    /// The full commit id for `rev`, or `NotFound` when it is unknown.
    pub(super) async fn resolve_commit(&self, rev: &str) -> Result<String, HostError> {
        let out = self
            .cmd([
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("{rev}^{{commit}}"),
            ])
            .output()
            .await?;
        if out.success() {
            Ok(out.stdout.trim().to_string())
        } else {
            Err(HostError::NotFound(format!("checkpoint {rev}")))
        }
    }

    /// A git command against the shadow repository.
    pub(super) fn cmd<I, S>(&self, args: I) -> GitCmd<'_>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let env = [
            ("GIT_DIR", self.git_dir.clone()),
            ("GIT_WORK_TREE", self.work_tree.clone()),
            ("GIT_INDEX_FILE", self.git_dir.join("index")),
        ];
        GitCmd::new(&self.work_tree, args).envs(&env)
    }

    async fn latest(&self) -> Result<Option<String>, HostError> {
        let out = self
            .cmd(["rev-parse", "--verify", "--quiet", REF])
            .output()
            .await?;
        Ok(out.success().then(|| out.stdout.trim().to_string()))
    }

    async fn init(&self) -> Result<(), HostError> {
        self.cmd(["init", "--quiet"]).run().await?;
        for (key, value) in CONFIG {
            self.cmd(["config", key, value]).run().await?;
        }
        // Overrides a global `core.hooksPath`; the shadow has no hooks.
        let hooks = self.git_dir.join("hooks");
        self.cmd([
            OsStr::new("config"),
            OsStr::new("core.hooksPath"),
            hooks.as_os_str(),
        ])
        .run()
        .await?;
        let attributes = self.git_dir.join("info").join("attributes");
        exclude::write_if_changed(&attributes, ATTRIBUTES)
    }

    async fn refresh_excludes(&self) -> Result<Exclusions, HostError> {
        let user_ignored = self.user_ignored().await;
        let work_tree = self.work_tree.clone();
        let git_dir = self.git_dir.clone();
        let max_files = self.max_files;
        run_blocking(move || {
            let exclusions = Exclusions {
                large: exclude::large_files(&work_tree, &git_dir, max_files)?,
                user_ignored,
            };
            let content = exclude::render(&git_dir, &work_tree, &exclusions);
            exclude::write_if_changed(&git_dir.join("info").join("exclude"), &content)?;
            Ok(exclusions)
        })
        .await
    }

    /// What the user's own repository ignores under the work tree, which
    /// can include rules from parent directories the shadow cannot see.
    /// Empty when the project is not inside a repository.
    async fn user_ignored(&self) -> Vec<String> {
        let listing = GitCmd::new(
            &self.work_tree,
            [
                "ls-files",
                "--others",
                "--ignored",
                "--exclude-standard",
                "--directory",
                "-z",
            ],
        )
        .output()
        .await;
        match listing {
            Ok(out) if out.success() => out
                .stdout
                .split('\0')
                .filter(|path| !path.is_empty())
                .take(MAX_USER_IGNORED)
                .map(str::to_string)
                .collect(),
            _ => Vec::new(),
        }
    }
}
