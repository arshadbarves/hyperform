//! Work tool inputs as kernel proposals. Only the input's shape is checked
//! here; the kernel decides whether the change is allowed.

use z_engine_protocol::verification::CheckKind;
use z_engine_protocol::work::{
    BlockReason, CriterionDraft, CriterionMethod, CriterionStatus, DesignDecision, Discovery,
    EntryBody, EvidenceRef, Experiment, ExperimentOutcome, PlanItemKind, TaskPatch, TaskScope,
    TaskStatus,
};
use z_engine_protocol::{EntryId, TaskId};
use z_engine_tools::{CriterionInput, NewTask, PlanNote, PlanOutline, TaskChange};
use z_engine_work::{EntryDraft, PlanState, Proposal, TaskDraft, dispatch};

use super::facts::task_id;
use super::ids::Ids;

/// Characters of a decision's text used as its title when none is given.
const TITLE_CHARS: usize = 80;

pub(crate) fn criteria(inputs: &[CriterionInput]) -> Result<Vec<CriterionDraft>, String> {
    inputs
        .iter()
        .map(|input| {
            let method = match input.method.trim() {
                "inspection" => CriterionMethod::Inspection,
                "user" => CriterionMethod::User,
                other => CriterionMethod::Check {
                    kind: CheckKind::parse(other).ok_or_else(|| {
                        format!(
                            "unknown criterion method `{other}`: use a check kind (test, build, lint, typecheck, format, custom), inspection or user"
                        )
                    })?,
                },
            };
            Ok(CriterionDraft {
                id: None,
                text: input.text.clone(),
                method,
            })
        })
        .collect()
}

pub(crate) fn new_task(ids: &mut Ids, task: &NewTask) -> Result<(TaskId, Proposal), String> {
    let id = ids.task();
    let draft = TaskDraft {
        id: Some(id.clone()),
        parent: task.parent_id.as_deref().map(task_id),
        title: task.subject.trim().to_string(),
        active_form: task.active_form.trim().to_string(),
        description: task.description.trim().to_string(),
        kind: task.kind.unwrap_or_default(),
        priority: task.priority.unwrap_or_default(),
        depends_on: task.blocked_by.iter().map(|id| task_id(id)).collect(),
        requirements: task.requirements.iter().map(|id| entry_id(id)).collect(),
        scope: scope(&task.scope),
        acceptance: criteria(&task.acceptance)?,
        ..TaskDraft::default()
    };
    Ok((id, Proposal::CreateTask(Box::new(draft))))
}

/// Edits first, then dependencies, confirmed criteria and the report, the
/// status last, so a completion sees the rest of the same call.
pub(crate) fn task_change(
    state: &PlanState,
    change: &TaskChange,
    max_attempts: u32,
) -> Result<Vec<Proposal>, String> {
    let id = task_id(&change.task_id);
    let task = state
        .task(&id)
        .ok_or_else(|| format!("no task {id} in this plan; TaskList shows the tasks"))?;
    let mut out = Vec::new();
    let patch = TaskPatch {
        title: change.subject.clone(),
        active_form: change.active_form.clone(),
        description: change.description.clone(),
        kind: change.kind,
        priority: change.priority,
        scope: change.scope.as_deref().map(scope),
        result: change.result.clone(),
        ..TaskPatch::default()
    };
    if patch != TaskPatch::default() {
        out.push(Proposal::EditTask {
            task_id: id.clone(),
            patch: Box::new(patch),
        });
    }
    if let Some(acceptance) = &change.acceptance {
        out.push(Proposal::SetCriteria {
            task_id: id.clone(),
            criteria: criteria(acceptance)?,
        });
    }
    let dependency = |task_id: TaskId, depends_on: TaskId| Proposal::AddDependency {
        task_id,
        depends_on,
    };
    out.extend(
        change
            .add_blocked_by
            .iter()
            .map(|dep| dependency(id.clone(), task_id(dep))),
    );
    out.extend(
        change
            .add_blocks
            .iter()
            .map(|other| dependency(task_id(other), id.clone())),
    );
    out.extend(
        change
            .remove_blocked_by
            .iter()
            .map(|dep| Proposal::RemoveDependency {
                task_id: id.clone(),
                depends_on: task_id(dep),
            }),
    );
    out.extend(
        change
            .met_criteria
            .iter()
            .map(|criterion| Proposal::SetCriterion {
                task_id: id.clone(),
                criterion_id: criterion.trim().to_string(),
                status: CriterionStatus::Met,
            }),
    );
    if let Some(draft) = &change.report {
        out.push(Proposal::Report {
            task_id: id.clone(),
            report: Some(Box::new(draft.clone())),
        });
    }
    if let Some(status) = change.status.as_deref() {
        let reason = change.reason.as_deref().map(str::trim).unwrap_or_default();
        out.push(match status.trim() {
            "pending" if matches!(task.status, TaskStatus::Failed { .. } | TaskStatus::Blocked { .. }) => {
                Proposal::RetryTask { task_id: id }
            }
            "in_progress"
                if task.status != TaskStatus::Active
                    && dispatch::attempts_used(task) >= max_attempts =>
            {
                return Err(format!(
                    "task {id} was already started {} times (the limit is {max_attempts}); mark it failed with a reason or ask the user how to continue",
                    dispatch::attempts_used(task)
                ));
            }
            other => Proposal::SetTaskStatus {
                task_id: id,
                status: status_of(other, reason)?,
            },
        });
    }
    Ok(out)
}

