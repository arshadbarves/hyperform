//! A task's run in a parallel helper: dispatching it, the helper's report
//! (or its absence), how the worktree change came back, an interruption
//! (and discarding the worktree it kept), and retries.

use hyperform_protocol::work::{
    Actor, BlockReason, Integration, ReportDraft, ResumePointer, TaskDispatch, TaskOwner,
    TaskPatch, TaskStatus, WorkEvent,
};
use hyperform_protocol::{AgentId, TaskId};

use super::status::transition;
use super::task_ops::find;
use super::{Ctx, Outcome, fail};
use crate::authority::Role;
use crate::dispatch;
use crate::error::Reason;
use crate::proposal::Proposal;
use crate::state::PlanState;

pub(super) fn evaluate(state: &PlanState, ctx: &Ctx, proposal: &Proposal) -> Outcome {
    match proposal {
        Proposal::DispatchTask { task_id, dispatch } => {
            dispatch_task(state, ctx, task_id, dispatch)
        }
        Proposal::Report { task_id, report } => reported(state, ctx, task_id, report.as_deref()),
        Proposal::Integrate {
            task_id,
            integration,
        } => integrate(state, ctx, task_id, integration),
        Proposal::RetrySerial { task_id } => retry(state, ctx, task_id, true),
        Proposal::InterruptTask { task_id, resume } => interrupt(state, ctx, task_id, resume),
        Proposal::DiscardResumeWorktree { task_id, agent_id } => {
            discard_kept(state, ctx, task_id, agent_id)
        }
        _ => Ok(vec![]),
    }
}

/// The worktree `agent` left for the task's resume pointer was discarded:
/// the pointer keeps its agent and note without it. A pointer that names
/// another agent, or no worktree, is left as it is.
fn discard_kept(state: &PlanState, ctx: &Ctx, task_id: &TaskId, agent: &AgentId) -> Outcome {
    if ctx.role != Role::Kernel {
        return fail(Reason::EngineOnly(
            "dropping a resume pointer's worktree".into(),
        ));
    }
    let task = find(state, task_id)?;
    let kept = task
        .resume
        .as_ref()
        .is_some_and(|resume| resume.agent_id == *agent && resume.worktree.is_some());
    if !kept {
        return Ok(vec![]);
    }
    Ok(vec![ctx.emit(WorkEvent::ResumeWorktreeDiscarded {
        task_id: task_id.clone(),
    })])
}

/// The task's helper (or another chat's main agent) stopped before it
/// finished: back to Pending, released, with a resume pointer. A task that
/// no longer runs for that agent is left as it is.
fn interrupt(state: &PlanState, ctx: &Ctx, task_id: &TaskId, resume: &ResumePointer) -> Outcome {
    if ctx.role != Role::Kernel {
        return fail(Reason::EngineOnly("interrupting a task's helper".into()));
    }
    let task = find(state, task_id)?;
    let owner = TaskOwner {
        session_id: resume.session_id.clone(),
        agent_id: resume.agent_id.clone(),
    };
    if task.status != TaskStatus::Active || task.owner.as_ref() != Some(&owner) {
        return Ok(vec![]);
    }
    Ok(vec![
        transition(ctx, task, TaskStatus::Pending),
        ctx.emit(WorkEvent::TaskReleased {
            task_id: task_id.clone(),
        }),
        ctx.emit(WorkEvent::TaskInterrupted {
            task_id: task_id.clone(),
            resume: resume.clone(),
        }),
    ])
}

/// Failed or Blocked back to Pending for a new run, releasing the owner.
/// An agent's retry needs attempts left; the user's resets the budget.
pub(super) fn retry(state: &PlanState, ctx: &Ctx, task_id: &TaskId, serial: bool) -> Outcome {
    let task = find(state, task_id)?;
    if !matches!(
        task.status,
        TaskStatus::Failed { .. } | TaskStatus::Blocked { .. }
    ) {
        return fail(Reason::Transition {
            task_id: task_id.clone(),
            from: task.status.clone(),
            to: TaskStatus::Pending,
        });
    }
    let fresh = ctx.role.is_user();
    if !fresh && let Some(reason) = dispatch::budget(task, &ctx.facts.limits) {
        return fail(reason);
    }
    let mut out = vec![
        transition(ctx, task, TaskStatus::Pending),
        ctx.emit(WorkEvent::TaskRetried {
            task_id: task_id.clone(),
            serial,
            fresh_attempts: fresh,
        }),
    ];
    if task.owner.is_some() {
        out.push(ctx.emit(WorkEvent::TaskReleased {
            task_id: task_id.clone(),
        }));
    }
    Ok(out)
}

