//! This chat's task-bound helpers that still run, so pausing the plan,
//! holding a task, stopping or abandoning the plan, closing the chat,
//! detaching the plan or moving it to another chat can stop them. A soft stop ends a
//! helper at its next round boundary (one waiting on an approval at once),
//! and its task then waits with a resume pointer; a hard stop cancels it
//! and records nothing more on the plan.

use std::sync::{Arc, Mutex};

use tokio_util::sync::CancellationToken;
use z_engine_protocol::{AgentId, AgentStatus, PlanId, TaskId};

use super::plan::PlanHandle;
use crate::orchestration::AgentTracker;
use crate::sync::lock;

/// Why a helper was stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Halt {
    /// Stopped before it finished: its task waits with a resume pointer
    /// whose note this is.
    Interrupt(&'static str),
    /// The plan was stopped or abandoned.
    Stopped,
}

/// The note of a helper stopped because the plan was paused.
pub(crate) const PAUSED: &str = "the plan was paused";
/// The note of a helper stopped because the user held its task.
pub(crate) const HELD: &str = "the user held the task";
/// The note of a helper stopped because its chat closed.
pub(crate) const CLOSED: &str = "the chat closed";
/// The note of a helper stopped because the plan moved to another chat.
pub(crate) const MOVED: &str = "the plan moved to another chat";
/// The note of a helper stopped because its chat let the plan go.
pub(crate) const DETACHED: &str = "the plan was detached from its chat";

#[derive(Debug)]
pub(crate) struct Bound {
    pub agent: AgentId,
    pub task: TaskId,
    pub plan: Arc<PlanHandle>,
    pub tracker: Arc<AgentTracker>,
    pub cancel: CancellationToken,
    pub halt: Option<Halt>,
}

#[derive(Debug, Default)]
pub(crate) struct BoundHelpers {
    inner: Mutex<Vec<Bound>>,
}

impl BoundHelpers {
    pub(crate) fn add(&self, bound: Bound) {
        let mut inner = lock(&self.inner);
        inner.retain(|other| other.agent != bound.agent);
        inner.push(bound);
    }

    /// The helper's entry, removed (its run ended).
    pub(crate) fn remove(&self, agent: &AgentId) -> Option<Bound> {
        let mut inner = lock(&self.inner);
        let at = inner.iter().position(|bound| bound.agent == *agent)?;
        Some(inner.remove(at))
    }

    /// Stops the helpers of `plan` whose task `select` picks; returns them.
    /// A hard stop overrides an earlier soft one, never the other way round.
    pub(crate) fn halt(
        &self,
        plan: &PlanId,
        halt: Halt,
        select: impl Fn(&TaskId) -> bool,
    ) -> Vec<AgentId> {
        let mut inner = lock(&self.inner);
        let picked = inner
            .iter_mut()
            .filter(|bound| bound.plan.id == *plan && select(&bound.task));
        let mut stopped = Vec::new();
        for bound in picked {
            stopped.push(bound.agent.clone());
            let before = bound.halt;
            let after = merged(before, halt);
            bound.halt = Some(after);
            if before == Some(after) {
                continue;
            }
            match after {
                Halt::Stopped => bound.cancel.cancel(),
                Halt::Interrupt(_) => {
                    bound.tracker.request_stop();
                    if bound.tracker.info().status == AgentStatus::Waiting {
                        bound.cancel.cancel();
                    }
                }
            }
        }
        stopped
    }

    /// Soft-stops every helper (the chat is closing).
    pub(crate) fn halt_all(&self, note: &'static str) {
        let plans: Vec<PlanId> = lock(&self.inner)
            .iter()
            .map(|bound| bound.plan.id.clone())
            .collect();
        for plan in plans {
            self.halt(&plan, Halt::Interrupt(note), |_| true);
        }
    }
}

/// A hard stop overrides a soft one; a second soft stop keeps the first
/// note.
fn merged(current: Option<Halt>, halt: Halt) -> Halt {
    match (current, halt) {
        (_, Halt::Stopped) => Halt::Stopped,
        (Some(current), Halt::Interrupt(_)) => current,
        (None, halt) => halt,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hard_stop_overrides_a_soft_one_and_not_the_reverse() {
        let order = [
            (Halt::Interrupt(PAUSED), Halt::Stopped, Halt::Stopped),
            (Halt::Stopped, Halt::Interrupt(PAUSED), Halt::Stopped),
            (
                Halt::Interrupt(PAUSED),
                Halt::Interrupt(HELD),
                Halt::Interrupt(PAUSED),
            ),
        ];
        for (first, then, kept) in order {
            assert_eq!(merged(Some(first), then), kept);
        }
        assert_eq!(
            merged(None, Halt::Interrupt(CLOSED)),
            Halt::Interrupt(CLOSED)
        );
    }
}
