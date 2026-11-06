//! One-line phrases for the parts of a plan, shared by the packet and the
//! plan views, and the template plumbing they render through.

use z_engine_protocol::TaskId;
use z_engine_protocol::work::{
    BlockReason, Entry, EntryBody, EntryStatus, EvidenceRef, ExperimentOutcome, PacketBlocker,
    PacketTaskLine, PlanItem, PlanStatus, ReconcileFinding,
};

use super::packet::task_status;
use crate::template::render_template;
use crate::text::single_line;

pub(super) fn render(template: &str, values: &[(&str, String)]) -> String {
    let pairs: Vec<(&str, &str)> = values.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let text = render_template(template, &pairs);
    text.trim_end().to_string()
}

/// `\n- a\n- b`: the value of a `Label:{{list?}}` line.
pub(super) fn list(lines: impl Iterator<Item = String>) -> String {
    lines.map(|line| format!("\n- {line}")).collect()
}

pub(super) fn line(task: &PacketTaskLine) -> String {
    format!(
        "{}: {} ({})",
        task.id,
        single_line(&task.title),
        task_status(&task.status)
    )
}

pub(super) fn ids(ids: &[TaskId]) -> String {
    ids.iter()
        .map(TaskId::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn items(items: &[PlanItem]) -> String {
    list(
        items
            .iter()
            .map(|item| format!("{}: {}", item.id, single_line(&item.text))),
    )
}

pub(super) fn blocker(blocker: &PacketBlocker) -> String {
    match blocker {
        PacketBlocker::Blocked { reason } => block_reason(reason),
        PacketBlocker::Waiting { task } => format!("waits for {}", line(task)),
        PacketBlocker::Held => "held by the user".into(),
        PacketBlocker::Plan {
            status: PlanStatus::Replanning,
        } => "held until the plan change is accepted".into(),
        PacketBlocker::Plan { status } => format!("the plan is {}", status.label()),
    }
}

pub(super) fn block_reason(reason: &BlockReason) -> String {
    match reason {
        BlockReason::Dependency { task_id } => format!("dependency {task_id} failed"),
        BlockReason::DependencySuperseded { task_id, by } => {
            format!("dependency {task_id} was superseded by {by}")
        }
        BlockReason::MergeConflict { detail } => format!("merge conflict: {}", single_line(detail)),
        BlockReason::Unreported => "its agent ended without a report".into(),
        BlockReason::ChangeDiscarded => "the change was discarded".into(),
        BlockReason::Other { text } => single_line(text),
    }
}

fn outcome(outcome: ExperimentOutcome) -> &'static str {
    match outcome {
        ExperimentOutcome::Open => "open",
        ExperimentOutcome::Confirmed => "confirmed",
        ExperimentOutcome::Refuted => "refuted",
        ExperimentOutcome::Inconclusive => "inconclusive",
    }
}

pub(super) fn evidence(evidence: &EvidenceRef) -> String {
    match evidence {
        EvidenceRef::Check {
            passed,
            label,
            kind,
            ..
        } => {
            let outcome = if *passed { "passed" } else { "failed" };
            let kind = kind
                .map(|kind| format!("{} ", kind.label()))
                .unwrap_or_default();
            format!("{kind}check `{}` {outcome}", single_line(label))
        }
        EvidenceRef::Diff { files, summary, .. } => {
            format!("diff of {} ({})", files.join(", "), single_line(summary))
        }
        EvidenceRef::Note { text } => single_line(text),
        EvidenceRef::External { url, title } => format!("{} <{url}>", single_line(title)),
    }
}

pub(super) fn entry(entry: &Entry) -> String {
    let body = match &entry.body {
        EntryBody::DesignDecision(d) => {
            let mut text = format!(
                "decision {}: {}",
                single_line(&d.title),
                single_line(&d.choice)
            );
            if !d.alternatives.is_empty() {
                text.push_str(&format!("; rejected: {}", d.alternatives.join(", ")));
            }
            format!("{text}; reason: {}", single_line(&d.reason))
        }
        EntryBody::Discovery(d) => match &d.source {
            Some(source) => format!("discovery: {} [{source}]", single_line(&d.text)),
            None => format!("discovery: {}", single_line(&d.text)),
        },
        EntryBody::Experiment(x) => {
            let mut text = format!(
                "experiment: {} ({})",
                single_line(&x.hypothesis),
                outcome(x.outcome)
            );
            if let Some(conclusion) = &x.conclusion {
                text.push_str(&format!(": {}", single_line(conclusion)));
            }
            text
        }
        EntryBody::Evidence(e) => format!("evidence for {}: {}", e.task_id, evidence(&e.evidence)),
        EntryBody::PlanChange(c) => format!("plan change: {}", single_line(&c.reason)),
    };
    let unconfirmed = if entry.status == EntryStatus::Proposed {
        " (unconfirmed)"
    } else {
        ""
    };
    format!("{}: {body}{unconfirmed}", entry.id)
}

pub(super) fn finding(finding: &ReconcileFinding) -> String {
    match finding {
        ReconcileFinding::ScopeDeleted { task_id, files } => {
            format!(
                "{task_id}: files in its scope were deleted: {}",
                files.join(", ")
            )
        }
        ReconcileFinding::StaleEvidence {
            task_id,
            files,
            records,
        } => {
            let mut what: Vec<String> = Vec::new();
            if !files.is_empty() {
                what.push(format!("files in its scope changed: {}", files.join(", ")));
            }
            if !records.is_empty() {
                what.push(format!("stale checks: {}", records.join(", ")));
            }
            format!(
                "{task_id} (completed): its evidence may no longer hold; {}",
                what.join("; ")
            )
        }
        ReconcileFinding::DependencyFailed {
            task_id,
            depends_on,
        } => {
            format!("{task_id} depends on {depends_on}, which failed")
        }
        ReconcileFinding::DependencySuperseded {
            task_id,
            depends_on,
            by,
        } => {
            format!("{task_id} depends on {depends_on}, which {by} superseded")
        }
        ReconcileFinding::Overlap { tasks, files } => format!(
            "{} change the same files without an order between them: {}",
            ids(tasks),
            files.join(", ")
        ),
        ReconcileFinding::Duplicate { tasks, title } => {
            format!("{} have the same title: {}", ids(tasks), single_line(title))
        }
    }
}
