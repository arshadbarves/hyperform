//! Hook events and the JSON payload each hook receives on stdin, with the
//! Claude Code field names (`session_id`, `tool_name`, `tool_input`, ...).

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Map, Value, json};
use z_engine_protocol::{ModeName, PermissionMode, SessionId};

use crate::settings::SessionSettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HookEvent {
    SessionStart,
    UserPromptSubmit,
    PreToolUse,
    PostToolUse,
    Stop,
    SubagentStop,
    PreCompact,
    Notification,
    SessionEnd,
}

impl HookEvent {
    /// The `[[hooks.<name>]]` key, as in `z_engine_config::HOOK_EVENTS`.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::SessionStart => "SessionStart",
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::Stop => "Stop",
            Self::SubagentStop => "SubagentStop",
            Self::PreCompact => "PreCompact",
            Self::Notification => "Notification",
            Self::SessionEnd => "SessionEnd",
        }
    }

    /// Plain stdout of a successful hook is context for the model.
    pub(crate) fn stdout_is_context(self) -> bool {
        matches!(self, Self::UserPromptSubmit | Self::SessionStart)
    }
}

/// Event-specific payload fields plus the value hook matchers test (tool
/// name, compaction trigger, or session source).
#[derive(Debug, Clone, Default)]
pub(crate) struct HookInput {
    pub target: Option<String>,
    pub fields: Map<String, Value>,
}

impl HookInput {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn tool(name: &str, input: &Value) -> Self {
        Self::new()
            .target(name)
            .with("tool_name", json!(name))
            .with("tool_input", input.clone())
    }

    pub(crate) fn target(mut self, target: &str) -> Self {
        self.target = Some(target.to_string());
        self
    }

    pub(crate) fn with(mut self, key: &str, value: Value) -> Self {
        self.fields.insert(key.to_string(), value);
        self
    }
}

/// Where hooks run and what every payload carries.
#[derive(Debug, Clone)]
pub(crate) struct HookEnv {
    pub settings: Arc<SessionSettings>,
    pub session_id: SessionId,
    pub transcript_path: PathBuf,
    pub cwd: PathBuf,
    pub mode: PermissionMode,
    pub planning: bool,
}

impl HookEnv {
    pub(crate) fn payload(&self, event: HookEvent, input: &HookInput) -> Value {
        let mut payload = Map::new();
        payload.insert("session_id".into(), json!(self.session_id));
        payload.insert(
            "transcript_path".into(),
            json!(self.transcript_path.to_string_lossy()),
        );
        payload.insert("cwd".into(), json!(self.cwd.to_string_lossy()));
        payload.insert("hook_event_name".into(), json!(event.name()));
        let mode = ModeName::of(self.planning, self.mode);
        payload.insert("permission_mode".into(), json!(mode.label()));
        payload.extend(input.fields.clone());
        Value::Object(payload)
    }
}
