//! The result of discovery and its limits.

use serde::{Deserialize, Serialize};

use crate::CheckSpec;

/// Detected project roots, the checks they suggest, and notes about
/// anything limited, skipped or unreadable. Nothing in it has been run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProfile {
    /// Ordered by path (the root first), then ecosystem.
    pub roots: Vec<ProjectRoot>,
    /// Ordered like `roots`.
    pub checks: Vec<CheckSpec>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRoot {
    /// Relative to the discovery root, `/`-separated; `.` for the root.
    pub path: String,
    /// `cargo`, `node`, `python`, `go`, `gradle`, `maven`, `dotnet`,
    /// `cmake`, `make`, `just` or `deno`.
    pub ecosystem: String,
    /// Root-relative path of the manifest that defines the root.
    pub manifest: String,
}

/// Bounds of one discovery walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscoveryOptions {
    /// Directory levels scanned below the root (0 scans only the root).
    pub max_depth: usize,
    /// Directory entries inspected before the walk stops.
    pub max_entries: usize,
    /// Larger manifests and ignore files are skipped with a note.
    pub max_manifest_bytes: u64,
}

impl Default for DiscoveryOptions {
    fn default() -> Self {
        Self {
            max_depth: 4,
            max_entries: 20_000,
            max_manifest_bytes: 512 * 1024,
        }
    }
}
