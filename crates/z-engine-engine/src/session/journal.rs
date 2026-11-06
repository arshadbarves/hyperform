//! The session log writer. Records are appended before the matching event
//! is published, so the GUI never shows a transition the log lost.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use z_engine_protocol::SessionId;
use z_engine_store::{LogRecord, SessionLog, SessionStore};

use crate::error::EngineError;
use crate::session::emitter::Emitter;
use crate::sync::lock;

pub(crate) struct Journal {
    events: Arc<Emitter>,
    store: SessionStore,
    session_id: SessionId,
    log: Mutex<SessionLog>,
    path: PathBuf,
}

impl fmt::Debug for Journal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Journal")
            .field("session_id", &self.session_id)
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl Journal {
    pub(crate) fn new(
        events: Arc<Emitter>,
        store: SessionStore,
        session_id: SessionId,
        log: SessionLog,
    ) -> Self {
        let path = log.path().to_path_buf();
        Self {
            events,
            store,
            session_id,
            log: Mutex::new(log),
            path,
        }
    }

    pub(crate) fn events(&self) -> &Emitter {
        &self.events
    }

    /// `<session>/log.jsonl`, the transcript path handed to hooks.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Appends one record. A failed write disables the handle, so the log
    /// is reopened (repairing a torn tail) and the record retried once.
    pub(crate) fn append(&self, record: &LogRecord) -> Result<(), EngineError> {
        let mut log = lock(&self.log);
        let Err(first) = log.append(record) else {
            return Ok(());
        };
        tracing::warn!(session = %self.session_id, error = %first, "session log write failed; reopening");
        match self.store.open_append(&self.session_id) {
            Ok(reopened) => {
                *log = reopened;
                log.append(record).map_err(EngineError::from)
            }
            Err(reopen) => {
                tracing::error!(session = %self.session_id, error = %reopen, "cannot reopen the session log");
                Err(first.into())
            }
        }
    }

    /// [`Journal::append`] for records whose loss must be visible but must
    /// not abort the caller; returns whether the record was persisted.
    pub(crate) fn append_or_report(&self, record: &LogRecord) -> bool {
        match self.append(record) {
            Ok(()) => true,
            Err(error) => {
                self.events
                    .error(format!("could not save the session log: {error}"));
                false
            }
        }
    }

    /// Makes everything appended so far durable (turn boundaries, close).
    pub(crate) fn sync(&self) -> Result<(), EngineError> {
        lock(&self.log).sync().map_err(EngineError::from)
    }

    pub(crate) fn sync_or_report(&self) {
        if let Err(error) = self.sync() {
            self.events
                .error(format!("could not flush the session log: {error}"));
        }
    }
}
