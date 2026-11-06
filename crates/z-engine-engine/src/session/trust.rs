//! Workspace trust from the chat: `TrustRequired` when an untrusted
//! project sets something only a trusted project may (see
//! `settings::trust_gated`), and `TrustWorkspace`, which records the answer
//! and reloads the session's settings.

use std::sync::Arc;

use z_engine_config::TrustStore;
use z_engine_protocol::{Event, NoticeLevel};

use super::reload::reload;
use crate::error::EngineError;
use crate::session::SessionCore;

/// Asks the GUI for trust when the project defines something withheld.
pub(crate) fn request_trust(core: &SessionCore) {
    let settings = core.settings();
    if settings.trusted || settings.withheld.is_empty() {
        return;
    }
    core.events.emit(Event::TrustRequired {
        project_root: core.root.to_string_lossy().into_owned(),
        defines: settings.withheld.clone(),
    });
}

pub(crate) async fn trust_workspace(core: &Arc<SessionCore>, trusted: bool) {
    if !trusted {
        core.events.notice(
            NoticeLevel::Info,
            "The workspace stays untrusted for this chat; its project settings that run programs \
             or loosen yours remain off.",
        );
        return;
    }
    if let Err(error) = record_trust(core) {
        core.events.notice(
            NoticeLevel::Warn,
            format!("could not trust this workspace: {error}"),
        );
        return;
    }
    reload(core).await;
    core.events.notice(
        NoticeLevel::Info,
        format!(
            "Trusted {}; all of its project settings now apply.",
            core.root.display()
        ),
    );
}

fn record_trust(core: &SessionCore) -> Result<(), EngineError> {
    let file = &core.shared.paths.trust_file;
    let mut store = TrustStore::load(file)?;
    if store.trust(&core.root) {
        store.save(file)?;
    }
    Ok(())
}
