//! Background jobs of one session: shells from the host registry and
//! background agents, both in the GUI's job list. Output updates are
//! throttled per job; an exit always emits and queues a "job finished"
//! reminder for the agent that started the job.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use hyperform_host::{BackgroundShells, BackgroundSpec, HostError, JobEvent, JobRead, JobSnapshot};
use hyperform_protocol::{AgentId, Event, JobId, JobInfo, JobKind, JobStatus};
use regex::Regex;
use tokio_util::sync::CancellationToken;

use super::agent_jobs::AgentJobs;
use crate::orchestration::AgentTracker;
use crate::session::{Emitter, ReminderBox};
use crate::sync::lock;

const OUTPUT_UPDATE_INTERVAL: Duration = Duration::from_millis(250);
/// How long a kill waits for a background agent to record its end.
const AGENT_KILL_TIMEOUT: Duration = Duration::from_secs(30);
/// How long closing the session waits for background agents.
const AGENT_STOP_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) struct JobHub {
    shells: BackgroundShells,
    agents: AgentJobs,
    events: Arc<Emitter>,
    reminders: Arc<ReminderBox>,
    last_update: Mutex<BTreeMap<JobId, Instant>>,
}

impl std::fmt::Debug for JobHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobHub")
            .field("shells", &self.shells)
            .field("agents", &self.agents)
            .finish_non_exhaustive()
    }
}

impl JobHub {
    /// The host registry reports through a weak handle, so dropping the
    /// hub drops the registry and kills whatever still runs.
    pub(crate) fn new(events: Arc<Emitter>, reminders: Arc<ReminderBox>) -> Arc<Self> {
        Arc::new_cyclic(|hub: &Weak<JobHub>| {
            let hub = hub.clone();
            let sink = Arc::new(move |event: JobEvent| {
                if let Some(hub) = hub.upgrade() {
                    hub.on_event(event);
                }
            });
            Self {
                shells: BackgroundShells::new(Some(sink)),
                agents: AgentJobs::default(),
                events,
                reminders,
                last_update: Mutex::new(BTreeMap::new()),
            }
        })
    }

    pub(crate) async fn spawn(&self, spec: BackgroundSpec) -> Result<JobInfo, HostError> {
        let id = self.shells.spawn(spec).await?;
        let info = self
            .info(&id)
            .ok_or_else(|| HostError::NotFound(format!("background job {id}")))?;
        self.events.emit(Event::JobUpdated { job: info.clone() });
        Ok(info)
    }

    /// Lists a background agent; `cancel` stops it on kill.
    pub(crate) fn spawn_agent(
        &self,
        owner: AgentId,
        tracker: Arc<AgentTracker>,
        cancel: CancellationToken,
    ) -> JobInfo {
        let agent = tracker.info();
        let info = JobInfo {
            job_id: JobId::new(),
            kind: JobKind::Agent,
            label: format!("{}: {}", agent.agent_type, agent.description),
            owner,
            agent_id: Some(agent.agent_id),
            status: JobStatus::Running,
            exit_code: None,
            started_at: agent.started_at,
            finished_at: None,
            output_tail: String::new(),
        };
        self.agents.insert(info.clone(), cancel, tracker);
        self.events.emit(Event::JobUpdated { job: info.clone() });
        info
    }

    /// A background agent ended: remind its owner and announce it before
    /// waking `JobOutput` waiters, so the reminder is queued by then.
    pub(crate) fn finish_agent(&self, id: &JobId, status: JobStatus) {
        if let Some(info) = self.agents.finish(id, status) {
            self.finished(info);
        }
        self.agents.release(id);
    }

    pub(crate) fn read(&self, id: &JobId, filter: Option<&Regex>) -> Result<JobRead, HostError> {
        match self.agents.read(id) {
            Some(read) => Ok(read),
            None => self.shells.read_new(id, filter),
        }
    }

    pub(crate) async fn wait(&self, id: &JobId, timeout: Duration) -> Result<JobRead, HostError> {
        match self.agents.wait(id, timeout).await {
            Some(read) => Ok(read),
            None => self.shells.wait(id, timeout).await,
        }
    }

    pub(crate) async fn kill(&self, id: &JobId) -> Result<(), HostError> {
        if self.agents.kill(id, AGENT_KILL_TIMEOUT).await {
            return Ok(());
        }
        self.shells.kill(id).await
    }

    pub(crate) async fn kill_all(&self) {
        tokio::join!(
            self.agents.stop_all(AGENT_STOP_TIMEOUT),
            self.shells.kill_all()
        );
    }

    /// An agent job whose run has read web content.
    pub(crate) fn read_web(&self, id: &JobId) -> bool {
        self.agents.read_web(id)
    }

    pub(crate) fn kind(&self, id: &JobId) -> JobKind {
        if self.agents.contains(id) {
            JobKind::Agent
        } else {
            JobKind::Shell
        }
    }

    pub(crate) fn info(&self, id: &JobId) -> Option<JobInfo> {
        self.agents
            .info(id)
            .or_else(|| self.shells.snapshot(id).as_ref().map(info_of))
    }

    pub(crate) fn list(&self) -> Vec<JobInfo> {
        let mut jobs: Vec<JobInfo> = self.shells.list().iter().map(info_of).collect();
        jobs.extend(self.agents.list());
        jobs.sort_by_key(|job| job.started_at);
        jobs
    }

    fn finished(&self, info: JobInfo) {
        self.reminders
            .push(&info.owner, hyperform_context::job_finished(&info));
        self.events.emit(Event::JobUpdated { job: info });
    }

    fn on_event(&self, event: JobEvent) {
        match event {
            JobEvent::Output { id, .. } => {
                let now = Instant::now();
                let due = {
                    let mut last = lock(&self.last_update);
                    let due = last
                        .get(&id)
                        .is_none_or(|at| now.duration_since(*at) >= OUTPUT_UPDATE_INTERVAL);
                    if due {
                        last.insert(id.clone(), now);
                    }
                    due
                };
                if let Some(info) = due.then(|| self.info(&id)).flatten() {
                    self.events.emit(Event::JobUpdated { job: info });
                }
            }
            JobEvent::Exited { id, .. } => {
                lock(&self.last_update).remove(&id);
                if let Some(info) = self.info(&id) {
                    self.finished(info);
                }
            }
        }
    }
}

fn info_of(snapshot: &JobSnapshot) -> JobInfo {
    JobInfo {
        job_id: snapshot.id.clone(),
        kind: JobKind::Shell,
        label: snapshot.label.clone(),
        owner: AgentId::from(snapshot.owner.as_str()),
        agent_id: None,
        status: snapshot.status,
        exit_code: snapshot.exit_code,
        started_at: snapshot.started_at,
        finished_at: snapshot.finished_at,
        output_tail: snapshot.tail.clone(),
    }
}
