//! Background agents as jobs: listed with the shells, read with the
//! agent's latest (finally its last) assistant text, awaited until the
//! agent finishes, and killed by cancelling the agent's token.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use hyperform_host::JobRead;
use hyperform_protocol::{JobId, JobInfo, JobStatus, now_ms};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::orchestration::AgentTracker;
use crate::sync::lock;

/// Characters of the agent's text kept as the jobs panel tail.
const TAIL_CHARS: usize = 2_000;

#[derive(Debug, Default)]
pub(crate) struct AgentJobs {
    jobs: Mutex<BTreeMap<JobId, AgentJob>>,
}

#[derive(Debug)]
struct AgentJob {
    info: JobInfo,
    cancel: CancellationToken,
    done: watch::Sender<bool>,
    tracker: Arc<AgentTracker>,
}

impl AgentJobs {
    pub(crate) fn insert(
        &self,
        info: JobInfo,
        cancel: CancellationToken,
        tracker: Arc<AgentTracker>,
    ) {
        let (done, _) = watch::channel(false);
        let job = AgentJob {
            info: info.clone(),
            cancel,
            done,
            tracker,
        };
        lock(&self.jobs).insert(info.job_id, job);
    }

    pub(crate) fn contains(&self, id: &JobId) -> bool {
        lock(&self.jobs).contains_key(id)
    }

    /// Records the job's end; waiters wake on [`AgentJobs::release`].
    pub(crate) fn finish(&self, id: &JobId, status: JobStatus) -> Option<JobInfo> {
        let mut jobs = lock(&self.jobs);
        let job = jobs.get_mut(id)?;
        job.info.status = status;
        job.info.finished_at = Some(now_ms());
        Some(snapshot(job))
    }

    pub(crate) fn release(&self, id: &JobId) {
        if let Some(job) = lock(&self.jobs).get(id) {
            job.done.send_replace(true);
        }
    }

    pub(crate) fn info(&self, id: &JobId) -> Option<JobInfo> {
        lock(&self.jobs).get(id).map(snapshot)
    }

    pub(crate) fn list(&self) -> Vec<JobInfo> {
        lock(&self.jobs).values().map(snapshot).collect()
    }

    pub(crate) fn read(&self, id: &JobId) -> Option<JobRead> {
        lock(&self.jobs).get(id).map(read_of)
    }

    /// The agent's run has read web content.
    pub(crate) fn read_web(&self, id: &JobId) -> bool {
        lock(&self.jobs)
            .get(id)
            .is_some_and(|job| job.tracker.read_web())
    }

    /// Waits up to `timeout` for the agent to finish, then reads.
    pub(crate) async fn wait(&self, id: &JobId, timeout: Duration) -> Option<JobRead> {
        let done = lock(&self.jobs).get(id)?.done.subscribe();
        if tokio::time::timeout(timeout, finished(done)).await.is_err() {
            tracing::debug!(job = %id, "agent job still running after the wait");
        }
        self.read(id)
    }

    /// Cancels the agent and waits (bounded) for it to wind down.
    pub(crate) async fn kill(&self, id: &JobId, timeout: Duration) -> bool {
        let Some(done) = lock(&self.jobs).get(id).map(|job| {
            job.cancel.cancel();
            job.done.subscribe()
        }) else {
            return false;
        };
        if tokio::time::timeout(timeout, finished(done)).await.is_err() {
            tracing::warn!(job = %id, "background agent did not stop in time");
        }
        true
    }

    /// Cancels every agent and waits (bounded) until all recorded their end.
    pub(crate) async fn stop_all(&self, timeout: Duration) {
        let waiting: Vec<watch::Receiver<bool>> = lock(&self.jobs)
            .values()
            .map(|job| {
                job.cancel.cancel();
                job.done.subscribe()
            })
            .collect();
        let all = futures::future::join_all(waiting.into_iter().map(finished));
        if tokio::time::timeout(timeout, all).await.is_err() {
            tracing::warn!("background agents did not stop in time");
        }
    }
}

/// Resolves once the job is marked finished (or its entry is dropped).
async fn finished(mut done: watch::Receiver<bool>) {
    if done.wait_for(|finished| *finished).await.is_err() {
        tracing::debug!("agent job dropped before it finished");
    }
}

fn snapshot(job: &AgentJob) -> JobInfo {
    let text = job.tracker.latest_text();
    let count = text.chars().count();
    let tail = if count > TAIL_CHARS {
        text.chars().skip(count - TAIL_CHARS).collect()
    } else {
        text
    };
    JobInfo {
        output_tail: tail,
        ..job.info.clone()
    }
}

fn read_of(job: &AgentJob) -> JobRead {
    let text = job.tracker.latest_text();
    let output = if text.trim().is_empty() {
        "(the agent has not reported yet)".to_string()
    } else {
        text
    };
    JobRead {
        output,
        status: job.info.status,
        exit_code: None,
        dropped_bytes: 0,
    }
}
