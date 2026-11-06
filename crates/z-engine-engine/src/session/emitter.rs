//! The one path from a session to the GUI: every event gets the next
//! per-session sequence number and is delivered in that order.

use std::fmt;
use std::sync::{Mutex, OnceLock};

use z_engine_protocol::{Event, EventEnvelope, NoticeLevel, SessionId};

use crate::options::EventSink;
use crate::sync::lock;

/// Called after each notice is delivered; must not block.
pub(crate) type NoticeWatch = Box<dyn Fn(NoticeLevel, &str) + Send + Sync>;

pub(crate) struct Emitter {
    session_id: SessionId,
    sink: EventSink,
    /// Held while the sink runs so delivery order matches `seq`.
    seq: Mutex<u64>,
    notice_watch: OnceLock<NoticeWatch>,
}

impl fmt::Debug for Emitter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Emitter")
            .field("session_id", &self.session_id)
            .field("seq", &*lock(&self.seq))
            .finish_non_exhaustive()
    }
}

impl Emitter {
    pub(crate) fn new(session_id: SessionId, sink: EventSink) -> Self {
        Self {
            session_id,
            sink,
            seq: Mutex::new(0),
            notice_watch: OnceLock::new(),
        }
    }

    pub(crate) fn emit(&self, event: Event) {
        let mut seq = lock(&self.seq);
        *seq += 1;
        (self.sink)(EventEnvelope {
            session_id: self.session_id.clone(),
            seq: *seq,
            event,
        });
    }

    pub(crate) fn notice(&self, level: NoticeLevel, text: impl Into<String>) {
        let text = text.into();
        let Some(watch) = self.notice_watch.get() else {
            self.emit(Event::notice(level, text));
            return;
        };
        self.emit(Event::notice(level, text.clone()));
        watch(level, &text);
    }

    /// Installs the one notice watcher (the decision layer's urgency
    /// scoring); later calls are ignored.
    pub(crate) fn watch_notices(&self, watch: NoticeWatch) {
        if self.notice_watch.set(watch).is_err() {
            tracing::debug!("notice watcher already installed");
        }
    }

    pub(crate) fn error(&self, message: impl Into<String>) {
        self.emit(Event::Error {
            message: message.into(),
        });
    }
}
