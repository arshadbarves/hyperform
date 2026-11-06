//! The context card's token breakdown for a live session: the estimate
//! `/context` prints, without adding a card to the transcript.

use z_engine_protocol::{ContextBreakdown, SessionId};

use crate::engine::Engine;
use crate::session::context_report;

impl Engine {
    /// Token estimate per prompt layer of the next main-agent request;
    /// `None` when the session is not open. The GUI also receives it as a
    /// `ContextReport` event, so the session view stays in step.
    pub fn context_breakdown(&self, session_id: &SessionId) -> Option<ContextBreakdown> {
        let handle = self.handle(session_id)?;
        Some(context_report(&handle.core))
    }
}
