//! Starting a subagent for an `Agent` call. Foreground agents run in their
//! own task (so a cancelled caller stops them through their token instead
//! of dropping them mid-round) and their report is awaited; background
//! agents become jobs owned by the caller and return at once. A report
//! from a run that read web content taints the caller's run (a background
//! one when the caller reads it with `JobOutput`). With `task_id` the
//! agent is a work-plan task's helper (see `task_run`).

use std::path::PathBuf;
use std::sync::Arc;

use hyperform_protocol::{AgentStatus, CallId, JobStatus};
use hyperform_tools::{SpawnOutcome, SpawnRequest};

use super::child::{ChildReport, run_child};
use super::launch::{Child, prepare};
use super::report::foreground_outcome;
use super::task_run;
use super::tracker::AgentTracker;
use crate::run::RunContext;
use crate::session::SessionCore;

pub(crate) async fn spawn(
    parent: &RunContext,
    call_id: &CallId,
    req: SpawnRequest,
) -> Result<SpawnOutcome, String> {
    let max_depth = parent.core.settings().settings.agents.max_depth;
    if parent.spec.depth >= max_depth {
        return Err(format!(
            "subagents may nest at most {max_depth} level(s) deep; do this work yourself"
        ));
    }
    let (req, admission) = task_run::admit(parent, req).await?;
    let child = prepare(parent, call_id, &req).await?;
    if let Some(admission) = admission {
        task_run::bind(parent, call_id, &child, &admission).await?;
    }
    if child.background {
        return Ok(background(parent, child));
    }
    let root: PathBuf = parent.core.root.clone();
    let tracker = Arc::clone(&child.tracker);
    let report = supervised(child).await;
    if tracker.read_web() {
        parent.web.mark();
    }
    foreground_outcome(&report?, &root)
}

fn background(parent: &RunContext, child: Child) -> SpawnOutcome {
    let core = Arc::clone(&child.ctx.core);
    let info = child.tracker.info();
    let job = core.jobs.spawn_agent(
        parent.spec.agent_id.clone(),
        Arc::clone(&child.tracker),
        child.ctx.cancel.clone(),
    );
    let job_id = job.job_id.clone();
    tokio::spawn(async move {
        let status = match supervised(child).await {
            Ok(report) => job_status(report.info.status),
            Err(_) => JobStatus::Failed,
        };
        core.jobs.finish_agent(&job_id, status);
    });
    SpawnOutcome {
        agent_id: info.agent_id.clone(),
        job_id: Some(job.job_id),
        text: format!(
            "Started the {} agent {} in the background. You will be notified when it \
             finishes; read its report with JobOutput.",
            info.agent_type, info.agent_id
        ),
        footer: String::new(),
    }
}

/// Runs the child in its own task; a panic still records the agent as
/// failed.
async fn supervised(child: Child) -> Result<ChildReport, String> {
    let core: Arc<SessionCore> = Arc::clone(&child.ctx.core);
    let tracker: Arc<AgentTracker> = Arc::clone(&child.tracker);
    match tokio::spawn(run_child(child)).await {
        Ok(report) => Ok(report),
        Err(error) => {
            let message = format!("the agent stopped unexpectedly: {error}");
            tracker.finish(&core, |info| {
                info.status = AgentStatus::Failed;
                info.error = Some(message.clone());
            });
            task_run::settle(&core, &tracker.info(), &[], tracker.read_web()).await;
            Err(message)
        }
    }
}

fn job_status(status: AgentStatus) -> JobStatus {
    match status {
        AgentStatus::Completed | AgentStatus::Running | AgentStatus::Waiting => {
            JobStatus::Completed
        }
        AgentStatus::Failed => JobStatus::Failed,
        AgentStatus::Cancelled => JobStatus::Killed,
    }
}
