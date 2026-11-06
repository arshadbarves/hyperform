//! The kernel: a submission either becomes the records of one revision or
//! is refused with every reason. Proposals are checked in order against a
//! scratch copy of the state, so later proposals see earlier ones; derived
//! follow-ups (failure propagation, reopening, an Active plan becoming Done
//! when its last open task closes, the deterministic audit when
//! `Facts::audit` is set) carry the `Kernel` actor.

mod audit_ops;
mod criteria;
mod gate;
mod ledger_ops;
mod plan_ops;
mod propagate;
mod research;
mod run_ops;
mod status;
mod task_ops;

use hyperform_protocol::work::{Actor, TaskOwner, WorkEvent, WorkRecord};

use crate::authority::{self, Role};
use crate::error::{Reason, Rejection};
use crate::facts::Facts;
use crate::proposal::{PlanDraft, Proposal, Submission};
use crate::state::PlanState;

pub(crate) type Emitted = Vec<(Actor, WorkEvent)>;
pub(crate) type Outcome = Result<Emitted, Vec<Reason>>;

pub(crate) struct Ctx<'a> {
    pub actor: &'a Actor,
    pub role: Role,
    /// `role` ignoring web content (see `Role::base`).
    pub base: Role,
    pub facts: &'a Facts,
}

impl Ctx<'_> {
    pub fn emit(&self, event: WorkEvent) -> (Actor, WorkEvent) {
        (self.actor.clone(), event)
    }

    /// The owner an acting agent becomes when it starts a task.
    pub fn owner(&self) -> Option<TaskOwner> {
        match self.actor {
            Actor::Agent {
                session_id,
                agent_id,
            } => Some(TaskOwner {
                session_id: session_id.clone(),
                agent_id: agent_id.clone(),
            }),
            _ => None,
        }
    }
}

pub(crate) fn fail(reason: Reason) -> Outcome {
    Err(vec![reason])
}

/// The records of one revision, or why not. `Ok(vec![])` means the
/// submission changes nothing (no revision).
pub fn decide(
    state: &PlanState,
    submission: &Submission,
    facts: &Facts,
) -> Result<Vec<WorkRecord>, Rejection> {
    let revision = state.revision() + 1;
    let emitted = run(state, submission, facts, revision)?;
    Ok(stamp(state.seq, revision, submission, facts, emitted))
}

/// The records of a new plan: `PlanCreated` plus `submission` (items and
/// tasks, possibly none), all in revision 1.
pub fn create(
    draft: &PlanDraft,
    submission: &Submission,
    facts: &Facts,
) -> Result<Vec<WorkRecord>, Rejection> {
    plan_ops::check_draft(draft, &submission.actor).map_err(Rejection::new)?;
    let first = WorkRecord {
        seq: 1,
        revision: 1,
        at: facts.now,
        actor: submission.actor.clone(),
        cause: submission.cause.clone(),
        event: WorkEvent::PlanCreated {
            plan_id: draft.plan_id.clone(),
            project_root: draft.project_root.clone(),
            goal: draft.goal.trim().to_string(),
            document: draft.document.clone(),
            depth: draft.depth,
            status: draft.status,
        },
    };
    let state = PlanState::created(&first)
        .map_err(|error| Rejection::new(vec![Reason::invalid(error.to_string())]))?;
    let emitted = run(&state, submission, facts, 1)?;
    let mut records = vec![first];
    records.extend(stamp(1, 1, submission, facts, emitted));
    Ok(records)
}

/// The audit record of a refused submission; it keeps the revision.
pub fn rejected(
    state: &PlanState,
    submission: &Submission,
    rejection: &Rejection,
    facts: &Facts,
) -> WorkRecord {
    WorkRecord {
        seq: state.seq + 1,
        revision: state.revision(),
        at: facts.now,
        actor: submission.actor.clone(),
        cause: submission.cause.clone(),
        event: WorkEvent::Rejected {
            reasons: rejection.messages(),
        },
    }
}

