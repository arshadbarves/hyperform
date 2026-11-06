//! Typed engine failures. Lower-layer errors keep their source so the GUI
//! backend can tell a storage failure from a provider or settings problem.

use hyperform_config::ConfigError;
use hyperform_host::HostError;
use hyperform_llm::LlmError;
use hyperform_policy::PolicyError;
use hyperform_protocol::SessionId;
use hyperform_store::StoreError;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Host(#[from] HostError),
    #[error(transparent)]
    Llm(#[from] LlmError),
    #[error(transparent)]
    Policy(#[from] PolicyError),
    /// No live session with this id; open it first.
    #[error("session {0} is not open")]
    NotOpen(SessionId),
    /// The session's actor has stopped (closed or shut down).
    #[error("session {0} is closed")]
    Closed(SessionId),
    #[error("invalid request: {0}")]
    Invalid(String),
    /// The operation needs an idle session.
    #[error("session is busy: {0}")]
    Busy(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("cannot encode {what}: {message}")]
    Encode { what: &'static str, message: String },
    #[error("the engine has shut down")]
    ShutDown,
}

impl EngineError {
    pub(crate) fn encode(what: &'static str, error: impl std::fmt::Display) -> Self {
        Self::Encode {
            what,
            message: error.to_string(),
        }
    }
}
