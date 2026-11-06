//! Idle / Busy / Waiting, derived from running activities (turns, manual
//! compaction) and pending user interactions; transitions emit
//! `StatusChanged`.

use std::sync::{Arc, Mutex};

use z_engine_protocol::{Event, SessionStatus};

use crate::session::emitter::Emitter;
use crate::sync::lock;

#[derive(Debug)]
pub(crate) struct StatusTracker {
    events: Arc<Emitter>,
    inner: Mutex<Inner>,
}

#[derive(Debug)]
struct Inner {
    active: u32,
    waiting: usize,
    current: SessionStatus,
}

impl StatusTracker {
    pub(crate) fn new(events: Arc<Emitter>) -> Self {
        Self {
            events,
            inner: Mutex::new(Inner {
                active: 0,
                waiting: 0,
                current: SessionStatus::Idle,
            }),
        }
    }

    pub(crate) fn current(&self) -> SessionStatus {
        lock(&self.inner).current
    }

    /// An activity (turn, manual compaction) started.
    pub(crate) fn begin(&self) {
        self.update(|inner| inner.active += 1);
    }

    pub(crate) fn end(&self) {
        self.update(|inner| inner.active = inner.active.saturating_sub(1));
    }

    /// Number of approvals, questions, and plans awaiting the user.
    pub(crate) fn set_waiting(&self, waiting: usize) {
        self.update(|inner| inner.waiting = waiting);
    }

    /// The event is emitted under the lock so transitions arrive in order.
    fn update(&self, change: impl FnOnce(&mut Inner)) {
        let mut inner = lock(&self.inner);
        change(&mut inner);
        let status = if inner.waiting > 0 {
            SessionStatus::Waiting
        } else if inner.active > 0 {
            SessionStatus::Busy
        } else {
            SessionStatus::Idle
        };
        if status != inner.current {
            inner.current = status;
            self.events.emit(Event::StatusChanged { status });
        }
    }
}
