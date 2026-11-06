//! `[context]`: prompt budget and compaction.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const MIN_COMPACT_AT_PERCENT: u32 = 50;
pub const MAX_COMPACT_AT_PERCENT: u32 = 99;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct ContextSettings {
    /// Compact when the context is this full, clamped to 50..=99.
    pub compact_at_percent: u32,
    /// Tool results kept verbatim when older ones are cleared.
    pub keep_recent_tool_results: u32,
    /// Include a repository map in the system prompt.
    pub repo_map: bool,
    pub repo_map_chars: u32,
}

impl Default for ContextSettings {
    fn default() -> Self {
        Self {
            compact_at_percent: 92,
            keep_recent_tool_results: 8,
            repo_map: true,
            repo_map_chars: 6_000,
        }
    }
}
