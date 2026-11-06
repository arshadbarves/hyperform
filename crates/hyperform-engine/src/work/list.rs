//! `TaskList`: the plan's tasks grouped by where they stand, with the next
//! ready task, open findings and plain counts for health.

use hyperform_context::{TaskHealth, TaskListView, task_list};
use hyperform_protocol::work::{CriterionStatus, PlanItemKind, PlanItemStatus, Task, TaskStatus};
use hyperform_work::{PlanState, graph};

pub(crate) fn list_text(state: &PlanState) -> String {
    let ready = graph::ready_tasks(state);
    let tasks: Vec<&Task> = graph::tree_order(state)
        .iter()
        .filter_map(|id| state.task(id))
        .collect();
    let mut view = TaskListView {
        header: &state.header,
        active: Vec::new(),
        ready: Vec::new(),
        waiting: Vec::new(),
        proposed: Vec::new(),
        closed: Vec::new(),
        next: graph::next_ready(state, None),
        findings: &state.findings,
        health: health(state, &tasks),
    };
    for task in tasks {
        let group = match task.status {
            TaskStatus::Active | TaskStatus::Verifying => &mut view.active,
            TaskStatus::Proposed => &mut view.proposed,
            _ if graph::is_closed(&task.status) => &mut view.closed,
            _ if ready.contains(&task.id) => &mut view.ready,
            _ => &mut view.waiting,
        };
        group.push(task);
    }
    task_list(&view)
}

fn health(state: &PlanState, tasks: &[&Task]) -> TaskHealth {
    let planned: Vec<&&Task> = tasks
        .iter()
        .filter(|task| {
            !matches!(
                task.status,
                TaskStatus::Proposed | TaskStatus::Cancelled | TaskStatus::Superseded { .. }
            )
        })
        .collect();
    let criteria = planned.iter().flat_map(|task| &task.acceptance);
    let requirements: Vec<_> = state
        .items()
        .filter(|item| item.kind == PlanItemKind::Requirement)
        .filter(|item| item.status == PlanItemStatus::Active)
        .collect();
    TaskHealth {
        tasks_done: planned
            .iter()
            .filter(|task| task.status == TaskStatus::Done)
            .count(),
        tasks: planned.len(),
        criteria_met: criteria
            .clone()
            .filter(|c| matches!(c.status, CriterionStatus::Met | CriterionStatus::Waived))
            .count(),
        criteria: criteria.count(),
        requirements_covered: requirements
            .iter()
            .filter(|item| {
                planned
                    .iter()
                    .any(|task| task.requirements.contains(&item.id))
            })
            .count(),
        requirements: requirements.len(),
    }
}
