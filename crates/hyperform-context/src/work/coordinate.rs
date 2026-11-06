//! Parallel work as the model reads it: a dispatched helper's first
//! message (its packet and the result contract), the coordinator's
//! reminder when a helper ends or its change comes back, the reminder for
//! an audit finding the user added, and the audit agent's request.

use hyperform_prompts::{reminders, work as layouts};
use hyperform_protocol::TaskId;
use hyperform_protocol::work::{AuditFinding, AuditRef, Task, TaskPacket};

use super::packet::{packet_text, task_status};
use super::phrases::{list, render};
use crate::reminders::wrap_reminder;
use crate::text::single_line;

/// A dispatched helper's first message: its packet, the result contract
/// and the caller's instructions.
pub fn helper_brief(packet: &TaskPacket, prompt: &str) -> String {
    let values = [
        ("task", packet.task.id.to_string()),
        ("packet", packet_text(packet)),
        ("prompt", prompt.trim().to_string()),
    ];
    render(layouts::HELPER, &values)
}

/// What the coordinator hears about one helper's task.
#[derive(Debug, Clone, Copy)]
pub struct HelperNews<'a> {
    pub agent: &'a str,
    /// The task as it is now (status, run report).
    pub task: &'a Task,
    /// What happened, such as `it ended and reported`.
    pub event: &'a str,
    /// Tasks this one's outcome made ready.
    pub ready: &'a [&'a Task],
    /// What `work_parallel` allows to run at once now.
    pub parallel: &'a [&'a Task],
    /// Tasks reconciliation findings hold until the plan change is accepted.
    pub held: &'a [&'a Task],
}

/// The coordinator's reminder about a helper.
pub fn work_coordinate(news: &HelperNews) -> String {
    let short = |task: &&Task| format!("{}: {}", task.id, single_line(&task.title));
    let summary = news
        .task
        .run
        .report
        .as_ref()
        .map(|report| single_line(&report.summary))
        .unwrap_or_default();
    let values = [
        ("agent", news.agent.to_string()),
        ("task", news.task.id.to_string()),
        ("event", news.event.to_string()),
        ("status", task_status(&news.task.status)),
        ("summary", summary),
        ("ready", list(news.ready.iter().map(short))),
        ("parallel", list(news.parallel.iter().map(short))),
        ("held", list(news.held.iter().map(short))),
    ];
    wrap_reminder(&render(reminders::WORK_COORDINATE, &values))
}

/// The user added `finding` to the plan as task `task`.
pub fn work_finding_added(finding: &AuditFinding, task: &TaskId) -> String {
    let refs: Vec<String> = finding.refs.iter().map(audit_ref).collect();
    let values = [
        ("task", task.to_string()),
        ("finding", single_line(&finding.text)),
        (
            "suggestion",
            finding
                .suggestion
                .as_deref()
                .map(single_line)
                .unwrap_or_default(),
        ),
        ("refs", refs.join(", ")),
    ];
    wrap_reminder(&render(reminders::WORK_FINDING_ADDED, &values))
}

/// The audit agent's request for the plan rendered as `plan_json`.
pub fn audit_request(plan_json: &str) -> String {
    render(layouts::AUDIT_REQUEST, &[("plan", plan_json.to_string())])
}

fn audit_ref(reference: &AuditRef) -> String {
    match reference {
        AuditRef::Task { task_id } => format!("task {task_id}"),
        AuditRef::Criterion {
            task_id,
            criterion_id,
        } => format!("criterion {criterion_id} of task {task_id}"),
        AuditRef::Item { item_id } => item_id.to_string(),
        AuditRef::File { path } => format!("file {path}"),
    }
}
