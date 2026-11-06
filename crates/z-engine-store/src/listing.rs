//! Listing by scanning the sessions directory: v2 directories from their
//! `meta.json` (rebuilt from the log when missing or corrupt) and v1 files
//! that have not been imported yet.

use std::collections::HashSet;
use std::fs;

use z_engine_protocol::{SessionId, SessionSummary};

use crate::error::StoreError;
use crate::heal::{heal_meta, rebuild_meta};
use crate::layout::{dir_session_id, v1_session_id};
use crate::legacy::summarize_v1;
use crate::meta::{SessionMeta, read_meta_file};
use crate::read::read_records;
use crate::replay::{ReplayState, replay};
use crate::store::SessionStore;

impl SessionStore {
    /// Every session, newest first (`updated_at`, then id). An unreadable
    /// session is skipped with a warning instead of failing the listing.
    pub fn list(&self) -> Result<Vec<SessionSummary>, StoreError> {
        let root = self.layout().root();
        let entries = match fs::read_dir(root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(StoreError::io(root, error)),
        };
        let mut dirs = HashSet::new();
        let mut v1_files = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| StoreError::io(root, error))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|error| StoreError::io(&path, error))?;
            if file_type.is_dir() {
                dirs.extend(dir_session_id(&entry.file_name()));
            } else if file_type.is_file() {
                v1_files.extend(v1_session_id(&path).map(|id| (id, path)));
            }
        }
        let mut sessions: Vec<SessionSummary> =
            dirs.iter().filter_map(|id| self.v2_summary(id)).collect();
        for (id, path) in v1_files {
            if dirs.contains(&id) {
                continue;
            }
            match summarize_v1(&id, &path) {
                Ok(summary) => sessions.push(summary),
                Err(error) => {
                    tracing::warn!(path = %path.display(), %error, "skipping unreadable v1 session");
                }
            }
        }
        sessions.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| b.session_id.cmp(&a.session_id))
        });
        Ok(sessions)
    }

    fn v2_summary(&self, id: &SessionId) -> Option<SessionSummary> {
        match self.v2_meta(id) {
            Ok(meta) => Some(SessionSummary::from(&meta)),
            Err(error) => {
                tracing::warn!(session = %id, %error, "skipping unreadable session");
                None
            }
        }
    }

    fn v2_meta(&self, id: &SessionId) -> Result<SessionMeta, StoreError> {
        let layout = self.layout();
        match read_meta_file(&layout.meta_path(id)?) {
            Err(StoreError::NotFound(_) | StoreError::Json { .. }) => {
                heal_meta(layout, id, &self.replayed(id)?)
            }
            // Written by a newer version: summarize, but never overwrite it.
            Err(StoreError::Invalid(_)) => rebuild_meta(layout, id, &self.replayed(id)?),
            other => other,
        }
    }

    fn replayed(&self, id: &SessionId) -> Result<ReplayState, StoreError> {
        let read = read_records(&self.layout().log_path(id)?)?;
        Ok(replay(&read.records))
    }
}
