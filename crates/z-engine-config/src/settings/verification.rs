//! `[verification]`: the verification mode and configured checks.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use z_engine_protocol::{CheckKind, VerificationMode};

use super::lenient;

pub const MAX_CONTINUATIONS: u32 = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct VerificationSettings {
    #[serde(deserialize_with = "lenient::verification_mode")]
    pub mode: VerificationMode,
    /// Automatic continuations after failed checks (at most 10).
    pub max_continuations: u32,
    /// Check kinds (`test`, `lint`, ...) or check ids run in auto/strict mode.
    pub auto_checks: Vec<String>,
    /// A later layer's check replaces an earlier one with the same `id`.
    pub checks: Vec<CheckConfig>,
}

impl Default for VerificationSettings {
    fn default() -> Self {
        Self {
            mode: VerificationMode::Report,
            max_continuations: 3,
            auto_checks: vec!["test".to_string()],
            checks: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct CheckConfig {
    pub id: String,
    /// Display name; empty uses `id`.
    pub label: String,
    pub command: String,
    #[serde(deserialize_with = "lenient::check_kind")]
    pub kind: CheckKind,
    /// Working directory relative to the project root.
    pub cwd: Option<String>,
    #[ts(type = "number")]
    pub timeout_secs: u64,
}

impl Default for CheckConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            command: String::new(),
            kind: CheckKind::Custom,
            cwd: None,
            timeout_secs: 600,
        }
    }
}
