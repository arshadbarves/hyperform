//! Whole-tree termination. Spawned shells lead their own process group
//! (unix) so one signal reaches every descendant; the system `kill` /
//! `taskkill` binaries do the work, keeping the crate free of `unsafe`.

use std::process::Stdio;

/// `CREATE_NO_WINDOW`: never flash a console window for agent processes.
#[cfg(windows)]
pub(crate) const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Kills `pid` and its process group (unix) or process tree (Windows).
/// Blocks briefly while the system tool runs; a tree that already exited
/// is not an error.
pub fn kill_tree(pid: u32) {
    match kill_command(pid).status() {
        Ok(status) if status.success() => {}
        Ok(status) => tracing::debug!(pid, %status, "process tree already gone"),
        Err(e) => tracing::warn!(pid, error = %e, "could not run the kill command"),
    }
}

#[cfg(unix)]
fn kill_command(pid: u32) -> std::process::Command {
    let mut cmd = std::process::Command::new("kill");
    cmd.arg("-9")
        .arg(format!("-{pid}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd
}

#[cfg(windows)]
fn kill_command(pid: u32) -> std::process::Command {
    use std::os::windows::process::CommandExt as _;
    let mut cmd = std::process::Command::new("taskkill");
    cmd.args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW);
    cmd
}
