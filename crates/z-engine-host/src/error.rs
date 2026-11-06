//! Typed failures for every host operation. Each variant names the class of
//! failure so callers can tell a missing file from a denied network target,
//! a timeout from a cancellation, and a git failure from git being absent.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("{}: {}", .path.display(), .source)]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("process failed: {0}")]
    Process(String),
    #[error("operation timed out")]
    Timeout,
    #[error("operation cancelled")]
    Cancelled,
    #[error("git {} failed: {}", .args.join(" "), .stderr)]
    Git { args: Vec<String>, stderr: String },
    #[error("git is not installed or not on PATH")]
    GitUnavailable,
    #[error("http request failed: {0}")]
    Http(String),
    #[error("blocked: {0}")]
    Blocked(String),
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("not found: {0}")]
    NotFound(String),
    /// The command sandbox cannot run on this machine or with this shell.
    #[error("sandbox unavailable: {0}")]
    SandboxUnavailable(String),
}

impl HostError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    /// The target does not exist: a missing file, directory, or job id.
    pub fn is_not_found(&self) -> bool {
        match self {
            Self::NotFound(_) => true,
            Self::Io { source, .. } => source.kind() == std::io::ErrorKind::NotFound,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_carry_context() {
        let io = HostError::io(
            "/tmp/x",
            std::io::Error::new(std::io::ErrorKind::NotFound, "gone"),
        );
        assert_eq!(io.to_string(), "/tmp/x: gone");
        assert!(io.is_not_found());
        let git = HostError::Git {
            args: vec!["status".into(), "-z".into()],
            stderr: "fatal: not a repo".into(),
        };
        assert_eq!(git.to_string(), "git status -z failed: fatal: not a repo");
        assert!(!git.is_not_found());
    }
}
