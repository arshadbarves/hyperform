//! `[shell]`: the shell that runs commands, its environment, and the
//! optional command sandbox (`[shell.sandbox]`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct ShellSettings {
    /// Shell executable; `None` detects one.
    pub path: Option<String>,
    /// Variables passed through from the app's environment. Unions across layers.
    pub env_passthrough: Vec<String>,
    /// Variables set for every command.
    pub env: BTreeMap<String, String>,
    pub sandbox: SandboxSettings,
}

/// Runs agent commands (Bash, background shells, checks) in an OS sandbox
/// that only lets them write inside the project, the additional
/// directories, temp directories, tool caches, and `extra_writable`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct SandboxSettings {
    pub enabled: bool,
    /// Off blocks every connection except to localhost.
    pub allow_network: bool,
    /// More writable directories; `~/` is the home directory and relative
    /// paths resolve against the project root. Unions across layers.
    pub extra_writable: Vec<String>,
    /// Run sandboxed commands that would otherwise ask without asking;
    /// deny and ask rules and plan mode still apply.
    pub auto_allow: bool,
}

impl Default for SandboxSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            allow_network: false,
            extra_writable: Vec::new(),
            auto_allow: true,
        }
    }
}

impl SandboxSettings {
    /// Whether the policy may auto-allow commands: the sandbox is enabled
    /// and auto-allow is on. The caller must also check that a sandbox
    /// backend is available on this machine.
    pub fn auto_allows(&self) -> bool {
        self.enabled && self.auto_allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_defaults_and_toml_shape() {
        let defaults = SandboxSettings::default();
        assert!(!defaults.enabled && !defaults.allow_network && defaults.auto_allow);
        assert!(!defaults.auto_allows());
        let shell: ShellSettings =
            toml::from_str("[sandbox]\nenabled = true\nextra_writable = [\"~/scratch\"]\n")
                .unwrap();
        assert!(shell.sandbox.enabled && shell.sandbox.auto_allows());
        assert_eq!(shell.sandbox.extra_writable, ["~/scratch"]);
        assert!(!shell.sandbox.allow_network);
    }
}
