//! Long-lived MCP and language servers: resolved on PATH, started with the
//! host's allowlisted environment plus common toolchain variables, piped
//! stdio, their own process group (unix), killed on drop, and torn down as
//! a whole tree with the host's `kill_tree`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::task::JoinHandle;
use z_engine_host::{EnvPolicy, kill_tree};

use super::stderr::StderrLog;
use crate::error::IntegrationError;

/// Toolchain variables servers commonly need beyond the host allowlist
/// (compilers, package managers, virtualenvs, proxies and certificates).
const PASSTHROUGH: &[&str] = &[
    "CARGO_HOME",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
    "GOPATH",
    "GOROOT",
    "GOFLAGS",
    "GOPROXY",
    "GOMODCACHE",
    "GOCACHE",
    "NODE_PATH",
    "NODE_OPTIONS",
    "NVM_DIR",
    "npm_config_cache",
    "VIRTUAL_ENV",
    "CONDA_PREFIX",
    "PYTHONPATH",
    "PYENV_ROOT",
    "UV_CACHE_DIR",
    "JAVA_HOME",
    "XDG_CONFIG_HOME",
    "XDG_CACHE_HOME",
    "XDG_DATA_HOME",
    "XDG_RUNTIME_DIR",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "no_proxy",
];

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug)]
pub(crate) struct ServerCommand<'a> {
    pub program: &'a str,
    pub args: &'a [String],
    pub cwd: Option<&'a Path>,
    /// Set on top of the allowlisted environment.
    pub env: &'a BTreeMap<String, String>,
}

#[derive(Debug)]
pub(crate) struct Spawned {
    pub process: ServerProcess,
    pub stdin: ChildStdin,
    pub stdout: ChildStdout,
}

/// The executable for `program`: explicit paths as given (relative ones
/// against `cwd`), bare names looked up on PATH.
pub(crate) fn find_program(program: &str, cwd: Option<&Path>) -> Option<PathBuf> {
    let path = Path::new(program);
    if path.is_absolute() || path.components().count() > 1 {
        let full = match cwd {
            Some(dir) if path.is_relative() => dir.join(path),
            _ => path.to_path_buf(),
        };
        return full.is_file().then_some(full);
    }
    which::which(program).ok()
}

pub(crate) fn spawn_server(
    command: &ServerCommand<'_>,
    stderr: &StderrLog,
) -> Result<Spawned, IntegrationError> {
    let spawn_error = |source| IntegrationError::Spawn {
        command: command.program.to_string(),
        source,
    };
    let program = find_program(command.program, command.cwd).ok_or_else(|| {
        spawn_error(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "not found on PATH",
        ))
    })?;
    let env = EnvPolicy {
        passthrough: PASSTHROUGH.iter().map(|name| (*name).to_string()).collect(),
        extra: command.env.clone(),
    };
    let mut cmd = tokio::process::Command::new(&program);
    cmd.args(command.args)
        .env_clear()
        .envs(env.build())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(cwd) = command.cwd {
        cmd.current_dir(cwd);
    }
    #[cfg(unix)]
    cmd.process_group(0);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let mut child = cmd.spawn().map_err(spawn_error)?;
    let (Some(stdin), Some(stdout), Some(err)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        return Err(IntegrationError::io(
            format!("starting `{}`", command.program),
            std::io::Error::other("the stdio pipes were not captured"),
        ));
    };
    let pid = child.id();
    let stderr_task = stderr.spawn_reader(err, command.program.to_string());
    tracing::debug!(program = %program.display(), ?pid, "server started");
    Ok(Spawned {
        process: ServerProcess {
            child,
            pid,
            program: command.program.to_string(),
            stderr_task,
        },
        stdin,
        stdout,
    })
}

/// A running server. Dropping it kills the whole process group.
#[derive(Debug)]
pub(crate) struct ServerProcess {
    child: Child,
    pid: Option<u32>,
    program: String,
    stderr_task: JoinHandle<()>,
}

impl ServerProcess {
    /// Waits up to `grace` for the server to exit after its input closed,
    /// then kills its whole tree and reaps it.
    pub(crate) async fn shutdown(&mut self, grace: Duration) {
        match tokio::time::timeout(grace, self.child.wait()).await {
            Ok(Ok(status)) => tracing::debug!(program = %self.program, %status, "server exited"),
            Ok(Err(e)) => {
                tracing::warn!(program = %self.program, error = %e, "could not wait for the server");
                self.kill().await;
            }
            Err(_) => self.kill().await,
        }
    }

    async fn kill(&mut self) {
        if let Some(pid) = self.pid
            && let Err(e) = tokio::task::spawn_blocking(move || kill_tree(pid)).await
        {
            tracing::warn!(program = %self.program, error = %e, "could not kill the server tree");
        }
        if let Err(e) = self.child.start_kill() {
            tracing::debug!(program = %self.program, error = %e, "server already exited");
        }
        if let Err(e) = self.child.wait().await {
            tracing::warn!(program = %self.program, error = %e, "could not reap the server");
        }
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        self.stderr_task.abort();
        // Only while the leader is unreaped is its process group ours.
        if let (Some(pid), Ok(None)) = (self.pid, self.child.try_wait()) {
            kill_tree(pid);
        }
    }
}
