//! Acceptance criteria: replacing a task's criteria from drafts, keeping
//! the status and evidence of the ones that stay.

use hyperform_protocol::TaskId;
use hyperform_protocol::work::{
    Criterion, CriterionDraft, CriterionStatus, TaskPatch, TaskStatus, WorkEvent,
};

use super::task_ops::find;
use super::{Ctx, Emitted, Outcome, propagate};
use crate::error::Reason;
use crate::state::PlanState;

/// Replace the criteria; a Done task goes back to Pending.
pub(super) fn set(
    state: &PlanState,
    ctx: &Ctx,
    task_id: &TaskId,
    drafts: &[CriterionDraft],
) -> Outcome {
    let task = find(state, task_id)?;
    let acceptance = build(task_id, &task.acceptance, drafts)?;
    if acceptance == task.acceptance {
        return Ok(vec![]);
    }
    let mut out: Emitted = vec![ctx.emit(WorkEvent::TaskEdited {
        task_id: task_id.clone(),
        patch: Box::new(TaskPatch {
            acceptance: Some(acceptance),
            ..TaskPatch::default()
        }),
    })];
    if task.status == TaskStatus::Done {
        out.extend(propagate::reopen(task));
    }
    Ok(out)
}

/// Criteria from drafts: known ids keep their status and evidence (a new
/// method resets the status), new ones get the next free `cN`.
pub(super) fn build(
    task_id: &TaskId,
    existing: &[Criterion],
    drafts: &[CriterionDraft],
) -> Result<Vec<Criterion>, Vec<Reason>> {
    let mut reasons = Vec::new();
    let mut next = existing
        .iter()
        .filter_map(|c| c.id.strip_prefix('c')?.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    let mut out: Vec<Criterion> = Vec::with_capacity(drafts.len());
    for draft in drafts {
        let text = draft.text.trim();
        if text.is_empty() {
            reasons.push(Reason::invalid("a criterion needs text"));
            continue;
        }
        let criterion = match &draft.id {
            Some(id) => match existing.iter().find(|c| c.id == *id) {
                Some(old) => Criterion {
                    text: text.to_string(),
                    method: draft.method,
                    status: if old.method == draft.method {
                        old.status
                    } else {
                        CriterionStatus::Pending
                    },
                    ..old.clone()
                },
                None => {
                    reasons.push(Reason::UnknownCriterion {
                        task_id: task_id.clone(),
                        criterion_id: id.clone(),
                    });
                    continue;
                }
            },
            None => {
                next += 1;
                Criterion {
                    id: format!("c{next}"),
                    text: text.to_string(),
                    method: draft.method,
                    status: CriterionStatus::Pending,
                    evidence: Vec::new(),
                }
            }
        };
        if out.iter().any(|c| c.id == criterion.id) {
            reasons.push(Reason::DuplicateId(format!("criterion {}", criterion.id)));
            continue;
        }
        out.push(criterion);
    }
    if reasons.is_empty() {
        Ok(out)
    } else {
        Err(reasons)
    }
}
