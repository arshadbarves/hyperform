//! Plan health from the plan alone: counts and levels for the coverage
//! line ("4 of 5 requirements covered", "evidence: Fair"). Evidence is
//! Good when every criterion of every done task is met (none waived), Poor
//! when a done task has no criteria or a failed one, and Fair otherwise
//! (or when nothing is done yet).

use z_engine_protocol::work::{
    AuditStatus, CriterionStatus, HealthLevel, PlanHealth, PlanItemStatus, Task, TaskStatus,
};

use crate::graph::{children, is_open};
use crate::state::PlanState;

pub fn health(state: &PlanState) -> PlanHealth {
    let planned: Vec<&Task> = state
        .tasks
        .values()
        .filter(|task| is_open(&task.status) || task.status == TaskStatus::Done)
        .collect();
    let done: Vec<&Task> = planned
        .iter()
        .copied()
        .filter(|task| task.status == TaskStatus::Done)
        .collect();
    let requirements: Vec<_> = state
        .header
        .requirements
        .iter()
        .filter(|item| item.status == PlanItemStatus::Active)
        .collect();
    let covered = requirements.iter().filter(|item| {
        planned
            .iter()
            .any(|task| task.requirements.contains(&item.id) && !task.acceptance.is_empty())
    });
    let open = || planned.iter().filter(|task| is_open(&task.status));
    PlanHealth {
        requirements: count(requirements.iter()),
        requirements_covered: count(covered),
        tasks: count(planned.iter()),
        tasks_done: count(done.iter()),
        tasks_without_criteria: count(
            open()
                .filter(|task| task.acceptance.is_empty() && children(state, &task.id).is_empty()),
        ),
        blocked: count(open().filter(|task| matches!(task.status, TaskStatus::Blocked { .. }))),
        failed: count(
            state
                .tasks
                .values()
                .filter(|task| matches!(task.status, TaskStatus::Failed { .. })),
        ),
        evidence: evidence(&done),
        evidence_stale: count(
            done.iter()
                .filter(|task| has(task, CriterionStatus::Failed)),
        ),
        open_findings: count(
            state
                .audit
                .iter()
                .filter(|finding| finding.status == AuditStatus::Open),
        ),
        reconcile_findings: count(state.findings.iter()),
        unmitigated_risks: count(open().filter(|task| {
            !task.notes.risks.is_empty()
                && task
                    .notes
                    .rollback
                    .as_deref()
                    .is_none_or(|text| text.trim().is_empty())
        })),
    }
}

fn evidence(done: &[&Task]) -> HealthLevel {
    if done.is_empty() {
        return HealthLevel::Fair;
    }
    if done
        .iter()
        .any(|task| task.acceptance.is_empty() || has(task, CriterionStatus::Failed))
    {
        return HealthLevel::Poor;
    }
    let solid = done.iter().all(|task| {
        task.acceptance
            .iter()
            .all(|criterion| criterion.status == CriterionStatus::Met)
    });
    if solid {
        HealthLevel::Good
    } else {
        HealthLevel::Fair
    }
}

fn has(task: &Task, status: CriterionStatus) -> bool {
    task.acceptance
        .iter()
        .any(|criterion| criterion.status == status)
}

fn count<T>(items: impl Iterator<Item = T>) -> u32 {
    u32::try_from(items.count()).unwrap_or(u32::MAX)
}

#[cfg(test)]
#[path = "health_tests.rs"]
mod tests;
