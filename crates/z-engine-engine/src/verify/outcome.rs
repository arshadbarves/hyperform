//! The current turn's badge from the session's check records, the changes
//! of the turn and the workspace as it is now.

use z_engine_host::workspace_fingerprint;
use z_engine_protocol::{Event, VerificationMode, VerificationOutcome};
use z_engine_verify::assess;

use crate::session::SessionCore;

pub(crate) async fn current_outcome(core: &SessionCore) -> VerificationOutcome {
    if core.settings().settings.verification.mode == VerificationMode::Off {
        return VerificationOutcome::NotApplicable;
    }
    let (mutation, records) = core.with_state(|state| (state.mutation, state.checks.clone()));
    if !mutation.mutated {
        return VerificationOutcome::NotApplicable;
    }
    let fingerprint = match workspace_fingerprint(&core.root).await {
        Ok(fingerprint) => Some(fingerprint),
        Err(error) => {
            tracing::warn!(%error, "workspace fingerprint unavailable; staleness not checked");
            None
        }
    };
    assess(&records, true, mutation.at, fingerprint.as_deref())
}

/// Recomputes the badge and tells the GUI.
pub(crate) async fn publish_outcome(core: &SessionCore) -> VerificationOutcome {
    let outcome = current_outcome(core).await;
    core.events.emit(Event::VerificationChanged {
        outcome: outcome.clone(),
    });
    outcome
}
