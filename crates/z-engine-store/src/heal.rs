//! Rebuilding `meta.json` from the log it caches.

use z_engine_protocol::SessionId;

use crate::durable::modified_ms;
use crate::error::StoreError;
use crate::layout::Layout;
use crate::meta::{SessionMeta, write_meta_file};
use crate::replay::ReplayState;

/// Meta derived from a replayed log. `updated_at` is the newest record
/// timestamp, else the log's modification time. `legacy` is inferred from
/// the v1 file the session was imported from still sitting beside it
/// (`create` refuses ids that collide with a v1 file).
pub(crate) fn rebuild_meta(
    layout: &Layout,
    id: &SessionId,
    state: &ReplayState,
) -> Result<SessionMeta, StoreError> {
    let log = layout.log_path(id)?;
    let updated_at = latest_activity(state)
        .or_else(|| modified_ms(&log))
        .unwrap_or(0);
    let legacy = layout.v1_path(id)?.is_file();
    Ok(SessionMeta::from_state(id, state, updated_at, legacy))
}

/// Rebuild and rewrite `meta.json`. A failed rewrite only leaves the cache
/// stale, so it is logged and the rebuilt summary is still returned.
pub(crate) fn heal_meta(
    layout: &Layout,
    id: &SessionId,
    state: &ReplayState,
) -> Result<SessionMeta, StoreError> {
    let meta = rebuild_meta(layout, id, state)?;
    if let Err(error) = write_meta_file(&layout.meta_path(id)?, &meta) {
        tracing::warn!(session = %id, %error, "could not rewrite session meta");
    }
    Ok(meta)
}

fn latest_activity(state: &ReplayState) -> Option<u64> {
    let messages = state.transcript.iter().map(|message| message.created_at);
    let turns = state.turns.iter().map(|turn| turn.finished_at);
    let started = state.info.iter().map(|(_, _, created_at)| *created_at);
    messages.chain(turns).chain(started).max()
}
