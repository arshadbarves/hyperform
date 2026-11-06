//! Reminders queued for an agent by something outside its run (finished
//! background jobs, `SessionStart` hook context); drained into that
//! agent's next user message.

use std::collections::BTreeMap;
use std::sync::Mutex;

use z_engine_protocol::AgentId;

use crate::sync::lock;

#[derive(Debug, Default)]
pub(crate) struct ReminderBox {
    queued: Mutex<BTreeMap<AgentId, Vec<String>>>,
}

impl ReminderBox {
    pub(crate) fn push(&self, agent: &AgentId, text: String) {
        lock(&self.queued)
            .entry(agent.clone())
            .or_default()
            .push(text);
    }

    pub(crate) fn take(&self, agent: &AgentId) -> Vec<String> {
        lock(&self.queued).remove(agent).unwrap_or_default()
    }
}
