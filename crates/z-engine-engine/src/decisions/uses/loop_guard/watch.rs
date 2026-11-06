//! `LoopWatch`: each agent's steps in the current turn, the reminders it
//! got, when the decision model last looked, and the flag that makes its
//! next allowed call ask the user. A new turn starts every agent afresh,
//! since the user's message may well ask to run the same thing again.

use std::collections::HashMap;
use std::sync::Mutex;

use z_engine_protocol::AgentId;

use super::steps::Step;
use crate::sync::lock;

/// Steps kept per agent.
const WINDOW: usize = 16;
/// Agents tracked at once; past this the watch starts over.
const AGENTS: usize = 64;

#[derive(Debug, Default)]
pub(crate) struct LoopWatch {
    agents: Mutex<HashMap<AgentId, History>>,
}

#[derive(Debug, Default)]
pub(super) struct History {
    turn: usize,
    pub steps: Vec<Step>,
    /// Steps seen this turn.
    pub seen: usize,
    /// `seen` when the decision model last looked.
    pub checked: Option<usize>,
    pub reminders: u32,
    /// The agent's next allowed call asks the user.
    pub ask: bool,
}

impl History {
    pub(super) fn push(&mut self, step: Step) {
        self.steps.push(step);
        if self.steps.len() > WINDOW {
            self.steps.remove(0);
        }
        self.seen += 1;
    }

    /// At least `every` steps since the decision model last looked.
    pub(super) fn due(&self, every: usize) -> bool {
        self.checked
            .is_none_or(|checked| self.seen - checked >= every)
    }
}

impl LoopWatch {
    /// Runs `change` on `agent`'s history, reset first when `turn` is new.
    pub(super) fn with<R>(
        &self,
        agent: &AgentId,
        turn: usize,
        change: impl FnOnce(&mut History) -> R,
    ) -> R {
        let mut agents = lock(&self.agents);
        if agents.len() >= AGENTS && !agents.contains_key(agent) {
            agents.clear();
        }
        let history = agents.entry(agent.clone()).or_default();
        if history.turn != turn {
            *history = History {
                turn,
                ..History::default()
            };
        }
        change(history)
    }
}
