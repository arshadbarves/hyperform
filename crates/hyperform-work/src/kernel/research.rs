//! What a turn that read web content may change. What it adds waits for
//! the user (proposed tasks and items, unconfirmed entries) and progress
//! follows the normal gates; edits to what the plan already says are
//! refused so the user makes them in the Plan tab. A task the research
//! itself added stays editable until the user takes it into the plan.

use hyperform_protocol::work::{EntryStatus, PlanStatus, Provenance, TaskPatch, TaskStatus};
use hyperform_protocol::{EntryId, TaskId};

use super::Ctx;
use super::task_ops::changed_fields;
use crate::error::Reason;
use crate::proposal::Proposal;
use crate::state::PlanState;

pub(super) fn check(state: &PlanState, ctx: &Ctx, proposal: &Proposal) -> Result<(), Vec<Reason>> {
    if !ctx.role.is_research() {
        return Ok(());
    }
    let refuse = |what: String| Err(vec![Reason::Untrusted(what)]);
    let editable = |task_id: &TaskId| awaits_user(state, task_id);
    match proposal {
        Proposal::EditPlan { .. } if !drafting(state) => {
            refuse("edit the goal or document of an approved plan".into())
        }
        Proposal::EditTask { task_id, patch }
            if !editable(task_id) && !progress_only(state, task_id, patch) =>
        {
            refuse(format!(
                "edit the title, description, scope or details of task {task_id}"
            ))
        }
        Proposal::MoveTask { task_id, .. } if !editable(task_id) => {
            refuse(format!("move task {task_id}"))
        }
        Proposal::SetCriteria { task_id, .. } if !editable(task_id) => {
            refuse(format!("edit the criteria of task {task_id}"))
        }
        Proposal::AddDependency { task_id, .. } | Proposal::RemoveDependency { task_id, .. }
            if !editable(task_id) =>
        {
            refuse(format!("change the dependencies of task {task_id}"))
        }
        Proposal::SetTaskStatus {
            task_id,
            status: TaskStatus::Cancelled | TaskStatus::Superseded { .. },
        } if !editable(task_id) => refuse(format!("cancel or supersede task {task_id}")),
        Proposal::SetTaskStatus {
            task_id,
            status: TaskStatus::Pending,
        } if proposed(state, task_id) => refuse(format!("accept proposed task {task_id}")),
        Proposal::ConcludeExperiment { entry_id, .. } if confirmed(state, entry_id) => {
            refuse(format!("conclude confirmed experiment {entry_id}"))
        }
        _ => Ok(()),
    }
}

/// The plan waits for the user's approval, which covers what research
/// adds now.
pub(super) fn drafting(state: &PlanState) -> bool {
    matches!(
        state.header.status,
        PlanStatus::Draft | PlanStatus::Replanning
    )
}

/// Unknown tasks pass so the operation reports them.
fn awaits_user(state: &PlanState, task_id: &TaskId) -> bool {
    state.task(task_id).is_none_or(|task| {
        task.provenance == Provenance::Research
            && (task.status == TaskStatus::Proposed || drafting(state))
    })
}

/// The patch changes only what reports progress: the active form and the
/// result.
fn progress_only(state: &PlanState, task_id: &TaskId, patch: &TaskPatch) -> bool {
    let Some(task) = state.task(task_id) else {
        return true;
    };
    let changed = changed_fields(task, patch);
    changed
        == TaskPatch {
            active_form: changed.active_form.clone(),
            result: changed.result.clone(),
            ..TaskPatch::default()
        }
}

fn proposed(state: &PlanState, task_id: &TaskId) -> bool {
    state
        .task(task_id)
        .is_some_and(|task| task.status == TaskStatus::Proposed)
}

fn confirmed(state: &PlanState, entry_id: &EntryId) -> bool {
    state
        .entry(entry_id)
        .is_some_and(|entry| entry.status != EntryStatus::Proposed)
}
