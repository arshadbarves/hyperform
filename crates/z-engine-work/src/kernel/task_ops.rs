//! Task proposals other than status changes: create, edit, move, hold,
//! bind, dependencies and steps.

use z_engine_protocol::work::{PlanItemKind, Task, TaskPatch, TaskStatus, WorkEvent};
use z_engine_protocol::{EntryId, TaskId};

use super::{Ctx, Emitted, Outcome, criteria, fail, propagate, research};
use crate::authority::Role;
use crate::error::Reason;
use crate::graph::{cycle_if_added, descendants, is_closed};
use crate::proposal::{Proposal, TaskDraft};
use crate::state::PlanState;

pub(super) fn evaluate(state: &PlanState, ctx: &Ctx, proposal: &Proposal) -> Outcome {
    match proposal {
        Proposal::CreateTask(draft) => create(state, ctx, draft),
        Proposal::EditTask { task_id, patch } => edit(state, ctx, task_id, patch),
        Proposal::MoveTask {
            task_id,
            parent,
            index,
        } => move_task(state, ctx, task_id, parent.as_ref(), *index),
        Proposal::HoldTask { task_id, held } => {
            let task = find(state, task_id)?;
            if task.held == *held {
                return Ok(vec![]);
            }
            Ok(vec![ctx.emit(WorkEvent::TaskHeld {
                task_id: task_id.clone(),
                held: *held,
            })])
        }
        Proposal::BindTask { task_id, owner } => {
            let task = find(state, task_id)?;
            if is_closed(&task.status) || task.status == TaskStatus::Proposed {
                return fail(Reason::invalid(format!(
                    "task {task_id} is {} and cannot be bound",
                    task.status.label()
                )));
            }
            match &task.owner {
                Some(current) if current == owner => Ok(vec![]),
                Some(current) => fail(Reason::Owned {
                    task_id: task_id.clone(),
                    owner: current.agent_id.to_string(),
                }),
                None => Ok(vec![ctx.emit(WorkEvent::TaskBound {
                    task_id: task_id.clone(),
                    owner: owner.clone(),
                })]),
            }
        }
        Proposal::ReleaseTask { task_id } => {
            let task = find(state, task_id)?;
            if task.owner.is_none() {
                return Ok(vec![]);
            }
            Ok(vec![ctx.emit(WorkEvent::TaskReleased {
                task_id: task_id.clone(),
            })])
        }
        Proposal::AddDependency {
            task_id,
            depends_on,
        } => add_dependency(state, ctx, task_id, depends_on),
        Proposal::RemoveDependency {
            task_id,
            depends_on,
        } => {
            let task = find(state, task_id)?;
            if !task.depends_on.contains(depends_on) {
                return Ok(vec![]);
            }
            Ok(vec![ctx.emit(WorkEvent::DependencyRemoved {
                task_id: task_id.clone(),
                depends_on: depends_on.clone(),
            })])
        }
        Proposal::RecordSteps { task_id, steps } => {
            let task = find(state, task_id)?;
            if task.steps == *steps {
                return Ok(vec![]);
            }
            Ok(vec![ctx.emit(WorkEvent::StepsRecorded {
                task_id: task_id.clone(),
                steps: steps.clone(),
            })])
        }
        Proposal::SetCriteria { task_id, criteria } => criteria::set(state, ctx, task_id, criteria),
        _ => Ok(vec![]),
    }
}

pub(super) fn find<'a>(state: &'a PlanState, task_id: &TaskId) -> Result<&'a Task, Vec<Reason>> {
    state
        .task(task_id)
        .ok_or_else(|| vec![Reason::UnknownTask(task_id.clone())])
}

fn create(state: &PlanState, ctx: &Ctx, draft: &TaskDraft) -> Outcome {
    let mut reasons = Vec::new();
    let title = draft.title.trim();
    if title.is_empty() {
        reasons.push(Reason::invalid("a task needs a title"));
    }
    let id = draft.id.clone().unwrap_or_default();
    if state.task(&id).is_some() {
        reasons.push(Reason::DuplicateId(id.to_string()));
    }
    if let Some(parent) = draft.parent.as_ref().filter(|p| state.task(p).is_none()) {
        reasons.push(Reason::UnknownTask(parent.clone()));
    }
    let mut depends_on: Vec<TaskId> = Vec::new();
    for dep in &draft.depends_on {
        if *dep == id {
            reasons.push(Reason::Cycle {
                path: vec![id.clone(), id.clone()],
            });
        } else if state.task(dep).is_none() {
            reasons.push(Reason::UnknownTask(dep.clone()));
        } else if !depends_on.contains(dep) {
            depends_on.push(dep.clone());
        }
    }
    reasons.extend(unknown_requirements(state, &draft.requirements));
    let waits = matches!(ctx.base, Role::Worker { .. })
        || (ctx.role.is_research() && !research::drafting(state));
    let status = match &draft.status {
        None | Some(TaskStatus::Proposed | TaskStatus::Pending) if waits => TaskStatus::Proposed,
        None => TaskStatus::Pending,
        Some(status @ (TaskStatus::Proposed | TaskStatus::Pending)) => status.clone(),
        Some(_) => {
            reasons.push(Reason::invalid("a new task starts proposed or pending"));
            TaskStatus::Pending
        }
    };
    let acceptance = criteria::build(&id, &[], &draft.acceptance).unwrap_or_else(|more| {
        reasons.extend(more);
        Vec::new()
    });
    if !reasons.is_empty() {
        return Err(reasons);
    }
    let task = Task {
        id,
        parent: draft.parent.clone(),
        title: title.to_string(),
        active_form: draft.active_form.trim().to_string(),
        description: draft.description.clone(),
        kind: draft.kind,
        priority: draft.priority,
        order: 0,
        status,
        held: false,
        run_alone: draft.run_alone,
        depends_on,
        requirements: draft.requirements.clone(),
        scope: draft.scope.clone(),
        notes: draft.notes.clone(),
        acceptance,
        steps: Vec::new(),
        owner: None,
        result: None,
        resume: None,
        attempts: 0,
        run: Default::default(),
        provenance: ctx.role.provenance(),
        created_at: 0,
        updated_at: 0,
        created_revision: 0,
        revision: 0,
    };
    Ok(vec![ctx.emit(WorkEvent::TaskCreated {
        task: Box::new(task),
    })])
}

