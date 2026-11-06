//! Registry of background shells (`run_in_background`): spawn, incremental
//! reads, waiting, and whole-tree kills. Dropping the last handle kills
//! every job still running, so no process outlives its owner.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use regex::Regex;
use z_engine_protocol::JobId;

use super::job::{Job, monitor};
use super::{BackgroundSpec, JobEventSink, JobRead, JobSnapshot};
use crate::HostError;
use crate::process::kill::kill_tree;
use crate::process::spawn::{check_command, shell_command, spawn_error};

/// How long `kill` waits for the monitor to observe the exit.
const KILL_WAIT: Duration = Duration::from_secs(5);

/// Cheap to clone; clones share the registry.
#[derive(Clone)]
pub struct BackgroundShells {
    registry: Arc<Registry>,
}

struct Registry {
    jobs: Mutex<BTreeMap<JobId, Arc<Job>>>,
    on_event: Option<JobEventSink>,
}

impl fmt::Debug for BackgroundShells {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BackgroundShells")
            .field("jobs", &self.registry.jobs().len())
            .field("on_event", &self.registry.on_event.is_some())
            .finish()
    }
}

impl BackgroundShells {
    pub fn new(on_event: Option<JobEventSink>) -> Self {
        Self {
            registry: Arc::new(Registry {
                jobs: Mutex::new(BTreeMap::new()),
                on_event,
            }),
        }
    }

    /// Starts `spec.command` in its own process group and returns at once.
    pub async fn spawn(&self, spec: BackgroundSpec) -> Result<JobId, HostError> {
        check_command(&spec.command, &spec.cwd)?;
        let child = shell_command(&spec.shell, &spec.command, &spec.cwd, &spec.env)
            .spawn()
            .map_err(|e| spawn_error(&spec.shell, e))?;
        let job = Arc::new(Job::new(spec.label, spec.owner, child.id()));
        let id = job.id.clone();
        self.registry.jobs().insert(id.clone(), Arc::clone(&job));
        tokio::spawn(monitor(job, child, self.registry.on_event.clone()));
        Ok(id)
    }

    /// Output produced since the previous read. With `filter`, only
    /// matching lines are returned; the rest is consumed regardless.
    pub fn read_new(&self, id: &JobId, filter: Option<&Regex>) -> Result<JobRead, HostError> {
        Ok(self.job(id)?.read_new(filter))
    }

    /// Waits until the job exits or `timeout` elapses, then reads new
    /// output. A job still running after the timeout is not an error.
    pub async fn wait(&self, id: &JobId, timeout: Duration) -> Result<JobRead, HostError> {
        let job = self.job(id)?;
        if tokio::time::timeout(timeout, job.finished()).await.is_err() {
            tracing::debug!(job = %id, "wait timed out; job still running");
        }
        Ok(job.read_new(None))
    }

    /// Kills the job's whole process tree and waits for its status to
    /// become `Killed`. Killing a finished job is a no-op.
    pub async fn kill(&self, id: &JobId) -> Result<(), HostError> {
        let job = self.job(id)?;
        if !job.request_kill() {
            return Ok(());
        }
        if let Some(pid) = job.pid {
            kill_tree(pid);
        }
        tokio::time::timeout(KILL_WAIT, job.finished())
            .await
            .map_err(|_| HostError::Timeout)
    }

    pub fn snapshot(&self, id: &JobId) -> Option<JobSnapshot> {
        self.registry.jobs().get(id).map(|job| job.snapshot())
    }

    /// Every job, oldest first.
    pub fn list(&self) -> Vec<JobSnapshot> {
        let mut jobs: Vec<Arc<Job>> = self.registry.jobs().values().cloned().collect();
        jobs.sort_by(|a, b| {
            a.started_at()
                .cmp(&b.started_at())
                .then_with(|| a.id.cmp(&b.id))
        });
        jobs.iter().map(|job| job.snapshot()).collect()
    }

    /// Kills every running job.
    pub async fn kill_all(&self) {
        let running: Vec<JobId> = self
            .registry
            .jobs()
            .values()
            .filter(|job| job.is_running())
            .map(|job| job.id.clone())
            .collect();
        for id in running {
            if let Err(e) = self.kill(&id).await {
                tracing::warn!(job = %id, error = %e, "could not kill background job");
            }
        }
    }

    fn job(&self, id: &JobId) -> Result<Arc<Job>, HostError> {
        self.registry
            .jobs()
            .get(id)
            .cloned()
            .ok_or_else(|| HostError::NotFound(format!("background job {id}")))
    }
}

impl Registry {
    /// Single inserts only; a poisoned table is still consistent.
    fn jobs(&self) -> MutexGuard<'_, BTreeMap<JobId, Arc<Job>>> {
        self.jobs.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        for job in self.jobs().values() {
            if let (true, Some(pid)) = (job.is_running(), job.pid) {
                job.request_kill();
                kill_tree(pid);
            }
        }
    }
}
