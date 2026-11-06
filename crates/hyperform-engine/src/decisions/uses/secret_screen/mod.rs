//! Secret screening (`decisions_secret_screen`): before a request sends new
//! tool results to the provider, the detectors flag well-known key formats
//! and high-entropy values assigned to secret-like names; the decision
//! model then judges the remaining assignments, but only when its endpoint
//! is local (`decisions.allow_remote` off). The request seam asks the user
//! before any flagged value is sent. Traces and the ledger keep
//! fingerprints, never the values.

mod detect;
mod ledger;
mod screen;
#[cfg(test)]
mod tests;

pub(crate) use ledger::{SecretLedger, Settled};
pub(crate) use screen::SECRET_SCREEN;
