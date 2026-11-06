//! The completion gate. Done needs every criterion met or waived (a check
//! criterion by a passing check linked by the engine on the current tree),
//! every subtask closed and every dependency satisfied. A refusal lists
//! everything still missing.

use hyperform_protocol::verification::CheckKind;
use hyperform_protocol::work::{
    Criterion, CriterionMethod, CriterionStatus, EvidenceRef, Task, WorkEvent,
};

use crate::error::Reason;
use crate::facts::Facts;
use crate::graph::{children, is_closed, unsatisfied_dependencies};
use crate::state::PlanState;

/// `CriterionChanged` events marking satisfied check criteria Met, or
/// every reason the task cannot be Done.
pub(super) fn completion(
    state: &PlanState,
    task: &Task,
    facts: &Facts,
) -> Result<Vec<WorkEvent>, Vec<Reason>> {
    let mut reasons = Vec::new();
    let mut met = Vec::new();
    for criterion in &task.acceptance {
        if criterion.status == CriterionStatus::Waived {
            continue;
        }
        match criterion.method {
            CriterionMethod::Check { kind } => match check_gap(criterion, kind, facts) {
                None if criterion.status != CriterionStatus::Met => {
                    met.push(WorkEvent::CriterionChanged {
                        task_id: task.id.clone(),
                        criterion_id: criterion.id.clone(),
                        status: CriterionStatus::Met,
                    });
                }
                None => {}
                Some(detail) => reasons.push(Reason::MissingCheck {
                    task_id: task.id.clone(),
                    criterion_id: criterion.id.clone(),
                    text: criterion.text.clone(),
                    kind: kind.label(),
                    detail,
                }),
            },
            CriterionMethod::Inspection | CriterionMethod::User => {
                if criterion.status != CriterionStatus::Met {
                    reasons.push(Reason::CriterionUnmet {
                        task_id: task.id.clone(),
                        criterion_id: criterion.id.clone(),
                        text: criterion.text.clone(),
                    });
                }
            }
        }
    }
    let open_children: Vec<_> = children(state, &task.id)
        .into_iter()
        .filter(|child| !is_closed(&child.status))
        .map(|child| child.id.clone())
        .collect();
    if !open_children.is_empty() {
        reasons.push(Reason::OpenChildren {
            task_id: task.id.clone(),
            children: open_children,
        });
    }
    let open_deps = unsatisfied_dependencies(state, task);
    if !open_deps.is_empty() {
        reasons.push(Reason::OpenDependencies {
            task_id: task.id.clone(),
            dependencies: open_deps,
        });
    }
    if reasons.is_empty() {
        Ok(met)
    } else {
        Err(reasons)
    }
}

/// `None` when a passing check of `kind` ran on the current tree and is not
/// stale; otherwise what is wrong with the latest one.
fn check_gap(criterion: &Criterion, kind: CheckKind, facts: &Facts) -> Option<String> {
    let checks: Vec<(&str, bool, &str)> = criterion
        .evidence
        .iter()
        .filter_map(|evidence| match evidence {
            EvidenceRef::Check {
                record_id,
                passed,
                fingerprint,
                kind: evidence_kind,
                ..
            } if evidence_kind.is_none_or(|k| k == kind) => {
                Some((record_id.as_str(), *passed, fingerprint.as_str()))
            }
            _ => None,
        })
        .collect();
    let Some(current) = facts.fingerprint.as_deref() else {
        return Some("the current tree's fingerprint is unknown".into());
    };
    let fresh = |record: &str, fingerprint: &str| {
        fingerprint == current && !facts.stale_records.contains(record)
    };
    let latest_fresh = checks
        .iter()
        .rev()
        .find(|(record, _, fingerprint)| fresh(record, fingerprint));
    match (latest_fresh, checks.is_empty()) {
        (Some((_, true, _)), _) => None,
        (Some((_, false, _)), _) => Some("the latest check on this tree failed".into()),
        (None, true) => Some("no check has been linked".into()),
        (None, false) => Some("the latest check ran on an older tree".into()),
    }
}
