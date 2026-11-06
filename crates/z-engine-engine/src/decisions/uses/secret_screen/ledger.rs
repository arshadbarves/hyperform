//! `SecretLedger`: which tool results were screened and what the user
//! decided about each flagged value, by fingerprint only. It lives as
//! long as the session; a reopened session screens its results again.

use std::collections::HashSet;
use std::sync::Mutex;

use serde_json::json;
use z_engine_decisions::DecisionRequest;
use z_engine_protocol::CallId;

use crate::sync::lock;

#[derive(Debug, Default)]
pub(crate) struct SecretLedger {
    inner: Mutex<Ledger>,
}

#[derive(Debug, Default)]
struct Ledger {
    screened: HashSet<CallId>,
    sent: HashSet<String>,
    withheld: HashSet<String>,
}

/// What the user already said about a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Settled {
    Sent,
    Withheld,
}

impl SecretLedger {
    /// Marks `id` screened; false when it already was.
    pub(crate) fn first_screen(&self, id: &CallId) -> bool {
        lock(&self.inner).screened.insert(id.clone())
    }

    pub(crate) fn settled(&self, value: &str) -> Option<Settled> {
        let print = fingerprint(value);
        let ledger = lock(&self.inner);
        if ledger.withheld.contains(&print) {
            Some(Settled::Withheld)
        } else if ledger.sent.contains(&print) {
            Some(Settled::Sent)
        } else {
            None
        }
    }

    pub(crate) fn settle(&self, value: &str, settled: Settled) {
        let print = fingerprint(value);
        let mut ledger = lock(&self.inner);
        match settled {
            Settled::Sent => ledger.sent.insert(print),
            Settled::Withheld => ledger.withheld.insert(print),
        };
    }
}

/// A short digest of `value`; traces and the ledger keep only this.
pub(crate) fn fingerprint(value: &str) -> String {
    DecisionRequest::new(json!({ "secret": value })).fingerprint()
}
