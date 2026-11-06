//! Code checkpoints: before each user turn the working tree is snapshotted
//! into the project's shadow repository (outside the project), so rewinds
//! can restore code, including changes made by shell commands.

use std::sync::atomic::{AtomicBool, Ordering};

use z_engine_host::{HostError, ShadowRepo, git_available};
use z_engine_protocol::{CheckpointId, CheckpointInfo, Event, MessageId, NoticeLevel, now_ms};
use z_engine_store::LogRecord;

use crate::session::SessionCore;

#[derive(Debug, Default)]
pub(crate) struct Checkpoints {
    repo: tokio::sync::Mutex<Option<ShadowRepo>>,
    /// Checkpoints were found unavailable and the user was told once.
    off: AtomicBool,
}

impl Checkpoints {
    /// The shadow repository, opened (and created) on first use.
    pub(crate) async fn repo(&self, core: &SessionCore) -> Result<ShadowRepo, HostError> {
        let mut slot = self.repo.lock().await;
        if let Some(repo) = slot.as_ref() {
            return Ok(repo.clone());
        }
        let repo = ShadowRepo::open(&core.shared.paths.checkpoints_dir, &core.root).await?;
        *slot = Some(repo.clone());
        Ok(repo)
    }

    pub(crate) fn available(&self) -> bool {
        !self.off.load(Ordering::SeqCst) && git_available()
    }
}

/// Snapshots the tree as the state before `message_id`. Unavailable
/// checkpoints (no git, tree too large) are announced once and skipped.
pub(crate) async fn take_checkpoint(core: &SessionCore, message_id: &MessageId) {
    if !git_available() {
        turn_off(core, "git is not installed");
        return;
    }
    if core.checkpoints.off.load(Ordering::SeqCst) {
        return;
    }
    let snapshot = match core.checkpoints.repo(core).await {
        Ok(repo) => repo.snapshot(&format!("before {message_id}")).await,
        Err(error) => Err(error),
    };
    match snapshot {
        Ok(commit) => {
            let info = CheckpointInfo {
                checkpoint_id: CheckpointId::new(),
                message_id: message_id.clone(),
                created_at: now_ms(),
            };
            let record = LogRecord::Checkpoint {
                info: info.clone(),
                snapshot: commit.clone(),
            };
            if core.journal.append_or_report(&record) {
                core.with_state(|state| state.checkpoints.push((info.clone(), commit)));
                core.events
                    .emit(Event::CheckpointCreated { checkpoint: info });
            }
        }
        Err(HostError::Blocked(reason)) => turn_off(core, &reason),
        Err(error) => core.events.notice(
            NoticeLevel::Warn,
            format!("no code checkpoint for this turn: {error}"),
        ),
    }
}

fn turn_off(core: &SessionCore, reason: &str) {
    if !core.checkpoints.off.swap(true, Ordering::SeqCst) {
        core.events.notice(
            NoticeLevel::Info,
            format!("Code checkpoints are off ({reason}); rewinding code is unavailable."),
        );
    }
}
