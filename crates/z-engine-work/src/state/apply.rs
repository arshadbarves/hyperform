//! Applying one record. Records were validated before they were appended,
//! so applying never fails: a record naming something unknown is counted
//! in `skipped` and otherwise ignored.

use z_engine_protocol::work::{
    Actor, Entry, EntryBody, PlanItem, PlanItemKind, PlanItemStatus, PlanStatus, Provenance, Task,
    TaskPatch, TaskStatus, WorkCause, WorkEvent, WorkRecord,
};
use z_engine_protocol::{EntryId, TaskId};

use super::{Changes, PlanState};

impl PlanState {
    pub fn apply(&mut self, record: &WorkRecord) -> Changes {
        let mut changes = Changes::default();
        self.seq = self.seq.max(record.seq);
        if record.revision > self.header.revision {
            self.header.revision = record.revision;
        }
        self.header.updated_at = self.header.updated_at.max(record.at);
        if let WorkCause::Change { entry_id } = &record.cause {
            changes.change = Some(entry_id.clone());
        }
        if !self.apply_event(record, &mut changes) {
            self.skipped += 1;
        }
        changes
    }

    /// False when the event named something unknown.
    fn apply_event(&mut self, record: &WorkRecord, changes: &mut Changes) -> bool {
        let rev = record.revision;
        match &record.event {
            WorkEvent::PlanCreated { .. } => return false,
            WorkEvent::PlanEdited { goal, document } => {
                if let Some(goal) = goal {
                    self.header.goal = goal.clone();
                }
                if let Some(document) = document {
                    self.header.document = document.clone();
                }
            }
            WorkEvent::PlanStatusChanged { to, .. } => {
                self.header.status = *to;
                self.replan_by_findings =
                    *to == PlanStatus::Replanning && record.actor == Actor::Kernel;
            }
            WorkEvent::ParallelChanged { parallel } => self.header.parallel = *parallel,
            WorkEvent::DepthChanged { depth } => self.header.depth = *depth,
            WorkEvent::ItemAdded { item } => {
                let mut item = item.clone();
                item.revision = rev;
                let list = match item.kind {
                    PlanItemKind::Requirement => &mut self.header.requirements,
                    PlanItemKind::Invariant => &mut self.header.invariants,
                };
                list.retain(|existing| existing.id != item.id);
                list.push(item);
            }
            WorkEvent::ItemEdited { item_id, text } => {
                let Some(item) = self.item_mut(item_id) else {
                    return false;
                };
                item.text = text.clone();
                item.revision = rev;
            }
            WorkEvent::ItemStatusChanged { item_id, status } => {
                let Some(item) = self.item_mut(item_id) else {
                    return false;
                };
                // An item the user accepted is the user's from then on.
                if item.status == PlanItemStatus::Proposed
                    && *status == PlanItemStatus::Active
                    && record.actor == Actor::User
                {
                    item.provenance = Provenance::User;
                }
                item.status = *status;
                item.revision = rev;
            }
            WorkEvent::TaskCreated { task } => {
                let mut task = Task::clone(task);
                task.order = self.sibling_count(task.parent.as_ref());
                task.created_at = record.at;
                task.updated_at = record.at;
                task.created_revision = rev;
                task.revision = rev;
                changes.tasks.insert(task.id.clone());
                self.tasks.insert(task.id.clone(), task);
            }
            WorkEvent::TaskMoved {
                task_id,
                parent,
                index,
            } => return self.move_task(record, task_id, parent.clone(), *index, changes),
            WorkEvent::EntryRecorded { entry } => {
                let mut entry = Entry::clone(entry);
                entry.created_at = record.at;
                entry.created_revision = rev;
                entry.revision = rev;
                changes.entries.insert(entry.id.clone());
                self.entries.insert(entry.id.clone(), entry);
            }
            WorkEvent::EntryStatusChanged { entry_id, status } => {
                let Some(entry) = self.entries.get_mut(entry_id) else {
                    return false;
                };
                entry.status = status.clone();
                entry.revision = rev;
                changes.entries.insert(entry_id.clone());
            }
            WorkEvent::ExperimentConcluded {
                entry_id,
                outcome,
                result,
                conclusion,
            } => {
                let Some(entry) = self.entries.get_mut(entry_id) else {
                    return false;
                };
                let EntryBody::Experiment(experiment) = &mut entry.body else {
                    return false;
                };
                experiment.outcome = *outcome;
                experiment.result.clone_from(result);
                experiment.conclusion.clone_from(conclusion);
                entry.revision = rev;
                changes.entries.insert(entry_id.clone());
            }
            WorkEvent::Reconciled { findings } => self.findings = findings.clone(),
            WorkEvent::Audited { findings } => self.apply_audited(findings, rev),
            WorkEvent::FindingStatusChanged { finding_id, status } => {
                return self.set_finding_status(finding_id, *status, rev);
            }
            WorkEvent::CheckpointLinked { fingerprint, .. } => {
                self.fingerprint = Some(fingerprint.clone());
            }
            WorkEvent::Rejected { .. } => {}
            event => return self.apply_task_event(record, event, changes),
        }
        true
    }

