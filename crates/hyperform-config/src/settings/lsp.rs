//! `[lsp]`: language servers used for diagnostics and navigation.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct LspSettings {
    pub enabled: bool,
    /// A later layer's server replaces an earlier one with the same name.
    pub servers: BTreeMap<String, LspServerConfig>,
}

impl Default for LspSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            servers: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct LspServerConfig {
    pub command: String,
    pub args: Vec<String>,
    /// File extensions without the dot, e.g. `rs`.
    pub extensions: Vec<String>,
    /// Files marking a workspace root, e.g. `Cargo.toml`.
    pub root_markers: Vec<String>,
    pub enabled: bool,
}

impl Default for LspServerConfig {
    fn default() -> Self {
        Self {
            command: String::new(),
            args: Vec::new(),
            extensions: Vec::new(),
            root_markers: Vec::new(),
            enabled: true,
        }
    }
}
