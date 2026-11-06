//! The deterministic plan audit: coverage gaps found from the plan alone.
//! Every requirement maps to a task with criteria, every check criterion
//! has a check to run, no open task lacks criteria, risky paths carry a
//! test or migration criterion, risks have a mitigation, and open tasks do
//! not overlap or repeat. Findings have stable ids so dismissals stick.

use z_engine_protocol::work::{
    AuditFinding, AuditKind, AuditRef, AuditSeverity, AuditSource, AuditStatus, CriterionMethod,
    PlanItemStatus, ReconcileFinding, Task, TaskStatus,
};
use z_engine_protocol::{CheckKind, TaskId};

use crate::graph::{children, is_open, tree_order};
use crate::reconcile::{duplicates, overlaps};
use crate::scope::named;
use crate::state::PlanState;

/// Persistence and protocol paths: a change there needs a test or a
/// migration criterion.
pub const RISKY_PATHS: [&str; 8] = [
    "**/migrations/**",
    "**/migrate/**",
    "**/*.sql",
    "**/schema*",
    "**/protocol/**",
    "**/*.proto",
    "**/store/**",
    "**/persistence/**",
];

#[derive(Debug, Clone, Copy)]
pub struct AuditInput<'a> {
    /// Check kinds the project can run; `None` skips the method check.
    pub check_kinds: Option<&'a [CheckKind]>,
    /// `RISKY_PATHS` plus `[work] sensitive_paths`, as the engine chooses.
    pub risky_paths: &'a [String],
}

/// Every finding, requirements first, then tasks in tree order.
pub fn audit(state: &PlanState, input: &AuditInput) -> Vec<AuditFinding> {
    let tasks: Vec<&Task> = tree_order(state)
        .iter()
        .filter_map(|id| state.task(id))
        .collect();
    let planned: Vec<&Task> = tasks
        .iter()
        .copied()
        .filter(|task| is_open(&task.status) || task.status == TaskStatus::Done)
        .collect();
    let mut out = uncovered_requirements(state, &planned);
    for task in tasks.iter().filter(|task| is_open(&task.status)) {
        let leaf = children(state, &task.id).is_empty();
        if leaf && task.acceptance.is_empty() {
            out.push(finding(
                AuditKind::TaskWithoutCriteria,
                AuditSeverity::Medium,
                format!("Task \"{}\" has no acceptance criteria", task.title),
                vec![task_ref(&task.id)],
                "Add a criterion that shows the task is done",
            ));
        }
        out.extend(unrunnable_criteria(task, input.check_kinds));
        out.extend(risky_without_test(task, input.risky_paths));
        out.extend(unmitigated(task));
    }
    out.extend(overlaps(state, &tasks).iter().filter_map(from_reconcile));
    out.extend(duplicates(&tasks).iter().filter_map(from_reconcile));
    out
}

fn uncovered_requirements(state: &PlanState, planned: &[&Task]) -> Vec<AuditFinding> {
    let active = state
        .header
        .requirements
        .iter()
        .filter(|item| item.status == PlanItemStatus::Active);
    active
        .filter_map(|item| {
            let serving: Vec<&&Task> = planned
                .iter()
                .filter(|task| task.requirements.contains(&item.id))
                .collect();
            if serving.iter().any(|task| !task.acceptance.is_empty()) {
                return None;
            }
            let mut refs = vec![AuditRef::Item {
                item_id: item.id.clone(),
            }];
            refs.extend(serving.iter().map(|task| task_ref(&task.id)));
            let (text, suggestion) = if serving.is_empty() {
                (
                    format!("Requirement \"{}\" has no task", item.text),
                    "Add a task that serves it, with an acceptance criterion",
                )
            } else {
                (
                    format!("Requirement \"{}\" has no task with criteria", item.text),
                    "Add an acceptance criterion to a task that serves it",
                )
            };
            Some(finding(
                AuditKind::UncoveredRequirement,
                AuditSeverity::High,
                text,
                refs,
                suggestion,
            ))
        })
        .collect()
}

