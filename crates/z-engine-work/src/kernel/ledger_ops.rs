//! Ledger proposals: evidence, design decisions, discoveries, experiments
//! and plan changes, and confirming or dismissing entries.

use z_engine_protocol::work::{
    Actor, CriterionMethod, CriterionStatus, Entry, EntryBody, EntryStatus, EvidenceEntry,
    EvidenceRef, ExperimentOutcome, Provenance, Task, WorkEvent,
};
use z_engine_protocol::{EntryId, TaskId};

use super::task_ops::find;
use super::{Ctx, Emitted, Outcome, fail};
use crate::authority::Role;
use crate::error::Reason;
use crate::proposal::{EntryDraft, Proposal};
use crate::state::PlanState;

pub(super) fn evaluate(state: &PlanState, ctx: &Ctx, proposal: &Proposal) -> Outcome {
    match proposal {
        Proposal::LinkEvidence {
            task_id,
            criterion_id,
            evidence,
        } => link(state, ctx, task_id, criterion_id.as_deref(), evidence),
        Proposal::Record(draft) => record(state, ctx, draft),
        Proposal::SetEntryStatus { entry_id, status } => set_status(state, ctx, entry_id, status),
        Proposal::ConcludeExperiment {
            entry_id,
            outcome,
            result,
            conclusion,
        } => {
            let Some(entry) = state.entry(entry_id) else {
                return fail(Reason::UnknownEntry(entry_id.clone()));
            };
            if !matches!(entry.body, EntryBody::Experiment(_)) {
                return fail(Reason::invalid(format!("{entry_id} is not an experiment")));
            }
            if *outcome == ExperimentOutcome::Open {
                return fail(Reason::invalid("a concluded experiment needs an outcome"));
            }
            Ok(vec![ctx.emit(WorkEvent::ExperimentConcluded {
                entry_id: entry_id.clone(),
                outcome: *outcome,
                result: result.clone(),
                conclusion: conclusion.clone(),
            })])
        }
        _ => Ok(vec![]),
    }
}

/// Evidence for a criterion is attached to it; a passing or failing check
/// of the criterion's kind also marks it Met or Failed. Evidence for the
/// task as a whole, or from a turn that read web content, becomes an
/// evidence entry (unconfirmed for the latter).
fn link(
    state: &PlanState,
    ctx: &Ctx,
    task_id: &TaskId,
    criterion_id: Option<&str>,
    evidence: &EvidenceRef,
) -> Outcome {
    let task = find(state, task_id)?;
    let Some(criterion_id) = criterion_id.filter(|_| !ctx.role.is_research()) else {
        let draft = EntryDraft {
            tasks: vec![task_id.clone()],
            files: evidence_files(evidence),
            ..EntryDraft::new(EntryBody::Evidence(EvidenceEntry {
                task_id: task_id.clone(),
                criterion_id: criterion_id.map(str::to_string),
                evidence: evidence.clone(),
            }))
        };
        return record(state, ctx, &draft);
    };
    let Some(criterion) = task.acceptance.iter().find(|c| c.id == criterion_id) else {
        return fail(Reason::UnknownCriterion {
            task_id: task_id.clone(),
            criterion_id: criterion_id.to_string(),
        });
    };
    let mut out: Emitted = vec![ctx.emit(WorkEvent::EvidenceLinked {
        task_id: task_id.clone(),
        criterion_id: Some(criterion_id.to_string()),
        evidence: evidence.clone(),
    })];
    if let (
        CriterionMethod::Check { kind },
        EvidenceRef::Check {
            passed,
            kind: evidence_kind,
            ..
        },
    ) = (criterion.method, evidence)
    {
        let status = if *passed {
            CriterionStatus::Met
        } else {
            CriterionStatus::Failed
        };
        let relevant = evidence_kind.is_none_or(|k| k == kind);
        if relevant && criterion.status != CriterionStatus::Waived && criterion.status != status {
            out.push((
                Actor::Kernel,
                WorkEvent::CriterionChanged {
                    task_id: task_id.clone(),
                    criterion_id: criterion_id.to_string(),
                    status,
                },
            ));
        }
    }
    Ok(out)
}

