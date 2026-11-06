//! Check evidence and the per-turn verification badge.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ids::AgentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CheckKind {
    Test,
    Build,
    Lint,
    Typecheck,
    Format,
    Custom,
}

impl CheckKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "test" | "tests" => Some(Self::Test),
            "build" | "compile" => Some(Self::Build),
            "lint" => Some(Self::Lint),
            "typecheck" | "types" | "check" => Some(Self::Typecheck),
            "format" | "fmt" => Some(Self::Format),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Test => "test",
            Self::Build => "build",
            Self::Lint => "lint",
            Self::Typecheck => "typecheck",
            Self::Format => "format",
            Self::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TestCounts {
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
}

/// One executed check. Fingerprints identify the workspace state before
/// and after the run; a later mutation makes the record stale.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CheckRecord {
    pub record_id: String,
    pub check_id: String,
    pub label: String,
    pub kind: CheckKind,
    pub command: String,
    pub cwd: String,
    pub agent_id: AgentId,
    pub exit_code: Option<i32>,
    pub passed: bool,
    pub timed_out: bool,
    #[ts(type = "number")]
    pub started_at: u64,
    #[ts(type = "number")]
    pub duration_ms: u64,
    pub tests: Option<TestCounts>,
    /// Path of the full output artifact.
    pub artifact: Option<String>,
    pub fingerprint_before: String,
    pub fingerprint_after: String,
    pub output_tail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum VerificationOutcome {
    /// Nothing was changed, so there is nothing to verify.
    NotApplicable,
    Unverified {
        reason: String,
    },
    /// Passing checks newer than the last change (record ids).
    Verified {
        checks: Vec<String>,
    },
    Failed {
        reason: String,
    },
}

impl VerificationOutcome {
    pub fn label(&self) -> &'static str {
        match self {
            Self::NotApplicable => "not applicable",
            Self::Unverified { .. } => "unverified",
            Self::Verified { .. } => "verified",
            Self::Failed { .. } => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum VerificationMode {
    Off,
    /// Show the badge only.
    #[default]
    Report,
    /// Run configured checks at stop when files changed; feed failures back.
    Auto,
    /// Like `Auto`, but keep going until checks pass or the budget ends.
    Strict,
}

impl VerificationMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "off" => Some(Self::Off),
            "report" => Some(Self::Report),
            "auto" => Some(Self::Auto),
            "strict" => Some(Self::Strict),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_is_tagged_by_status() {
        let json = serde_json::to_value(VerificationOutcome::Unverified {
            reason: "no checks".into(),
        })
        .unwrap();
        assert_eq!(json["status"], "unverified");
        assert_eq!(json["reason"], "no checks");
    }

    #[test]
    fn check_kind_parses_aliases() {
        assert_eq!(CheckKind::parse("tests"), Some(CheckKind::Test));
        assert_eq!(CheckKind::parse("fmt"), Some(CheckKind::Format));
    }
}
