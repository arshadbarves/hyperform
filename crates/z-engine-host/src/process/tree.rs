//! Ownership of a shell's process group. `kill_on_drop` only reaches the
//! shell itself, so a call whose future is dropped mid-flight (a cancelled
//! task, a runtime torn down by a crash) would orphan its descendants
//! (`sleep`, servers started with `&`). The guard kills the whole group
//! unless the owner finished with it normally.

use super::kill::kill_tree;

#[derive(Debug)]
pub(crate) struct TreeGuard {
    pid: Option<u32>,
}

impl TreeGuard {
    pub(crate) fn new(pid: Option<u32>) -> Self {
        Self { pid }
    }

    /// The owner reaped the shell and drained (or killed) its tree.
    pub(crate) fn disarm(mut self) {
        self.pid = None;
    }
}

impl Drop for TreeGuard {
    fn drop(&mut self) {
        if let Some(pid) = self.pid.take() {
            tracing::debug!(pid, "process owner dropped; killing its tree");
            kill_tree(pid);
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    fn group_alive(pgid: u32) -> bool {
        std::process::Command::new("kill")
            .args(["-0", &format!("-{pgid}")])
            .status()
            .is_ok_and(|status| status.success())
    }

    #[test]
    fn dropping_an_armed_guard_kills_the_group_and_disarming_spares_it() {
        let spawn = || {
            use std::os::unix::process::CommandExt as _;
            std::process::Command::new("sh")
                .args(["-c", "sleep 30 & wait"])
                .process_group(0)
                .spawn()
                .unwrap()
        };
        let mut armed = spawn();
        let pid = armed.id();
        drop(TreeGuard::new(Some(pid)));
        armed.wait().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while group_alive(pid) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!group_alive(pid), "the sleeping grandchild survived");

        let mut spared = spawn();
        let spared_pid = spared.id();
        TreeGuard::new(Some(spared_pid)).disarm();
        assert!(group_alive(spared_pid));
        kill_tree(spared_pid);
        spared.wait().unwrap();
    }
}
