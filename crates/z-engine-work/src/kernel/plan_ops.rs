//! Plan-level proposals: the goal and document, status, parallel policy,
//! requirements and invariants, reconciliation and checkpoints.

use z_engine_protocol::EntryId;
use z_engine_protocol::work::{
    Actor, PlanItem, PlanItemKind, PlanItemStatus, PlanStatus, Provenance, ReconcileFinding,
    TaskStatus, WorkEvent,
};

use super::{Ctx, Outcome, fail};
use crate::error::Reason;
use crate::graph::is_closed;
use crate::proposal::{PlanDraft, Proposal};
use crate::state::PlanState;
use crate::transitions::{plan_move_allowed, plan_move_needs_user};

pub(super) fn evaluate(state: &PlanState, ctx: &Ctx, proposal: &Proposal) -> Outcome {
    match proposal {
        Proposal::EditPlan { goal, document } => edit_plan(state, ctx, goal, document),
        Proposal::SetPlanStatus { status, only_from } => set_status(state, ctx, *status, only_from),
        Proposal::StopPlan => stop(state, ctx),
        Proposal::SetParallel { parallel } if *parallel != state.header.parallel => {
            Ok(vec![ctx.emit(WorkEvent::ParallelChanged {
                parallel: *parallel,
            })])
        }
        Proposal::SetDepth { depth } if *depth != state.header.depth => {
            Ok(vec![ctx.emit(WorkEvent::DepthChanged { depth: *depth })])
        }
        Proposal::AddItem { id, kind, text } => add_item(state, ctx, id.clone(), *kind, text),
        Proposal::EditItem { item_id, text } => edit_item(state, ctx, item_id, text),
        Proposal::SetItemStatus { item_id, status } => {
            set_item_status(state, ctx, item_id, *status)
        }
        Proposal::Reconcile { findings } => reconcile(state, ctx, findings),
        Proposal::LinkCheckpoint {
            fingerprint,
            checkpoint,
        } => {
            if state.fingerprint.as_deref() == Some(fingerprint) && checkpoint.is_none() {
                return Ok(vec![]);
            }
            Ok(vec![ctx.emit(WorkEvent::CheckpointLinked {
                fingerprint: fingerprint.clone(),
                checkpoint: checkpoint.clone(),
            })])
        }
        _ => Ok(vec![]),
    }
}

/// Only the user, the main agent (even after reading web content) or the
/// engine creates plans.
pub(super) fn check_draft(draft: &PlanDraft, actor: &Actor) -> Result<(), Vec<Reason>> {
    let mut reasons = Vec::new();
    match actor {
        Actor::User | Actor::Kernel => {}
        Actor::Agent { agent_id, .. } if agent_id.is_main() => {}
        Actor::Agent { .. } => reasons.push(Reason::WorkerScope {
            bound: None,
            detail: "only the main agent creates plans".into(),
        }),
        Actor::Compaction => reasons.push(Reason::Compaction("a plan".into())),
    }
    if draft.goal.trim().is_empty() {
        reasons.push(Reason::invalid("a plan needs a goal"));
    }
    if !matches!(draft.status, PlanStatus::Draft | PlanStatus::Active) {
        reasons.push(Reason::invalid("a plan starts as draft or active"));
    }
    if reasons.is_empty() {
        Ok(())
    } else {
        Err(reasons)
    }
}

fn edit_plan(
    state: &PlanState,
    ctx: &Ctx,
    goal: &Option<String>,
    document: &Option<String>,
) -> Outcome {
    let goal = goal.as_deref().map(str::trim);
    if goal == Some("") {
        return fail(Reason::invalid("a plan needs a goal"));
    }
    let goal = goal
        .filter(|goal| *goal != state.header.goal)
        .map(String::from);
    let document = document
        .clone()
        .filter(|document| *document != state.header.document);
    if goal.is_none() && document.is_none() {
        return Ok(vec![]);
    }
    Ok(vec![ctx.emit(WorkEvent::PlanEdited { goal, document })])
}

fn set_status(state: &PlanState, ctx: &Ctx, to: PlanStatus, only_from: &[PlanStatus]) -> Outcome {
    let from = state.header.status;
    if from == to {
        return Ok(vec![]);
    }
    if (!only_from.is_empty() && !only_from.contains(&from)) || !plan_move_allowed(from, to) {
        return fail(Reason::PlanTransition { from, to });
    }
    if plan_move_needs_user(to) {
        ctx.role
            .require_user("approve, resume, pause or abandon the plan")
            .map_err(|reason| vec![reason])?;
    }
    if to == PlanStatus::Done {
        let open: Vec<_> = state
            .tasks
            .values()
            .filter(|task| !is_closed(&task.status))
            .map(|task| task.id.clone())
            .collect();
        if !open.is_empty() {
            return fail(Reason::OpenTasks { tasks: open });
        }
    }
    Ok(vec![ctx.emit(WorkEvent::PlanStatusChanged { from, to })])
}

