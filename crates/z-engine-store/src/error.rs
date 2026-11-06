//! Typed store failures. I/O and JSON errors name the file involved so a
//! persistence failure can be shown and located.

use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("session store I/O failed at {}: {source}", .path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// `line` is 1-based, or 0 when the failure is not tied to one line.
    #[error("invalid JSON in {} at line {line}: {message}", .path.display())]
    Json {
        path: PathBuf,
        line: usize,
        message: String,
    },
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid session data: {0}")]
    Invalid(String),
}

impl StoreError {
    pub(crate) fn io(path: &Path, source: std::io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            source,
        }
    }

    /// Opening or inspecting a file: missing is `NotFound`, anything else `Io`.
    pub(crate) fn open(path: &Path, source: std::io::Error) -> Self {
        if source.kind() == std::io::ErrorKind::NotFound {
            Self::NotFound(path.display().to_string())
        } else {
            Self::io(path, source)
        }
    }

    pub(crate) fn json(path: &Path, line: usize, error: &serde_json::Error) -> Self {
        Self::Json {
            path: path.to_path_buf(),
            line,
            message: error.to_string(),
        }
    }
}
