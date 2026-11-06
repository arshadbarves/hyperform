//! The verification seam at the main agent's stop boundary, and the record
//! of what the current turn changed.

use std::fmt;
use std::path::PathBuf;

use async_trait::async_trait;
use z_engine_protocol::VerificationOutcome;

use crate::run::RunContext;

/// What the stop boundary does after verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StopVerdict {
    /// End the turn with this badge.
    Done(VerificationOutcome),
    /// Continue the run with this reminder (failed or missing checks).
    Continue { reminder: String },
}

#[async_trait]
pub(crate) trait Verifier: Send + Sync + fmt::Debug {
    /// `continuations`: how often this verifier already continued the
    /// turn. `changed`: files the run wrote (absolute).
    async fn at_stop(
        &self,
        ctx: &RunContext,
        continuations: u32,
        changed: &[PathBuf],
    ) -> StopVerdict;
}

/// Changes made during the current main-agent turn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Mutation {
    pub mutated: bool,
    /// Epoch ms of the last change the engine saw; `None` when the only
    /// change happened before the turn (a `!cmd`).
    pub at: Option<u64>,
}

impl Mutation {
    pub(crate) fn seeded(mutated: bool) -> Self {
        Self { mutated, at: None }
    }

    pub(crate) fn touch(&mut self, at: u64) {
        self.mutated = true;
        self.at = Some(self.at.map_or(at, |previous| previous.max(at)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touching_keeps_the_latest_change() {
        let mut mutation = Mutation::seeded(false);
        assert!(!mutation.mutated);
        mutation.touch(20);
        mutation.touch(10);
        assert_eq!(
            mutation,
            Mutation {
                mutated: true,
                at: Some(20)
            }
        );
        assert_eq!(Mutation::seeded(true).at, None);
    }
}
