//! Completion claims no check backs (`decisions_completion_check`). The
//! verifier asks only while the outcome is `Unverified`. In `report` mode,
//! and whenever no check can run, the turn's receipt says "claimed, not
//! checked"; in `auto` and `strict` the project's Test, Build and Typecheck
//! checks run through the usual recorded path. A claim never marks a turn
//! Verified and never changes a recorded check.

use std::path::PathBuf;

use z_engine_protocol::decisions::UncheckedClaim;
use z_engine_protocol::{Event, VerificationOutcome};

use crate::decisions::seams::review_completion;
use crate::run::RunContext;

const CLAIM_CHECKS: [&str; 3] = ["test", "build", "typecheck"];

/// The checks a claim faces when `verification.auto_checks` matched none.
pub(super) fn claim_checks() -> Vec<String> {
    CLAIM_CHECKS.map(String::from).to_vec()
}

/// What the final message claims, asked only while no check backs the turn.
pub(super) async fn unchecked_claim(
    ctx: &RunContext,
    outcome: &VerificationOutcome,
    changed: &[PathBuf],
) -> Option<UncheckedClaim> {
    if !matches!(outcome, VerificationOutcome::Unverified { .. }) {
        return None;
    }
    review_completion(ctx, changed).await
}

/// Puts the claim on the turn's receipt.
pub(super) fn show(ctx: &RunContext, claim: UncheckedClaim) {
    ctx.core
        .events
        .emit(Event::CompletionClaimUnchecked { claim });
}
