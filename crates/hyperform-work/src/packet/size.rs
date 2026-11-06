//! Plain text of packet parts, used only to estimate their token cost (the
//! prompt rendering lives in `hyperform-context`), and evidence freshness.

use hyperform_protocol::work::{
    Entry, EntryBody, EvidenceRef, PacketBlocker, PacketTaskLine, PlanItem, Task,
};

use crate::facts::Facts;

pub(super) fn core_text(
    goal: &str,
    invariants: &[PlanItem],
    requirements: &[PlanItem],
    task: &Task,
    parent: Option<&PacketTaskLine>,
    blockers: &[PacketBlocker],
) -> String {
    let mut parts: Vec<&str> = vec![goal, &task.title, &task.active_form, &task.description];
    parts.extend(
        invariants
            .iter()
            .chain(requirements)
            .map(|i| i.text.as_str()),
    );
    parts.extend(task.acceptance.iter().map(|c| c.text.as_str()));
    parts.extend(task.steps.iter().map(|s| s.content.as_str()));
    parts.extend(
        task.scope
            .files
            .iter()
            .chain(&task.scope.symbols)
            .map(String::as_str),
    );
    let notes = &task.notes;
    parts.extend(
        notes
            .assumptions
            .iter()
            .chain(&notes.constraints)
            .chain(&notes.risks)
            .chain(&notes.rollback)
            .map(String::as_str),
    );
    if let Some(parent) = parent {
        parts.push(&parent.title);
    }
    let blockers = blockers
        .iter()
        .map(|blocker| match blocker {
            PacketBlocker::Waiting { task } => task.title.clone(),
            other => format!("{other:?}"),
        })
        .collect::<Vec<_>>()
        .join(" ");
    parts.push(&blockers);
    parts.join("\n")
}

pub(super) fn entry_text(entry: &Entry) -> String {
    let fields: Vec<&str> = match &entry.body {
        EntryBody::DesignDecision(d) => {
            let mut v = vec![d.title.as_str(), &d.choice, &d.reason];
            v.extend(d.alternatives.iter().map(String::as_str));
            v
        }
        EntryBody::Discovery(d) => vec![&d.text, d.source.as_deref().unwrap_or("")],
        EntryBody::Experiment(e) => vec![
            &e.hypothesis,
            &e.method,
            e.result.as_deref().unwrap_or(""),
            e.conclusion.as_deref().unwrap_or(""),
        ],
        EntryBody::Evidence(e) => return evidence_text(&e.evidence),
        EntryBody::PlanChange(c) => vec![&c.original, &c.new, &c.reason],
    };
    fields.join("\n")
}

pub(super) fn evidence_text(evidence: &EvidenceRef) -> String {
    match evidence {
        EvidenceRef::Check {
            record_id, label, ..
        } => format!("{label} {record_id}"),
        EvidenceRef::Diff { files, summary, .. } => format!("{summary} {}", files.join(" ")),
        EvidenceRef::Note { text } => text.clone(),
        EvidenceRef::External { url, title } => format!("{title} {url}"),
    }
}

/// A check that ran on another tree, or that the engine judged stale.
pub(super) fn is_stale(evidence: &EvidenceRef, facts: &Facts) -> bool {
    match evidence {
        EvidenceRef::Check {
            record_id,
            fingerprint,
            ..
        } => {
            facts.stale_records.contains(record_id)
                || facts
                    .fingerprint
                    .as_ref()
                    .is_some_and(|current| current != fingerprint)
        }
        _ => false,
    }
}
