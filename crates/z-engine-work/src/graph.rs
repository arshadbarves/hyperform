//! The task hierarchy (`parent`) and the dependency DAG (`depends_on`), and
//! the derived `Ready` / `Waiting` states. Traversals guard against loops
//! so a corrupt log cannot hang them.

use std::collections::{BTreeSet, VecDeque};

use z_engine_protocol::TaskId;
use z_engine_protocol::work::{PlanStatus, Task, TaskPriority, TaskStatus};

use crate::held;
use crate::state::PlanState;

/// Derived state of a task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Readiness {
    /// Pending, dependencies satisfied, not held, plan Active (or
    /// Replanning for findings that do not affect it).
    Ready,
    /// Pending with unsatisfied dependencies.
    Waiting { on: Vec<TaskId> },
    /// Pending and satisfied, but held by the user.
    Held,
    /// Pending and satisfied, but reconciliation findings affect it (see
    /// `held`): held until the user accepts the plan change.
    ReplanHeld,
    /// Pending and satisfied, but the plan is not Active.
    PlanNotActive(PlanStatus),
    /// Not Pending: the stored status says it all.
    NotPending,
}

/// A dependency no longer holds its dependents back: it is Done, or it was
/// Cancelled (marked not needed).
pub fn satisfies(status: &TaskStatus) -> bool {
    matches!(status, TaskStatus::Done | TaskStatus::Cancelled)
}

/// Finished for good: Done, Cancelled or Superseded.
pub fn is_closed(status: &TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Done | TaskStatus::Cancelled | TaskStatus::Superseded { .. }
    )
}

/// Planned work that is not closed (Pending, Active, Blocked, Verifying).
pub fn is_open(status: &TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Pending
            | TaskStatus::Active
            | TaskStatus::Blocked { .. }
            | TaskStatus::Verifying
    )
}

pub fn readiness(state: &PlanState, id: &TaskId) -> Option<Readiness> {
    let task = state.task(id)?;
    if task.status != TaskStatus::Pending {
        return Some(Readiness::NotPending);
    }
    let on = unsatisfied_dependencies(state, task);
    Some(if !on.is_empty() {
        Readiness::Waiting { on }
    } else if task.held {
        Readiness::Held
    } else if held::partial(state) {
        if held::holds(state, id) {
            Readiness::ReplanHeld
        } else {
            Readiness::Ready
        }
    } else if state.header.status != PlanStatus::Active {
        Readiness::PlanNotActive(state.header.status)
    } else {
        Readiness::Ready
    })
}

/// Dependencies that do not satisfy (unknown ids included).
pub fn unsatisfied_dependencies(state: &PlanState, task: &Task) -> Vec<TaskId> {
    task.depends_on
        .iter()
        .filter(|dep| !state.task(dep).is_some_and(|dep| satisfies(&dep.status)))
        .cloned()
        .collect()
}

/// Ready tasks, higher priority first, then in tree order.
pub fn ready_tasks(state: &PlanState) -> Vec<TaskId> {
    ranked(state, |readiness| *readiness == Readiness::Ready)
}

/// Tasks that would be Ready if the plan were Active, in the same order.
pub fn startable_tasks(state: &PlanState) -> Vec<TaskId> {
    ranked(state, |readiness| {
        matches!(
            readiness,
            Readiness::Ready | Readiness::ReplanHeld | Readiness::PlanNotActive(_)
        )
    })
}

/// Ready tasks and the ones only the findings hold, in the same order.
pub fn ready_or_replan_held(state: &PlanState) -> Vec<TaskId> {
    ranked(state, |readiness| {
        matches!(readiness, Readiness::Ready | Readiness::ReplanHeld)
    })
}

fn ranked(state: &PlanState, keep: impl Fn(&Readiness) -> bool) -> Vec<TaskId> {
    let mut found: Vec<(usize, &Task)> = tree_order(state)
        .iter()
        .enumerate()
        .filter(|(_, id)| readiness(state, id).is_some_and(|readiness| keep(&readiness)))
        .filter_map(|(index, id)| state.task(id).map(|task| (index, task)))
        .collect();
    found.sort_by_key(|(index, task)| (priority_rank(task.priority), *index));
    found.into_iter().map(|(_, task)| task.id.clone()).collect()
}

/// The first ready task other than `except`.
pub fn next_ready<'a>(state: &'a PlanState, except: Option<&TaskId>) -> Option<&'a Task> {
    ready_tasks(state)
        .into_iter()
        .find(|id| Some(id) != except)
        .and_then(|id| state.task(&id))
}