fn stop(state: &PlanState, ctx: &Ctx) -> Outcome {
    let from = state.header.status;
    let mut out = Vec::new();
    if from != PlanStatus::Paused {
        if !plan_move_allowed(from, PlanStatus::Paused) {
            return fail(Reason::PlanTransition {
                from,
                to: PlanStatus::Paused,
            });
        }
        out.push(ctx.emit(WorkEvent::PlanStatusChanged {
            from,
            to: PlanStatus::Paused,
        }));
    }
    for task in state.tasks.values().filter(|task| task.owner.is_some()) {
        if task.status == TaskStatus::Active {
            out.push(ctx.emit(WorkEvent::TaskTransitioned {
                task_id: task.id.clone(),
                from: TaskStatus::Active,
                to: TaskStatus::Pending,
            }));
        }
        out.push(ctx.emit(WorkEvent::TaskReleased {
            task_id: task.id.clone(),
        }));
    }
    Ok(out)
}

fn add_item(
    state: &PlanState,
    ctx: &Ctx,
    id: Option<EntryId>,
    kind: PlanItemKind,
    text: &str,
) -> Outcome {
    let text = text.trim();
    if text.is_empty() {
        return fail(Reason::invalid("a requirement or invariant needs text"));
    }
    let id = id.unwrap_or_default();
    if state.item(&id).is_some() || state.entry(&id).is_some() {
        return fail(Reason::DuplicateId(id.to_string()));
    }
    let status = if ctx.role.is_user() {
        PlanItemStatus::Active
    } else {
        PlanItemStatus::Proposed
    };
    let item = PlanItem {
        id,
        kind,
        text: text.to_string(),
        provenance: ctx.role.provenance(),
        status,
        revision: 0,
    };
    Ok(vec![ctx.emit(WorkEvent::ItemAdded { item })])
}

fn edit_item(state: &PlanState, ctx: &Ctx, item_id: &EntryId, text: &str) -> Outcome {
    let Some(item) = state.item(item_id) else {
        return fail(Reason::UnknownItem(item_id.clone()));
    };
    let text = text.trim();
    if text.is_empty() {
        return fail(Reason::invalid("a requirement or invariant needs text"));
    }
    if item.provenance == Provenance::User {
        ctx.role
            .require_user("reword a requirement or invariant the user set")
            .map_err(|reason| vec![reason])?;
    }
    if item.text == text {
        return Ok(vec![]);
    }
    Ok(vec![ctx.emit(WorkEvent::ItemEdited {
        item_id: item_id.clone(),
        text: text.to_string(),
    })])
}

fn set_item_status(
    state: &PlanState,
    ctx: &Ctx,
    item_id: &EntryId,
    status: PlanItemStatus,
) -> Outcome {
    let Some(item) = state.item(item_id) else {
        return fail(Reason::UnknownItem(item_id.clone()));
    };
    if item.status == status {
        return Ok(vec![]);
    }
    let check = match status {
        PlanItemStatus::Proposed => Err(Reason::invalid("an item cannot go back to proposed")),
        PlanItemStatus::Active => ctx
            .role
            .require_user("accept or restore a requirement or invariant"),
        PlanItemStatus::Retired if item.provenance == Provenance::User => ctx
            .role
            .require_user("retire a requirement or invariant the user set"),
        PlanItemStatus::Retired => Ok(()),
    };
    check.map_err(|reason| vec![reason])?;
    Ok(vec![ctx.emit(WorkEvent::ItemStatusChanged {
        item_id: item_id.clone(),
        status,
    })])
}

fn reconcile(state: &PlanState, ctx: &Ctx, findings: &[ReconcileFinding]) -> Outcome {
    if findings.is_empty() && state.findings.is_empty() {
        return Ok(vec![]);
    }
    let mut out = vec![ctx.emit(WorkEvent::Reconciled {
        findings: findings.to_vec(),
    })];
    if !findings.is_empty() && state.header.status == PlanStatus::Active {
        out.push(ctx.emit(WorkEvent::PlanStatusChanged {
            from: PlanStatus::Active,
            to: PlanStatus::Replanning,
        }));
    }
    Ok(out)
}
