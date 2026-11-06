//! `meta.json`, the listing summary, rewritten from the session state
//! after turns, title changes and model switches.

use z_engine_protocol::{NoticeLevel, TurnOutcome};
use z_engine_store::{SESSION_SCHEMA, SessionMeta};

use crate::session::SessionCore;

pub(crate) fn write_meta(core: &SessionCore) {
    let meta = core.with_state(|state| SessionMeta {
        schema: SESSION_SCHEMA,
        session_id: core.id.clone(),
        title: state.title.clone(),
        project_root: core.root.to_string_lossy().into_owned(),
        model: state.model.clone(),
        created_at: state.created_at,
        updated_at: state.updated_at.max(state.created_at),
        message_count: u32::try_from(state.transcript.len()).unwrap_or(u32::MAX),
        cost_usd: state.cost_usd,
        last_outcome: state
            .turns
            .last()
            .map(|turn| turn.outcome.clone())
            .or_else(|| state.interrupted.then_some(TurnOutcome::Interrupted)),
        legacy: state.legacy,
    });
    if let Err(error) = core.shared.store.write_meta(&meta) {
        core.events.notice(
            NoticeLevel::Warn,
            format!("could not update the session listing: {error}"),
        );
    }
}
