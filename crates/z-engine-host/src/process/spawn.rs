//! Builds and supervises shell child processes: allowlisted environment,
//! closed stdin, own process group, no console window, killed on drop, and
//! the reaping of pipe readers once the shell is gone.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::env::EnvPolicy;
use super::kill::kill_tree;
use super::shell::ShellSpec;
use crate::HostError;

/// How long pipe readers may keep draining after the shell exited.
const DRAIN_GRACE: Duration = Duration::from_millis(500);

pub(crate) fn shell_command(
    shell: &ShellSpec,
    script: &str,
    cwd: &Path,
    env: &EnvPolicy,
) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(&shell.program);
    cmd.args(&shell.args)
        .arg(script)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .envs(env.build())
        .kill_on_drop(true);
    #[cfg(unix)]
    cmd.process_group(0);
    #[cfg(windows)]
    cmd.creation_flags(super::kill::CREATE_NO_WINDOW);
    cmd
}

pub(crate) fn check_command(command: &str, cwd: &Path) -> Result<(), HostError> {
    if command.trim().is_empty() {
        return Err(HostError::Invalid("command is empty".to_string()));
    }
    if !cwd.is_dir() {
        return Err(HostError::NotFound(format!(
            "working directory {}",
            cwd.display()
        )));
    }
    Ok(())
}

pub(crate) fn spawn_error(shell: &ShellSpec, error: std::io::Error) -> HostError {
    HostError::Process(format!(
        "could not start {}: {error}",
        shell.program.display()
    ))
}

/// Kills the whole tree and reaps the shell.
pub(crate) async fn terminate(child: &mut tokio::process::Child, pid: Option<u32>) {
    if let Some(pid) = pid {
        kill_tree(pid);
    }
    if let Err(e) = child.start_kill() {
        tracing::debug!(error = %e, "shell already exited");
    }
    if let Err(e) = child.wait().await {
        tracing::warn!(error = %e, "could not reap the shell");
    }
}

/// Waits for the pipe readers after the shell exited. Readers still open
/// after a grace period mean a descendant kept the pipes (e.g. `cmd &`):
/// the process group is killed so nothing outlives its owner. `cancel`
/// cuts the grace period short.
pub(crate) async fn drain(
    readers: &mut Vec<JoinHandle<()>>,
    pid: Option<u32>,
    cancel: Option<&CancellationToken>,
) {
    if join_within(readers, cancel).await {
        return;
    }
    if let Some(pid) = pid {
        kill_tree(pid);
    }
    if !join_within(readers, cancel).await {
        for reader in readers.drain(..) {
            reader.abort();
        }
    }
}

async fn join_within(
    readers: &mut Vec<JoinHandle<()>>,
    cancel: Option<&CancellationToken>,
) -> bool {
    let all = async {
        while let Some(reader) = readers.last_mut() {
            if let Err(e) = reader.await {
                tracing::debug!(error = %e, "pipe reader ended abnormally");
            }
            readers.pop();
        }
    };
    let cancelled = async {
        match cancel {
            Some(token) => token.cancelled().await,
            None => std::future::pending().await,
        }
    };
    tokio::select! {
        biased;
        () = all => true,
        () = cancelled => false,
        () = tokio::time::sleep(DRAIN_GRACE) => false,
    }
}
