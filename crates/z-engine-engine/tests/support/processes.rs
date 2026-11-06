//! Process probes for resource-hygiene tests: pid files written by the
//! processes themselves (`echo $$ > file`), and whether a process group
//! still has a live (non-zombie) member.

use std::path::Path;
use std::time::Duration;

/// Generous so a loaded machine still passes; the probe returns early.
pub const PROCESS_WAIT: Duration = Duration::from_secs(10);

/// Waits for `path` to hold a pid.
pub async fn read_pid(path: &Path) -> u32 {
    let deadline = tokio::time::Instant::now() + PROCESS_WAIT;
    loop {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        if let Ok(pid) = text.trim().parse() {
            return pid;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "no pid in {}",
            path.display()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Whether any process of group `pgid` is alive (zombies do not count:
/// they hold no resources and their parent reaps them).
pub fn group_running(pgid: u32) -> bool {
    let output = std::process::Command::new("ps")
        .args(["-A", "-o", "pgid=,stat="])
        .output()
        .expect("ps runs");
    String::from_utf8_lossy(&output.stdout).lines().any(|line| {
        let mut fields = line.split_whitespace();
        let group = fields.next().and_then(|field| field.parse::<u32>().ok());
        let state = fields.next().unwrap_or_default();
        group == Some(pgid) && !state.starts_with('Z')
    })
}

/// Waits until group `pgid` has no live process; false on timeout.
pub async fn group_gone(pgid: u32) -> bool {
    let deadline = tokio::time::Instant::now() + PROCESS_WAIT;
    while group_running(pgid) {
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    true
}

/// A shell command line that records its pid in `pid_file`, then `exec`s
/// `program` (so the pid is the program's, and its process group's).
pub fn exec_recording_pid(pid_file: &Path, program: &Path) -> (String, Vec<String>) {
    let script = format!(
        "echo $$ > '{}'; exec '{}'",
        pid_file.display(),
        program.display()
    );
    ("sh".to_string(), vec!["-c".to_string(), script])
}
