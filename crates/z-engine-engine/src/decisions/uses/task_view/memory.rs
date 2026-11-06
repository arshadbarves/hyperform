//! What `decisions_task_view` keeps for the session: the view planned at
//! turn start, waiting for the opening message to be saved; the model's
//! standing-rule verdict per user message (for one provider revision); and
//! the files set-aside exchanges were saved to, to notice readbacks.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use z_engine_context::compaction::{Exchange, TaskViewPlan};
use z_engine_protocol::MessageId;

use crate::sync::lock;

/// Bounds the verdict map; when full it starts over.
const CAPACITY: usize = 4_096;

#[derive(Debug, Default)]
pub(crate) struct TaskViewMemory {
    pending: Mutex<Option<PendingView>>,
    rules: Mutex<(String, HashMap<MessageId, bool>)>,
    spilled: Mutex<HashSet<String>>,
}

/// A view planned for the history with the `history` ids.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PendingView {
    pub history: Vec<MessageId>,
    pub exchanges: Vec<Exchange>,
    pub plan: TaskViewPlan,
    /// Estimated tokens of the set-aside exchanges.
    pub tokens: u64,
}

impl TaskViewMemory {
    pub(super) fn set_pending(&self, view: Option<PendingView>) {
        *lock(&self.pending) = view;
    }

    pub(super) fn take_pending(&self) -> Option<PendingView> {
        lock(&self.pending).take()
    }

    /// The standing-rule verdicts for `revision`; another one forgets them.
    pub(super) fn rules(&self, revision: &str) -> HashMap<MessageId, bool> {
        let mut rules = lock(&self.rules);
        if rules.0 != revision {
            *rules = (revision.to_string(), HashMap::new());
        }
        rules.1.clone()
    }

    pub(super) fn remember_rule(&self, revision: &str, message: MessageId, standing: bool) {
        let mut rules = lock(&self.rules);
        if rules.0 != revision {
            return;
        }
        if rules.1.len() >= CAPACITY {
            rules.1.clear();
        }
        rules.1.insert(message, standing);
    }

    pub(super) fn spilled(&self, paths: impl IntoIterator<Item = String>) {
        lock(&self.spilled).extend(paths);
    }

    pub(super) fn is_spilled(&self, path: &str) -> bool {
        lock(&self.spilled).contains(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_verdicts_belong_to_one_revision() {
        let memory = TaskViewMemory::default();
        let id = MessageId::from("msg_1");
        assert!(memory.rules("r1").is_empty());
        memory.remember_rule("r1", id.clone(), true);
        assert_eq!(memory.rules("r1").get(&id), Some(&true));
        assert!(memory.rules("r2").is_empty());
    }

    #[test]
    fn pending_views_are_taken_once() {
        let memory = TaskViewMemory::default();
        let view = PendingView {
            history: Vec::new(),
            exchanges: Vec::new(),
            plan: TaskViewPlan::default(),
            tokens: 1,
        };
        memory.set_pending(Some(view.clone()));
        assert_eq!(memory.take_pending(), Some(view));
        assert_eq!(memory.take_pending(), None);
    }
}
