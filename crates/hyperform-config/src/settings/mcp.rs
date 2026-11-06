//! `[mcp]`: Model Context Protocol servers (stdio or streamable HTTP).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct McpSettings {
    /// A later layer's server replaces an earlier one with the same name.
    pub servers: BTreeMap<String, McpServerConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct McpServerConfig {
    /// Executable of a stdio server. Exactly one of `command` and `url` is set.
    pub command: Option<String>,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: Option<String>,
    /// Endpoint of a streamable HTTP server.
    pub url: Option<String>,
    pub headers: BTreeMap<String, String>,
    pub enabled: bool,
    /// Tools of this server hidden from the model.
    pub disabled_tools: Vec<String>,
    #[ts(type = "number")]
    pub timeout_secs: u64,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            command: None,
            args: Vec::new(),
            env: BTreeMap::new(),
            cwd: None,
            url: None,
            headers: BTreeMap::new(),
            enabled: true,
            disabled_tools: Vec::new(),
            timeout_secs: 60,
        }
    }
}

impl McpServerConfig {
    /// Why the server cannot be started, if its transport is ambiguous.
    pub fn transport_error(&self) -> Option<&'static str> {
        let set = |value: &Option<String>| value.as_deref().is_some_and(|v| !v.trim().is_empty());
        match (set(&self.command), set(&self.url)) {
            (true, false) | (false, true) => None,
            (true, true) => Some("set only one of `command` and `url`"),
            (false, false) => Some("set `command` or `url`"),
        }
    }
}
