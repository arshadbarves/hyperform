//! The session's tool registry: built-in tools (the `Agent` description
//! listing the current agent types; while `work_plans` is On, `Agent` with
//! `task_id` and `ExitPlanMode` with the structured plan fields) followed by
//! the MCP catalog.

use std::sync::Arc;

use hyperform_config::{FeatureId, Settings};
use hyperform_tools::builtin::{AgentTool, ExitPlanModeTool};
use hyperform_tools::{AgentCard, ToolRegistry};

use crate::session::SessionCore;
use crate::sync::write;

pub(crate) fn builtin_registry(cards: Vec<AgentCard>, settings: &Settings) -> ToolRegistry {
    if !settings.feature(FeatureId::WorkPlans).acts() {
        return ToolRegistry::builtin(cards);
    }
    let mut registry = ToolRegistry::builtin(cards.clone());
    registry.register(Arc::new(ExitPlanModeTool::structured()));
    registry.register(Arc::new(AgentTool::with_tasks(cards)));
    registry
}

/// Rebuilds the registry after agent types, MCP tools or settings changed.
pub(crate) fn rebuild_tools(core: &SessionCore) {
    let settings = core.settings();
    let mut registry = builtin_registry(core.agents.registry().cards(), &settings.settings);
    core.mcp.catalog().register(&mut registry);
    *write(&core.tools) = Arc::new(registry);
}
