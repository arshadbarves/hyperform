//! One chat drives a plan at a time: the chat that attached it last
//! (`PlanSummary::last_session_id`, kept in meta.json), which holds it
//! until it lets it go (`attached_to`, cleared by a detach). Attaching a
//! plan another live chat holds moves it: that chat detaches it (logging
//! `PlanDetached`), its task helpers stop with resume pointers (each
//! records its own, so resuming in the new chat leaves their tasks), and it
//! gets a snapshot without the plan and a notice. A chat that was closed
//! when the plan moved detaches it the next time it opens.

use std::sync::{Arc, Weak};

use hyperform_protocol::work::PlanSummary;
use hyperform_protocol::{AgentId, NoticeLevel, SessionId};
use hyperform_store::LogRecord;

use super::bound::{Halt, MOVED};
use super::plan::PlanHandle;
use crate::session::{SessionCore, emit_snapshot};
use crate::sync::lock;

/// The notice of the chat a plan moved away from.
pub(crate) const MOVED_NOTICE: &str = "This plan moved to another chat.";

/// The chat that drives a plan.
#[derive(Debug, Default)]
pub(crate) struct Driver {
    session: Option<SessionId>,
    /// `session` holds the plan now (open or closed), until it lets it go.
    attached: bool,
    live: Weak<SessionCore>,
}

impl Driver {
    /// As meta.json recorded it.
    pub(crate) fn recorded(summary: &PlanSummary) -> Self {
        Self {
            session: summary.last_session_id.clone(),
            attached: summary.attached_to.is_some()
                && summary.attached_to == summary.last_session_id,
            live: Weak::new(),
        }
    }
}

impl PlanHandle {
    /// The chat that attached the plan last.
    pub(crate) fn driver(&self) -> Option<SessionId> {
        lock(&self.driver).session.clone()
    }

    /// Names the plan's last chat in `summary`, and the chat it is
    /// attached to now.
    pub(crate) fn stamp(&self, summary: &mut PlanSummary) {
        let driver = lock(&self.driver);
        summary.last_session_id = driver.session.clone();
        summary.attached_to = driver.session.clone().filter(|_| driver.attached);
    }

    /// `core` no longer holds the plan (it detached it, or attached another
    /// plan): no chat is attached to it until one attaches it.
    pub(crate) fn released(&self, core: &SessionCore) {
        {
            let mut driver = lock(&self.driver);
            if driver.session.as_ref() != Some(&core.id) {
                return;
            }
            driver.attached = false;
        }
        self.save_meta();
    }

    /// `core` drives the plan from now on; returns the other live chat
    /// that drove it before.
    fn take_over(&self, core: &Arc<SessionCore>) -> Option<Arc<SessionCore>> {
        let previous = std::mem::replace(
            &mut *lock(&self.driver),
            Driver {
                session: Some(core.id.clone()),
                attached: true,
                live: Arc::downgrade(core),
            },
        );
        previous
            .live
            .upgrade()
            .filter(|previous| previous.id != core.id)
    }
}

/// `core` attached `plan`: it drives it, the chat that held it lets it go,
/// and meta.json names `core` as the plan's last chat and its holder.
/// Returns that chat's helpers, still stopping.
pub(crate) fn drive(core: &Arc<SessionCore>, plan: &Arc<PlanHandle>) -> Vec<(SessionId, AgentId)> {
    let mut stopping = Vec::new();
    if let Some(previous) = plan.take_over(core) {
        let holds = previous
            .work
            .plan()
            .is_some_and(|current| Arc::ptr_eq(&current, plan));
        if holds {
            stopping = moved_away(&previous, plan);
        }
    }
    plan.save_meta();
    stopping
}

/// The plan another chat took: `core` detaches it; returns the helpers it
/// stopped.
fn moved_away(core: &SessionCore, plan: &PlanHandle) -> Vec<(SessionId, AgentId)> {
    core.journal.append_or_report(&LogRecord::PlanDetached);
    let stopped = core
        .work
        .helpers()
        .halt(&plan.id, Halt::Interrupt(MOVED), |_| true);
    if let Some(previous) = core.work.replace(None, false) {
        previous.unwatch(&core.events);
    }
    emit_snapshot(core);
    core.events.notice(NoticeLevel::Info, MOVED_NOTICE);
    let session = || core.id.clone();
    stopped
        .into_iter()
        .map(|agent| (session(), agent))
        .collect()
}

/// A reopening chat whose plan another chat drives now: the log records
/// that it no longer holds it. `true` when it may attach the plan.
pub(crate) fn may_reattach(core: &SessionCore, plan: &PlanHandle) -> bool {
    match plan.driver() {
        Some(driver) if driver != core.id => {
            core.journal.append_or_report(&LogRecord::PlanDetached);
            false
        }
        _ => true,
    }
}
