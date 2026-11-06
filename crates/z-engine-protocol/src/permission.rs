//! Permission levels, the one-name form of a mode, and the approval
//! exchange.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use ts_rs::TS;

use crate::ids::{AgentId, CallId, RequestId};

/// How often the agent asks. Planning is a separate switch next to it
/// (`SessionInfo::planning`), so a session can plan at any level.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PermissionMode {
    /// Ask before gated actions.
    #[default]
    Default,
    /// Auto-approve file edits inside allowed directories.
    AcceptEdits,
    /// Approve everything except explicit deny rules.
    Bypass,
}

impl PermissionMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::AcceptEdits => "acceptEdits",
            Self::Bypass => "bypass",
        }
    }

    /// Accepts v2 names plus v1 and Claude Code spellings of a level; the
    /// old `plan` rung is read by [`ModeName::parse`].
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "default" | "normal" | "ask" => Some(Self::Default),
            "acceptEdits" | "accept_edits" | "accept-edits" | "auto-accept edits" => {
                Some(Self::AcceptEdits)
            }
            "bypass" | "bypassPermissions" => Some(Self::Bypass),
            _ => None,
        }
    }
}

/// A mode as one name, the form settings, agent frontmatter, session logs
/// and hook payloads (`permission_mode`) use. Before planning became its
/// own switch, `plan` was a rung of the permission ladder; read back, it
/// means planning on at the Ask level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeName {
    Level(PermissionMode),
    Plan,
}

impl ModeName {
    /// `plan` while planning, otherwise the level.
    pub fn of(planning: bool, level: PermissionMode) -> Self {
        if planning {
            Self::Plan
        } else {
            Self::Level(level)
        }
    }

    /// Every level spelling, plus `plan`, `readOnly` and `read-only`.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "plan" | "readOnly" | "read-only" => Some(Self::Plan),
            other => PermissionMode::parse(other).map(Self::Level),
        }
    }

    pub fn level(self) -> PermissionMode {
        match self {
            Self::Level(level) => level,
            Self::Plan => PermissionMode::Default,
        }
    }

    pub fn planning(self) -> bool {
        self == Self::Plan
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Level(level) => level.label(),
            Self::Plan => "plan",
        }
    }
}

impl From<PermissionMode> for ModeName {
    fn from(level: PermissionMode) -> Self {
        Self::Level(level)
    }
}

impl Serialize for ModeName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.label())
    }
}

impl<'de> Deserialize<'de> for ModeName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw)
            .ok_or_else(|| D::Error::custom(format!("unknown permission mode `{raw}`")))
    }
}

/// The user's answer to an approval request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ApprovalDecision {
    AllowOnce,
    /// Allow and add `rule` for the rest of the session.
    AllowSession {
        rule: String,
    },
    /// Allow and persist `rule` to the project's local settings.
    AllowProject {
        rule: String,
    },
    /// Refuse; optional feedback is returned to the model.
    Deny {
        #[serde(default)]
        feedback: Option<String>,
    },
}

impl ApprovalDecision {
    pub fn allows(&self) -> bool {
        !matches!(self, Self::Deny { .. })
    }
}

/// Rich detail rendered by the approval card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Preview {
    Diff {
        path: String,
        diff: String,
    },
    Command {
        command: String,
        #[serde(default)]
        description: Option<String>,
    },
    Text {
        text: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ApprovalRequest {
    pub request_id: RequestId,
    pub agent_id: AgentId,
    pub call_id: CallId,
    pub tool: String,
    /// One-line description, e.g. "Edit src/main.rs".
    pub title: String,
    pub input: Value,
    pub preview: Option<Preview>,
    /// Why the policy asked (matched rule, outside directory, ...).
    pub reason: String,
    pub suggested_rule: Option<String>,
    /// False for targets outside allowed directories.
    pub can_persist: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_parse_accepts_legacy_spellings() {
        assert_eq!(
            PermissionMode::parse("normal"),
            Some(PermissionMode::Default)
        );
        assert_eq!(
            PermissionMode::parse("bypassPermissions"),
            Some(PermissionMode::Bypass)
        );
        assert_eq!(PermissionMode::parse("nope"), None);
        assert_eq!(PermissionMode::parse("plan"), None);
    }

    #[test]
    fn plan_names_read_as_planning_at_the_ask_level() {
        for name in ["plan", "readOnly", "read-only"] {
            let mode = ModeName::parse(name).unwrap();
            assert!(mode.planning(), "{name}");
            assert_eq!(mode.level(), PermissionMode::Default);
        }
        let bypass = ModeName::parse("bypassPermissions").unwrap();
        assert_eq!(bypass, ModeName::Level(PermissionMode::Bypass));
        assert!(!bypass.planning());
        assert_eq!(ModeName::parse("sideways"), None);
    }

    #[test]
    fn mode_names_round_trip_as_strings() {
        let back: ModeName = serde_json::from_value(serde_json::json!("plan")).unwrap();
        assert_eq!(back, ModeName::Plan);
        let level = ModeName::from(PermissionMode::AcceptEdits);
        assert_eq!(serde_json::to_value(level).unwrap(), "acceptEdits");
        assert!(serde_json::from_value::<ModeName>(serde_json::json!("nope")).is_err());
        assert_eq!(ModeName::of(true, PermissionMode::Bypass).label(), "plan");
        assert_eq!(
            ModeName::of(false, PermissionMode::Bypass).label(),
            "bypass"
        );
    }

    #[test]
    fn decisions_are_tagged() {
        let json = serde_json::to_value(ApprovalDecision::Deny {
            feedback: Some("use cargo".into()),
        })
        .unwrap();
        assert_eq!(json["type"], "deny");
        assert_eq!(json["feedback"], "use cargo");
        let back: ApprovalDecision =
            serde_json::from_value(serde_json::json!({"type": "allowOnce"})).unwrap();
        assert!(back.allows());
    }
}
