//! Relevance ranking of ledger entries for one task: linked to the task,
//! then to its ancestors, then to its dependencies, then sharing scope
//! files, then unlinked recent entries; newest first within a tier.
//! Dismissed and superseded entries, and entries about unrelated tasks,
//! are left out.

use std::collections::BTreeSet;

use z_engine_protocol::TaskId;
use z_engine_protocol::work::{Entry, EntryStatus, PacketReason, Task};

use crate::graph::ancestors;
use crate::scope::overlap;
use crate::state::PlanState;

pub(super) fn ranked<'a>(state: &'a PlanState, task: &Task) -> Vec<(&'a Entry, PacketReason)> {
    let ancestors: BTreeSet<TaskId> = ancestors(state, &task.id).into_iter().collect();
    let mut out: Vec<(&Entry, PacketReason)> = state
        .entries
        .values()
        .filter(|entry| {
            !matches!(
                entry.status,
                EntryStatus::Dismissed | EntryStatus::Superseded { .. }
            )
        })
        .filter_map(|entry| reason(entry, task, &ancestors).map(|reason| (entry, reason)))
        .collect();
    out.sort_by(|(a, ra), (b, rb)| {
        ra.cmp(rb)
            .then_with(|| b.revision.cmp(&a.revision))
            .then_with(|| b.id.cmp(&a.id))
    });
    out
}

fn reason(entry: &Entry, task: &Task, ancestors: &BTreeSet<TaskId>) -> Option<PacketReason> {
    let links = &entry.tasks;
    if links.contains(&task.id) {
        Some(PacketReason::Task)
    } else if links.iter().any(|id| ancestors.contains(id)) {
        Some(PacketReason::Ancestor)
    } else if links.iter().any(|id| task.depends_on.contains(id)) {
        Some(PacketReason::Dependency)
    } else if !overlap(&entry.files, &task.scope.files).is_empty() {
        Some(PacketReason::Scope)
    } else if links.is_empty() {
        Some(PacketReason::Recent)
    } else {
        None
    }
}
