//! Session-scope review: what changed in the project since the session's
//! first code checkpoint, read from the shadow repository without taking a
//! new snapshot.

use std::path::PathBuf;

use hyperform_host::{ChangeKind, PathChange, ShadowRepo, git_available};
use hyperform_protocol::SessionId;
use hyperform_store::SessionStore;
use serde::Serialize;

use crate::engine::Engine;
use crate::error::EngineError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangedKind {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedPath {
    /// Relative to the project root, `/`-separated.
    pub path: String,
    pub kind: ChangedKind,
}

impl From<PathChange> for ChangedPath {
    fn from(change: PathChange) -> Self {
        let kind = match change.kind {
            ChangeKind::Added => ChangedKind::Added,
            ChangeKind::Modified => ChangedKind::Modified,
            ChangeKind::Deleted => ChangedKind::Deleted,
        };
        Self {
            path: change.path,
            kind,
        }
    }
}

impl Engine {
    /// Paths changed since the session's first checkpoint, sorted; empty
    /// when the session has no checkpoint (or git is unavailable).
    pub async fn session_changes(
        &self,
        session_id: &SessionId,
    ) -> Result<Vec<ChangedPath>, EngineError> {
        let Some((repo, first)) = baseline(self, session_id).await? else {
            return Ok(Vec::new());
        };
        let changes = repo.diff_worktree(&first).await?;
        Ok(changes.into_iter().map(ChangedPath::from).collect())
    }

    /// Unified diff of one project-relative path since the session's first
    /// checkpoint; empty when unchanged or without a checkpoint.
    pub async fn session_file_diff(
        &self,
        session_id: &SessionId,
        path: &str,
    ) -> Result<String, EngineError> {
        let Some((repo, first)) = baseline(self, session_id).await? else {
            return Ok(String::new());
        };
        Ok(repo.diff_worktree_file(&first, path).await?)
    }
}

/// The session's shadow repository and first snapshot commit. The log is
/// read from disk, which live sessions append to before announcing a
/// checkpoint.
async fn baseline(
    engine: &Engine,
    session_id: &SessionId,
) -> Result<Option<(ShadowRepo, String)>, EngineError> {
    if !git_available() {
        return Ok(None);
    }
    let store = SessionStore::new(engine.paths().sessions_dir.clone());
    let id = session_id.clone();
    let loaded = tokio::task::spawn_blocking(move || store.load(&id))
        .await
        .map_err(|error| EngineError::Invalid(format!("session read failed: {error}")))??;
    let Some((_, first)) = loaded.state.checkpoints.first() else {
        return Ok(None);
    };
    let root = PathBuf::from(&loaded.meta.project_root);
    let repo = ShadowRepo::open(&engine.paths().checkpoints_dir, &root).await?;
    Ok(Some((repo, first.clone())))
}

#[cfg(test)]
mod tests {
    use hyperform_protocol::{CheckpointId, CheckpointInfo, MessageId, PermissionMode};
    use hyperform_store::{LogRecord, NewSession};

    use super::super::testing::engine;
    use super::*;

    fn session(engine: &Engine, root: &std::path::Path, snapshots: &[&str]) -> SessionId {
        let store = SessionStore::new(engine.paths().sessions_dir.clone());
        let id = SessionId::new();
        let (mut log, _) = store
            .create(NewSession {
                session_id: id.clone(),
                project_root: root.to_string_lossy().into_owned(),
                model: "m".into(),
                mode: PermissionMode::Default,
                planning: false,
            })
            .unwrap();
        for snapshot in snapshots {
            let info = CheckpointInfo {
                checkpoint_id: CheckpointId::new(),
                message_id: MessageId::new(),
                created_at: 1,
            };
            let snapshot = snapshot.to_string();
            log.append(&LogRecord::Checkpoint { info, snapshot })
                .unwrap();
        }
        log.sync().unwrap();
        id
    }

    #[tokio::test]
    async fn changes_are_measured_from_the_first_checkpoint() {
        if !git_available() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("a.txt"), "one\n").unwrap();
        let engine = engine(dir.path());
        let repo = ShadowRepo::open(&engine.paths().checkpoints_dir, &root)
            .await
            .unwrap();
        let first = repo.snapshot("first").await.unwrap();
        std::fs::write(root.join("a.txt"), "two\n").unwrap();
        let second = repo.snapshot("second").await.unwrap();
        std::fs::write(root.join("b.txt"), "new\n").unwrap();
        let id = session(&engine, &root, &[&first, &second]);

        let changes = engine.session_changes(&id).await.unwrap();
        let kinds: Vec<(&str, ChangedKind)> = changes
            .iter()
            .map(|change| (change.path.as_str(), change.kind))
            .collect();
        assert_eq!(
            kinds,
            [
                ("a.txt", ChangedKind::Modified),
                ("b.txt", ChangedKind::Added)
            ]
        );
        let diff = engine.session_file_diff(&id, "a.txt").await.unwrap();
        assert!(diff.contains("-one") && diff.contains("+two"), "{diff}");
        let json = serde_json::to_value(&changes[1]).unwrap();
        assert_eq!(json, serde_json::json!({"path": "b.txt", "kind": "added"}));
    }

    #[tokio::test]
    async fn sessions_without_checkpoints_have_no_changes() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let id = session(&engine, dir.path(), &[]);
        assert!(engine.session_changes(&id).await.unwrap().is_empty());
        assert_eq!(engine.session_file_diff(&id, "a.txt").await.unwrap(), "");
        let missing = engine.session_changes(&SessionId::new()).await;
        assert!(missing.is_err() || !git_available());
    }
}
