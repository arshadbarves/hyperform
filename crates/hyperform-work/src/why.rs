//! `why(task)`: the records that shaped a task, each with the plan change
//! it belonged to and the discovery or experiment behind that change.

use hyperform_protocol::TaskId;
use hyperform_protocol::work::{Entry, EntryBody, WhyStep, WorkCause, WorkEvent, WorkRecord};

use crate::state::PlanState;

pub fn why(records: &[WorkRecord], state: &PlanState, task_id: &TaskId) -> Vec<WhyStep> {
    records
        .iter()
        .filter(|record| shapes(&record.event, task_id))
        .map(|record| WhyStep {
            revision: record.revision,
            at: record.at,
            actor: record.actor.clone(),
            cause: record.cause.clone(),
            event: record.event.clone(),
            entries: behind(record, state),
        })
        .collect()
}

/// Creation, edits, moves, status changes, holds and dependency changes of
/// the task, and ledger entries about it (not steps, evidence or binding).
fn shapes(event: &WorkEvent, task_id: &TaskId) -> bool {
    match event {
        WorkEvent::TaskCreated { task } => task.id == *task_id,
        WorkEvent::TaskEdited { task_id: id, .. }
        | WorkEvent::TaskMoved { task_id: id, .. }
        | WorkEvent::TaskTransitioned { task_id: id, .. }
        | WorkEvent::TaskHeld { task_id: id, .. }
        | WorkEvent::DependencyAdded { task_id: id, .. }
        | WorkEvent::DependencyRemoved { task_id: id, .. } => id == task_id,
        WorkEvent::EntryRecorded { entry } => {
            entry.tasks.contains(task_id) && !matches!(entry.body, EntryBody::Evidence(_))
        }
        _ => false,
    }
}

/// The plan change of the record's cause (or the change it records) and
/// the entry that change came from, as they are now.
fn behind(record: &WorkRecord, state: &PlanState) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    let mut push = |entry: Option<&Entry>| {
        if let Some(entry) = entry {
            if !out.iter().any(|seen| seen.id == entry.id) {
                out.push(entry.clone());
            }
        }
    };
    if let WorkCause::Change { entry_id } = &record.cause {
        let change = state.entry(entry_id);
        push(change);
        push(source_of(change, state));
    }
    if let WorkEvent::EntryRecorded { entry } = &record.event {
        push(source_of(Some(entry.as_ref()), state));
    }
    out
}

fn source_of<'a>(entry: Option<&Entry>, state: &'a PlanState) -> Option<&'a Entry> {
    match &entry?.body {
        EntryBody::PlanChange(change) => state.entry(change.source.as_ref()?),
        _ => None,
    }
}

#[cfg(test)]
#[path = "why_tests.rs"]
mod tests;
