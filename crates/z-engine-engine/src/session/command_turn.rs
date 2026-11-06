//! A prompt command's turn: its expansion (or the notice why it cannot
//! run), and the grants and model it applies for that turn only.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use z_engine_protocol::NoticeLevel;

use super::grants::{grant_turn_rules, revoke_turn_rules};
use crate::commands::{CommandCall, Expansion, expand};
use crate::session::SessionCore;

/// The message texts (the visible invocation, then the wrapped body) and
/// the expansion; `None` after a notice when the command cannot run.
pub(crate) async fn command_texts(
    core: &SessionCore,
    call: &CommandCall,
    cancel: &CancellationToken,
) -> Option<(Vec<String>, Expansion)> {
    match expand(core, call, cancel).await {
        Ok(expansion) => Some((vec![call.invocation(), expansion.body.clone()], expansion)),
        Err(reason) => {
            core.events.notice(NoticeLevel::Warn, reason);
            None
        }
    }
}

/// Keeps a command's `allowed-tools` and `model` in force; dropping it
/// (when the turn ends, however it ends) takes both back.
pub(crate) struct TurnScope {
    core: Arc<SessionCore>,
    granted: Vec<String>,
    model: bool,
}

impl TurnScope {
    pub(crate) fn apply(core: &Arc<SessionCore>, expansion: &Expansion) -> Self {
        let granted = grant_turn_rules(core, &expansion.grants);
        let model = expansion.model.clone();
        let overridden = model.is_some();
        if overridden {
            core.with_state(|state| state.turn_model = model);
        }
        Self {
            core: Arc::clone(core),
            granted,
            model: overridden,
        }
    }
}

impl Drop for TurnScope {
    fn drop(&mut self) {
        revoke_turn_rules(&self.core, &self.granted);
        if self.model {
            self.core.with_state(|state| state.turn_model = None);
        }
    }
}
