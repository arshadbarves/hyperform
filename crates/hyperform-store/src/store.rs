//! `SessionStore`: create, reopen, and load sessions (importing a v1 file on
//! first open), plus metadata, subagent transcripts, artifacts, and
//! deletion. Listing lives in `listing.rs`.

use std::fs;
use std::path::{Path, PathBuf};

use hyperform_protocol::{AgentId, Message, PermissionMode, SessionId, now_ms};

use crate::append::Appender;
use crate::artifacts::Artifacts;
use crate::durable::sync_dir;
use crate::error::StoreError;
use crate::heal::heal_meta;
use crate::layout::{LOG_FILE, Layout, META_FILE};
use crate::legacy::import_v1;
use crate::log::{AgentLog, SessionLog};
use crate::meta::{SessionMeta, read_meta_file, write_meta_file};
use crate::read::read_records;
use crate::record::{LogRecord, SESSION_SCHEMA};
use crate::replay::{ReplayState, replay};

/// The sessions directory. Holds no open files; cheap to clone.
#[derive(Debug, Clone)]
pub struct SessionStore {
    layout: Layout,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewSession {
    pub session_id: SessionId,
    pub project_root: String,
    pub model: String,
    pub mode: PermissionMode,
    pub planning: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedSession {
    /// As stored in `meta.json`, which may lag the log after a crash;
    /// `state` is authoritative when they disagree.
    pub meta: SessionMeta,
    pub state: ReplayState,
    /// The log ended in a torn record, which was ignored.
    pub torn_tail: bool,
    /// Log lines skipped as corrupt or of an unknown kind.
    pub corrupt_lines: usize,
}

impl SessionStore {
    pub fn new(sessions_dir: impl Into<PathBuf>) -> Self {
        Self {
            layout: Layout::new(sessions_dir),
        }
    }

    pub fn dir(&self) -> &Path {
        self.layout.root()
    }

    pub(crate) fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Create `<id>/` holding a synced `SessionStarted` record and
    /// `meta.json`. An id already used by a v2 or v1 session is `Invalid`.
    pub fn create(&self, new: NewSession) -> Result<(SessionLog, SessionMeta), StoreError> {
        let id = &new.session_id;
        let dir = self.layout.session_dir(id)?;
        if self.layout.v1_path(id)?.exists() {
            return Err(StoreError::Invalid(format!(
                "a v1 session {id} already exists"
            )));
        }
        let root = self.layout.root();
        fs::create_dir_all(root).map_err(|error| StoreError::io(root, error))?;
        fs::create_dir(&dir).map_err(|error| match error.kind() {
            std::io::ErrorKind::AlreadyExists => {
                StoreError::Invalid(format!("session {id} already exists"))
            }
            _ => StoreError::io(&dir, error),
        })?;
        initialize(root, &dir, &new).inspect_err(|_| remove_partial(&dir))
    }

    /// Append handle for an existing session, importing a v1 file first.
    pub fn open_append(&self, id: &SessionId) -> Result<SessionLog, StoreError> {
        self.ensure_v2(id)?;
        let log = Appender::open(&self.layout.log_path(id)?, false)?;
        Ok(SessionLog::new(log))
    }

    /// Read and replay a session, importing a v1 file first. `meta.json` is
    /// rebuilt from the log when missing or corrupt. A log written by a
    /// newer schema is `Invalid` rather than misread.
    pub fn load(&self, id: &SessionId) -> Result<LoadedSession, StoreError> {
        self.ensure_v2(id)?;
        let log_path = self.layout.log_path(id)?;
        let read = read_records(&log_path)?;
        if let Some(schema) = newer_schema(&read.records) {
            return Err(StoreError::Invalid(format!(
                "{} has schema {schema}, newer than supported {SESSION_SCHEMA}",
                log_path.display()
            )));
        }
        let state = replay(&read.records);
        let meta = match self.read_meta(id) {
            Err(StoreError::NotFound(_) | StoreError::Json { .. }) => {
                heal_meta(&self.layout, id, &state)?
            }
            other => other?,
        };
        Ok(LoadedSession {
            meta,
            state,
            torn_tail: read.torn_tail,
            corrupt_lines: read.corrupt_lines,
        })
    }

    /// Remove the session directory and any v1 file with this id. Open
    /// handles to the removed log fail on their next write.
    pub fn delete(&self, id: &SessionId) -> Result<(), StoreError> {
        let dir = self.layout.session_dir(id)?;
        let removed_dir = removed(fs::remove_dir_all(&dir), &dir)?;
        let v1 = self.layout.v1_path(id)?;
        let removed_v1 = removed(fs::remove_file(&v1), &v1)?;
        if removed_dir || removed_v1 {
            Ok(())
        } else {
            Err(StoreError::NotFound(format!("session {id}")))
        }
    }

    /// Atomically rewrite `meta.json` of an existing session directory.
    pub fn write_meta(&self, meta: &SessionMeta) -> Result<(), StoreError> {
        let dir = self.layout.session_dir(&meta.session_id)?;
        if !dir.is_dir() {
            return Err(StoreError::NotFound(format!("session {}", meta.session_id)));
        }
        write_meta_file(&dir.join(META_FILE), meta)
    }

    pub fn read_meta(&self, id: &SessionId) -> Result<SessionMeta, StoreError> {
        read_meta_file(&self.layout.meta_path(id)?)
    }

    /// Artifact directory of a session. For an invalid id every write fails.
    pub fn artifacts(&self, id: &SessionId) -> Artifacts {
        match self.layout.artifacts_dir(id) {
            Ok(dir) => Artifacts::new(dir),
            Err(error) => Artifacts::rejected(self.layout.root(), error.to_string()),
        }
    }

    /// Append handle for a subagent transcript, created on first use.
    pub fn agent_log(&self, id: &SessionId, agent_id: &AgentId) -> Result<AgentLog, StoreError> {
        let path = self.layout.agent_path(id, agent_id)?;
        let session_dir = self.layout.session_dir(id)?;
        if !session_dir.is_dir() {
            return Err(StoreError::NotFound(format!("session {id}")));
        }
        let agents_dir = self.layout.agents_dir(id)?;
        fs::create_dir_all(&agents_dir).map_err(|error| StoreError::io(&agents_dir, error))?;
        let log = Appender::open(&path, true)?;
        sync_dir(&agents_dir)?;
        sync_dir(&session_dir)?;
        Ok(AgentLog::new(log))
    }

    /// Messages of a subagent transcript, in order.
    pub fn load_agent_transcript(
        &self,
        id: &SessionId,
        agent_id: &AgentId,
    ) -> Result<Vec<Message>, StoreError> {
        let read = read_records(&self.layout.agent_path(id, agent_id)?)?;
        Ok(read
            .records
            .into_iter()
            .filter_map(|record| match record {
                LogRecord::Message { message, .. } => Some(message),
                _ => None,
            })
            .collect())
    }

    /// True for a v2 session or a v1 file that `load` would import.
    pub fn exists(&self, id: &SessionId) -> bool {
        let is_file = |path: Result<PathBuf, StoreError>| path.is_ok_and(|path| path.is_file());
        is_file(self.layout.log_path(id)) || is_file(self.layout.v1_path(id))
    }

    fn ensure_v2(&self, id: &SessionId) -> Result<(), StoreError> {
        if self.layout.session_dir(id)?.is_dir() {
            return Ok(());
        }
        let v1 = self.layout.v1_path(id)?;
        if !v1.is_file() {
            return Err(StoreError::NotFound(format!("session {id}")));
        }
        import_v1(self.layout.root(), &v1).map(|_| ())
    }
}

fn initialize(
    root: &Path,
    dir: &Path,
    new: &NewSession,
) -> Result<(SessionLog, SessionMeta), StoreError> {
    let created_at = now_ms();
    let mut log = SessionLog::new(Appender::create_new(&dir.join(LOG_FILE))?);
    log.append(&LogRecord::SessionStarted {
        schema: SESSION_SCHEMA,
        session_id: new.session_id.clone(),
        project_root: new.project_root.clone(),
        model: new.model.clone(),
        mode: new.mode.into(),
        planning: new.planning,
        created_at,
    })?;
    log.sync()?;
    let meta = SessionMeta {
        schema: SESSION_SCHEMA,
        session_id: new.session_id.clone(),
        title: None,
        project_root: new.project_root.clone(),
        model: new.model.clone(),
        created_at,
        updated_at: created_at,
        message_count: 0,
        cost_usd: 0.0,
        last_outcome: None,
        legacy: false,
    };
    write_meta_file(&dir.join(META_FILE), &meta)?;
    sync_dir(root)?;
    Ok((log, meta))
}

fn newer_schema(records: &[LogRecord]) -> Option<u32> {
    records
        .iter()
        .find_map(|record| match record {
            LogRecord::SessionStarted { schema, .. } => Some(*schema),
            _ => None,
        })
        .filter(|schema| *schema > SESSION_SCHEMA)
}

fn removed(result: std::io::Result<()>, path: &Path) -> Result<bool, StoreError> {
    match result {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(StoreError::io(path, error)),
    }
}

fn remove_partial(dir: &Path) {
    if let Err(error) = fs::remove_dir_all(dir) {
        tracing::warn!(path = %dir.display(), %error, "could not remove partially created session");
    }
}
