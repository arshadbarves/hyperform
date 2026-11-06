//! Where routing runs. At a turn's start the engine decides whether a new
//! task starts (the chat's first turn, a chat idle longer than the prompt
//! cache lives, or the model's yes to "is this a new task?") and only then
//! offers a route; a follow-up keeps the task's route, so the cached prompt
//! stays valid. Ending a task records its cost and the share of its input
//! read from the prompt cache. A subagent whose model is `inherit` is
//! routed once, at launch, and only between the main and the fast model.

use z_engine_config::{FeatureId, FeatureMode};
use z_engine_decisions::{AbstainReason, Answer, DecisionRecord};
use z_engine_protocol::decisions::RouteInfo;
use z_engine_protocol::{AgentId, Effort, Event, Usage, now_ms};

use super::memory::{Route, Task, since};
use super::signals::is_reply;
use crate::decisions::seams::{RouteTask, route_task};
use crate::decisions::uses::digest::head;
use crate::run::RunContext;
use crate::session::SessionCore;
use crate::settings::models::context_window;

/// Providers keep a cached prompt prefix about this long after its last
/// use (Anthropic's default lifetime).
pub(super) const CACHE_LIFETIME_MS: u64 = 5 * 60 * 1_000;
pub(super) const TASK_COST: &str = "routing_task_cost";
const REQUEST_CHARS: usize = 600;

/// Called once the user's message is in the transcript, before the run.
pub(crate) async fn route_new_task(ctx: &RunContext, text: &str) {
    let core = &ctx.core;
    let mode = core.decisions.service().mode(FeatureId::DecisionsRouting);
    if !mode.runs() {
        return;
    }
    let memory = core.decisions.routes();
    let current = memory.current();
    let now = now_ms();
    let (idle, effort, model, pinned, usage, cost) = core.with_state(|state| {
        let idle = state
            .turns
            .last()
            .map(|turn| now.saturating_sub(turn.finished_at));
        let pinned = state.turn_model.is_some();
        (
            idle,
            state.effort,
            state.model.clone(),
            pinned,
            state.usage,
            state.cost_usd,
        )
    });
    let sure = current.is_none() || idle.is_none_or(|idle| idle > CACHE_LIFETIME_MS);
    if !sure && is_reply(text) {
        memory.follow_up();
        return;
    }
    let task = RouteTask {
        prompt: text.to_string(),
        agent_type: None,
        previous: current.filter(|_| !sure).map(|task| task.request),
        effort_free: effort.is_none(),
        models: if pinned {
            None
        } else {
            switchable(core, &model)
        },
    };
    let advice = if task.effort_free || task.models.is_some() {
        route_task(ctx, task.clone()).await
    } else {
        None
    };
    if !sure && advice.is_none() {
        memory.follow_up();
        return;
    }
    let route = advice.as_ref().map(|advice| Route {
        effort: advice.effort.filter(|_| task.effort_free),
        model: (advice.model.as_ref())
            .filter(|chosen| offered(&task, chosen))
            .map(|chosen| (chosen.clone(), model.clone())),
    });
    let ended = memory.start(Task {
        request: head(text, REQUEST_CHARS),
        turns: 1,
        usage_at_start: usage,
        cost_at_start: cost,
        route: route.clone(),
    });
    if let Some(ended) = ended {
        record_cost(core, mode, &ended, usage, cost);
    }
    if let (Some(advice), Some(route)) = (advice, route) {
        let model = route.model.map(|(model, _)| model);
        announce(core, AgentId::main(), route.effort, model, advice.reason);
    }
}

/// The fast model for a subagent whose definition inherits its model, when
/// switching is allowed and the decision model calls its task simple.
pub(crate) async fn route_subagent(
    parent: &RunContext,
    agent_type: &str,
    agent_id: &AgentId,
    prompt: &str,
) -> Option<String> {
    let service = parent.core.decisions.service();
    if !service.mode(FeatureId::DecisionsRouting).runs() {
        return None;
    }
    let models = switchable(&parent.core, &parent.model())?;
    let fast = models.1.clone();
    let task = RouteTask {
        prompt: prompt.to_string(),
        agent_type: Some(agent_type.to_string()),
        previous: None,
        effort_free: false,
        models: Some(models),
    };
    let advice = route_task(parent, task).await?;
    let model = advice.model.filter(|model| *model == fast)?;
    announce(
        &parent.core,
        agent_id.clone(),
        None,
        Some(model.clone()),
        advice.reason,
    );
    Some(model)
}

/// `(current, fast)` when a task may move to the fast model: switching is
/// allowed, the session runs the main model, and the fast model differs and
/// holds at least as much context.
fn switchable(core: &SessionCore, current: &str) -> Option<(String, String)> {
    let settings = core.settings();
    let settings = &settings.settings;
    if !settings.decisions.routing.allow_model_switch || current != settings.model.main {
        return None;
    }
    let fast = settings.model.fast_model();
    let catalog = core.catalog();
    let window = |model: &str| context_window(settings, catalog.as_deref(), model);
    (fast != current && window(fast) >= window(current))
        .then(|| (current.to_string(), fast.to_string()))
}

fn offered(task: &RouteTask, model: &str) -> bool {
    (task.models.as_ref()).is_some_and(|(_, fast)| fast == model)
}

fn announce(
    core: &SessionCore,
    agent_id: AgentId,
    effort: Option<Effort>,
    model: Option<String>,
    reason: String,
) {
    if effort.is_none() && model.is_none() {
        return;
    }
    let route = RouteInfo {
        agent_id,
        effort,
        model,
        reason,
    };
    core.events.emit(Event::RouteChosen { route });
}

/// The ended task's cost and cache share, as a trace record.
fn record_cost(core: &SessionCore, mode: FeatureMode, ended: &Task, usage: Usage, cost: f64) {
    let spent = since(usage, ended.usage_at_start);
    let prompt = spent.prompt_tokens();
    let cached = (spent.cache_read_tokens * 100)
        .checked_div(prompt)
        .unwrap_or(0);
    let turns = ended.turns;
    let plural = if turns == 1 { "" } else { "s" };
    let outcome = format!(
        "task of {turns} turn{plural}: ${:.4}, {cached}% of input from the prompt cache ({})",
        (cost - ended.cost_at_start).max(0.0),
        route_label(ended.route.as_ref()),
    );
    let answer = Answer::abstained(TASK_COST, AbstainReason::Rules, "rules");
    let shadow = mode == FeatureMode::Shadow;
    let record = DecisionRecord::of(FeatureId::DecisionsRouting.as_str(), &answer, "", shadow);
    core.decisions.trace().record(record.outcome(&outcome));
}

fn route_label(route: Option<&Route>) -> String {
    let effort = route.and_then(|route| route.effort);
    let fast = route.is_some_and(|route| route.model.is_some());
    match (effort, fast) {
        (Some(effort), true) => format!("effort {}, fast model", effort.label()),
        (Some(effort), false) => format!("effort {}", effort.label()),
        (None, true) => "fast model".to_string(),
        (None, false) => "not routed".to_string(),
    }
}
