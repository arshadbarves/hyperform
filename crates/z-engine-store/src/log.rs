//! Public append handles: the main session log and subagent transcripts.

use std::path::Path;

use z_engine_protocol::Message;

use crate::append::Appender;
use crate::error::StoreError;
use crate::record::LogRecord;

/// Append handle for `<session>/log.jsonl`.
///
/// `append` writes and flushes one line; `sync` makes everything appended so
/// far durable (the engine calls it at turn boundaries). After a failed write
/// every handle to the log refuses further records until
/// `SessionStore::open_append` reopens it, which repairs a torn tail.
#[derive(Debug)]
pub struct SessionLog {
    inner: Appender,
}

impl SessionLog {
    pub(crate) fn new(inner: Appender) -> Self {
        Self { inner }
    }

    pub fn append(&mut self, record: &LogRecord) -> Result<(), StoreError> {
        self.inner.append(record)
    }

    pub fn sync(&mut self) -> Result<(), StoreError> {
        self.inner.sync()
    }

    pub fn path(&self) -> &Path {
        self.inner.path()
    }
}

/// Append handle for `<session>/agents/<agent>.jsonl`: a subagent's
/// transcript, stored as `LogRecord::Message` lines only. Same failure
/// behavior as `SessionLog`.
#[derive(Debug)]
pub struct AgentLog {
    inner: Appender,
}

impl AgentLog {
    pub(crate) fn new(inner: Appender) -> Self {
        Self { inner }
    }

    /// Anything but `LogRecord::Message` is refused with `StoreError::Invalid`.
    pub fn append(&mut self, record: &LogRecord) -> Result<(), StoreError> {
        match record {
            LogRecord::Message { .. } => self.inner.append(record),
            _ => Err(StoreError::Invalid(
                "agent transcripts only store message records".into(),
            )),
        }
    }

    pub fn append_message(&mut self, message: &Message) -> Result<(), StoreError> {
        self.inner.append(&LogRecord::Message {
            message: message.clone(),
            turn_id: None,
        })
    }

    pub fn sync(&mut self) -> Result<(), StoreError> {
        self.inner.sync()
    }

    pub fn path(&self) -> &Path {
        self.inner.path()
    }
}
