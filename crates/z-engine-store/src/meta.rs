//! `meta.json`: the listing summary of one session. It caches facts of the
//! log, is rewritten atomically, and is rebuilt when missing or corrupt.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use z_engine_protocol::{SessionId, SessionSummary, TurnOutcome};

use crate::durable::write_atomic;
use crate::error::StoreError;
use crate::layout::id_timestamp_ms;
use crate::record::SESSION_SCHEMA;
use crate::replay::ReplayState;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    pub schema: u32,
    pub session_id: SessionId,
    pub title: Option<String>,
    pub project_root: String,
    pub model: String,
    pub created_at: u64,
    pub updated_at: u64,
    /// Transcript messages, tool-result messages included.
    pub message_count: u32,
    pub cost_usd: f64,
    pub last_outcome: Option<TurnOutcome>,
    /// Imported from a v1 session file.
    pub legacy: bool,
}

impl SessionMeta {
    /// Summary of a replayed log. Without `SessionStarted` the creation time
    /// falls back to the id's ULID time and the project root stays empty.
    pub(crate) fn from_state(
        id: &SessionId,
        state: &ReplayState,
        updated_at: u64,
        legacy: bool,
    ) -> Self {
        let (project_root, created_at) = match &state.info {
            Some((_, root, created_at)) => (root.clone(), *created_at),
            None => (String::new(), id_timestamp_ms(id).unwrap_or(updated_at)),
        };
        let last_outcome = if state.open_turn.is_some() {
            Some(TurnOutcome::Interrupted)
        } else {
            state.turns.last().map(|turn| turn.outcome.clone())
        };
        Self {
            schema: SESSION_SCHEMA,
            session_id: id.clone(),
            title: state.title.clone(),
            project_root,
            model: state.model.clone().unwrap_or_default(),
            created_at,
            updated_at: updated_at.max(created_at),
            message_count: u32::try_from(state.transcript.len()).unwrap_or(u32::MAX),
            cost_usd: state.cost_usd,
            last_outcome,
            legacy,
        }
    }
}

impl From<&SessionMeta> for SessionSummary {
    fn from(meta: &SessionMeta) -> Self {
        Self {
            session_id: meta.session_id.clone(),
            title: meta.title.clone(),
            project_root: meta.project_root.clone(),
            created_at: meta.created_at,
            updated_at: meta.updated_at,
            message_count: meta.message_count,
            cost_usd: meta.cost_usd,
            last_outcome: meta.last_outcome.clone(),
            legacy: meta.legacy,
        }
    }
}

pub(crate) fn write_meta_file(path: &Path, meta: &SessionMeta) -> Result<(), StoreError> {
    let mut bytes =
        serde_json::to_vec_pretty(meta).map_err(|error| StoreError::json(path, 0, &error))?;
    bytes.push(b'\n');
    write_atomic(path, &bytes)
}

/// A schema newer than this crate writes is `Invalid`, so callers never
/// overwrite metadata they do not understand.
pub(crate) fn read_meta_file(path: &Path) -> Result<SessionMeta, StoreError> {
    let bytes = fs::read(path).map_err(|error| StoreError::open(path, error))?;
    let meta: SessionMeta = serde_json::from_slice(&bytes)
        .map_err(|error| StoreError::json(path, error.line(), &error))?;
    if meta.schema > SESSION_SCHEMA {
        return Err(StoreError::Invalid(format!(
            "{} has schema {}, newer than supported {SESSION_SCHEMA}",
            path.display(),
            meta.schema
        )));
    }
    Ok(meta)
}
