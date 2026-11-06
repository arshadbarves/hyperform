//! `PlanState`: the fold of a plan log, plus the views the GUI receives.

mod apply;
mod audit;
mod run;

use std::collections::{BTreeMap, BTreeSet};

use hyperform_protocol::work::{
    AuditFinding, Entry, PlanHeader, PlanItem, PlanSummary, ReconcileFinding, Task, TaskStatus,
    WorkEvent, WorkRecord, WorkSnapshot, WorkUpdate,
};
use hyperform_protocol::{EntryId, TaskId};

use crate::error::FoldError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanState {
    pub header: PlanHeader,
    /// Keyed by id; generated ids are time-ordered.
    pub tasks: BTreeMap<TaskId, Task>,
    pub entries: BTreeMap<EntryId, Entry>,
    /// Findings of the latest reconciliation.
    pub findings: Vec<ReconcileFinding>,
    /// Audit findings in the order first found, resolved ones included.
    pub audit: Vec<AuditFinding>,
    /// The workspace fingerprint of the latest `CheckpointLinked`.
    pub fingerprint: Option<String>,
    /// The kernel moved the plan to Replanning (reconciliation findings),
    /// not the user: only the tasks the findings affect are held.
    pub replan_by_findings: bool,
    /// `seq` of the last applied record.
    pub seq: u64,
    /// Records that named something unknown and were ignored.
    pub skipped: u32,
}

/// What one or more records changed, for building a `WorkUpdate`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    pub tasks: BTreeSet<TaskId>,
    pub entries: BTreeSet<EntryId>,
    /// The `PlanChange` entry the records belong to.
    pub change: Option<EntryId>,
}

impl Changes {
    pub fn merge(&mut self, other: Changes) {
        self.tasks.extend(other.tasks);
        self.entries.extend(other.entries);
        if other.change.is_some() {
            self.change = other.change;
        }
    }
}

impl PlanState {
    /// Fold a whole plan log; it must start with `PlanCreated`.
    pub fn fold(records: &[WorkRecord]) -> Result<Self, FoldError> {
        let (first, rest) = records.split_first().ok_or(FoldError::Empty)?;
        let mut state = Self::created(first)?;
        state.apply_all(rest);
        Ok(state)
    }

    /// The state right after a `PlanCreated` record.
    pub fn created(record: &WorkRecord) -> Result<Self, FoldError> {
        let WorkEvent::PlanCreated {
            plan_id,
            project_root,
            goal,
            document,
            depth,
            status,
        } = &record.event
        else {
            return Err(FoldError::NotAPlanLog);
        };
        Ok(Self {
            header: PlanHeader {
                plan_id: plan_id.clone(),
                project_root: project_root.clone(),
                goal: goal.clone(),
                document: document.clone(),
                status: *status,
                depth: *depth,
                parallel: Default::default(),
                revision: record.revision,
                requirements: Vec::new(),
                invariants: Vec::new(),
                created_at: record.at,
                updated_at: record.at,
            },
            tasks: BTreeMap::new(),
            entries: BTreeMap::new(),
            findings: Vec::new(),
            audit: Vec::new(),
            fingerprint: None,
            replan_by_findings: false,
            seq: record.seq,
            skipped: 0,
        })
    }

    pub fn apply_all(&mut self, records: &[WorkRecord]) -> Changes {
        let mut changes = Changes::default();
        for record in records {
            changes.merge(self.apply(record));
        }
        changes
    }

    pub fn revision(&self) -> u64 {
        self.header.revision
    }

    pub fn task(&self, id: &TaskId) -> Option<&Task> {
        self.tasks.get(id)
    }

    pub fn entry(&self, id: &EntryId) -> Option<&Entry> {
        self.entries.get(id)
    }

    /// A requirement or invariant.
    pub fn item(&self, id: &EntryId) -> Option<&PlanItem> {
        self.items().find(|item| item.id == *id)
    }

    pub fn items(&self) -> impl Iterator<Item = &PlanItem> {
        self.header
            .requirements
            .iter()
            .chain(&self.header.invariants)
    }

    pub fn finding(&self, id: &str) -> Option<&AuditFinding> {
        self.audit.iter().find(|finding| finding.id == id)
    }

    pub fn snapshot(&self) -> WorkSnapshot {
        WorkSnapshot {
            plan: self.header.clone(),
            tasks: self.tasks.values().cloned().collect(),
            entries: self.entries.values().cloned().collect(),
            findings: self.findings.clone(),
            audit: self.audit.clone(),
            health: crate::health::health(self),
            replan_held: crate::held::replan_held(self).into_iter().collect(),
        }
    }

    /// The upserts of `changes` against the current state, with the
    /// current findings and health.
    pub fn update(&self, changes: &Changes) -> WorkUpdate {
        WorkUpdate {
            plan_id: self.header.plan_id.clone(),
            revision: self.revision(),
            plan: self.header.clone(),
            tasks: changes
                .tasks
                .iter()
                .filter_map(|id| self.tasks.get(id).cloned())
                .collect(),
            entries: changes
                .entries
                .iter()
                .filter_map(|id| self.entries.get(id).cloned())
                .collect(),
            change: changes.change.clone(),
            audit: self.audit.clone(),
            findings: self.findings.clone(),
            health: crate::health::health(self),
            replan_held: crate::held::replan_held(self).into_iter().collect(),
        }
    }

    /// Progress counts leave out proposed, cancelled and superseded tasks.
    /// `last_session_id` and `attached_to` are the engine's to fill in.
    pub fn summary(&self) -> PlanSummary {
        let planned = self.tasks.values().filter(|task| {
            !matches!(
                task.status,
                TaskStatus::Proposed | TaskStatus::Cancelled | TaskStatus::Superseded { .. }
            )
        });
        let (total, done) = planned.fold((0u32, 0u32), |(total, done), task| {
            (total + 1, done + u32::from(task.status == TaskStatus::Done))
        });
        let count = |keep: fn(&TaskStatus) -> bool| {
            let found = self
                .tasks
                .values()
                .filter(|task| keep(&task.status))
                .count();
            u32::try_from(found).unwrap_or(u32::MAX)
        };
        PlanSummary {
            plan_id: self.header.plan_id.clone(),
            project_root: self.header.project_root.clone(),
            goal: self.header.goal.clone(),
            status: self.header.status,
            depth: self.header.depth,
            revision: self.revision(),
            tasks_total: total,
            tasks_done: done,
            blocked: count(|status| matches!(status, TaskStatus::Blocked { .. })),
            needs_you: crate::needs::needs_you(self),
            running: count(|status| *status == TaskStatus::Active),
            last_session_id: None,
            attached_to: None,
            created_at: self.header.created_at,
            updated_at: self.header.updated_at,
        }
    }
}

#[cfg(test)]
mod tests;
