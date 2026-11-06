//! `[compat]`: compatibility with other agent harnesses' files.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct CompatSettings {
    /// Also read `.claude/` folders and `CLAUDE.md` files.
    pub claude: bool,
}

impl Default for CompatSettings {
    fn default() -> Self {
        Self { claude: true }
    }
}
