//! Task status changes and criterion status changes: starting needs a
//! Ready task and a free owner slot, completing passes the gate, and
//! leaving work releases the owner.

use hyperform_protocol::TaskId;
use hyperform_protocol::work::{
    Actor, CriterionMethod, CriterionStatus, Integration, Provenance, Task, TaskStatus, WorkEvent,
};

use super::task_ops::find;
use super::{Ctx, Emitted, Outcome, fail, gate, propagate, run_ops};
use crate::authority::Role;
use crate::dispatch;
use crate::error::Reason;
use crate::proposal::Proposal;
use crate::state::PlanState;
use crate::transitions::task_move_allowed;

pub(super) fn evaluate(state: &PlanState, ctx: &Ctx, proposal: &Proposal) -> Outcome {
    match proposal {
        Proposal::SetTaskStatus { task_id, status } => set_status(state, ctx, task_id, status),
        Proposal::RetryTask { task_id } => run_ops::retry(state, ctx, task_id, false),
        Proposal::Recheck { task_id } => match find(state, task_id)?.status {
            TaskStatus::Verifying => Ok(vec![]),
            _ => fail(Reason::invalid(format!(
                "task {task_id} is not verifying; only a Verifying task's checks run again"
            ))),
        },
        Proposal::SetCriterion {
            task_id,
            criterion_id,
            status,
        } => set_criterion(state, ctx, task_id, criterion_id, *status),
        _ => Ok(vec![]),
    }
}

fn set_status(state: &PlanState, ctx: &Ctx, task_id: &TaskId, to: &TaskStatus) -> Outcome {
    let task = find(state, task_id)?;
    let from = &task.status;
    if from == to {
        return Ok(vec![]);
    }
    if !task_move_allowed(from, to) {
        return fail(Reason::Transition {
            task_id: task_id.clone(),
            from: from.clone(),
            to: to.clone(),
        });
    }
    let mut out = Emitted::new();
    match to {
        TaskStatus::Active => {
            check_start(state, ctx, task)?;
            out.push(transition(ctx, task, to.clone()));
            if let Some(owner) = ctx
                .owner()
                .filter(|owner| task.owner.as_ref() != Some(owner))
            {
                out.push(ctx.emit(WorkEvent::TaskBound {
                    task_id: task_id.clone(),
                    owner,
                }));
            }
            return Ok(out);
        }
        TaskStatus::Done => {
            if matches!(ctx.base, Role::Worker { .. }) && awaits_integration(task) {
                return fail(Reason::invalid(format!(
                    "task {task_id}'s change is still in your worktree: send your report with TaskUpdate `report` and end your run; the engine integrates it"
                )));
            }
            let met = gate::completion(state, task, ctx.facts)?;
            out.extend(met.into_iter().map(|event| (Actor::Kernel, event)));
        }
        TaskStatus::Superseded { by } => {
            if by == task_id {
                return fail(Reason::invalid("a task cannot supersede itself"));
            }
            find(state, by)?;
        }
        TaskStatus::Pending if *from == TaskStatus::Proposed => {
            if matches!(ctx.role, Role::Worker { .. }) {
                return fail(Reason::WorkerScope {
                    bound: None,
                    detail: "accepting proposed tasks belongs to the coordinator".into(),
                });
            }
            if task.provenance == Provenance::Research {
                ctx.role
                    .require_user("accept a task proposed from web content")
                    .map_err(|reason| vec![reason])?;
            }
        }
        _ => {}
    }
    out.push(transition(ctx, task, to.clone()));
    if task.owner.is_some() && releases(to) {
        out.push(ctx.emit(WorkEvent::TaskReleased {
            task_id: task_id.clone(),
        }));
    }
    Ok(out)
}

/// Pending -> Active needs a Ready task; any start needs the owner slot
/// free or held by the same agent, and an agent's start needs attempts
/// left. A helper starting beside a running task must be parallel-eligible.
fn check_start(state: &PlanState, ctx: &Ctx, task: &Task) -> Result<(), Vec<Reason>> {
    if task.status == TaskStatus::Pending
        && let Some(reason) = dispatch::not_ready(state, task)
    {
        return Err(vec![reason]);
    }
    let agent_limits = (!ctx.role.is_user())
        .then(|| dispatch::budget(task, &ctx.facts.limits))
        .flatten();
    let helper = matches!(ctx.base, Role::Worker { .. })
        .then(|| dispatch::parallel_reason(state, task, ctx.facts))
        .flatten();
    if let Some(reason) = agent_limits.or(helper) {
        return Err(vec![reason]);
    }
    match (&task.owner, ctx.owner()) {
        (Some(current), Some(actor)) if *current != actor => Err(vec![Reason::Owned {
            task_id: task.id.clone(),
            owner: current.agent_id.to_string(),
        }]),
        _ => Ok(()),
    }
}

fn set_criterion(
    state: &PlanState,
    ctx: &Ctx,
    task_id: &TaskId,
    criterion_id: &str,
    status: CriterionStatus,
) -> Outcome {
    let task = find(state, task_id)?;
    let Some(criterion) = task.acceptance.iter().find(|c| c.id == criterion_id) else {
        return fail(Reason::UnknownCriterion {
            task_id: task_id.clone(),
            criterion_id: criterion_id.to_string(),
        });
    };
    if criterion.status == status {
        return Ok(vec![]);
    }
    let check = match (status, criterion.method) {
        (CriterionStatus::Waived, _) => ctx.role.require_user("waive a criterion"),
        (_, _) if criterion.status == CriterionStatus::Waived => {
            ctx.role.require_user("take back a waived criterion")
        }
        (CriterionStatus::Met, CriterionMethod::User) => {
            ctx.role.require_user("confirm a criterion the user checks")
        }
        (CriterionStatus::Met, CriterionMethod::Check { .. }) if ctx.role != Role::Kernel => Err(
            Reason::EngineOnly("meeting a check criterion (a linked passing check)".into()),
        ),
        _ => Ok(()),
    };
    check.map_err(|reason| vec![reason])?;
    let mut out: Emitted = vec![ctx.emit(WorkEvent::CriterionChanged {
        task_id: task_id.clone(),
        criterion_id: criterion_id.to_string(),
        status,
    })];
    let weakened = matches!(status, CriterionStatus::Pending | CriterionStatus::Failed);
    if task.status == TaskStatus::Done && weakened {
        out.extend(propagate::reopen(task));
    }
    Ok(out)
}

pub(super) fn transition(ctx: &Ctx, task: &Task, to: TaskStatus) -> (Actor, WorkEvent) {
    ctx.emit(WorkEvent::TaskTransitioned {
        task_id: task.id.clone(),
        from: task.status.clone(),
        to,
    })
}

/// The task was dispatched into a worktree whose change is not applied.
fn awaits_integration(task: &Task) -> bool {
    let in_worktree = task
        .run
        .dispatch
        .as_ref()
        .is_some_and(|dispatch| dispatch.worktree.is_some());
    in_worktree && !matches!(task.run.integration, Some(Integration::Applied { .. }))
}

/// Leaving active work frees the owner slot.
fn releases(to: &TaskStatus) -> bool {
    matches!(
        to,
        TaskStatus::Pending
            | TaskStatus::Done
            | TaskStatus::Failed { .. }
            | TaskStatus::Cancelled
            | TaskStatus::Superseded { .. }
    )
}
