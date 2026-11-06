//! The coordinator's side of parallel work: when a helper ends, is
//! interrupted or its change comes back, the main agent is reminded with
//! the report, the tasks that became ready, what may run in parallel now
//! and what reconciliation findings hold, and the plan is reconciled with
//! the workspace.

use z_engine_context::{HelperNews, work_coordinate};
use z_engine_protocol::work::Task;
use z_engine_protocol::{AgentId, TaskId};
use z_engine_work::{PlanState, graph, parallel, replan_held};

use super::facts::instant;
use super::lifecycle::attached;
use super::reconcile::{Trigger, reconcile};
use crate::session::SessionCore;

/// What happened to a helper's task, for the coordinator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum News {
    Ended,
    Unreported,
    WaitsForApply,
    Conflict,
    Discarded,
    Verified,
    CheckFailed,
    StillVerifying,
    /// Paused or held before it finished.
    Interrupted,
}

impl News {
    fn phrase(self) -> &'static str {
        match self {
            Self::Ended => "it ended and reported",
            Self::Unreported => "it ended without a report, so the task is blocked",
            Self::WaitsForApply => "it reported, and its change waits for the user to apply it",
            Self::Conflict => {
                "its change conflicts with the project tree; the tree is untouched and the worktree is kept"
            }
            Self::Discarded => "the user discarded its change",
            Self::Verified => "its change passed the task's checks in the project tree",
            Self::CheckFailed => "its change failed the task's checks in the project tree",
            Self::StillVerifying => {
                "its change is in the project tree; the task waits for its remaining criteria"
            }
            Self::Interrupted => {
                "it was stopped before it finished; the task waits with a resume pointer to that helper"
            }
        }
    }
}

/// Reminds the main agent about `helper`'s task and reconciles the plan.
pub(crate) async fn tell_coordinator(
    core: &SessionCore,
    helper: &AgentId,
    task: &TaskId,
    news: News,
) {
    let Some(plan) = attached(core) else {
        return;
    };
    let facts = instant(core);
    let text = plan.read(|state| {
        let task = state.task(task)?;
        let ready = newly_ready(state, &task.id);
        let set = parallel::eligible(state, &facts);
        let parallel: Vec<&Task> = set
            .eligible
            .iter()
            .filter_map(|id| state.task(id))
            .collect();
        let held: Vec<&Task> = replan_held(state)
            .iter()
            .filter_map(|id| state.task(id))
            .collect();
        Some(work_coordinate(&HelperNews {
            agent: helper.as_str(),
            task,
            event: news.phrase(),
            ready: &ready,
            parallel: &parallel,
            held: &held,
        }))
    });
    if let Some(text) = text {
        core.reminders.push(&AgentId::main(), text);
    }
    reconcile(core, &plan, Trigger::Helper).await;
}

/// Ready tasks that wait on `task`.
fn newly_ready<'a>(state: &'a PlanState, task: &TaskId) -> Vec<&'a Task> {
    let ready = graph::ready_tasks(state);
    graph::dependents(state, task)
        .into_iter()
        .filter(|id| ready.contains(id))
        .filter_map(|id| state.task(&id))
        .collect()
}
