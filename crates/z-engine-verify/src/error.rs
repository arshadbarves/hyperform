//! Typed verification failures. A failing check is a record, not an error:
//! an error means the check could not run or its evidence could not be
//! kept.

use std::path::PathBuf;

use z_engine_host::HostError;

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("check could not run: {0}")]
    Host(#[source] HostError),
    #[error("verification I/O failed at {}: {source}", .path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("check cancelled")]
    Cancelled,
    #[error("invalid check: {0}")]
    Invalid(String),
}

impl VerifyError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

/// A host cancellation is the check's cancellation, not a host failure.
impl From<HostError> for VerifyError {
    fn from(error: HostError) -> Self {
        match error {
            HostError::Cancelled => Self::Cancelled,
            other => Self::Host(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::*;

    #[test]
    fn host_cancellation_maps_to_cancelled() {
        assert!(matches!(
            VerifyError::from(HostError::Cancelled),
            VerifyError::Cancelled
        ));
        let error = VerifyError::from(HostError::Process("spawn failed".into()));
        assert_eq!(
            error.to_string(),
            "check could not run: process failed: spawn failed"
        );
        assert!(error.source().is_some());
    }

    #[test]
    fn io_errors_name_the_path() {
        let error = VerifyError::io(
            "/tmp/artifacts/x.log",
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        );
        assert_eq!(
            error.to_string(),
            "verification I/O failed at /tmp/artifacts/x.log: denied"
        );
    }
}
