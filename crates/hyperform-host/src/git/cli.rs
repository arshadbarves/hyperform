//! The one place `git` is spawned: non-interactive environment, bounded
//! runtime, and typed failures.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;

use tokio::io::AsyncWriteExt;

use crate::HostError;

pub(crate) const GIT_TIMEOUT: Duration = Duration::from_secs(30);

const BASE_ENV: &[(&str, &str)] = &[
    ("GIT_TERMINAL_PROMPT", "0"),
    ("GIT_PAGER", "cat"),
    ("PAGER", "cat"),
    ("LC_ALL", "C"),
    // Read-only commands must not take the index lock from under the user.
    ("GIT_OPTIONAL_LOCKS", "0"),
];

/// Runs `git args...` in `cwd` and returns stdout; a non-zero exit is
/// `HostError::Git` with stderr, a missing binary `GitUnavailable`.
pub async fn git(cwd: &Path, args: &[&str]) -> Result<String, HostError> {
    GitCmd::new(cwd, args).run().await
}

/// [`git`] with extra environment variables (e.g. `GIT_DIR`).
pub async fn git_with_env(
    cwd: &Path,
    args: &[&str],
    env: &[(&str, &str)],
) -> Result<String, HostError> {
    GitCmd::new(cwd, args).envs(env).run().await
}

/// Whether a `git` binary is on `PATH` (checked once per process).
pub fn git_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| which::which("git").is_ok())
}

/// Result of a git invocation regardless of its exit status.
#[derive(Debug)]
pub(crate) struct GitOutput {
    pub(crate) code: Option<i32>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    args: Vec<String>,
}

impl GitOutput {
    pub(crate) fn success(&self) -> bool {
        self.code == Some(0)
    }

    pub(crate) fn into_error(self) -> HostError {
        HostError::Git {
            args: self.args,
            stderr: self.stderr.trim().to_string(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct GitCmd<'a> {
    cwd: &'a Path,
    args: Vec<OsString>,
    env: Vec<(OsString, OsString)>,
    stdin: Option<Vec<u8>>,
    timeout: Duration,
}

impl<'a> GitCmd<'a> {
    pub(crate) fn new<I, S>(cwd: &'a Path, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        Self {
            cwd,
            args: args
                .into_iter()
                .map(|a| a.as_ref().to_os_string())
                .collect(),
            env: Vec::new(),
            stdin: None,
            timeout: GIT_TIMEOUT,
        }
    }

    pub(crate) fn envs<K: AsRef<OsStr>, V: AsRef<OsStr>>(mut self, pairs: &[(K, V)]) -> Self {
        self.env.extend(
            pairs
                .iter()
                .map(|(k, v)| (k.as_ref().to_os_string(), v.as_ref().to_os_string())),
        );
        self
    }

    pub(crate) fn stdin(mut self, input: &[u8]) -> Self {
        self.stdin = Some(input.to_vec());
        self
    }

    pub(crate) fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Stdout of a successful run, else `HostError::Git`.
    pub(crate) async fn run(self) -> Result<String, HostError> {
        let output = self.output().await?;
        if output.success() {
            Ok(output.stdout)
        } else {
            Err(output.into_error())
        }
    }

    /// Runs git and reports any exit status.
    pub(crate) async fn output(self) -> Result<GitOutput, HostError> {
        let args: Vec<String> = self
            .args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        if !self.cwd.is_dir() {
            return Err(HostError::NotFound(format!(
                "directory {}",
                self.cwd.display()
            )));
        }
        let mut cmd = tokio::process::Command::new("git");
        cmd.args(&self.args)
            .current_dir(self.cwd)
            .envs(BASE_ENV.iter().copied())
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .stdin(if self.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(crate::process::CREATE_NO_WINDOW);
        let mut child = cmd.spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => HostError::GitUnavailable,
            _ => HostError::Process(format!("could not start git: {e}")),
        })?;
        let writer = match (self.stdin, child.stdin.take()) {
            (Some(input), Some(mut pipe)) => Some(tokio::spawn(async move {
                pipe.write_all(&input).await?;
                pipe.shutdown().await
            })),
            _ => None,
        };
        let output = tokio::time::timeout(self.timeout, child.wait_with_output())
            .await
            .map_err(|_| HostError::Timeout)?
            .map_err(|e| HostError::Process(format!("git {}: {e}", args.join(" "))))?;
        if let Some(writer) = writer {
            match writer.await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => tracing::debug!(error = %e, "git closed stdin early"),
                Err(e) => tracing::debug!(error = %e, "git stdin writer failed"),
            }
        }
        Ok(GitOutput {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            args,
        })
    }
}
