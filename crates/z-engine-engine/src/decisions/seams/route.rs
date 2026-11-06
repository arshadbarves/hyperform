//! Seam: a task starts, the main agent's new task (`session/turn.rs`) or a
//! subagent whose model is `inherit` (`orchestration/launch.rs`). The first
//! route from an `on` use wins; the caller offers only what the task allows
//! (an effort the user left unset, the configured main and fast models).

use std::sync::Arc;

use z_engine_protocol::Effort;

use super::dispatch::{active, dispatch};
use crate::decisions::registry::{DecisionUse, Seam, USES};
use crate::run::RunContext;

/// What a starting task may be routed to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RouteTask {
    /// The start of the user's request, or of the subagent's prompt.
    pub prompt: String,
    /// The subagent's type; `None` for the main agent.
    pub agent_type: Option<String>,
    /// The start of the current task's request when the message may only
    /// continue it; `None` when a new task starts for sure.
    pub previous: Option<String>,
    /// The user left the effort unset, so a route may pick one.
    pub effort_free: bool,
    /// `(current, fast)` when a route may move the task to the fast model.
    pub models: Option<(String, String)>,
}

/// A new task's route. `None` fields keep the session's choice.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RouteAdvice {
    pub effort: Option<Effort>,
    pub model: Option<String>,
    pub reason: String,
}

/// `None` when nothing routed the task, or when it continues the current
/// one (`previous` was set and no use saw a new task).
pub(crate) async fn route_task(ctx: &RunContext, task: RouteTask) -> Option<RouteAdvice> {
    route_task_with(USES, ctx, task).await
}

pub(super) async fn route_task_with(
    uses: &[&'static dyn DecisionUse],
    ctx: &RunContext,
    task: RouteTask,
) -> Option<RouteAdvice> {
    let active = active(&ctx.core, uses, Seam::Route);
    if active.is_empty() {
        return None;
    }
    let task = Arc::new(task);
    let routes = dispatch(&ctx.core, active, &ctx.cancel, move |decision_use, cx| {
        let task = Arc::clone(&task);
        Box::pin(async move { decision_use.route(&cx, &task).await })
    })
    .await;
    routes.into_iter().flatten().next()
}
