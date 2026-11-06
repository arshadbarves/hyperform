//! One background job: its output ring, read cursor, lifecycle state, and
//! the monitor task that owns the child process until it exits.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use regex::Regex;
use tokio::io::AsyncRead;
use tokio::process::Child;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use z_engine_protocol::{JobId, JobStatus, now_ms};

use super::ring::OutputRing;
use super::{JobEvent, JobEventSink, JobRead, JobSnapshot};
use crate::process::lines::pump;
use crate::process::spawn::drain;
use crate::process::tree::TreeGuard;

const RING_BYTES: usize = 1024 * 1024;
const SNAPSHOT_TAIL_BYTES: usize = 4 * 1024;

#[derive(Debug)]
pub(super) struct Job {
    pub(super) id: JobId,
    label: String,
    owner: String,
    started_at: u64,
    pub(super) pid: Option<u32>,
    state: Mutex<JobState>,
    done: watch::Sender<bool>,
}

#[derive(Debug)]
struct JobState {
    ring: OutputRing,
    /// Absolute offset the model has read up to.
    cursor: u64,
    status: JobStatus,
    exit_code: Option<i32>,
    finished_at: Option<u64>,
    kill_requested: bool,
}

impl Job {
    pub(super) fn new(label: String, owner: String, pid: Option<u32>) -> Self {
        Self {
            id: JobId::new(),
            label,
            owner,
            started_at: now_ms(),
            pid,
            state: Mutex::new(JobState {
                ring: OutputRing::new(RING_BYTES),
                cursor: 0,
                status: JobStatus::Running,
                exit_code: None,
                finished_at: None,
                kill_requested: false,
            }),
            done: watch::channel(false).0,
        }
    }

    /// Output since the previous `read_new`; the cursor always advances to
    /// the end, so filtered-out lines are consumed too.
    pub(super) fn read_new(&self, filter: Option<&Regex>) -> JobRead {
        let mut state = self.state();
        let (text, dropped_bytes) = state.ring.read_from(state.cursor);
        state.cursor = state.ring.end();
        let output = match filter {
            Some(re) => text
                .lines()
                .filter(|line| re.is_match(line))
                .map(|line| format!("{line}\n"))
                .collect(),
            None => text,
        };
        JobRead {
            output,
            status: state.status,
            exit_code: state.exit_code,
            dropped_bytes,
        }
    }

    pub(super) fn snapshot(&self) -> JobSnapshot {
        let state = self.state();
        JobSnapshot {
            id: self.id.clone(),
            label: self.label.clone(),
            owner: self.owner.clone(),
            status: state.status,
            exit_code: state.exit_code,
            started_at: self.started_at,
            finished_at: state.finished_at,
            tail: state.ring.tail(SNAPSHOT_TAIL_BYTES),
        }
    }

    pub(super) fn is_running(&self) -> bool {
        self.state().status == JobStatus::Running
    }

    /// Marks a running job as being killed; `false` if it already finished.
    pub(super) fn request_kill(&self) -> bool {
        let mut state = self.state();
        if state.status.is_terminal() {
            return false;
        }
        state.kill_requested = true;
        true
    }

    pub(super) fn started_at(&self) -> u64 {
        self.started_at
    }

    /// Resolves once the job has finished (immediately if it already has).
    pub(super) async fn finished(&self) {
        let mut done = self.done.subscribe();
        if done.wait_for(|finished| *finished).await.is_err() {
            tracing::debug!(job = %self.id, "job state dropped while waiting");
        }
    }

    fn append(&self, text: &str) {
        self.state().ring.push(text.as_bytes());
    }

    fn finish(&self, status: Option<std::process::ExitStatus>) -> (JobStatus, Option<i32>) {
        let mut state = self.state();
        state.exit_code = status.and_then(|s| s.code());
        state.status = if state.kill_requested {
            JobStatus::Killed
        } else if status.is_some_and(|s| s.success()) {
            JobStatus::Completed
        } else {
            JobStatus::Failed
        };
        state.finished_at = Some(now_ms());
        let outcome = (state.status, state.exit_code);
        drop(state);
        self.done.send_replace(true);
        outcome
    }

    /// Only whole-field updates happen under the lock, so a poisoned state
    /// is still consistent.
    fn state(&self) -> MutexGuard<'_, JobState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Owns `child` until it exits: streams its output into the job, then
/// records the final status and emits `JobEvent::Exited`.
pub(super) async fn monitor(job: Arc<Job>, mut child: Child, events: Option<JobEventSink>) {
    let tree = TreeGuard::new(job.pid);
    let mut readers = vec![
        reader(child.stdout.take(), &job, &events),
        reader(child.stderr.take(), &job, &events),
    ];
    let status = match child.wait().await {
        Ok(status) => Some(status),
        Err(e) => {
            tracing::warn!(job = %job.id, error = %e, "waiting for background job failed");
            None
        }
    };
    drain(&mut readers, job.pid, None).await;
    tree.disarm();
    let (status, exit_code) = job.finish(status);
    if let Some(events) = &events {
        events(JobEvent::Exited {
            id: job.id.clone(),
            status,
            exit_code,
        });
    }
}

fn reader<R>(pipe: Option<R>, job: &Arc<Job>, events: &Option<JobEventSink>) -> JoinHandle<()>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    let job = Arc::clone(job);
    let events = events.clone();
    tokio::spawn(async move {
        let Some(pipe) = pipe else { return };
        pump(pipe, move |line| {
            job.append(line);
            if let Some(events) = &events {
                events(JobEvent::Output {
                    id: job.id.clone(),
                    text: line.to_string(),
                });
            }
        })
        .await;
    })
}
