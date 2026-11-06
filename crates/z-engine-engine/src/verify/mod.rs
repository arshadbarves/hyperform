//! Verification: the session's checks (discovered and configured), checks
//! run as recorded evidence, the turn's badge, and the verification modes
//! at the main agent's stop boundary. Verification never blocks the user.

mod catalog;
mod claims;
mod modes;
mod outcome;
mod record;
mod verdict;

pub(crate) use catalog::{CheckHub, discover_checks};
pub(crate) use modes::ModeVerifier;
pub(crate) use outcome::{current_outcome, publish_outcome};
pub(crate) use record::{CheckRun, run_recorded};
pub(crate) use verdict::{Mutation, StopVerdict, Verifier};
