//! What `decisions_compaction` keeps for the session: the model's confident
//! verdict per tool call for one provider revision, so repeated pressure
//! only asks about new results and a kept result stays kept; and the
//! results cleared on the model's say, to notice when the agent reads one
//! again.

use std::collections::HashMap;
use std::sync::Mutex;

use z_engine_protocol::CallId;

use crate::sync::lock;

/// Bounds both maps; when full they start over.
const CAPACITY: usize = 4_096;

#[derive(Debug, Default)]
pub(crate) struct CompactionMemory {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    revision: String,
    verdicts: HashMap<CallId, bool>,
    /// Reread key of each result cleared on the model's say.
    cleared: HashMap<String, CallId>,
}

impl CompactionMemory {
    /// The verdicts for `revision`; another revision forgets them.
    pub(super) fn verdicts(&self, revision: &str) -> HashMap<CallId, bool> {
        let mut inner = lock(&self.inner);
        if inner.revision != revision {
            inner.revision = revision.to_string();
            inner.verdicts.clear();
        }
        inner.verdicts.clone()
    }

    /// `needed` is the model's confident verdict on `call`.
    pub(super) fn remember(&self, revision: &str, call: CallId, needed: bool) {
        let mut inner = lock(&self.inner);
        if inner.revision != revision {
            return;
        }
        if inner.verdicts.len() >= CAPACITY {
            inner.verdicts.clear();
        }
        inner.verdicts.insert(call, needed);
    }

    /// The result of `call`, found again under `key`, was cleared on the
    /// model's say.
    pub(super) fn cleared(&self, key: String, call: CallId) {
        let mut inner = lock(&self.inner);
        if inner.cleared.len() >= CAPACITY {
            inner.cleared.clear();
        }
        inner.cleared.insert(key, call);
    }

    /// The model-cleared call behind `key`, once.
    pub(super) fn take_reread(&self, key: &str) -> Option<CallId> {
        lock(&self.inner).cleared.remove(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdicts_belong_to_one_revision() {
        let memory = CompactionMemory::default();
        assert!(memory.verdicts("r1").is_empty());
        memory.remember("r1", CallId::from("c1"), true);
        memory.remember("r0", CallId::from("c2"), false);
        assert_eq!(memory.verdicts("r1").len(), 1);
        assert!(memory.verdicts("r2").is_empty());
        memory.remember("r1", CallId::from("c3"), true);
        assert!(memory.verdicts("r2").is_empty());
    }

    #[test]
    fn a_reread_is_reported_once() {
        let memory = CompactionMemory::default();
        memory.cleared("Read a.rs".into(), CallId::from("c1"));
        assert_eq!(memory.take_reread("Read a.rs"), Some(CallId::from("c1")));
        assert_eq!(memory.take_reread("Read a.rs"), None);
    }
}
