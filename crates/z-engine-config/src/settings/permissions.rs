//! `[permissions]`: the permission level, whether new chats start in plan
//! mode, and the policy engine's rule lists.

use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;
use z_engine_protocol::{ModeName, PermissionMode};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct PermissionSettings {
    /// The permission level new chats start at.
    pub mode: PermissionMode,
    /// New chats start in plan mode. A project may turn it on even when
    /// untrusted, since it only restricts.
    pub planning: bool,
    /// Rules such as `Read`, `Bash(cargo test:*)`, or `Edit(src/**)`.
    /// Rule lists union across layers.
    pub allow: Vec<String>,
    pub ask: Vec<String>,
    pub deny: Vec<String>,
    /// Directories outside the project the agent may work in.
    pub additional_directories: Vec<String>,
    /// Run recognised read-only shell commands without asking.
    pub auto_allow_read_only_bash: bool,
}

impl Default for PermissionSettings {
    fn default() -> Self {
        Self {
            mode: PermissionMode::Default,
            planning: false,
            allow: Vec::new(),
            ask: Vec::new(),
            deny: Vec::new(),
            additional_directories: Vec::new(),
            auto_allow_read_only_bash: true,
        }
    }
}

/// The table as written. `mode` takes every spelling of a level, and
/// `"plan"`, the plan rung from before planning became its own switch,
/// reads as the Ask level with planning on.
#[derive(Deserialize)]
#[serde(default)]
struct Written {
    mode: ModeName,
    planning: bool,
    allow: Vec<String>,
    ask: Vec<String>,
    deny: Vec<String>,
    additional_directories: Vec<String>,
    auto_allow_read_only_bash: bool,
}

impl Default for Written {
    fn default() -> Self {
        let settings = PermissionSettings::default();
        Self {
            mode: settings.mode.into(),
            planning: settings.planning,
            allow: settings.allow,
            ask: settings.ask,
            deny: settings.deny,
            additional_directories: settings.additional_directories,
            auto_allow_read_only_bash: settings.auto_allow_read_only_bash,
        }
    }
}

impl<'de> Deserialize<'de> for PermissionSettings {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let written = Written::deserialize(deserializer)?;
        Ok(Self {
            mode: written.mode.level(),
            planning: written.planning || written.mode.planning(),
            allow: written.allow,
            ask: written.ask,
            deny: written.deny,
            additional_directories: written.additional_directories,
            auto_allow_read_only_bash: written.auto_allow_read_only_bash,
        })
    }
}

/// Which permission list a rule belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub enum RuleKind {
    Allow,
    Ask,
    Deny,
}

impl RuleKind {
    /// The key of this list inside `[permissions]`.
    pub fn key(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Ask => "ask",
            Self::Deny => "deny",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(toml: &str) -> PermissionSettings {
        toml::from_str(toml).unwrap()
    }

    #[test]
    fn the_old_plan_mode_reads_as_planning_at_the_ask_level() {
        for name in ["plan", "readOnly"] {
            let settings = parse(&format!("mode = \"{name}\"\nallow = [\"Read\"]"));
            assert_eq!(settings.mode, PermissionMode::Default, "{name}");
            assert!(settings.planning, "{name}");
            assert_eq!(settings.allow, ["Read"]);
        }
    }

    #[test]
    fn planning_and_the_level_are_separate_keys() {
        let settings = parse("mode = \"bypassPermissions\"\nplanning = true");
        assert_eq!(settings.mode, PermissionMode::Bypass);
        assert!(settings.planning);
        assert_eq!(parse(""), PermissionSettings::default());
        assert!(toml::from_str::<PermissionSettings>("mode = \"sideways\"").is_err());
    }
}