    /// Events that change one existing task.
    fn apply_task_event(
        &mut self,
        record: &WorkRecord,
        event: &WorkEvent,
        changes: &mut Changes,
    ) -> bool {
        let Some(task_id) = task_of(event) else {
            return false;
        };
        let Some(task) = self.tasks.get_mut(task_id) else {
            return false;
        };
        match event {
            WorkEvent::TaskEdited { patch, .. } => apply_patch(task, patch),
            WorkEvent::TaskTransitioned { to, .. } => {
                if *to == TaskStatus::Active && task.status != TaskStatus::Active {
                    task.attempts += 1;
                    task.resume = None;
                }
                task.status = to.clone();
            }
            WorkEvent::TaskHeld { held, .. } => task.held = *held,
            WorkEvent::TaskBound { owner, .. } => task.owner = Some(owner.clone()),
            WorkEvent::TaskReleased { .. } => task.owner = None,
            WorkEvent::DependencyAdded { depends_on, .. } => {
                if !task.depends_on.contains(depends_on) {
                    task.depends_on.push(depends_on.clone());
                }
            }
            WorkEvent::DependencyRemoved { depends_on, .. } => {
                task.depends_on.retain(|dep| dep != depends_on);
            }
            WorkEvent::StepsRecorded { steps, .. } => task.steps.clone_from(steps),
            WorkEvent::CriterionChanged {
                criterion_id,
                status,
                ..
            } => {
                let Some(criterion) = task.acceptance.iter_mut().find(|c| c.id == *criterion_id)
                else {
                    return false;
                };
                criterion.status = *status;
            }
            WorkEvent::EvidenceLinked {
                criterion_id,
                evidence,
                ..
            } => {
                if let Some(criterion_id) = criterion_id {
                    let Some(criterion) =
                        task.acceptance.iter_mut().find(|c| c.id == *criterion_id)
                    else {
                        return false;
                    };
                    criterion.evidence.push(evidence.clone());
                }
            }
            event => {
                if !super::run::apply(task, event) {
                    return false;
                }
            }
        }
        task.revision = record.revision;
        task.updated_at = record.at;
        changes.tasks.insert(task_id.clone());
        true
    }

    fn move_task(
        &mut self,
        record: &WorkRecord,
        task_id: &TaskId,
        parent: Option<TaskId>,
        index: u32,
        changes: &mut Changes,
    ) -> bool {
        let Some(task) = self.tasks.get_mut(task_id) else {
            return false;
        };
        let old_parent = std::mem::replace(&mut task.parent, parent.clone());
        task.revision = record.revision;
        task.updated_at = record.at;
        changes.tasks.insert(task_id.clone());
        let mut siblings = self.ordered_children(parent.as_ref(), Some(task_id));
        let at = usize::try_from(index)
            .unwrap_or(usize::MAX)
            .min(siblings.len());
        siblings.insert(at, task_id.clone());
        self.renumber(&siblings, record, changes);
        if old_parent != parent {
            let old = self.ordered_children(old_parent.as_ref(), None);
            self.renumber(&old, record, changes);
        }
        true
    }

