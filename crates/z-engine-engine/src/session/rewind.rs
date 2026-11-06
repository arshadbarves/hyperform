//! Rewind to the state before a user message: code via the shadow
//! checkpoint taken before it, conversation via a `Rewound` record and a
//! state rebuild from the log. Idle sessions only (the actor enforces it).

use z_engine_host::RestoreReport;
use z_engine_protocol::{MessageId, NoticeLevel, RewindScope, Role, now_ms};
use z_engine_store::LogRecord;

use super::meta::write_meta;
use super::snapshot::emit_snapshot;
use crate::error::EngineError;
use crate::session::{SessionCore, SessionState};
use crate::sync::lock;

pub(crate) async fn rewind(
    core: &SessionCore,
    message_id: &MessageId,
    scope: RewindScope,
) -> Result<(), EngineError> {
    let code = matches!(scope, RewindScope::Code | RewindScope::Both);
    let conversation = matches!(scope, RewindScope::Conversation | RewindScope::Both);
    let (known, snapshot) = core.with_state(|state| {
        let known = state
            .transcript
            .iter()
            .any(|message| message.id == *message_id && message.role == Role::User);
        let snapshot = state
            .checkpoints
            .iter()
            .find(|(info, _)| info.message_id == *message_id)
            .map(|(_, commit)| commit.clone());
        (known, snapshot)
    });
    if !known {
        return Err(EngineError::NotFound(format!("user message {message_id}")));
    }
    let report = if code {
        let commit = snapshot.ok_or_else(|| {
            EngineError::NotFound(format!("a code checkpoint before message {message_id}"))
        })?;
        let repo = core.checkpoints.repo(core).await?;
        Some(repo.restore(&commit).await?)
    } else {
        None
    };
    core.journal.append(&LogRecord::Rewound {
        message_id: message_id.clone(),
        conversation,
        code,
    })?;
    if conversation {
        rebuild(core)?;
    }
    emit_snapshot(core);
    if let Some(report) = report {
        core.events.notice(NoticeLevel::Info, describe(&report));
    }
    write_meta(core);
    Ok(())
}

/// Replays the log again; what the agent read is forgotten with the
/// conversation that read it.
fn rebuild(core: &SessionCore) -> Result<(), EngineError> {
    let loaded = core.shared.store.load(&core.id)?;
    core.with_state(|state| {
        let mut rebuilt = SessionState::from_replay(
            loaded.state,
            state.legacy,
            now_ms(),
            &state.model,
            state.effort,
        );
        rebuilt.queue = std::mem::take(&mut state.queue);
        rebuilt.context_limit = state.context_limit;
        *state = rebuilt;
    });
    let files = &core.main.files;
    for path in files.tracked() {
        files.forget(&path);
    }
    lock(&core.main.seen_instructions).clear();
    Ok(())
}

fn describe(report: &RestoreReport) -> String {
    let list = |paths: &[String]| {
        if paths.is_empty() {
            "none".to_string()
        } else {
            paths.join(", ")
        }
    };
    let mut text = format!(
        "Code restored. Restored: {}. Deleted: {}.",
        list(&report.restored),
        list(&report.deleted)
    );
    if !report.skipped.is_empty() {
        text.push_str(&format!(
            " Left alone (not stored in the checkpoint): {}.",
            report.skipped.join(", ")
        ));
    }
    text
}
