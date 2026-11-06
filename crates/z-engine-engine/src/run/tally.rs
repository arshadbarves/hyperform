//! What happens on a run's behalf outside its own model rounds: usage of
//! foreground subagents (for the spawning turn's record) and changes they
//! or an applied worktree made to the run's tree (for verification).

use std::sync::Mutex;

use z_engine_protocol::Usage;

use crate::sync::lock;

#[derive(Debug, Default)]
pub(crate) struct ChildTally {
    inner: Mutex<Absorbed>,
}

/// Accumulated since the run last took it.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Absorbed {
    pub usage: Usage,
    pub cost_usd: f64,
    pub mutated: bool,
}

impl ChildTally {
    pub(crate) fn add_usage(&self, usage: Usage, cost_usd: f64) {
        let mut inner = lock(&self.inner);
        inner.usage += usage;
        inner.cost_usd += cost_usd;
    }

    pub(crate) fn mark_mutated(&self) {
        lock(&self.inner).mutated = true;
    }

    pub(crate) fn take(&self) -> Absorbed {
        std::mem::take(&mut *lock(&self.inner))
    }
}