fn dispatch_task(
    state: &PlanState,
    ctx: &Ctx,
    task_id: &TaskId,
    dispatch: &TaskDispatch,
) -> Outcome {
    let owner = TaskOwner {
        session_id: dispatch.session_id.clone(),
        agent_id: dispatch.agent_id.clone(),
    };
    if owner.agent_id.is_main() {
        return fail(Reason::invalid(
            "a task is dispatched to a helper, not to the main agent",
        ));
    }
    let check = dispatch::check(state, task_id, Some(&owner), ctx.facts)?;
    let worktree = dispatch.worktree.as_deref().map(str::trim);
    if check.worktree_required && worktree.is_none_or(str::is_empty) {
        return fail(Reason::Parallel {
            task_id: task_id.clone(),
            detail: "another task is running, so the helper needs its own worktree".into(),
        });
    }
    let task = find(state, task_id)?;
    let mut out = vec![
        ctx.emit(WorkEvent::TaskDispatched {
            task_id: task_id.clone(),
            dispatch: TaskDispatch {
                worktree: worktree.filter(|w| !w.is_empty()).map(String::from),
                attempt: check.attempt,
                at: ctx.facts.now,
                ..dispatch.clone()
            },
        }),
        transition(ctx, task, TaskStatus::Active),
    ];
    if task.owner.as_ref() != Some(&owner) {
        out.push(ctx.emit(WorkEvent::TaskBound {
            task_id: task_id.clone(),
            owner,
        }));
    }
    Ok(out)
}

fn reported(
    state: &PlanState,
    ctx: &Ctx,
    task_id: &TaskId,
    draft: Option<&ReportDraft>,
) -> Outcome {
    let task = find(state, task_id)?;
    if draft.is_none() && ctx.role != Role::Kernel {
        return fail(Reason::EngineOnly("recording a missing report".into()));
    }
    let running = match draft {
        Some(_) => matches!(task.status, TaskStatus::Active | TaskStatus::Verifying),
        None => task.status == TaskStatus::Active,
    };
    if !running {
        return fail(Reason::invalid(format!(
            "task {task_id} is {}; only running work reports",
            task.status.label()
        )));
    }
    let attempt = task
        .run
        .dispatch
        .as_ref()
        .map_or(task.attempts, |dispatch| dispatch.attempt);
    let mut folded = crate::report::fold(draft, &ctx.facts.run, attempt, ctx.facts.now)
        .map_err(|reason| vec![reason])?;
    folded.provenance = ctx.role.provenance();
    let summary = folded.summary.clone();
    let mut out = vec![ctx.emit(WorkEvent::TaskReported {
        task_id: task_id.clone(),
        report: Box::new(folded),
    })];
    if draft.is_none() {
        let blocked = TaskStatus::Blocked {
            reason: BlockReason::Unreported,
        };
        out.push(transition(ctx, task, blocked));
        out.extend(release(ctx, task_id, task.owner.is_some()));
    } else if task.result.is_none() {
        out.push(ctx.emit(WorkEvent::TaskEdited {
            task_id: task_id.clone(),
            patch: Box::new(TaskPatch {
                result: Some(summary),
                ..TaskPatch::default()
            }),
        }));
    }
    Ok(out)
}

fn integrate(state: &PlanState, ctx: &Ctx, task_id: &TaskId, integration: &Integration) -> Outcome {
    if ctx.role != Role::Kernel {
        return fail(Reason::EngineOnly("integrating a helper's change".into()));
    }
    let task = find(state, task_id)?;
    let conflicted = matches!(
        task.status,
        TaskStatus::Blocked {
            reason: BlockReason::MergeConflict { .. }
        }
    );
    let allowed = match integration {
        Integration::Applied { .. } | Integration::Discarded => {
            task.status == TaskStatus::Active || conflicted
        }
        _ => task.status == TaskStatus::Active,
    };
    if !allowed {
        return fail(Reason::invalid(format!(
            "task {task_id} is {}; only a running task's change is integrated",
            task.status.label()
        )));
    }
    let to = match integration {
        Integration::PendingApply { worktree } if worktree.trim().is_empty() => {
            return fail(Reason::invalid("a pending change needs its worktree"));
        }
        Integration::Conflict { paths, worktree }
            if paths.is_empty() || worktree.trim().is_empty() =>
        {
            return fail(Reason::invalid(
                "a conflict names its paths and the kept worktree",
            ));
        }
        Integration::PendingApply { .. } => None,
        Integration::Applied { .. } => Some(TaskStatus::Verifying),
        Integration::Conflict { paths, .. } => Some(TaskStatus::Blocked {
            reason: BlockReason::MergeConflict {
                detail: format!("applying the change conflicts in {}", paths.join(", ")),
            },
        }),
        Integration::Discarded => Some(TaskStatus::Blocked {
            reason: BlockReason::ChangeDiscarded,
        }),
    };
    let mut out = vec![ctx.emit(WorkEvent::TaskIntegrated {
        task_id: task_id.clone(),
        integration: integration.clone(),
    })];
    if let Some(to) = to {
        out.push(transition(ctx, task, to));
        out.extend(release(ctx, task_id, task.owner.is_some()));
    }
    Ok(out)
}

fn release(ctx: &Ctx, task_id: &TaskId, bound: bool) -> Option<(Actor, WorkEvent)> {
    bound.then(|| {
        ctx.emit(WorkEvent::TaskReleased {
            task_id: task_id.clone(),
        })
    })
}
