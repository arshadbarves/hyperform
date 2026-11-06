//! `PlanHandle`: one open plan. Its lock serializes every writer: a
//! submission is decided against the current state, appended and synced,
//! and only then applied and announced to the sessions watching the plan.
//! After a failed write the plan stays readable but refuses changes until
//! it is opened again. meta.json follows every revision and attachment.

use std::sync::{Arc, Mutex, Weak};

use hyperform_protocol::work::{
    ChangeCause, EntryBody, PlanStatus, PlanSummary, WorkCause, WorkEvent, WorkRecord, WorkUpdate,
};
use hyperform_protocol::{EntryId, Event, PlanId};
use hyperform_store::{PlanLog, PlanStore};
use hyperform_work::{Changes, Facts, PlanState, Rejection, Submission, decide, rejected};

use super::driver::Driver;
use crate::session::Emitter;
use crate::sync::lock;

/// What became of a submission.
#[derive(Debug)]
pub(crate) enum Submitted {
    /// The records of the new revision.
    Accepted(Vec<WorkRecord>),
    /// Valid, but it changes nothing.
    Unchanged,
    Rejected(Rejection),
}

#[derive(Debug)]
pub(crate) struct PlanHandle {
    pub id: PlanId,
    /// The project key of the plan directory.
    key: String,
    store: PlanStore,
    slot: Mutex<Slot>,
    watchers: Mutex<Vec<Weak<Emitter>>>,
    pub(super) driver: Mutex<Driver>,
    /// Held from a dispatch's first check until its task is bound, so
    /// helpers started together are admitted one at a time.
    admissions: Arc<tokio::sync::Mutex<()>>,
}

#[derive(Debug)]
struct Slot {
    state: PlanState,
    records: Vec<WorkRecord>,
    /// `None` once the plan is read-only.
    log: Option<PlanLog>,
    read_only: Option<String>,
}

