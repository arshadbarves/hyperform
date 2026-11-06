//! Typed failures for settings, credential, trust, and extension files.

use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Neither the override variable nor a platform default is available.
    #[error("cannot locate the {kind} directory; set {env_var}")]
    NoDirectory {
        kind: &'static str,
        env_var: &'static str,
    },
    #[error("cannot access {}: {source}", .path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot parse {}: {message}", .path.display())]
    Parse { path: PathBuf, message: String },
    #[error("{} is larger than {limit} bytes", .path.display())]
    TooLarge { path: PathBuf, limit: usize },
    /// A write was refused because the edited file would not load.
    #[error("refusing to write {}: {message}", .path.display())]
    Invalid { path: PathBuf, message: String },
    #[error("invalid settings key `{key}`: {reason}")]
    KeyPath { key: String, reason: String },
    #[error("cannot serialize {what}: {message}")]
    Serialize { what: &'static str, message: String },
}

impl ConfigError {
    pub(crate) fn io(path: &Path, source: std::io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            source,
        }
    }

    pub(crate) fn parse(path: &Path, message: impl std::fmt::Display) -> Self {
        Self::Parse {
            path: path.to_path_buf(),
            message: message.to_string(),
        }
    }

    pub(crate) fn key_path(key_path: &[&str], reason: impl Into<String>) -> Self {
        Self::KeyPath {
            key: key_path.join("."),
            reason: reason.into(),
        }
    }
}
