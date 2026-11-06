//! Typed failures of the decision layer. Callers never see them as errors
//! of a turn: the hybrid provider turns each into an abstained answer.

#[derive(Debug, thiserror::Error)]
pub enum DecisionError {
    /// A question or template cannot be asked as written.
    #[error("invalid question: {0}")]
    InvalidQuestion(String),
    #[error("invalid decision endpoint `{endpoint}`: {reason}")]
    InvalidEndpoint { endpoint: String, reason: String },
    #[error(
        "the decision endpoint {0} is not on this machine; set decisions.allow_remote to use it"
    )]
    RemoteNotAllowed(String),
    #[error("the decision server did not answer within {0} ms")]
    Timeout(u64),
    #[error("the decision server is unreachable: {0}")]
    Unavailable(String),
    #[error("the decision server answered HTTP {status}: {body}")]
    Status { status: u16, body: String },
    #[error("the decision server's reply could not be read: {0}")]
    Malformed(String),
    #[error("the decision was cancelled")]
    Cancelled,
}