fn run(
    state: &PlanState,
    submission: &Submission,
    facts: &Facts,
    revision: u64,
) -> Result<Emitted, Rejection> {
    let ctx = Ctx {
        actor: &submission.actor,
        role: Role::of(&submission.actor, state, facts),
        base: Role::base(&submission.actor, state, facts),
        facts,
    };
    let mut scratch = state.clone();
    let mut emitted = Emitted::new();
    let mut reasons = Vec::new();
    for proposal in &submission.proposals {
        let outcome = authority::admit(&ctx.role, &ctx.base, proposal)
            .map_err(|reason| vec![reason])
            .and_then(|()| research::check(&scratch, &ctx, proposal))
            .and_then(|()| evaluate(&scratch, &ctx, proposal));
        match outcome {
            Ok(events) => {
                for (actor, event) in events {
                    apply(&mut scratch, &actor, &event, revision, submission, facts);
                    emitted.push((actor, event));
                }
            }
            Err(more) => reasons.extend(more),
        }
    }
    if !reasons.is_empty() {
        return Err(Rejection::new(reasons));
    }
    for event in propagate::follow_ups(state, &scratch) {
        apply(
            &mut scratch,
            &Actor::Kernel,
            &event,
            revision,
            submission,
            facts,
        );
        emitted.push((Actor::Kernel, event));
    }
    if let Some(event) = propagate::finished(state, &scratch) {
        apply(
            &mut scratch,
            &Actor::Kernel,
            &event,
            revision,
            submission,
            facts,
        );
        emitted.push((Actor::Kernel, event));
    }
    let audit = facts.audit.as_ref().filter(|_| !emitted.is_empty());
    if let Some(event) = audit.and_then(|scope| audit_ops::follow_up(&scratch, scope)) {
        emitted.push((Actor::Kernel, event));
    }
    Ok(emitted)
}

fn evaluate(state: &PlanState, ctx: &Ctx, proposal: &Proposal) -> Outcome {
    match proposal {
        Proposal::EditPlan { .. }
        | Proposal::SetPlanStatus { .. }
        | Proposal::StopPlan
        | Proposal::SetParallel { .. }
        | Proposal::SetDepth { .. }
        | Proposal::AddItem { .. }
        | Proposal::EditItem { .. }
        | Proposal::SetItemStatus { .. }
        | Proposal::Reconcile { .. }
        | Proposal::LinkCheckpoint { .. } => plan_ops::evaluate(state, ctx, proposal),
        Proposal::SetTaskStatus { .. }
        | Proposal::RetryTask { .. }
        | Proposal::Recheck { .. }
        | Proposal::SetCriterion { .. } => status::evaluate(state, ctx, proposal),
        Proposal::LinkEvidence { .. }
        | Proposal::Record(_)
        | Proposal::SetEntryStatus { .. }
        | Proposal::ConcludeExperiment { .. } => ledger_ops::evaluate(state, ctx, proposal),
        Proposal::DispatchTask { .. }
        | Proposal::Report { .. }
        | Proposal::Integrate { .. }
        | Proposal::RetrySerial { .. }
        | Proposal::InterruptTask { .. }
        | Proposal::DiscardResumeWorktree { .. } => run_ops::evaluate(state, ctx, proposal),
        Proposal::Audit { .. } | Proposal::SetFindingStatus { .. } => {
            audit_ops::evaluate(state, ctx, proposal)
        }
        _ => task_ops::evaluate(state, ctx, proposal),
    }
}

fn apply(
    scratch: &mut PlanState,
    actor: &Actor,
    event: &WorkEvent,
    revision: u64,
    submission: &Submission,
    facts: &Facts,
) {
    scratch.apply(&WorkRecord {
        seq: scratch.seq + 1,
        revision,
        at: facts.now,
        actor: actor.clone(),
        cause: submission.cause.clone(),
        event: event.clone(),
    });
}

fn stamp(
    last_seq: u64,
    revision: u64,
    submission: &Submission,
    facts: &Facts,
    emitted: Emitted,
) -> Vec<WorkRecord> {
    (last_seq + 1..)
        .zip(emitted)
        .map(|(seq, (actor, event))| WorkRecord {
            seq,
            revision,
            at: facts.now,
            actor,
            cause: submission.cause.clone(),
            event,
        })
        .collect()
}

#[cfg(test)]
mod tests;
