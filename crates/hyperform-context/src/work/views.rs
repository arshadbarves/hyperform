//! Plan-wide views: the turn-start pin, the reconciliation reminder and
//! the `TaskList` result.

use hyperform_prompts::{reminders, work as layouts};
use hyperform_protocol::work::{PlanHeader, ReconcileFinding, Task};

use super::packet::task_line;
use super::phrases::{finding, ids, list, render};
use crate::reminders::wrap_reminder;
use crate::text::single_line;

/// What the turn-start pin shows.
#[derive(Debug, Clone, Copy)]
pub struct WorkPin<'a> {
    pub header: &'a PlanHeader,
    pub current: Option<&'a Task>,
    pub next: Option<&'a Task>,
}

/// The short turn-start pin: goal, current task, next ready task.
pub fn work_pin(pin: &WorkPin) -> String {
    let short = |task: &Task| format!("{}: {}", task.id, single_line(&task.title));
    let values = [
        ("goal", single_line(&pin.header.goal)),
        ("status", pin.header.status.label().to_string()),
        ("revision", pin.header.revision.to_string()),
        ("current", pin.current.map(short).unwrap_or_default()),
        ("next", pin.next.map(short).unwrap_or_default()),
    ];
    wrap_reminder(&render(reminders::WORK_PIN, &values))
}

/// Findings moved the plan to replanning.
pub fn work_reconcile(findings: &[ReconcileFinding]) -> String {
    let lines = finding_lines(findings);
    let values = [("findings", lines.trim_start().to_string())];
    wrap_reminder(&render(reminders::WORK_RECONCILE, &values))
}

/// One `- ...` line per finding, each after a newline.
pub fn finding_lines(findings: &[ReconcileFinding]) -> String {
    list(findings.iter().map(finding))
}

/// Counts for the task list's health line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TaskHealth {
    pub tasks_done: usize,
    pub tasks: usize,
    pub criteria_met: usize,
    pub criteria: usize,
    pub requirements_covered: usize,
    pub requirements: usize,
}

/// The plan's tasks, grouped by the caller.
#[derive(Debug, Clone)]
pub struct TaskListView<'a> {
    pub header: &'a PlanHeader,
    pub active: Vec<&'a Task>,
    pub ready: Vec<&'a Task>,
    pub waiting: Vec<&'a Task>,
    pub proposed: Vec<&'a Task>,
    pub closed: Vec<&'a Task>,
    pub next: Option<&'a Task>,
    pub findings: &'a [ReconcileFinding],
    pub health: TaskHealth,
}

/// The `TaskList` result.
pub fn task_list(view: &TaskListView) -> String {
    let tasks = |tasks: &[&Task]| list(tasks.iter().map(|task| listed(task)));
    let h = view.health;
    let health = format!(
        "tasks done {}/{} · criteria met {}/{} · requirements covered {}/{}",
        h.tasks_done, h.tasks, h.criteria_met, h.criteria, h.requirements_covered, h.requirements
    );
    let values = [
        ("goal", single_line(&view.header.goal)),
        ("status", view.header.status.label().to_string()),
        ("revision", view.header.revision.to_string()),
        ("active", tasks(&view.active)),
        ("ready", tasks(&view.ready)),
        ("waiting", tasks(&view.waiting)),
        ("proposed", tasks(&view.proposed)),
        ("closed", tasks(&view.closed)),
        ("next", view.next.map(task_line).unwrap_or_default()),
        ("findings", finding_lines(view.findings)),
        ("health", health),
    ];
    render(layouts::TASK_LIST, &values)
}

fn listed(task: &Task) -> String {
    let mut text = task_line(task);
    if let Some(parent) = &task.parent {
        text.push_str(&format!(", under {parent}"));
    }
    if !task.depends_on.is_empty() {
        text.push_str(&format!(", after {}", ids(&task.depends_on)));
    }
    text
}
