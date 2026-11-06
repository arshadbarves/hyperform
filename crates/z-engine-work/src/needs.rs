//! What a plan waits on the user for, the same set as the Plan tab's Needs
//! you strip: a merge conflict, a helper's change waiting at its apply
//! card, a run that ended without a report or whose change was discarded,
//! a Verifying task whose check criteria did not run or are stale, a
//! proposed task, requirement or invariant, and an open high-severity
//! audit finding. Medium and low findings stay in the plan's findings list
//! without waiting on the user.

use std::collections::BTreeSet;

use z_engine_protocol::work::{
    AuditSeverity, AuditStatus, BlockReason, CriterionMethod, CriterionStatus, EvidenceRef,
    Integration, PlanItemStatus, ReconcileFinding, Task, TaskStatus,
};

use crate::graph::is_closed;
use crate::state::PlanState;

/// How many items wait on the user.
pub fn needs_you(state: &PlanState) -> u32 {
    let tasks = state.tasks.values().filter(|task| task_needs(state, task));
    let items = state
        .items()
        .filter(|item| item.status == PlanItemStatus::Proposed);
    let findings = state.audit.iter().filter(|finding| {
        finding.status == AuditStatus::Open && finding.severity == AuditSeverity::High
    });
    let total = tasks.count() + items.count() + findings.count();
    u32::try_from(total).unwrap_or(u32::MAX)
}

fn task_needs(state: &PlanState, task: &Task) -> bool {
    if is_closed(&task.status) {
        return false;
    }
    match (&task.status, &task.run.integration) {
        (_, Some(Integration::Conflict { .. })) => true,
        (TaskStatus::Active, Some(Integration::PendingApply { .. })) => true,
        (TaskStatus::Blocked { reason }, _) => matches!(
            reason,
            BlockReason::MergeConflict { .. }
                | BlockReason::Unreported
                | BlockReason::ChangeDiscarded
        ),
        (TaskStatus::Verifying, _) => needs_recheck(state, task),
        (TaskStatus::Proposed, _) => true,
        _ => false,
    }
}

/// Verifying with a check criterion that has not run, or whose evidence
/// reconciliation judged stale.
pub fn needs_recheck(state: &PlanState, task: &Task) -> bool {
    if task.status != TaskStatus::Verifying {
        return false;
    }
    let stale: BTreeSet<&str> = state
        .findings
        .iter()
        .filter_map(|finding| match finding {
            ReconcileFinding::StaleEvidence {
                task_id, records, ..
            } if *task_id == task.id => Some(records),
            _ => None,
        })
        .flatten()
        .map(String::as_str)
        .collect();
    task.acceptance.iter().any(|criterion| {
        matches!(criterion.method, CriterionMethod::Check { .. })
            && (criterion.status == CriterionStatus::Pending
                || criterion.evidence.iter().any(|evidence| {
                    matches!(evidence, EvidenceRef::Check { record_id, .. } if stale.contains(record_id.as_str()))
                }))
    })
}

#[cfg(test)]
mod tests {
    use z_engine_protocol::verification::CheckKind;
    use z_engine_protocol::work::{
        Actor, AuditFinding, AuditKind, AuditSource, PlanItemKind, PlanStatus,
    };

    use super::*;
    use crate::proposal::{Proposal, TaskDraft};
    use crate::testkit::{
        add, check_criterion, create_task, main_agent, plan, set, submit, task, tid,
    };

    #[test]
    fn counts_what_waits_on_the_user() {
        let mut state = plan(PlanStatus::Active);
        add(&mut state, "a", &[]);
        assert_eq!(needs_you(&state), 0);
        let proposed = TaskDraft {
            status: Some(TaskStatus::Proposed),
            ..task("p")
        };
        let item = Proposal::AddItem {
            id: None,
            kind: PlanItemKind::Requirement,
            text: "Retries are bounded".into(),
        };
        submit(&mut state, main_agent(), vec![create_task(proposed), item]);
        assert_eq!(needs_you(&state), 2, "a proposed task and item");

        let unreported = TaskStatus::Blocked {
            reason: BlockReason::Unreported,
        };
        submit(&mut state, main_agent(), vec![set("a", TaskStatus::Active)]);
        submit(&mut state, Actor::Kernel, vec![set("a", unreported)]);
        assert_eq!(needs_you(&state), 3, "an unreported run");
        assert_eq!(state.summary().needs_you, 3);
        assert_eq!(state.summary().blocked, 1);
    }

    fn finding(id: &str, severity: AuditSeverity, status: AuditStatus) -> AuditFinding {
        AuditFinding {
            id: id.into(),
            kind: AuditKind::TaskWithoutCriteria,
            severity,
            text: "Task a has no acceptance criteria".into(),
            refs: Vec::new(),
            suggestion: None,
            status,
            source: AuditSource::Deterministic,
            created_revision: 1,
            revision: 1,
        }
    }

    #[test]
    fn only_open_high_findings_wait_on_the_user() {
        let mut state = plan(PlanStatus::Active);
        state.audit = vec![
            finding("low", AuditSeverity::Low, AuditStatus::Open),
            finding("medium", AuditSeverity::Medium, AuditStatus::Open),
            finding("dismissed", AuditSeverity::High, AuditStatus::Dismissed),
        ];
        assert_eq!(needs_you(&state), 0, "medium and low findings only show");
        state
            .audit
            .push(finding("high", AuditSeverity::High, AuditStatus::Open));
        assert_eq!(needs_you(&state), 1);
        assert_eq!(state.summary().needs_you, 1);
    }

    #[test]
    fn a_verifying_task_with_an_unrun_check_needs_a_recheck() {
        let mut state = plan(PlanStatus::Active);
        let draft = TaskDraft {
            acceptance: vec![check_criterion("tests pass", CheckKind::Test)],
            ..task("v")
        };
        submit(&mut state, main_agent(), vec![create_task(draft)]);
        let task = state.tasks.get_mut(&tid("v")).unwrap();
        task.status = TaskStatus::Verifying;
        let task = state.task(&tid("v")).unwrap();
        assert!(needs_recheck(&state, task));
        assert_eq!(needs_you(&state), 1);
    }
}