fn status_of(status: &str, reason: &str) -> Result<TaskStatus, String> {
    let needs_reason = || format!("a {status} task needs a reason");
    Ok(match status {
        "pending" => TaskStatus::Pending,
        "in_progress" => TaskStatus::Active,
        "completed" => TaskStatus::Done,
        "deleted" | "cancelled" => TaskStatus::Cancelled,
        "blocked" if reason.is_empty() => return Err(needs_reason()),
        "blocked" => TaskStatus::Blocked {
            reason: BlockReason::Other {
                text: reason.to_string(),
            },
        },
        "failed" if reason.is_empty() => return Err(needs_reason()),
        "failed" => TaskStatus::Failed {
            reason: reason.to_string(),
        },
        other => {
            return Err(format!(
                "unknown status `{other}`: use pending, in_progress, completed, blocked, failed or deleted"
            ));
        }
    })
}

/// A `PlanRecord` call; `Some` id when it records a new entry or item.
pub(crate) fn note(
    ids: &mut Ids,
    note: &PlanNote,
) -> Result<(Vec<Proposal>, Option<EntryId>), String> {
    let text = note.text.trim().to_string();
    let tasks: Vec<TaskId> = note.task_ids.iter().map(|id| task_id(id)).collect();
    let body = match note.kind.trim() {
        "decision" => EntryBody::DesignDecision(DesignDecision {
            title: note
                .title
                .clone()
                .filter(|title| !title.trim().is_empty())
                .unwrap_or_else(|| text.chars().take(TITLE_CHARS).collect()),
            choice: text,
            alternatives: note.alternatives.clone(),
            reason: note.reason.clone().unwrap_or_default(),
        }),
        "discovery" => EntryBody::Discovery(Discovery {
            text,
            source: note.source.clone(),
        }),
        "experiment" => match note.experiment_id.as_deref() {
            Some(experiment) => return conclude(experiment, note).map(|p| (vec![p], None)),
            None => EntryBody::Experiment(Experiment {
                hypothesis: text,
                method: note.method.clone().unwrap_or_default(),
                result: None,
                conclusion: None,
                outcome: ExperimentOutcome::Open,
            }),
        },
        "evidence" => {
            let [task_id] = tasks.as_slice() else {
                return Err("evidence names exactly one task in taskIds".into());
            };
            let link = Proposal::LinkEvidence {
                task_id: task_id.clone(),
                criterion_id: note.criterion_id.clone(),
                evidence: EvidenceRef::Note { text },
            };
            return Ok((vec![link], None));
        }
        kind @ ("requirement" | "invariant") => {
            let kind = if kind == "requirement" {
                PlanItemKind::Requirement
            } else {
                PlanItemKind::Invariant
            };
            let id = ids.item(kind);
            let item = Proposal::AddItem {
                id: Some(id.clone()),
                kind,
                text,
            };
            return Ok((vec![item], Some(id)));
        }
        other => {
            return Err(format!(
                "unknown kind `{other}`: use decision, discovery, experiment, evidence, requirement or invariant"
            ));
        }
    };
    let id = ids.entry();
    let draft = EntryDraft {
        id: Some(id.clone()),
        tasks,
        files: note.files.clone(),
        ..EntryDraft::new(body)
    };
    Ok((vec![Proposal::Record(Box::new(draft))], Some(id)))
}

fn conclude(experiment: &str, note: &PlanNote) -> Result<Proposal, String> {
    let outcome = match note.outcome.as_deref().map(str::trim) {
        Some("confirmed") => ExperimentOutcome::Confirmed,
        Some("refuted") => ExperimentOutcome::Refuted,
        Some("inconclusive") => ExperimentOutcome::Inconclusive,
        _ => {
            return Err(
                "concluding an experiment needs outcome confirmed, refuted or inconclusive".into(),
            );
        }
    };
    Ok(Proposal::ConcludeExperiment {
        entry_id: entry_id(experiment),
        outcome,
        result: note.result.clone(),
        conclusion: note.conclusion.clone(),
    })
}

/// The goal, document and new requirements and invariants of a structured
/// plan; items the plan already has (same text) are not added again.
pub(crate) fn outline(ids: &mut Ids, outline: &PlanOutline) -> Vec<Proposal> {
    let state = ids.state();
    let goal = outline.goal.trim();
    let document = outline.document.trim();
    let mut out = Vec::new();
    let changed =
        |new: &str, old: &str| (!new.is_empty() && new != old.trim()).then(|| new.to_string());
    let (goal, document) = (
        changed(goal, &state.header.goal),
        changed(document, &state.header.document),
    );
    if goal.is_some() || document.is_some() {
        out.push(Proposal::EditPlan { goal, document });
    }
    let known: Vec<String> = state.items().map(|item| normalized(&item.text)).collect();
    let items = outline
        .requirements
        .iter()
        .map(|text| (PlanItemKind::Requirement, text))
        .chain(
            outline
                .invariants
                .iter()
                .map(|text| (PlanItemKind::Invariant, text)),
        );
    for (kind, text) in items {
        if text.trim().is_empty() || known.contains(&normalized(text)) {
            continue;
        }
        out.push(Proposal::AddItem {
            id: Some(ids.item(kind)),
            kind,
            text: text.trim().to_string(),
        });
    }
    out
}

fn normalized(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn scope(files: &[String]) -> TaskScope {
    TaskScope {
        files: files
            .iter()
            .map(|file| file.trim().to_string())
            .filter(|file| !file.is_empty())
            .collect(),
        symbols: Vec::new(),
    }
}

fn entry_id(raw: &str) -> EntryId {
    EntryId(raw.trim().to_string())
}