    fn renumber(&mut self, ids: &[TaskId], record: &WorkRecord, changes: &mut Changes) {
        for (order, id) in ids.iter().enumerate() {
            let Some(task) = self.tasks.get_mut(id) else {
                continue;
            };
            let order = u32::try_from(order).unwrap_or(u32::MAX);
            if task.order != order {
                task.order = order;
                task.revision = record.revision;
                task.updated_at = record.at;
                changes.tasks.insert(id.clone());
            }
        }
    }

    /// Children of `parent` by order, leaving out `except`.
    pub(crate) fn ordered_children(
        &self,
        parent: Option<&TaskId>,
        except: Option<&TaskId>,
    ) -> Vec<TaskId> {
        let mut children: Vec<&Task> = self
            .tasks
            .values()
            .filter(|task| task.parent.as_ref() == parent && Some(&task.id) != except)
            .collect();
        children.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
        children.into_iter().map(|task| task.id.clone()).collect()
    }

    fn sibling_count(&self, parent: Option<&TaskId>) -> u32 {
        let count = self
            .tasks
            .values()
            .filter(|task| task.parent.as_ref() == parent)
            .count();
        u32::try_from(count).unwrap_or(u32::MAX)
    }

    fn item_mut(&mut self, id: &EntryId) -> Option<&mut PlanItem> {
        self.header
            .requirements
            .iter_mut()
            .chain(self.header.invariants.iter_mut())
            .find(|item| item.id == *id)
    }
}

fn task_of(event: &WorkEvent) -> Option<&TaskId> {
    match event {
        WorkEvent::TaskEdited { task_id, .. }
        | WorkEvent::TaskTransitioned { task_id, .. }
        | WorkEvent::TaskHeld { task_id, .. }
        | WorkEvent::TaskBound { task_id, .. }
        | WorkEvent::TaskReleased { task_id }
        | WorkEvent::DependencyAdded { task_id, .. }
        | WorkEvent::DependencyRemoved { task_id, .. }
        | WorkEvent::StepsRecorded { task_id, .. }
        | WorkEvent::CriterionChanged { task_id, .. }
        | WorkEvent::EvidenceLinked { task_id, .. }
        | WorkEvent::TaskDispatched { task_id, .. }
        | WorkEvent::TaskReported { task_id, .. }
        | WorkEvent::TaskIntegrated { task_id, .. }
        | WorkEvent::TaskInterrupted { task_id, .. }
        | WorkEvent::ResumeWorktreeDiscarded { task_id }
        | WorkEvent::TaskRetried { task_id, .. } => Some(task_id),
        _ => None,
    }
}

fn apply_patch(task: &mut Task, patch: &TaskPatch) {
    if let Some(title) = &patch.title {
        task.title = title.clone();
    }
    if let Some(active_form) = &patch.active_form {
        task.active_form = active_form.clone();
    }
    if let Some(description) = &patch.description {
        task.description = description.clone();
    }
    if let Some(kind) = patch.kind {
        task.kind = kind;
    }
    if let Some(priority) = patch.priority {
        task.priority = priority;
    }
    if let Some(scope) = &patch.scope {
        task.scope = scope.clone();
    }
    if let Some(notes) = &patch.notes {
        task.notes = notes.clone();
    }
    if let Some(acceptance) = &patch.acceptance {
        task.acceptance = acceptance.clone();
    }
    if let Some(requirements) = &patch.requirements {
        task.requirements = requirements.clone();
    }
    if let Some(run_alone) = patch.run_alone {
        task.run_alone = run_alone;
    }
    if let Some(result) = &patch.result {
        task.result = Some(result.clone());
    }
}