fn record(state: &PlanState, ctx: &Ctx, draft: &EntryDraft) -> Outcome {
    let mut reasons = Vec::new();
    let id = draft.id.clone().unwrap_or_default();
    if state.entry(&id).is_some() || state.item(&id).is_some() {
        reasons.push(Reason::DuplicateId(id.to_string()));
    }
    let mut tasks: Vec<TaskId> = Vec::new();
    let linked = draft.tasks.iter().chain(body_tasks(&draft.body));
    for task_id in linked {
        if state.task(task_id).is_none() {
            reasons.push(Reason::UnknownTask(task_id.clone()));
        } else if !tasks.contains(task_id) {
            tasks.push(task_id.clone());
        }
    }
    reasons.extend(body_problems(state, ctx, &draft.body));
    if !reasons.is_empty() {
        return Err(reasons);
    }
    let unconfirmed = matches!(ctx.role, Role::Untrusted(_)) || draft.proposed;
    let status = if unconfirmed && !ctx.role.is_user() {
        EntryStatus::Proposed
    } else {
        EntryStatus::Accepted
    };
    let entry = Entry {
        id,
        body: draft.body.clone(),
        provenance: ctx.role.provenance(),
        status,
        tasks,
        files: draft.files.clone(),
        created_at: 0,
        created_revision: 0,
        revision: 0,
    };
    Ok(vec![ctx.emit(WorkEvent::EntryRecorded {
        entry: Box::new(entry),
    })])
}

fn body_tasks(body: &EntryBody) -> Vec<&TaskId> {
    match body {
        EntryBody::Evidence(evidence) => vec![&evidence.task_id],
        EntryBody::PlanChange(change) => change.affected.iter().collect(),
        _ => Vec::new(),
    }
}

fn body_problems(state: &PlanState, ctx: &Ctx, body: &EntryBody) -> Vec<Reason> {
    let missing = |what: &str| vec![Reason::invalid(format!("a {} needs {what}", body.label()))];
    match body {
        EntryBody::DesignDecision(d) if d.title.trim().is_empty() || d.choice.trim().is_empty() => {
            missing("a title and a choice")
        }
        EntryBody::Discovery(d) if d.text.trim().is_empty() => missing("text"),
        EntryBody::Experiment(e) if e.hypothesis.trim().is_empty() => missing("a hypothesis"),
        EntryBody::Evidence(e) => {
            let mut out = Vec::new();
            if matches!(e.evidence, EvidenceRef::Check { .. }) && ctx.role != Role::Kernel {
                out.push(Reason::EngineOnly("linking check evidence".into()));
            }
            let task = state.task(&e.task_id);
            if let (Some(task), Some(criterion)) = (task, &e.criterion_id) {
                if !has_criterion(task, criterion) {
                    out.push(Reason::UnknownCriterion {
                        task_id: e.task_id.clone(),
                        criterion_id: criterion.clone(),
                    });
                }
            }
            out
        }
        EntryBody::PlanChange(change) => match &change.source {
            Some(source) if state.entry(source).is_none() => {
                vec![Reason::UnknownEntry(source.clone())]
            }
            _ if change.reason.trim().is_empty() => missing("a reason"),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

fn set_status(state: &PlanState, ctx: &Ctx, entry_id: &EntryId, status: &EntryStatus) -> Outcome {
    let Some(entry) = state.entry(entry_id) else {
        return fail(Reason::UnknownEntry(entry_id.clone()));
    };
    if entry.status == *status {
        return Ok(vec![]);
    }
    let mut checks = Vec::new();
    if entry.provenance == Provenance::User {
        checks.push(ctx.role.require_user("change an entry the user recorded"));
    }
    checks.push(match status {
        EntryStatus::Proposed => Err(Reason::invalid("an entry cannot go back to unconfirmed")),
        EntryStatus::Accepted => ctx.role.require_user("confirm or restore an entry"),
        EntryStatus::Superseded { by } if by == entry_id => {
            Err(Reason::invalid("an entry cannot supersede itself"))
        }
        EntryStatus::Superseded { by } if state.entry(by).is_none() => {
            Err(Reason::UnknownEntry(by.clone()))
        }
        EntryStatus::Superseded { .. } | EntryStatus::Dismissed => Ok(()),
    });
    let reasons: Vec<Reason> = checks.into_iter().filter_map(Result::err).collect();
    if !reasons.is_empty() {
        return Err(reasons);
    }
    Ok(vec![ctx.emit(WorkEvent::EntryStatusChanged {
        entry_id: entry_id.clone(),
        status: status.clone(),
    })])
}

fn has_criterion(task: &Task, criterion_id: &str) -> bool {
    task.acceptance.iter().any(|c| c.id == criterion_id)
}

fn evidence_files(evidence: &EvidenceRef) -> Vec<String> {
    match evidence {
        EvidenceRef::Diff { files, .. } => files.clone(),
        _ => Vec::new(),
    }
}
