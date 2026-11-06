//! What a check is: the command, where it runs, which kind of evidence it
//! produces, and whether a manifest or the user's settings defined it.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use z_engine_protocol::CheckKind;

/// Timeout of every discovered check and of configured checks that set 0.
pub const DEFAULT_CHECK_TIMEOUT_SECS: u64 = 600;

/// One runnable check. Commands are repository-derived shell text: they
/// run only after the engine's gate, never during discovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckSpec {
    /// `<tool>:<name>` (`cargo:test`, `npm:lint`, `custom:<id>`); checks of
    /// nested roots are prefixed with the root (`web/npm:test`).
    pub id: String,
    /// Display name used in outcomes, e.g. `npm test (web)`.
    pub label: String,
    pub kind: CheckKind,
    pub command: String,
    /// Relative to the project root (`.` for the root itself); `run_check`
    /// resolves it against `CheckEnv::project_root`. Absolute paths are
    /// used as they are.
    pub cwd: PathBuf,
    pub source: CheckSource,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CheckSource {
    /// Suggested by a manifest (root-relative path, e.g. `web/package.json`).
    Discovered { manifest: String },
    /// Defined in settings.
    Configured,
}

/// A check defined in settings; the engine maps its settings into this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfiguredCheck {
    /// An id with a `:` (e.g. `cargo:test`) is used verbatim and replaces
    /// the discovered check with that id; a bare id `e2e` becomes
    /// `custom:e2e`.
    pub id: String,
    /// Empty uses the command.
    pub label: String,
    pub command: String,
    pub kind: CheckKind,
    /// Relative to the project root; `None` is the root.
    pub cwd: Option<String>,
    /// 0 uses [`DEFAULT_CHECK_TIMEOUT_SECS`].
    pub timeout_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_is_tagged_by_type() {
        let json = serde_json::to_value(CheckSource::Discovered {
            manifest: "web/package.json".into(),
        })
        .unwrap();
        assert_eq!(json["type"], "discovered");
        assert_eq!(json["manifest"], "web/package.json");
        let json = serde_json::to_value(CheckSource::Configured).unwrap();
        assert_eq!(json["type"], "configured");
    }
}
