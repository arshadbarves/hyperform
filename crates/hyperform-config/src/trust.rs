//! Workspace trust: project roots the user allowed to supply project-level
//! settings, hooks, MCP servers, and extensions. `trust.json` stores
//! canonical paths; trust is per root, not inherited by subdirectories.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::ConfigError;
use crate::files::{read_data_file, write_atomic};

const TRUST_FILE_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrustStore {
    roots: BTreeSet<String>,
}

#[derive(Serialize, Deserialize)]
struct TrustFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    trusted: Vec<String>,
}

/// What `load` accepts: the versioned file, or a bare list of roots.
#[derive(Deserialize)]
#[serde(untagged)]
enum Stored {
    File(TrustFile),
    Roots(Vec<String>),
}

impl TrustStore {
    /// A missing file trusts nothing; a malformed one is an error so it is
    /// never overwritten.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let Some(text) = read_data_file(path)? else {
            return Ok(Self::default());
        };
        if text.trim().is_empty() {
            return Ok(Self::default());
        }
        let stored: Stored =
            serde_json::from_str(&text).map_err(|error| ConfigError::parse(path, error))?;
        let roots = match stored {
            Stored::File(file) => file.trusted,
            Stored::Roots(roots) => roots,
        };
        Ok(Self {
            roots: roots.into_iter().collect(),
        })
    }

    pub fn is_trusted(&self, root: &Path) -> bool {
        self.roots.contains(&canonical(root))
    }

    /// Returns whether the root was newly trusted.
    pub fn trust(&mut self, root: &Path) -> bool {
        self.roots.insert(canonical(root))
    }

    /// Returns whether the root was trusted. Accepts the stored form too,
    /// so a deleted directory can still be revoked.
    pub fn revoke(&mut self, root: &Path) -> bool {
        let stored = self.roots.remove(&root.to_string_lossy().into_owned());
        self.roots.remove(&canonical(root)) || stored
    }

    /// Trusted roots, sorted.
    pub fn roots(&self) -> impl Iterator<Item = &str> {
        self.roots.iter().map(String::as_str)
    }

    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let file = TrustFile {
            version: TRUST_FILE_VERSION,
            trusted: self.roots.iter().cloned().collect(),
        };
        let text = serde_json::to_string_pretty(&file).map_err(|error| ConfigError::Serialize {
            what: "trust store",
            message: error.to_string(),
        })?;
        write_atomic(path, format!("{text}\n").as_bytes(), false)
    }
}

/// Symlinks resolved when the directory exists, else the absolute path.
fn canonical(root: &Path) -> String {
    std::fs::canonicalize(root)
        .or_else(|_| std::path::absolute(root))
        .unwrap_or_else(|_| PathBuf::from(root))
        .to_string_lossy()
        .into_owned()
}