fn unrunnable_criteria(task: &Task, kinds: Option<&[CheckKind]>) -> Vec<AuditFinding> {
    let Some(kinds) = kinds else {
        return Vec::new();
    };
    task.acceptance
        .iter()
        .filter_map(|criterion| match criterion.method {
            CriterionMethod::Check { kind } if !kinds.contains(&kind) => Some(finding(
                AuditKind::CriterionWithoutMethod,
                AuditSeverity::High,
                format!(
                    "Criterion \"{}\" of \"{}\" needs a {} check, and the project has none",
                    criterion.text,
                    task.title,
                    kind.label()
                ),
                vec![AuditRef::Criterion {
                    task_id: task.id.clone(),
                    criterion_id: criterion.id.clone(),
                }],
                "Configure that check, or change how the criterion is shown",
            )),
            _ => None,
        })
        .collect()
}

fn risky_without_test(task: &Task, risky: &[String]) -> Option<AuditFinding> {
    let touched = named(&task.scope.files, risky);
    let covered = task.acceptance.iter().any(|criterion| {
        matches!(
            criterion.method,
            CriterionMethod::Check {
                kind: CheckKind::Test
            }
        ) || criterion.text.to_lowercase().contains("migrat")
    });
    if touched.is_empty() || covered {
        return None;
    }
    let mut refs = vec![task_ref(&task.id)];
    refs.extend(
        touched
            .iter()
            .map(|path| AuditRef::File { path: path.clone() }),
    );
    Some(finding(
        AuditKind::RiskyPathWithoutTest,
        AuditSeverity::High,
        format!(
            "Task \"{}\" touches {} without a test or migration criterion",
            task.title,
            touched.join(", ")
        ),
        refs,
        "Add a test criterion, or one for the migration",
    ))
}

fn unmitigated(task: &Task) -> Option<AuditFinding> {
    let rollback = task.notes.rollback.as_deref().map(str::trim);
    if task.notes.risks.is_empty() || rollback.is_some_and(|text| !text.is_empty()) {
        return None;
    }
    Some(finding(
        AuditKind::RiskWithoutMitigation,
        AuditSeverity::Medium,
        format!(
            "Task \"{}\" notes risks without a mitigation: {}",
            task.title,
            task.notes.risks.join("; ")
        ),
        vec![task_ref(&task.id)],
        "Add a rollback note, or a criterion that covers the risk",
    ))
}

fn from_reconcile(found: &ReconcileFinding) -> Option<AuditFinding> {
    let (kind, severity, text, suggestion) = match found {
        ReconcileFinding::Overlap { files, .. } => (
            AuditKind::Overlap,
            AuditSeverity::Medium,
            format!("Unrelated tasks overlap in {}", files.join(", ")),
            "Order them with a dependency, or split their scopes",
        ),
        ReconcileFinding::Duplicate { title, .. } => (
            AuditKind::Duplicate,
            AuditSeverity::Low,
            format!("Several open tasks are called \"{title}\""),
            "Merge them, or mark one superseded",
        ),
        _ => return None,
    };
    let refs = found.tasks().iter().map(|id| task_ref(id)).collect();
    Some(finding(kind, severity, text, refs, suggestion))
}

fn finding(
    kind: AuditKind,
    severity: AuditSeverity,
    text: String,
    refs: Vec<AuditRef>,
    suggestion: &str,
) -> AuditFinding {
    AuditFinding {
        id: finding_id(kind, &refs),
        kind,
        severity,
        text,
        refs,
        suggestion: Some(suggestion.to_string()),
        status: AuditStatus::Open,
        source: AuditSource::Deterministic,
        created_revision: 0,
        revision: 0,
    }
}

/// `kind:ref+ref`, from the task, criterion and item refs (files left out
/// so a scope edit does not renew the finding).
fn finding_id(kind: AuditKind, refs: &[AuditRef]) -> String {
    let keys: Vec<String> = refs
        .iter()
        .filter_map(|reference| match reference {
            AuditRef::Task { task_id } => Some(task_id.to_string()),
            AuditRef::Criterion {
                task_id,
                criterion_id,
            } => Some(format!("{task_id}/{criterion_id}")),
            AuditRef::Item { item_id } => Some(item_id.to_string()),
            AuditRef::File { .. } => None,
        })
        .take(match kind {
            AuditKind::UncoveredRequirement => 1,
            _ => usize::MAX,
        })
        .collect();
    format!("{}:{}", kind.label(), keys.join("+"))
}

fn task_ref(task_id: &TaskId) -> AuditRef {
    AuditRef::Task {
        task_id: task_id.clone(),
    }
}

#[cfg(test)]
#[path = "audit_tests.rs"]
mod tests;
