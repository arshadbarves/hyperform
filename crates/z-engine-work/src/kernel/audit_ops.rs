//! Audit findings: the engine records them (deterministic or from the
//! audit agent) and the user adds them to the plan or dismisses them. A
//! finding keeps the user's status across audits; a deterministic audit
//! resolves the open or added findings it no longer finds.

use std::collections::BTreeSet;

use z_engine_protocol::work::{
    AuditFinding, AuditKind, AuditRef, AuditSource, AuditStatus, WorkEvent,
};

use super::{Ctx, Outcome, fail};
use crate::audit::AuditInput;
use crate::authority::Role;
use crate::error::Reason;
use crate::facts::AuditScope;
use crate::proposal::Proposal;
use crate::state::PlanState;

pub(super) fn evaluate(state: &PlanState, ctx: &Ctx, proposal: &Proposal) -> Outcome {
    match proposal {
        Proposal::Audit { source, findings } => audit(state, ctx, *source, findings),
        Proposal::SetFindingStatus { finding_id, status } => {
            set_status(state, ctx, finding_id, *status)
        }
        _ => Ok(vec![]),
    }
}

fn audit(state: &PlanState, ctx: &Ctx, source: AuditSource, findings: &[AuditFinding]) -> Outcome {
    if ctx.role != Role::Kernel {
        return fail(Reason::EngineOnly("recording audit findings".into()));
    }
    let reasons: Vec<Reason> = findings
        .iter()
        .flat_map(|finding| invalid(state, source, finding))
        .collect();
    if !reasons.is_empty() {
        return Err(reasons);
    }
    let upserts = upserts(state, source, findings);
    if upserts.is_empty() {
        return Ok(vec![]);
    }
    Ok(vec![ctx.emit(WorkEvent::Audited { findings: upserts })])
}

/// The deterministic audit of `state` as an `Audited` event, when its
/// findings differ from the recorded ones.
pub(super) fn follow_up(state: &PlanState, scope: &AuditScope) -> Option<WorkEvent> {
    let input = AuditInput {
        check_kinds: scope.check_kinds.as_deref(),
        risky_paths: &scope.risky_paths,
    };
    let findings = crate::audit::audit(state, &input);
    let upserts = upserts(state, AuditSource::Deterministic, &findings);
    (!upserts.is_empty()).then_some(WorkEvent::Audited { findings: upserts })
}

/// New and changed findings; a deterministic audit also resolves the
/// open or added findings it no longer finds.
fn upserts(state: &PlanState, source: AuditSource, findings: &[AuditFinding]) -> Vec<AuditFinding> {
    let mut next_review = state
        .audit
        .iter()
        .filter(|finding| finding.source == AuditSource::AuditAgent)
        .count()
        + 1;
    let mut seen = BTreeSet::new();
    let mut upserts = Vec::new();
    for finding in findings {
        let mut finding = AuditFinding {
            source,
            ..finding.clone()
        };
        if finding.id.trim().is_empty() {
            finding.id = format!("review-{next_review}");
            next_review += 1;
        }
        if !seen.insert(finding.id.clone()) {
            continue;
        }
        let old = state.finding(&finding.id);
        finding.status = match old.map(|old| old.status) {
            None | Some(AuditStatus::Resolved) => AuditStatus::Open,
            Some(status) => status,
        };
        finding.created_revision = old.map_or(0, |old| old.created_revision);
        finding.revision = old.map_or(0, |old| old.revision);
        if old != Some(&finding) {
            upserts.push(finding);
        }
    }
    if source == AuditSource::Deterministic {
        let gone = state.audit.iter().filter(|old| {
            old.source == AuditSource::Deterministic
                && matches!(old.status, AuditStatus::Open | AuditStatus::Added)
                && !seen.contains(&old.id)
        });
        upserts.extend(gone.map(|old| AuditFinding {
            status: AuditStatus::Resolved,
            ..old.clone()
        }));
    }
    upserts
}

fn invalid(state: &PlanState, source: AuditSource, finding: &AuditFinding) -> Vec<Reason> {
    let mut out = Vec::new();
    if finding.text.trim().is_empty() {
        out.push(Reason::invalid("an audit finding needs text"));
    }
    let review = finding.kind == AuditKind::ModelReview;
    if review != (source == AuditSource::AuditAgent) {
        out.push(Reason::invalid(
            "the audit agent adds model reviews; deterministic findings name their check",
        ));
    }
    for reference in &finding.refs {
        match reference {
            AuditRef::Task { task_id } if state.task(task_id).is_none() => {
                out.push(Reason::UnknownTask(task_id.clone()));
            }
            AuditRef::Criterion {
                task_id,
                criterion_id,
            } => match state.task(task_id) {
                None => out.push(Reason::UnknownTask(task_id.clone())),
                Some(task) if !task.acceptance.iter().any(|c| c.id == *criterion_id) => {
                    out.push(Reason::UnknownCriterion {
                        task_id: task_id.clone(),
                        criterion_id: criterion_id.clone(),
                    });
                }
                Some(_) => {}
            },
            AuditRef::Item { item_id } if state.item(item_id).is_none() => {
                out.push(Reason::UnknownItem(item_id.clone()));
            }
            _ => {}
        }
    }
    out
}

fn set_status(state: &PlanState, ctx: &Ctx, finding_id: &str, status: AuditStatus) -> Outcome {
    ctx.role
        .require_user("add an audit finding to the plan or dismiss it")
        .map_err(|reason| vec![reason])?;
    let Some(finding) = state.finding(finding_id) else {
        return fail(Reason::UnknownFinding(finding_id.to_string()));
    };
    if status == AuditStatus::Resolved {
        return fail(Reason::invalid(
            "a finding is resolved by the next audit, not by hand",
        ));
    }
    if finding.status == status {
        return Ok(vec![]);
    }
    Ok(vec![ctx.emit(WorkEvent::FindingStatusChanged {
        finding_id: finding_id.to_string(),
        status,
    })])
}
