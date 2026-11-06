//! Deterministic reconciliation: compare the plan with what the engine saw
//! (deleted and changed files, stale check records) and with itself
//! (failed checks behind done tasks, failed or superseded dependencies,
//! overlapping or duplicate tasks). No findings means no replanning and no
//! model call.

use std::collections::BTreeMap;

use z_engine_protocol::TaskId;
use z_engine_protocol::work::{CriterionStatus, EvidenceRef, ReconcileFinding, Task, TaskStatus};

use crate::facts::Facts;
use crate::graph::{ancestors, is_open, ordered, tree_order};
use crate::scope::{matches_any, overlap};
use crate::state::PlanState;

/// Every finding, in tree order of the tasks involved.
pub fn findings(state: &PlanState, facts: &Facts) -> Vec<ReconcileFinding> {
    let tasks: Vec<&Task> = tree_order(state)
        .iter()
        .filter_map(|id| state.task(id))
        .collect();
    let mut out = Vec::new();
    for task in &tasks {
        out.extend(scope_deleted(task, facts));
        out.extend(stale_evidence(task, facts));
        out.extend(broken_dependencies(state, task));
    }
    out.extend(overlaps(state, &tasks));
    out.extend(duplicates(&tasks));
    out
}

fn scope_deleted(task: &Task, facts: &Facts) -> Option<ReconcileFinding> {
    if !is_open(&task.status) && task.status != TaskStatus::Proposed {
        return None;
    }
    let files = files_in_scope(task, facts.deleted_files.iter());
    (!files.is_empty()).then(|| ReconcileFinding::ScopeDeleted {
        task_id: task.id.clone(),
        files,
    })
}

fn stale_evidence(task: &Task, facts: &Facts) -> Option<ReconcileFinding> {
    if task.status != TaskStatus::Done {
        return None;
    }
    let own_changes = facts
        .changed_since
        .is_some_and(|since| task.updated_at >= since);
    let files = if own_changes {
        Vec::new()
    } else {
        files_in_scope(task, facts.changed_files.iter().chain(&facts.deleted_files))
    };
    let mut records: Vec<String> = task
        .acceptance
        .iter()
        .flat_map(|criterion| &criterion.evidence)
        .filter_map(|evidence| match evidence {
            EvidenceRef::Check { record_id, .. } if facts.stale_records.contains(record_id) => {
                Some(record_id.clone())
            }
            _ => None,
        })
        .collect();
    records.dedup();
    for failed in failed_checks(task) {
        if !records.contains(&failed) {
            records.push(failed);
        }
    }
    (!files.is_empty() || !records.is_empty()).then(|| ReconcileFinding::StaleEvidence {
        task_id: task.id.clone(),
        files,
        records,
    })
}

/// The failing check behind each failed criterion of a task (a check that
/// failed after the task was done does not reopen it by itself).
fn failed_checks(task: &Task) -> impl Iterator<Item = String> + '_ {
    task.acceptance
        .iter()
        .filter(|criterion| criterion.status == CriterionStatus::Failed)
        .filter_map(|criterion| {
            criterion
                .evidence
                .iter()
                .rev()
                .find_map(|evidence| match evidence {
                    EvidenceRef::Check {
                        record_id,
                        passed: false,
                        ..
                    } => Some(record_id.clone()),
                    _ => None,
                })
        })
}

fn broken_dependencies(state: &PlanState, task: &Task) -> Vec<ReconcileFinding> {
    if !is_open(&task.status) && task.status != TaskStatus::Proposed {
        return Vec::new();
    }
    task.depends_on
        .iter()
        .filter_map(|dep_id| {
            let dep = state.task(dep_id)?;
            match &dep.status {
                TaskStatus::Failed { .. } => Some(ReconcileFinding::DependencyFailed {
                    task_id: task.id.clone(),
                    depends_on: dep_id.clone(),
                }),
                TaskStatus::Superseded { by } => Some(ReconcileFinding::DependencySuperseded {
                    task_id: task.id.clone(),
                    depends_on: dep_id.clone(),
                    by: by.clone(),
                }),
                _ => None,
            }
        })
        .collect()
}

/// Open tasks with no ordering and no hierarchy between them whose scopes
/// overlap.
pub(crate) fn overlaps(state: &PlanState, tasks: &[&Task]) -> Vec<ReconcileFinding> {
    let open: Vec<&&Task> = tasks
        .iter()
        .filter(|task| is_open(&task.status) && !task.scope.files.is_empty())
        .collect();
    let mut out = Vec::new();
    for (index, a) in open.iter().enumerate() {
        for b in &open[index + 1..] {
            if related(state, &a.id, &b.id) {
                continue;
            }
            let files = overlap(&a.scope.files, &b.scope.files);
            if !files.is_empty() {
                out.push(ReconcileFinding::Overlap {
                    tasks: vec![a.id.clone(), b.id.clone()],
                    files,
                });
            }
        }
    }
    out
}

pub(crate) fn duplicates(tasks: &[&Task]) -> Vec<ReconcileFinding> {
    let mut groups: BTreeMap<String, Vec<TaskId>> = BTreeMap::new();
    let mut first_seen: Vec<String> = Vec::new();
    for task in tasks.iter().filter(|task| is_open(&task.status)) {
        let key = normalize_title(&task.title);
        if key.is_empty() {
            continue;
        }
        if !groups.contains_key(&key) {
            first_seen.push(key.clone());
        }
        groups.entry(key).or_default().push(task.id.clone());
    }
    first_seen
        .into_iter()
        .filter_map(|key| {
            let ids = groups.remove(&key)?;
            let title = tasks.iter().find(|t| t.id == ids[0])?.title.clone();
            (ids.len() > 1).then_some(ReconcileFinding::Duplicate { tasks: ids, title })
        })
        .collect()
}

fn related(state: &PlanState, a: &TaskId, b: &TaskId) -> bool {
    ordered(state, a, b) || ancestors(state, a).contains(b) || ancestors(state, b).contains(a)
}

fn files_in_scope<'a>(task: &Task, files: impl Iterator<Item = &'a String>) -> Vec<String> {
    let mut out: Vec<String> = files
        .filter(|file| matches_any(&task.scope.files, file))
        .cloned()
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Lowercase words of letters and digits.
pub fn normalize_title(title: &str) -> String {
    title
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
#[path = "reconcile_tests.rs"]
mod tests;