impl PlanHandle {
    pub(crate) fn load(store: PlanStore, key: String, plan_id: &PlanId) -> Result<Self, String> {
        let loaded = store
            .load(&key, plan_id)
            .map_err(|error| format!("plan {plan_id} could not be read: {error}"))?;
        if loaded.torn_tail || loaded.corrupt_lines > 0 {
            tracing::warn!(plan = %plan_id, corrupt = loaded.corrupt_lines, torn = loaded.torn_tail, "skipped damaged plan log lines");
        }
        let state = PlanState::fold(&loaded.records)
            .map_err(|error| format!("plan {plan_id} could not be read: {error}"))?;
        let (log, read_only) = match store.open_append(&key, plan_id) {
            Ok(log) => (Some(log), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let driver = store.read_meta(&key, plan_id).ok();
        let handle = Self::new(store, key, state, loaded.records, log, read_only);
        if let Some(meta) = driver {
            *lock(&handle.driver) = Driver::recorded(&meta);
        }
        Ok(handle)
    }

    /// Writes a new plan directory holding `records`.
    pub(crate) fn create(
        store: PlanStore,
        key: String,
        records: Vec<WorkRecord>,
    ) -> Result<Self, String> {
        let state = PlanState::fold(&records).map_err(|error| error.to_string())?;
        let log = store
            .create(&key, &records, &state.summary())
            .map_err(|error| format!("the plan could not be saved: {error}"))?;
        Ok(Self::new(store, key, state, records, Some(log), None))
    }

    fn new(
        store: PlanStore,
        key: String,
        state: PlanState,
        records: Vec<WorkRecord>,
        log: Option<PlanLog>,
        read_only: Option<String>,
    ) -> Self {
        Self {
            id: state.header.plan_id.clone(),
            key,
            store,
            slot: Mutex::new(Slot {
                state,
                records,
                log,
                read_only,
            }),
            watchers: Mutex::default(),
            driver: Mutex::default(),
            admissions: Arc::default(),
        }
    }

    pub(crate) fn key(&self) -> &str {
        &self.key
    }

    /// Waits for the dispatches admitted before to be bound.
    pub(crate) async fn admitting(&self) -> tokio::sync::OwnedMutexGuard<()> {
        Arc::clone(&self.admissions).lock_owned().await
    }

    pub(crate) fn read<R>(&self, view: impl FnOnce(&PlanState) -> R) -> R {
        view(&lock(&self.slot).state)
    }

    pub(crate) fn read_log<R>(&self, view: impl FnOnce(&[WorkRecord], &PlanState) -> R) -> R {
        let slot = lock(&self.slot);
        view(&slot.records, &slot.state)
    }

    /// Every task and entry as upserts, for a session that just attached.
    pub(crate) fn full_update(&self) -> WorkUpdate {
        self.read(|state| {
            state.update(&Changes {
                tasks: state.tasks.keys().cloned().collect(),
                entries: state.entries.keys().cloned().collect(),
                change: None,
            })
        })
    }

    /// The plan's list entry, naming the chat that drives it.
    pub(crate) fn summary(&self) -> PlanSummary {
        let mut summary = self.read(PlanState::summary);
        self.stamp(&mut summary);
        summary
    }

    /// Writes meta.json now (after an attachment).
    pub(crate) fn save_meta(&self) {
        let summary = self.summary();
        if let Err(error) = self.store.write_meta(&self.key, &summary) {
            tracing::warn!(plan = %self.id, %error, "plan summary not updated");
        }
    }

    /// Why the plan refuses changes, if it does.
    pub(crate) fn read_only(&self) -> Option<String> {
        lock(&self.slot).read_only.clone()
    }

    /// `events` hears `workUpdated` for every accepted revision.
    pub(crate) fn watch(&self, events: &Arc<Emitter>) {
        let mut watchers = lock(&self.watchers);
        watchers.retain(|weak| weak.strong_count() > 0);
        if !watchers
            .iter()
            .any(|weak| weak.as_ptr() == Arc::as_ptr(events))
        {
            watchers.push(Arc::downgrade(events));
        }
    }

    pub(crate) fn unwatch(&self, events: &Arc<Emitter>) {
        lock(&self.watchers)
            .retain(|weak| weak.strong_count() > 0 && weak.as_ptr() != Arc::as_ptr(events));
    }

    /// Decides the submission `build` makes from the current state. A
    /// rejection is kept in the log as an audit record.
    pub(crate) fn submit(
        &self,
        facts: &Facts,
        build: impl FnOnce(&PlanState) -> Result<Submission, String>,
    ) -> Result<Submitted, String> {
        let (outcome, update) = {
            let mut slot = lock(&self.slot);
            let mut submission = build(&slot.state)?;
            if let Some(entry_id) = open_change(&slot.records, &slot.state) {
                submission.cause = grouped(submission.cause, entry_id);
            }
            match decide(&slot.state, &submission, facts) {
                Ok(records) if records.is_empty() => (Submitted::Unchanged, None),
                Ok(records) => {
                    slot.write(&records)?;
                    let changes = slot.state.apply_all(&records);
                    slot.records.extend(records.iter().cloned());
                    let update = slot.state.update(&changes);
                    self.write_meta(&slot.state);
                    (Submitted::Accepted(records), Some(update))
                }
                Err(rejection) => {
                    let record = rejected(&slot.state, &submission, &rejection, facts);
                    let audit = std::slice::from_ref(&record);
                    match slot.write(audit) {
                        Ok(()) => {
                            slot.state.apply_all(audit);
                            slot.records.push(record);
                        }
                        Err(error) => {
                            tracing::warn!(plan = %self.id, %error, "rejection not logged")
                        }
                    }
                    (Submitted::Rejected(rejection), None)
                }
            }
        };
        if let Some(update) = update {
            self.announce(&update);
        }
        Ok(outcome)
    }

    fn announce(&self, update: &WorkUpdate) {
        let watchers: Vec<Arc<Emitter>> = lock(&self.watchers)
            .iter()
            .filter_map(Weak::upgrade)
            .collect();
        for events in watchers {
            events.emit(Event::WorkUpdated {
                update: Box::new(update.clone()),
            });
        }
    }

    fn write_meta(&self, state: &PlanState) {
        let mut summary = state.summary();
        self.stamp(&mut summary);
        if let Err(error) = self.store.write_meta(&self.key, &summary) {
            tracing::warn!(plan = %self.id, %error, "plan summary not updated");
        }
    }
}

impl Slot {
    fn write(&mut self, records: &[WorkRecord]) -> Result<(), String> {
        let Some(log) = self.log.as_mut() else {
            let why = self
                .read_only
                .as_deref()
                .unwrap_or("it could not be opened");
            return Err(format!(
                "the plan is read-only ({why}); detach and attach it again"
            ));
        };
        if let Err(error) = log.append_all(records).and_then(|()| log.sync()) {
            self.log = None;
            self.read_only = Some(error.to_string());
            return Err(format!(
                "the plan could not be saved ({error}); it is read-only until it is opened again"
            ));
        }
        Ok(())
    }
}

/// While the plan is Replanning: the `PlanChange` entry recorded when it
/// entered Replanning, which every later change belongs to (a resume's
/// entry only when resuming moved the plan to Replanning).
pub(crate) fn open_change(records: &[WorkRecord], state: &PlanState) -> Option<EntryId> {
    if state.header.status != PlanStatus::Replanning {
        return None;
    }
    let since = records.iter().rev().find_map(|record| match record.event {
        WorkEvent::PlanStatusChanged {
            to: PlanStatus::Replanning,
            ..
        } => Some(record.revision),
        _ => None,
    })?;
    state
        .entries
        .values()
        .filter(|entry| entry.created_revision >= since)
        .filter(|entry| match &entry.body {
            EntryBody::PlanChange(change) => {
                change.cause != ChangeCause::Resume || entry.created_revision == since
            }
            _ => false,
        })
        .max_by_key(|entry| (entry.created_revision, entry.created_at))
        .map(|entry| entry.id.clone())
}

/// Tool calls and edits join the open change; approvals, reconciliation
/// and compaction keep their own cause.
fn grouped(cause: WorkCause, entry_id: EntryId) -> WorkCause {
    match cause {
        WorkCause::Tool { .. } | WorkCause::Edit => WorkCause::Change { entry_id },
        other => other,
    }
}
