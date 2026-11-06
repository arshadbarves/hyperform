//! `RouteMemory`: the session's current task (its opening request, turns,
//! the usage and cost when it started, its route) and the read side the
//! engine applies: the routed effort while the user leaves effort on auto,
//! and the routed model while the session still runs the model it was
//! routed from. Turning the feature off stops both at once.

use std::sync::Mutex;

use z_engine_config::{FeatureId, FeatureMode};
use z_engine_protocol::{Effort, Usage};

use crate::session::SessionCore;
use crate::sync::lock;

#[derive(Debug, Default)]
pub(crate) struct RouteMemory {
    task: Mutex<Option<Task>>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Task {
    /// The start of the request that opened the task.
    pub request: String,
    pub turns: u32,
    pub usage_at_start: Usage,
    pub cost_at_start: f64,
    pub route: Option<Route>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Route {
    pub effort: Option<Effort>,
    /// The fast model, and the session model it stands in for.
    pub model: Option<(String, String)>,
}

impl RouteMemory {
    pub(super) fn current(&self) -> Option<Task> {
        lock(&self.task).clone()
    }

    /// Starts a task, returning the one it ends.
    pub(super) fn start(&self, task: Task) -> Option<Task> {
        lock(&self.task).replace(task)
    }

    /// One more turn of the current task.
    pub(super) fn follow_up(&self) {
        if let Some(task) = lock(&self.task).as_mut() {
            task.turns += 1;
        }
    }

    fn route(&self) -> Option<Route> {
        lock(&self.task).as_ref()?.route.clone()
    }
}

/// The current task's effort when the user left effort unset.
pub(crate) fn routed_effort(core: &SessionCore) -> Option<Effort> {
    if !routing_on(core) {
        return None;
    }
    core.decisions.routes().route()?.effort
}

/// The fast model while the session runs the model the task was routed
/// from and model switching is still allowed.
pub(crate) fn routed_model(core: &SessionCore, session_model: &str) -> Option<String> {
    if !routing_on(core)
        || !core
            .settings()
            .settings
            .decisions
            .routing
            .allow_model_switch
    {
        return None;
    }
    let (model, from) = core.decisions.routes().route()?.model?;
    (from == session_model).then_some(model)
}

fn routing_on(core: &SessionCore) -> bool {
    core.decisions.service().mode(FeatureId::DecisionsRouting) == FeatureMode::On
}

/// Usage between two snapshots of a session's running total.
pub(super) fn since(now: Usage, start: Usage) -> Usage {
    Usage {
        input_tokens: now.input_tokens.saturating_sub(start.input_tokens),
        output_tokens: now.output_tokens.saturating_sub(start.output_tokens),
        cache_read_tokens: now
            .cache_read_tokens
            .saturating_sub(start.cache_read_tokens),
        cache_write_tokens: now
            .cache_write_tokens
            .saturating_sub(start.cache_write_tokens),
        reasoning_tokens: now.reasoning_tokens.saturating_sub(start.reasoning_tokens),
    }
}
