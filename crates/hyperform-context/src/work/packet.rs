//! The task packet: what `TaskGet` returns and the packet reminder wraps.

use hyperform_prompts::{reminders, work as layouts};
use hyperform_protocol::TodoStatus;
use hyperform_protocol::work::{CriterionMethod, CriterionStatus, Task, TaskPacket, TaskStatus};

use super::phrases::{block_reason, blocker, entry, evidence, items, line, list, render};
use crate::reminders::wrap_reminder;
use crate::text::single_line;

/// A task's status as `TaskUpdate` names it, with its reason.
pub fn task_status(status: &TaskStatus) -> String {
    match status {
        TaskStatus::Active => "in_progress".into(),
        TaskStatus::Done => "completed".into(),
        TaskStatus::Blocked { reason } => format!("blocked: {}", block_reason(reason)),
        TaskStatus::Failed { reason } => format!("failed: {}", single_line(reason)),
        TaskStatus::Superseded { by } => format!("superseded by {by}"),
        other => other.label().into(),
    }
}

/// `t1: Title (status)`.
pub fn task_line(task: &Task) -> String {
    format!(
        "{}: {} ({})",
        task.id,
        single_line(&task.title),
        task_status(&task.status)
    )
}

/// The packet as `TaskGet` returns it.
pub fn packet_text(packet: &TaskPacket) -> String {
    let task = &packet.task;
    let dropped = (packet.dropped > 0).then(|| packet.dropped.to_string());
    let values = [
        ("goal", single_line(&packet.goal)),
        ("invariants", items(&packet.invariants)),
        ("requirements", items(&packet.requirements)),
        ("task", task.id.to_string()),
        ("title", single_line(&task.title)),
        ("status", task_status(&task.status)),
        ("description", task.description.trim().to_string()),
        (
            "parent",
            packet.parent.as_ref().map(line).unwrap_or_default(),
        ),
        ("criteria", criteria(packet)),
        (
            "dependencies",
            list(packet.dependencies.iter().map(|dep| {
                let result = dep.result.as_deref().map(single_line);
                let head = format!(
                    "{}: {} ({})",
                    dep.id,
                    single_line(&dep.title),
                    task_status(&dep.status)
                );
                match result {
                    Some(result) => format!("{head}, result: {result}"),
                    None => head,
                }
            })),
        ),
        ("blockers", list(packet.blockers.iter().map(blocker))),
        ("scope", packet.scope.join(", ")),
        ("steps", steps(task)),
        (
            "resume",
            packet
                .resume
                .as_ref()
                .map(|r| single_line(&r.note))
                .unwrap_or_default(),
        ),
        (
            "entries",
            list(packet.entries.iter().map(|e| entry(&e.entry))),
        ),
        (
            "next",
            packet.next_ready.as_ref().map(line).unwrap_or_default(),
        ),
        ("dropped", dropped.unwrap_or_default()),
    ];
    render(layouts::PACKET, &values)
}

/// The full packet as a reminder.
pub fn work_packet(packet: &TaskPacket) -> String {
    let body = packet_text(packet);
    let values = [
        ("task", packet.task.id.to_string()),
        ("revision", packet.revision.to_string()),
        ("packet", body),
    ];
    wrap_reminder(&render(reminders::WORK_PACKET, &values))
}

fn criteria(packet: &TaskPacket) -> String {
    list(packet.task.acceptance.iter().map(|criterion| {
        let status = match criterion.status {
            CriterionStatus::Pending => "pending",
            CriterionStatus::Met => "met",
            CriterionStatus::Failed => "failed",
            CriterionStatus::Waived => "waived",
        };
        let method = match criterion.method {
            CriterionMethod::Check { kind } => format!("{} check", kind.label()),
            CriterionMethod::Inspection => "inspection".into(),
            CriterionMethod::User => "user".into(),
        };
        let mut text = format!(
            "{} [{status}] {} ({method})",
            criterion.id,
            single_line(&criterion.text)
        );
        let latest = packet
            .evidence
            .iter()
            .find(|e| e.criterion_id == criterion.id);
        if let Some(latest) = latest {
            let stale = if latest.stale { ", stale" } else { "" };
            text.push_str(&format!(
                "; evidence: {}{stale}",
                evidence(&latest.evidence)
            ));
        }
        text
    }))
}

fn steps(task: &Task) -> String {
    let marker = |status: TodoStatus| match status {
        TodoStatus::Completed => "[x]",
        TodoStatus::InProgress => "[~]",
        TodoStatus::Pending => "[ ]",
    };
    list(
        task.steps
            .iter()
            .map(|step| format!("{} {}", marker(step.status), single_line(&step.content))),
    )
}
