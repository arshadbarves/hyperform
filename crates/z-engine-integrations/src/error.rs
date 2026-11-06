//! Typed failures of the JSON-RPC core and the MCP and LSP clients. The
//! variants keep apart what callers must treat differently: a request that
//! never reached the server (safe to retry) from a connection lost with the
//! request in flight (outcome unknown), a timeout from a cancellation, and a
//! server's JSON-RPC error from a malformed reply.

use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum IntegrationError {
    /// The server process could not be started (missing binary, bad cwd).
    #[error("could not start `{command}`: {source}")]
    Spawn {
        command: String,
        #[source]
        source: std::io::Error,
    },
    /// Local I/O failed: reading a source file or a server pipe.
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    /// The peer broke the protocol: malformed JSON, framing, or result shape.
    #[error("protocol error: {0}")]
    Protocol(String),
    /// The server answered with a JSON-RPC error object.
    #[error("server error {code}: {message}")]
    Rpc { code: i64, message: String },
    /// No response within the deadline. The server may still act on the
    /// request; a cancellation notice was sent.
    #[error("`{method}` timed out after {}ms", .after.as_millis())]
    Timeout { method: String, after: Duration },
    #[error("request cancelled")]
    Cancelled,
    /// The connection closed while the request was in flight: the server may
    /// or may not have acted on it.
    #[error("server disconnected: {0}")]
    Disconnected(String),
    /// The connection was closed (or the session expired) before the request
    /// was delivered, so the server never saw it and a retry cannot repeat
    /// an effect.
    #[error("request not sent: {0}")]
    NotSent(String),
    /// A streamable HTTP server rejected the request.
    #[error("HTTP {status}: {body}")]
    Http { status: u16, body: String },
    /// The server or client lacks the capability (or it is disabled).
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("not found: {0}")]
    NotFound(String),
}

impl IntegrationError {
    pub(crate) fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }

    /// The request never reached the server, so repeating it on a fresh
    /// connection cannot duplicate a side effect.
    pub fn is_retry_safe(&self) -> bool {
        matches!(self, Self::NotSent(_))
    }

    /// The connection is unusable: closed before sending or lost mid-request.
    pub fn is_transport_failure(&self) -> bool {
        matches!(self, Self::NotSent(_) | Self::Disconnected(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_and_classification() {
        let timeout = IntegrationError::Timeout {
            method: "tools/call".into(),
            after: Duration::from_millis(1500),
        };
        assert_eq!(timeout.to_string(), "`tools/call` timed out after 1500ms");
        assert!(!timeout.is_retry_safe() && !timeout.is_transport_failure());

        let not_sent = IntegrationError::NotSent("server exited".into());
        assert_eq!(not_sent.to_string(), "request not sent: server exited");
        assert!(not_sent.is_retry_safe() && not_sent.is_transport_failure());

        let lost = IntegrationError::Disconnected("eof".into());
        assert!(!lost.is_retry_safe() && lost.is_transport_failure());

        let spawn = IntegrationError::Spawn {
            command: "gopls".into(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "not found on PATH"),
        };
        assert_eq!(
            spawn.to_string(),
            "could not start `gopls`: not found on PATH"
        );
    }
}
