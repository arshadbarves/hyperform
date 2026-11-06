//! Why a tool call did not produce a result. The engine renders every error
//! as an `is_error` tool result, so messages are written for the model.

use z_engine_host::HostError;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ToolError {
    /// The input is malformed or names something that cannot be used.
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// The call was valid but did not succeed (missing file, stale read,
    /// ambiguous edit, failed port call, ...).
    #[error("{0}")]
    Failed(String),
    /// The call's cancellation token fired before it finished.
    #[error("cancelled by user")]
    Cancelled,
    /// The capability is not available to this agent (missing port, wrong
    /// permission mode, subagent restrictions).
    #[error("{0}")]
    Unavailable(String),
}

impl ToolError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::InvalidInput(message.into())
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed(message.into())
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::Unavailable(message.into())
    }

    /// A host failure of a valid call: the host's message becomes `Failed`
    /// (cancellation stays `Cancelled`).
    pub(crate) fn host_failed(error: HostError) -> Self {
        match error {
            HostError::Cancelled => Self::Cancelled,
            HostError::Invalid(message) | HostError::NotFound(message) => Self::Failed(message),
            other => Self::Failed(other.to_string()),
        }
    }
}

/// Host failures keep their message; cancellation and invalid input keep
/// their class.
impl From<HostError> for ToolError {
    fn from(error: HostError) -> Self {
        match error {
            HostError::Cancelled => Self::Cancelled,
            HostError::Invalid(message) => Self::InvalidInput(message),
            other => Self::Failed(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_errors_keep_their_class() {
        assert_eq!(ToolError::from(HostError::Cancelled), ToolError::Cancelled);
        assert_eq!(
            ToolError::from(HostError::Invalid("bad glob".into())),
            ToolError::InvalidInput("bad glob".into())
        );
        assert_eq!(
            ToolError::from(HostError::Timeout).to_string(),
            "operation timed out"
        );
        assert_eq!(
            ToolError::invalid("offset is 0").to_string(),
            "invalid input: offset is 0"
        );
    }
}
