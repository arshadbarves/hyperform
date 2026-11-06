//! The discovered extension set and where each definition came from.

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{AgentDef, CommandDef, OutputStyleDef, RuleDef, SkillDef};

/// Where a definition was found; variants are ordered by precedence,
/// lowest first, and a higher scope replaces a same-named definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "config/")]
pub enum ExtensionScope {
    /// `~/.claude/`
    ClaudeUser,
    /// The z-engine config directory.
    User,
    /// `<project>/.claude/`
    ClaudeProject,
    /// `<project>/.z-engine/`
    Project,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "config/")]
pub struct ExtensionSource {
    pub scope: ExtensionScope,
    pub path: String,
}

impl ExtensionSource {
    pub fn new(scope: ExtensionScope, path: &Path) -> Self {
        Self {
            scope,
            path: path.to_string_lossy().into_owned(),
        }
    }

    /// File name without extension, the default definition name.
    pub(crate) fn stem(&self) -> Option<String> {
        Path::new(&self.path)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .filter(|stem| !stem.is_empty())
    }
}

/// A file that was skipped, with the reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "config/")]
pub struct ExtensionError {
    pub path: String,
    pub message: String,
}

impl ExtensionError {
    pub(crate) fn new(path: &Path, message: impl Into<String>) -> Self {
        Self {
            path: path.to_string_lossy().into_owned(),
            message: message.into(),
        }
    }
}

/// Every user-authored extension after precedence, each list sorted by name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "config/")]
pub struct Extensions {
    pub agents: Vec<AgentDef>,
    pub commands: Vec<CommandDef>,
    pub skills: Vec<SkillDef>,
    pub rules: Vec<RuleDef>,
    pub output_styles: Vec<OutputStyleDef>,
    pub errors: Vec<ExtensionError>,
}
