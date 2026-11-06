//! `[hooks]`: shell commands run on agent lifecycle events.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Hook events, named as in Claude Code.
pub const HOOK_EVENTS: [&str; 9] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "Stop",
    "SubagentStop",
    "PreCompact",
    "Notification",
    "SessionEnd",
];

pub fn is_hook_event(name: &str) -> bool {
    HOOK_EVENTS.contains(&name)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct HookConfig {
    /// Regex over tool names for tool events; `None` matches every tool.
    pub matcher: Option<String>,
    pub command: String,
    #[ts(type = "number")]
    pub timeout_secs: u64,
}

impl Default for HookConfig {
    fn default() -> Self {
        Self {
            matcher: None,
            command: String::new(),
            timeout_secs: 60,
        }
    }
}
