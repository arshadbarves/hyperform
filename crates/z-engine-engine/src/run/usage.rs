//! Usage and cost: priced per model, accumulated per run, per agent and per
//! session, and announced with `UsageUpdated`. Side requests (titles,
//! summaries, extraction) are also persisted as `LogRecord::Usage`; model
//! rounds are persisted by the turn's `TurnFinished`.

use z_engine_llm::cost_usd;
use z_engine_protocol::{AgentId, Event, Usage};
use z_engine_store::LogRecord;

use crate::session::SessionCore;
use crate::settings::models;

/// USD for `usage` on `model`; unknown prices count as free.
pub(crate) fn price(core: &SessionCore, model: &str, usage: &Usage) -> f64 {
    let settings = core.settings();
    let catalog = core.catalog();
    models::pricing(&settings.settings, catalog.as_deref(), model)
        .map_or(0.0, |pricing| cost_usd(&pricing, usage))
}

/// Adds one model round to the agent and session totals and emits
/// `UsageUpdated`; returns the round's cost. `context_tokens` updates the
/// main agent's context gauge.
pub(crate) fn account(
    core: &SessionCore,
    agent: &AgentId,
    model: &str,
    usage: Usage,
    context_tokens: Option<u64>,
) -> f64 {
    let cost = price(core, model, &usage);
    // Emitted under the state lock so concurrent requests publish totals
    // in the order they were added.
    core.with_state(|state| {
        state.usage += usage;
        state.cost_usd += cost;
        let agent_usage = state.agent_usage.entry(agent.clone()).or_default();
        *agent_usage += usage;
        let agent_usage = *agent_usage;
        if let Some(tokens) = context_tokens {
            state.context_tokens = tokens;
        }
        core.events.emit(Event::UsageUpdated {
            agent_id: agent.clone(),
            usage: agent_usage,
            session_usage: state.usage,
            cost_usd: state.cost_usd,
            context_tokens: state.context_tokens,
            context_limit: state.context_limit,
        });
    });
    cost
}

/// A side request's usage: persisted on its own, then accounted.
pub(crate) fn record_side(core: &SessionCore, agent: &AgentId, model: &str, usage: Usage) {
    if usage.is_empty() {
        return;
    }
    let cost = price(core, model, &usage);
    core.journal.append_or_report(&LogRecord::Usage {
        agent_id: agent.clone(),
        usage,
        cost_usd: cost,
    });
    account(core, agent, model, usage, None);
}
