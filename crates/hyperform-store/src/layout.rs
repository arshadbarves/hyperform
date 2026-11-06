//! Where things live under the sessions directory. Identifiers become path
//! segments, so only `[A-Za-z0-9_-]` (at most 128 bytes) is accepted.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use hyperform_protocol::{AgentId, SessionId};

use crate::error::StoreError;

pub(crate) const LOG_FILE: &str = "log.jsonl";
pub(crate) const META_FILE: &str = "meta.json";
const AGENTS_DIR: &str = "agents";
const ARTIFACTS_DIR: &str = "artifacts";
const JSONL: &str = "jsonl";
const MAX_ID_LEN: usize = 128;

#[derive(Debug, Clone)]
pub(crate) struct Layout {
    root: PathBuf,
}

impl Layout {
    pub(crate) fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn session_dir(&self, id: &SessionId) -> Result<PathBuf, StoreError> {
        check_id("session id", id.as_str())?;
        Ok(self.root.join(id.as_str()))
    }

    pub(crate) fn log_path(&self, id: &SessionId) -> Result<PathBuf, StoreError> {
        Ok(self.session_dir(id)?.join(LOG_FILE))
    }

    pub(crate) fn meta_path(&self, id: &SessionId) -> Result<PathBuf, StoreError> {
        Ok(self.session_dir(id)?.join(META_FILE))
    }

    pub(crate) fn artifacts_dir(&self, id: &SessionId) -> Result<PathBuf, StoreError> {
        Ok(self.session_dir(id)?.join(ARTIFACTS_DIR))
    }

    pub(crate) fn agents_dir(&self, id: &SessionId) -> Result<PathBuf, StoreError> {
        Ok(self.session_dir(id)?.join(AGENTS_DIR))
    }

    pub(crate) fn agent_path(
        &self,
        id: &SessionId,
        agent_id: &AgentId,
    ) -> Result<PathBuf, StoreError> {
        check_id("agent id", agent_id.as_str())?;
        Ok(self.agents_dir(id)?.join(format!("{agent_id}.{JSONL}")))
    }

    /// The v1 transcript that shares this session's id.
    pub(crate) fn v1_path(&self, id: &SessionId) -> Result<PathBuf, StoreError> {
        check_id("session id", id.as_str())?;
        Ok(self.root.join(format!("{id}.{JSONL}")))
    }
}

pub(crate) fn is_valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ID_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

pub(crate) fn check_id(what: &str, value: &str) -> Result<(), StoreError> {
    if is_valid_id(value) {
        Ok(())
    } else {
        Err(StoreError::Invalid(format!(
            "{what} {value:?} is not a valid path segment"
        )))
    }
}

/// Session id named by a directory entry, if it is a valid id.
pub(crate) fn dir_session_id(name: &OsStr) -> Option<SessionId> {
    name.to_str()
        .filter(|name| is_valid_id(name))
        .map(SessionId::from)
}

/// Session id of a v1 transcript path (`<id>.jsonl`).
pub(crate) fn v1_session_id(path: &Path) -> Option<SessionId> {
    if path.extension().and_then(OsStr::to_str) != Some(JSONL) {
        return None;
    }
    path.file_stem()
        .and_then(OsStr::to_str)
        .filter(|stem| is_valid_id(stem))
        .map(SessionId::from)
}

/// Creation time encoded in a ULID session id.
pub(crate) fn id_timestamp_ms(id: &SessionId) -> Option<u64> {
    ulid::Ulid::from_string(id.as_str())
        .ok()
        .map(|ulid| ulid.timestamp_ms())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_ids_that_escape_or_nest() {
        let layout = Layout::new("/sessions");
        for bad in ["", "..", "../evil", "a/b", "a\\b", "x.jsonl", "sp ace"] {
            assert!(
                layout.session_dir(&SessionId::from(bad)).is_err(),
                "{bad:?} accepted"
            );
        }
        let long = "a".repeat(MAX_ID_LEN + 1);
        assert!(layout.session_dir(&SessionId::from(long.as_str())).is_err());
        let agent = AgentId::from("../x");
        assert!(layout.agent_path(&SessionId::new(), &agent).is_err());
    }

    #[test]
    fn builds_paths_for_valid_ids() {
        let layout = Layout::new("/sessions");
        let id = SessionId::from("01J8Z3K4M5N6P7Q8R9S0T1V2W3");
        let agent = AgentId::from("agt_01j8z3k4m5");
        assert_eq!(
            layout.agent_path(&id, &agent).unwrap(),
            Path::new("/sessions/01J8Z3K4M5N6P7Q8R9S0T1V2W3/agents/agt_01j8z3k4m5.jsonl")
        );
        assert_eq!(
            layout.v1_path(&id).unwrap(),
            Path::new("/sessions/01J8Z3K4M5N6P7Q8R9S0T1V2W3.jsonl")
        );
    }

    #[test]
    fn recognizes_v1_files_and_ulid_times() {
        let id = v1_session_id(Path::new("/s/01J8Z3K4M5N6P7Q8R9S0T1V2W3.jsonl")).unwrap();
        assert_eq!(id.as_str(), "01J8Z3K4M5N6P7Q8R9S0T1V2W3");
        assert!(v1_session_id(Path::new("/s/index.json")).is_none());
        assert!(v1_session_id(Path::new("/s/bad name.jsonl")).is_none());
        let ulid = ulid::Ulid::from_parts(1_700_000_000_000, 7);
        assert_eq!(
            id_timestamp_ms(&SessionId::from(ulid.to_string())),
            Some(1_700_000_000_000)
        );
        assert_eq!(id_timestamp_ms(&SessionId::from("not-a-ulid")), None);
    }
}
