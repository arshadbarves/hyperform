//! Reconciliation: the kernel's deterministic checks, run when the
//! workspace changed since the last turn start, after a discovery about
//! open tasks, when a task's helper ends or its change comes back, and on
//! `/replan`. New findings are recorded by the engine,
//! move an Active plan to Replanning with a plan change, and remind the
//! main agent. No findings: nothing is recorded and no model call follows.

use z_engine_context::work_reconcile;
use z_engine_host::workspace_fingerprint;
use z_engine_protocol::work::{
    Actor, ChangeCause, EntryBody, EvidenceRef, PlanChange, PlanStatus, ReconcileFinding,
    WorkCause, WorkEvent, WorkRecord,
};
use z_engine_protocol::{AgentId, TaskId};
use z_engine_work::{EntryDraft, Facts, Proposal, Submission, findings, graph};

use super::facts::{changed_files, current};
use super::ids::Ids;
use super::lifecycle::attached;
use super::plan::{PlanHandle, Submitted};
use super::resume::remind;
use crate::session::SessionCore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Trigger {
    TurnStart,
    Discovery,
    /// A task's helper ended or its change came back.
    Helper,
    /// `/replan`: every current finding counts, not only new ones.
    Replan,
}

/// At a turn start: the resume reminder when the plan was just resumed,
/// then reconciliation when the workspace fingerprint changed.
pub(crate) async fn at_turn_start(core: &SessionCore) {
    let Some(plan) = attached(core) else {
        return;
    };
    remind(core, &plan);
    let fingerprint = workspace_fingerprint(&core.root).await.ok();
    let previous = core.work.swap_fingerprint(fingerprint.clone());
    if previous.is_some() && previous != fingerprint {
        reconcile(core, &plan, Trigger::TurnStart).await;
    }
}

/// A plan this session just started: the next turn start compares the
/// workspace with how it is now.
pub(crate) async fn baseline(core: &SessionCore) {
    let fingerprint = workspace_fingerprint(&core.root).await.ok();
    core.work.swap_fingerprint(fingerprint);
}

/// After accepted records: a discovery about open tasks reconciles.
pub(crate) async fn after_records(core: &SessionCore, plan: &PlanHandle, records: &[WorkRecord]) {
    let affects = plan.read(|state| {
        records.iter().any(|record| match &record.event {
            WorkEvent::EntryRecorded { entry } => {
                matches!(entry.body, EntryBody::Discovery(_))
                    && entry.tasks.iter().any(|id| {
                        state
                            .task(id)
                            .is_some_and(|task| graph::is_open(&task.status))
                    })
            }
            _ => false,
        })
    });
    if affects {
        reconcile(core, plan, Trigger::Discovery).await;
    }
}

/// The findings reported to the main agent (empty: nothing happened).
pub(crate) async fn reconcile(
    core: &SessionCore,
    plan: &PlanHandle,
    trigger: Trigger,
) -> Vec<ReconcileFinding> {
    let state = plan.read(Clone::clone);
    if matches!(
        state.header.status,
        PlanStatus::Done | PlanStatus::Abandoned
    ) {
        return Vec::new();
    }
    let mut facts = current(core, &core.root).await;
    changed_files(core, &mut facts).await;
    let found = findings(&state, &facts);
    let reported: Vec<ReconcileFinding> = match trigger {
        Trigger::Replan => found.clone(),
        _ => found
            .iter()
            .filter(|finding| !state.findings.contains(finding))
            .cloned()
            .collect(),
    };
    if reported.is_empty() {
        if found.is_empty() && !state.findings.is_empty() {
            clear(plan, &facts);
        }
        return Vec::new();
    }
    let touched: Vec<String> = facts
        .changed_files
        .iter()
        .chain(&facts.deleted_files)
        .cloned()
        .collect();
    let result = plan.submit(&facts, |state| {
        let mut proposals = vec![Proposal::Reconcile {
            findings: found.clone(),
        }];
        if state.header.status == PlanStatus::Active {
            let mut ids = Ids::new(state);
            proposals.push(change(&mut ids, &reported, touched));
        }
        Ok(Submission::new(
            Actor::Kernel,
            WorkCause::Reconciliation,
            proposals,
        ))
    });
    match result {
        Ok(Submitted::Accepted(_)) => {
            core.reminders
                .push(&AgentId::main(), work_reconcile(&reported));
            reported
        }
        Ok(Submitted::Unchanged) => Vec::new(),
        Ok(Submitted::Rejected(rejection)) => {
            tracing::warn!(reasons = ?rejection.messages(), "reconciliation not recorded");
            Vec::new()
        }
        Err(error) => {
            tracing::warn!(%error, "reconciliation not recorded");
            Vec::new()
        }
    }
}

/// The earlier findings no longer hold.
fn clear(plan: &PlanHandle, facts: &Facts) {
    let result = plan.submit(facts, |_| {
        let proposals = vec![Proposal::Reconcile {
            findings: Vec::new(),
        }];
        Ok(Submission::new(
            Actor::Kernel,
            WorkCause::Reconciliation,
            proposals,
        ))
    });
    if let Err(error) = result {
        tracing::warn!(%error, "resolved findings not cleared");
    }
}

fn change(ids: &mut Ids, findings: &[ReconcileFinding], files: Vec<String>) -> Proposal {
    let mut affected: Vec<TaskId> = Vec::new();
    for id in findings.iter().flat_map(ReconcileFinding::tasks) {
        if !affected.contains(id) {
            affected.push(id.clone());
        }
    }
    let evidence = if files.is_empty() {
        Vec::new()
    } else {
        vec![EvidenceRef::Diff {
            files,
            checkpoint: None,
            summary: String::new(),
        }]
    };
    let change = PlanChange {
        cause: ChangeCause::Reconciliation,
        source: None,
        original: format!("revision {}", ids.state().revision()),
        new: String::new(),
        reason: format!(
            "Reconciliation found {} problem(s) with the plan",
            findings.len()
        ),
        evidence,
        affected,
    };
    Proposal::Record(Box::new(EntryDraft {
        id: Some(ids.entry()),
        ..EntryDraft::new(EntryBody::PlanChange(change))
    }))
}
