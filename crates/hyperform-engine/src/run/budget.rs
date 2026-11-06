//! Budgets checked before every round: the run's model turns
//! (`agents.max_turns` or the definition's) and the session cost cap.

use super::spec::RunContext;

/// Why the run must stop, if it must.
pub(crate) fn exhausted(ctx: &RunContext, rounds: u32) -> Option<String> {
    let max_turns = ctx.spec.max_turns;
    if rounds >= max_turns {
        return Some(format!("reached the limit of {max_turns} model turns"));
    }
    let cap = ctx.core.settings().settings.agents.session_cost_cap_usd;
    let spent = ctx.core.with_state(|state| state.cost_usd);
    (cap > 0.0 && spent >= cap)
        .then(|| format!("reached the session cost cap of ${cap:.2} (spent ${spent:.2})"))
}
