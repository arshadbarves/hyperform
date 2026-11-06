//! Typed transport errors.

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LlmError {
    #[error("provider misconfigured: {0}")]
    Config(String),
    #[error("provider returned HTTP {status}: {body}")]
    Http { status: u16, body: String },
    #[error("rate limited after {attempts} attempts: {detail}")]
    RateLimited { attempts: u32, detail: String },
    #[error("provider overloaded after {attempts} attempts: {detail}")]
    Overloaded { attempts: u32, detail: String },
    #[error("connection failed after {attempts} attempts: {cause}")]
    Connect { attempts: u32, cause: String },
    #[error("stream interrupted: {0}")]
    Stream(String),
    #[error("could not decode provider response: {0}")]
    Decode(String),
    #[error("request cancelled")]
    Cancelled,
}

const OVERFLOW_MARKERS: &[&str] = &[
    "context length",
    "context_length",
    "maximum context",
    "prompt is too long",
    "too many tokens",
    "context window",
];

impl LlmError {
    /// Worth retrying, possibly on a fallback model.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::RateLimited { .. }
            | Self::Overloaded { .. }
            | Self::Connect { .. }
            | Self::Stream(_) => true,
            Self::Http { status, .. } => *status == 408 || *status == 429 || *status >= 500,
            Self::Config(_) | Self::Decode(_) | Self::Cancelled => false,
        }
    }

    /// The prompt exceeded the model's context window.
    pub fn is_context_overflow(&self) -> bool {
        match self {
            Self::Http { status, body } if *status == 400 || *status == 413 => {
                let lower = body.to_ascii_lowercase();
                OVERFLOW_MARKERS.iter().any(|marker| lower.contains(marker))
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_retryable_and_overflow() {
        assert!(
            LlmError::Http {
                status: 529,
                body: String::new()
            }
            .is_retryable()
        );
        assert!(
            !LlmError::Http {
                status: 401,
                body: String::new()
            }
            .is_retryable()
        );
        let overflow = LlmError::Http {
            status: 400,
            body: "prompt is too long: 210000 tokens > 200000 maximum".into(),
        };
        assert!(overflow.is_context_overflow());
        assert!(!overflow.is_retryable());
    }
}