/// Every task, depth first by sibling order.
pub fn tree_order(state: &PlanState) -> Vec<TaskId> {
    let mut out = Vec::with_capacity(state.tasks.len());
    let mut seen = BTreeSet::new();
    let mut stack: Vec<TaskId> = state.ordered_children(None, None);
    stack.reverse();
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        let mut children = state.ordered_children(Some(&id), None);
        children.reverse();
        stack.extend(children);
        out.push(id);
    }
    // Orphans (a parent that does not exist) still appear, at the end.
    out.extend(state.tasks.keys().filter(|id| !seen.contains(*id)).cloned());
    out
}

pub fn children<'a>(state: &'a PlanState, id: &TaskId) -> Vec<&'a Task> {
    state
        .ordered_children(Some(id), None)
        .iter()
        .filter_map(|child| state.task(child))
        .collect()
}

/// Parent chain, nearest first.
pub fn ancestors(state: &PlanState, id: &TaskId) -> Vec<TaskId> {
    let mut out = Vec::new();
    let mut current = state.task(id).and_then(|task| task.parent.clone());
    while let Some(parent) = current {
        if parent == *id || out.contains(&parent) {
            break;
        }
        current = state.task(&parent).and_then(|task| task.parent.clone());
        out.push(parent);
    }
    out
}

pub fn descendants(state: &PlanState, id: &TaskId) -> BTreeSet<TaskId> {
    walk(id, |current| {
        state
            .tasks
            .values()
            .filter(|task| task.parent.as_ref() == Some(current))
            .map(|task| task.id.clone())
            .collect()
    })
}

/// Tasks that depend on `id` directly.
pub fn dependents(state: &PlanState, id: &TaskId) -> Vec<TaskId> {
    state
        .tasks
        .values()
        .filter(|task| task.depends_on.contains(id))
        .map(|task| task.id.clone())
        .collect()
}

pub fn transitive_dependents(state: &PlanState, id: &TaskId) -> BTreeSet<TaskId> {
    walk(id, |current| dependents(state, current))
}

pub fn transitive_dependencies(state: &PlanState, id: &TaskId) -> BTreeSet<TaskId> {
    walk(id, |current| {
        state
            .task(current)
            .map(|task| task.depends_on.clone())
            .unwrap_or_default()
    })
}

/// Either task (transitively) depends on the other.
pub fn ordered(state: &PlanState, a: &TaskId, b: &TaskId) -> bool {
    transitive_dependencies(state, a).contains(b) || transitive_dependencies(state, b).contains(a)
}

/// The cycle that `task depends_on dep` would close, as a path that starts
/// and ends at `task`; `None` when the edge is safe.
pub fn cycle_if_added(state: &PlanState, task: &TaskId, dep: &TaskId) -> Option<Vec<TaskId>> {
    if task == dep {
        return Some(vec![task.clone(), dep.clone()]);
    }
    let mut path = dependency_path(state, dep, task)?;
    path.insert(0, task.clone());
    Some(path)
}

/// A shortest `from -> ... -> to` path along `depends_on` edges.
pub fn dependency_path(state: &PlanState, from: &TaskId, to: &TaskId) -> Option<Vec<TaskId>> {
    let mut previous: std::collections::BTreeMap<TaskId, TaskId> = Default::default();
    let mut queue = VecDeque::from([from.clone()]);
    let mut seen = BTreeSet::from([from.clone()]);
    while let Some(current) = queue.pop_front() {
        if current == *to {
            let mut path = vec![current.clone()];
            let mut cursor = &current;
            while let Some(prev) = previous.get(cursor) {
                path.push(prev.clone());
                cursor = prev;
            }
            path.reverse();
            return Some(path);
        }
        for dep in state
            .task(&current)
            .map(|t| t.depends_on.as_slice())
            .unwrap_or_default()
        {
            if seen.insert(dep.clone()) {
                previous.insert(dep.clone(), current.clone());
                queue.push_back(dep.clone());
            }
        }
    }
    None
}

/// Any dependency cycle in the plan (validating a loaded log).
pub fn find_cycle(state: &PlanState) -> Option<Vec<TaskId>> {
    state.tasks.values().find_map(|task| {
        task.depends_on
            .iter()
            .find_map(|dep| dependency_path(state, dep, &task.id))
            .map(|mut path| {
                path.insert(0, task.id.clone());
                path
            })
    })
}

fn walk(start: &TaskId, next: impl Fn(&TaskId) -> Vec<TaskId>) -> BTreeSet<TaskId> {
    let mut seen = BTreeSet::new();
    let mut stack = next(start);
    while let Some(id) = stack.pop() {
        if id != *start && seen.insert(id.clone()) {
            stack.extend(next(&id));
        }
    }
    seen
}

fn priority_rank(priority: TaskPriority) -> u8 {
    match priority {
        TaskPriority::High => 0,
        TaskPriority::Normal => 1,
        TaskPriority::Low => 2,
    }
}

#[cfg(test)]
#[path = "graph_tests.rs"]
mod tests;
