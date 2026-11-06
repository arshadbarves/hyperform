//! Shared HTTP send loop: classified retries with cancellable backoff.
//!
//! Only failures before a response stream starts are retried here; a stream
//! that breaks later surfaces as `LlmError::Stream` for the caller to judge.

use std::future::Future;
use std::time::Duration;

use reqwest::header::HeaderMap;
use reqwest::{RequestBuilder, Response, StatusCode};
use tokio_util::sync::CancellationToken;

use crate::error::LlmError;
use crate::types::ModelEvent;

/// Characters of an error response body kept in error details.
pub(crate) const ERROR_BODY_LIMIT: usize = 2_000;

/// Attempt budget and delays for [`send_with_retry`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RetryPolicy {
    pub max_attempts: u32,
    pub base_backoff: Duration,
    pub max_backoff: Duration,
    /// Upper bound for a server-requested `retry-after` delay.
    pub max_server_delay: Duration,
}

impl RetryPolicy {
    pub(crate) const DEFAULT: Self = Self {
        max_attempts: 5,
        base_backoff: Duration::from_millis(500),
        max_backoff: Duration::from_secs(16),
        max_server_delay: Duration::from_secs(60),
    };

    /// Exponential delay before retry number `retry` (1-based).
    fn backoff(&self, retry: u32) -> Duration {
        let factor = 2u32.saturating_pow(retry.saturating_sub(1));
        self.base_backoff
            .saturating_mul(factor)
            .min(self.max_backoff)
    }

    /// `retry-after-ms`, else `retry-after` in seconds, capped.
    fn server_delay(&self, headers: &HeaderMap) -> Option<Duration> {
        let number = |name: &str| {
            headers
                .get(name)?
                .to_str()
                .ok()?
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite() && *value >= 0.0)
        };
        let seconds = number("retry-after-ms")
            .map(|millis| millis / 1_000.0)
            .or_else(|| number("retry-after"))?;
        let capped = seconds.min(self.max_server_delay.as_secs_f64());
        Some(Duration::from_secs_f64(capped))
    }
}

/// A retry about to be attempted, reported before its backoff sleep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RetryNotice {
    /// 1 for the first retry.
    pub attempt: u32,
    pub delay: Duration,
    pub reason: String,
}

impl From<RetryNotice> for ModelEvent {
    fn from(notice: RetryNotice) -> Self {
        ModelEvent::Retrying {
            attempt: notice.attempt,
            delay_ms: u64::try_from(notice.delay.as_millis()).unwrap_or(u64::MAX),
            reason: notice.reason,
        }
    }
}

/// Send the request produced by `build`, retrying transient failures.
///
/// Retries connect/timeout errors and HTTP 408, 429, 500, 502, 503, 504 and
/// 529 within `policy`, sleeping for the server's `retry-after(-ms)` delay or
/// exponential backoff. `on_retry` runs before each sleep. Cancelling
/// `cancel` aborts an in-flight send or sleep with `LlmError::Cancelled`.
pub(crate) async fn send_with_retry<B, F, Fut>(
    build: &B,
    policy: &RetryPolicy,
    cancel: &CancellationToken,
    mut on_retry: F,
) -> Result<Response, LlmError>
where
    B: Fn() -> RequestBuilder,
    F: FnMut(RetryNotice) -> Fut,
    Fut: Future<Output = ()>,
{
    let mut attempt = 1;
    loop {
        let sent = tokio::select! {
            () = cancel.cancelled() => return Err(LlmError::Cancelled),
            sent = build().send() => sent,
        };
        let (delay, reason) = match sent {
            Ok(response) if response.status().is_success() => return Ok(response),
            Ok(response) => {
                let status = response.status();
                let server_delay = policy.server_delay(response.headers());
                let body = tokio::select! {
                    () = cancel.cancelled() => return Err(LlmError::Cancelled),
                    body = error_body(response) => body,
                };
                if !is_retryable_status(status) || attempt >= policy.max_attempts {
                    return Err(status_error(status, body, attempt));
                }
                let delay = server_delay.unwrap_or_else(|| policy.backoff(attempt));
                (delay, retry_reason(status))
            }
            Err(error) if error.is_builder() => {
                return Err(LlmError::Config(format!("invalid request: {error}")));
            }
            Err(error) => {
                let cause = describe(&error);
                if !(error.is_connect() || error.is_timeout()) || attempt >= policy.max_attempts {
                    return Err(LlmError::Connect {
                        attempts: attempt,
                        cause,
                    });
                }
                (
                    policy.backoff(attempt),
                    format!("connection failed: {cause}"),
                )
            }
        };
        tracing::info!(attempt, ?delay, %reason, "retrying model request");
        on_retry(RetryNotice {
            attempt,
            delay,
            reason,
        })
        .await;
        tokio::select! {
            () = cancel.cancelled() => return Err(LlmError::Cancelled),
            () = tokio::time::sleep(delay) => {}
        }
        attempt += 1;
    }
}

/// A reqwest error with its source chain, for diagnostics.
pub(crate) fn describe(error: &reqwest::Error) -> String {
    let mut message = error.to_string();
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}

/// The first [`ERROR_BODY_LIMIT`] characters of a response body.
pub(crate) async fn error_body(response: Response) -> String {
    match response.text().await {
        Ok(text) => text.chars().take(ERROR_BODY_LIMIT).collect(),
        Err(error) => format!("(unreadable response body: {})", describe(&error)),
    }
}

fn is_retryable_status(status: StatusCode) -> bool {
    matches!(status.as_u16(), 408 | 429 | 500 | 502 | 503 | 504 | 529)
}

fn retry_reason(status: StatusCode) -> String {
    let code = status.as_u16();
    match code {
        429 => format!("rate limited (HTTP {code})"),
        503 | 529 => format!("provider overloaded (HTTP {code})"),
        408 => format!("request timed out (HTTP {code})"),
        _ => format!("server error (HTTP {code})"),
    }
}

fn status_error(status: StatusCode, body: String, attempts: u32) -> LlmError {
    let detail = if body.trim().is_empty() {
        status.to_string()
    } else {
        body
    };
    match status.as_u16() {
        429 => LlmError::RateLimited { attempts, detail },
        503 | 529 => LlmError::Overloaded { attempts, detail },
        code => LlmError::Http {
            status: code,
            body: detail,
        },
    }
}

#[cfg(test)]
mod tests;
