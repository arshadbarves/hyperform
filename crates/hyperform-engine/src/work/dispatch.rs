//! Dispatching a work-plan task to a helper (`Agent { task_id }`): the
//! kernel's dispatch check, the parallel gate (more than one task-bound
//! helper needs `work_parallel` On and the plan's parallel work approved,
//! and at most `max_parallel_tasks` run, beside the agents' own slots),
//! the helper's first message with its packet and the result contract,
//! and the binding once the helper has its place. Resuming an interrupted
//! task's helper (`Agent { task_id, resume }`) is for the agent its resume
//! pointer names, in the chat it ran in. Admissions of one plan take turns
//! from the first check to the binding, so helpers started in one batch
//! are cleared in call order, each seeing the ones before it.

use std::sync::Arc;

use hyperform_config::FeatureId;
use hyperform_context::{estimate_text, helper_brief};
use hyperform_protocol::work::ParallelPolicy;
use hyperform_protocol::{AgentId, CallId, Event, TaskId};
use hyperform_work::{dispatch, select};
use tokio::sync::OwnedMutexGuard;
use tokio_util::sync::CancellationToken;

use super::bound::Bound;
use super::facts::{OFF, current, enabled, task_id};
use super::helper::bind;
use super::lifecycle::attached;
use super::plan::PlanHandle;
use crate::orchestration::AgentTracker;
use crate::session::SessionCore;

/// A task cleared for a helper that has not started yet.
#[derive(Debug)]
pub(crate) struct Admission {
    plan: Arc<PlanHandle>,
    pub task: TaskId,
    /// Another task runs: the helper needs its own worktree.
    pub worktree_required: bool,
    /// The helper gets its own worktree when one can be made: beside
    /// another task, and whenever the plan's parallel work is approved (so
    /// tasks dispatched together each get one).
    pub worktree_wanted: bool,
    /// The helper's first message.
    pub brief: String,
    tokens: u64,
    /// The plan's admission turn, released when this is dropped (after
    /// the binding).
    _turn: OwnedMutexGuard<()>,
}

/// Whether `caller` may dispatch the task `raw` now (to the agent `resume`
/// names, when continuing one), with the helper's first message built
/// from `prompt`.
pub(crate) async fn admit(
    core: &SessionCore,
    caller: &AgentId,
    raw: &str,
    prompt: &str,
    resume: Option<&AgentId>,
) -> Result<Admission, String> {
    if !enabled(core) {
        return Err(OFF.into());
    }
    let plan =
        attached(core).ok_or("no work plan is attached to this chat; TaskCreate starts one")?;
    if !caller.is_main() {
        return Err("only the main agent dispatches plan tasks; work on your own task".into());
    }
    let task = task_id(raw);
    let turn = plan.admitting().await;
    let settings = core.settings();
    let parallel = settings.settings.feature(FeatureId::WorkParallel).acts()
        && plan.read(|state| state.header.parallel == ParallelPolicy::Approved);
    let busy = running_helpers(core);
    if !busy.is_empty() {
        let list = busy
            .iter()
            .map(TaskId::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        if !parallel {
            return Err(format!(
                "a helper already works on {list}; more than one task-bound helper at a time needs work_parallel on and the plan's parallel work approved in the Plan tab, so wait for it to finish"
            ));
        }
        let max = settings.settings.max_parallel_tasks() as usize;
        if busy.len() >= max {
            return Err(format!(
                "helpers already work on {list}, the most allowed at once (work.max_parallel {max}); wait for one to finish"
            ));
        }
    }
    let facts = current(core, &core.root).await;
    let budget = u64::from(settings.settings.work.packet_tokens);
    let cleared = plan.read(|state| {
        let check = dispatch::check(state, &task, None, &facts).map_err(|reasons| {
            let lines: Vec<String> = reasons.iter().map(|reason| format!("- {reason}")).collect();
            format!("Task {task} cannot be dispatched:\n{}", lines.join("\n"))
        })?;
        if let (Some(agent), Some(found)) = (resume, state.task(&task)) {
            dispatch::resume_check(found, &core.id, agent)
                .map_err(|reason| format!("Task {task} cannot be resumed: {reason}"))?;
        }
        let packet = select(state, &task, budget, &facts, &estimate_text)
            .map_err(|error| error.to_string())?;
        Ok::<_, String>((check, packet))
    });
    let (check, packet) = cleared?;
    Ok(Admission {
        plan,
        worktree_required: check.worktree_required,
        worktree_wanted: check.worktree_required || parallel,
        brief: helper_brief(&packet, prompt),
        tokens: packet.tokens,
        task,
        _turn: turn,
    })
}

impl Admission {
    /// Binds the task to `helper`, which `caller` started in `worktree`;
    /// the packet in its first message then counts as sent.
    pub(crate) fn bind(
        &self,
        core: &SessionCore,
        caller: &AgentId,
        call_id: &CallId,
        helper: &AgentId,
        worktree: Option<String>,
    ) -> Result<(), String> {
        bind(
            core, &self.plan, caller, call_id, &self.task, helper, worktree,
        )?;
        core.events.emit(Event::TaskPacketSent {
            agent_id: helper.clone(),
            task_id: self.task.clone(),
            tokens: self.tokens,
            reason: "dispatch".to_string(),
        });
        Ok(())
    }

    /// Keeps the bound helper stoppable until its run ends.
    pub(crate) fn watch(
        &self,
        core: &SessionCore,
        helper: &AgentId,
        tracker: Arc<AgentTracker>,
        cancel: CancellationToken,
    ) {
        core.work.helpers().add(Bound {
            agent: helper.clone(),
            task: self.task.clone(),
            plan: Arc::clone(&self.plan),
            tracker,
            cancel,
            halt: None,
        });
    }
}

/// The tasks of helpers that still run.
fn running_helpers(core: &SessionCore) -> Vec<TaskId> {
    core.with_state(|state| {
        let running = state
            .agents
            .iter()
            .filter(|info| !info.status.is_terminal());
        running.filter_map(|info| info.task_id.clone()).collect()
    })
}
