//! Dispatching a task to a parallel helper (`Agent { task_id }`): the task
//! must be Ready and free, its attempt budget not used up, and beside a
//! running task it must be parallel-eligible and get its own worktree.

use hyperform_protocol::work::{Task, TaskOwner, TaskStatus};
use hyperform_protocol::{AgentId, SessionId, TaskId};

use crate::error::Reason;
use crate::facts::{Facts, Limits};
use crate::graph::{Readiness, readiness};
use crate::parallel::{beside_running, describe};
use crate::state::PlanState;

/// Why a task the findings affect cannot start (refusals and packets).
pub const HELD_FOR_REPLAN: &str = "it is held until the plan change is accepted";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchCheck {
    /// Which start this will be (1-based).
    pub attempt: u32,
    /// Another task is running: the helper must work in its own worktree.
    pub worktree_required: bool,
}

/// Whether `task_id` may be dispatched now; `owner` is the helper when it
/// is already known (a task bound to it passes).
pub fn check(
    state: &PlanState,
    task_id: &TaskId,
    owner: Option<&TaskOwner>,
    facts: &Facts,
) -> Result<DispatchCheck, Vec<Reason>> {
    let task = state
        .task(task_id)
        .ok_or_else(|| vec![Reason::UnknownTask(task_id.clone())])?;
    let mut reasons = Vec::new();
    if task.status == TaskStatus::Pending {
        reasons.extend(not_ready(state, task));
    } else {
        reasons.push(Reason::NotReady {
            task_id: task_id.clone(),
            detail: format!("it is {}", task.status.label()),
        });
    }
    if let Some(current) = task
        .owner
        .as_ref()
        .filter(|current| Some(*current) != owner)
    {
        reasons.push(Reason::Owned {
            task_id: task_id.clone(),
            owner: current.agent_id.to_string(),
        });
    }
    reasons.extend(budget(task, &facts.limits));
    reasons.extend(parallel_reason(state, task, facts));
    if !reasons.is_empty() {
        return Err(reasons);
    }
    Ok(DispatchCheck {
        attempt: task.attempts + 1,
        worktree_required: others_running(state, task_id),
    })
}

/// Starts that count against `max_attempts`.
pub fn attempts_used(task: &Task) -> u32 {
    task.attempts.saturating_sub(task.run.forgiven_attempts)
}

/// Starts left before only the user can retry; `None` when unlimited.
pub fn attempts_left(task: &Task, limits: &Limits) -> Option<u32> {
    limits
        .max_attempts
        .map(|max| max.saturating_sub(attempts_used(task)))
}

/// The attempt budget is used up.
pub(crate) fn budget(task: &Task, limits: &Limits) -> Option<Reason> {
    let max = limits.max_attempts?;
    let attempts = attempts_used(task);
    (attempts >= max).then(|| Reason::AttemptsUsed {
        task_id: task.id.clone(),
        attempts,
        max,
    })
}

/// A Pending task that is not Ready, in words.
pub(crate) fn not_ready(state: &PlanState, task: &Task) -> Option<Reason> {
    let detail = match readiness(state, &task.id)? {
        Readiness::Ready | Readiness::NotPending => return None,
        Readiness::Waiting { on } => {
            let on: Vec<&str> = on.iter().map(TaskId::as_str).collect();
            format!("it waits on {}", on.join(", "))
        }
        Readiness::Held => "the user holds it".into(),
        Readiness::ReplanHeld => HELD_FOR_REPLAN.into(),
        Readiness::PlanNotActive(status) => format!("the plan is {}", status.label()),
    };
    Some(Reason::NotReady {
        task_id: task.id.clone(),
        detail,
    })
}

/// `Agent { task_id, resume }`: only the agent the task's resume pointer
/// names continues it, in the chat it ran in; a fresh helper (no `resume`)
/// may always start it instead and discards the pointer.
pub fn resume_check(task: &Task, session: &SessionId, agent: &AgentId) -> Result<(), Reason> {
    let detail = match &task.resume {
        Some(pointer) if pointer.agent_id == *agent && pointer.session_id == *session => {
            return Ok(());
        }
        Some(pointer) if pointer.agent_id == *agent => {
            "its helper ran in another chat; start a new helper with task_id alone".to_string()
        }
        Some(pointer) => format!(
            "only agent {} (its resume pointer) continues it; start a new helper with task_id alone",
            pointer.agent_id
        ),
        None => "it has no resume pointer; start a new helper with task_id alone".to_string(),
    };
    Err(Reason::NotReady {
        task_id: task.id.clone(),
        detail,
    })
}

/// Starting `task` beside the running tasks is not allowed.
pub(crate) fn parallel_reason(state: &PlanState, task: &Task, facts: &Facts) -> Option<Reason> {
    beside_running(state, task, facts).map(|block| Reason::Parallel {
        task_id: task.id.clone(),
        detail: describe(&block),
    })
}

fn others_running(state: &PlanState, task_id: &TaskId) -> bool {
    state
        .tasks
        .values()
        .any(|task| task.id != *task_id && task.status == TaskStatus::Active)
}

#[cfg(test)]
#[path = "dispatch_tests.rs"]
mod tests;
