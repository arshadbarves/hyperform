//! `[agents]`: budgets for agent runs and subagents.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct AgentSettings {
    /// Subagents running at once (at least 1).
    pub max_concurrent: u32,
    /// Nesting depth of subagents; 0 disables subagents.
    pub max_depth: u32,
    /// Model turns per agent run (at least 1).
    pub max_turns: u32,
    /// Stop the session at this spend; 0 disables the cap.
    pub session_cost_cap_usd: f64,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            max_concurrent: 6,
            max_depth: 2,
            max_turns: 200,
            session_cost_cap_usd: 0.0,
        }
    }
}