fn edit(state: &PlanState, ctx: &Ctx, task_id: &TaskId, patch: &TaskPatch) -> Outcome {
    let task = find(state, task_id)?;
    if patch.acceptance.is_some() {
        return fail(Reason::invalid("criteria change through SetCriteria"));
    }
    if patch.title.as_deref().is_some_and(|t| t.trim().is_empty()) {
        return fail(Reason::invalid("a task needs a title"));
    }
    if let Some(requirements) = &patch.requirements {
        let unknown = unknown_requirements(state, requirements);
        if !unknown.is_empty() {
            return Err(unknown);
        }
    }
    let effective = changed_fields(task, patch);
    if effective == TaskPatch::default() {
        return Ok(vec![]);
    }
    let reopen = task.status == TaskStatus::Done && effective.scope.is_some();
    let mut out: Emitted = vec![ctx.emit(WorkEvent::TaskEdited {
        task_id: task_id.clone(),
        patch: Box::new(effective),
    })];
    if reopen {
        out.extend(propagate::reopen(task));
    }
    Ok(out)
}

fn move_task(
    state: &PlanState,
    ctx: &Ctx,
    task_id: &TaskId,
    parent: Option<&TaskId>,
    index: u32,
) -> Outcome {
    let task = find(state, task_id)?;
    if let Some(parent) = parent {
        if state.task(parent).is_none() {
            return fail(Reason::UnknownTask(parent.clone()));
        }
        if parent == task_id || descendants(state, task_id).contains(parent) {
            return fail(Reason::invalid(
                "a task cannot move under itself or its subtasks",
            ));
        }
    }
    if task.parent.as_ref() == parent && task.order == index {
        return Ok(vec![]);
    }
    Ok(vec![ctx.emit(WorkEvent::TaskMoved {
        task_id: task_id.clone(),
        parent: parent.cloned(),
        index,
    })])
}

fn add_dependency(state: &PlanState, ctx: &Ctx, task_id: &TaskId, dep: &TaskId) -> Outcome {
    let task = find(state, task_id)?;
    find(state, dep)?;
    if task.depends_on.contains(dep) {
        return Ok(vec![]);
    }
    if let Some(path) = cycle_if_added(state, task_id, dep) {
        return fail(Reason::Cycle { path });
    }
    Ok(vec![ctx.emit(WorkEvent::DependencyAdded {
        task_id: task_id.clone(),
        depends_on: dep.clone(),
    })])
}

fn unknown_requirements(state: &PlanState, ids: &[EntryId]) -> Vec<Reason> {
    ids.iter()
        .filter(|id| {
            !state
                .item(id)
                .is_some_and(|item| item.kind == PlanItemKind::Requirement)
        })
        .map(|id| Reason::UnknownItem(id.clone()))
        .collect()
}

/// The fields of `patch` that differ from `task`.
pub(super) fn changed_fields(task: &Task, patch: &TaskPatch) -> TaskPatch {
    fn differs<T: PartialEq + Clone>(new: &Option<T>, old: &T) -> Option<T> {
        new.as_ref().filter(|new| *new != old).cloned()
    }
    TaskPatch {
        title: differs(
            &patch.title.as_ref().map(|t| t.trim().to_string()),
            &task.title,
        ),
        active_form: differs(&patch.active_form, &task.active_form),
        description: differs(&patch.description, &task.description),
        kind: differs(&patch.kind, &task.kind),
        priority: differs(&patch.priority, &task.priority),
        scope: differs(&patch.scope, &task.scope),
        notes: differs(&patch.notes, &task.notes),
        acceptance: None,
        requirements: differs(&patch.requirements, &task.requirements),
        run_alone: differs(&patch.run_alone, &task.run_alone),
        result: patch
            .result
            .as_ref()
            .filter(|result| task.result.as_ref() != Some(*result))
            .cloned(),
    }
}
