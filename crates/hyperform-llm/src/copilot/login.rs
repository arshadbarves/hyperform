//! GitHub's OAuth device flow, one stateless request per call: the caller
//! shows the user code, then polls at the interval GitHub asks for.

use std::fmt;
use std::time::Duration;

use reqwest::StatusCode;
use reqwest::header::{ACCEPT, CONTENT_TYPE, USER_AGENT};
use serde::Deserialize;
use serde_json::{Value, json};

use super::identity::CLIENT_ID;
use crate::error::LlmError;
use crate::opencode;
use crate::retry::{ERROR_BODY_LIMIT, describe};

const SCOPE: &str = "read:user";
const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
/// RFC 8628's default polling interval and its `slow_down` increment.
const DEFAULT_INTERVAL: u64 = 5;
const SLOW_DOWN_STEP: u64 = 5;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// A started sign-in: show `user_code`, open `verification_uri`, then poll
/// with `device_code` every `interval` seconds for up to `expires_in`.
#[derive(Clone, PartialEq, Eq)]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub interval: u64,
    pub expires_in: u64,
}

impl fmt::Debug for DeviceCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceCode")
            .field("user_code", &self.user_code)
            .field("verification_uri", &self.verification_uri)
            .field("device_code", &"<redacted>")
            .field("interval", &self.interval)
            .field("expires_in", &self.expires_in)
            .finish()
    }
}

/// The outcome of one poll.
#[derive(Clone, PartialEq, Eq)]
pub enum LoginPoll {
    /// Not approved yet: poll again after the interval.
    Pending,
    /// Polling too fast: wait `interval` seconds between polls from now on.
    SlowDown { interval: u64 },
    /// Signed in: the GitHub token, used as the Copilot API key.
    Done(String),
    /// The sign-in ended (expired, denied, ...); start over.
    Failed(String),
}

impl fmt::Debug for LoginPoll {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => f.write_str("Pending"),
            Self::SlowDown { interval } => f
                .debug_struct("SlowDown")
                .field("interval", interval)
                .finish(),
            Self::Done(_) => f.debug_tuple("Done").field(&"<redacted>").finish(),
            Self::Failed(reason) => f.debug_tuple("Failed").field(reason).finish(),
        }
    }
}

/// `POST {login_url}/login/device/code`. `login_url` is the GitHub web
/// origin (see [`super::copilot_login_url`]).
pub async fn start_login(http: &reqwest::Client, login_url: &str) -> Result<DeviceCode, LlmError> {
    let body = json!({"client_id": CLIENT_ID, "scope": SCOPE});
    let (status, text) = post(http, login_url, "/login/device/code", &body).await?;
    device_code(status, &text)
}

/// `POST {login_url}/login/oauth/access_token`, once.
pub async fn poll_login(
    http: &reqwest::Client,
    login_url: &str,
    device_code: &str,
) -> Result<LoginPoll, LlmError> {
    let body = json!({
        "client_id": CLIENT_ID,
        "device_code": device_code,
        "grant_type": DEVICE_GRANT,
    });
    let (status, text) = post(http, login_url, "/login/oauth/access_token", &body).await?;
    poll_outcome(status, &text)
}

#[derive(Deserialize)]
struct DeviceReply {
    device_code: String,
    user_code: String,
    verification_uri: String,
    interval: Option<u64>,
    expires_in: Option<u64>,
}

#[derive(Default, Deserialize)]
struct TokenReply {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
    interval: Option<u64>,
}

async fn post(
    http: &reqwest::Client,
    login_url: &str,
    path: &str,
    body: &Value,
) -> Result<(StatusCode, String), LlmError> {
    let url = format!("{}{path}", login_url.trim().trim_end_matches('/'));
    let response = http
        .post(url)
        .header(ACCEPT, "application/json")
        .header(CONTENT_TYPE, "application/json")
        .header(USER_AGENT, opencode::USER_AGENT)
        .timeout(REQUEST_TIMEOUT)
        .body(body.to_string())
        .send()
        .await
        .map_err(|error| {
            if error.is_builder() {
                LlmError::Config(format!("invalid GitHub sign-in URL: {error}"))
            } else {
                LlmError::Connect {
                    attempts: 1,
                    cause: describe(&error),
                }
            }
        })?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| LlmError::Stream(describe(&error)))?;
    Ok((status, text))
}

fn device_code(status: StatusCode, text: &str) -> Result<DeviceCode, LlmError> {
    let error = || LlmError::Http {
        status: status.as_u16(),
        body: reason(text).unwrap_or_else(|| clip(text)),
    };
    if !status.is_success() {
        return Err(error());
    }
    let reply: DeviceReply = serde_json::from_str(text).map_err(|decode| {
        if reason(text).is_some() {
            error()
        } else {
            LlmError::Decode(format!("GitHub device code: {decode}"))
        }
    })?;
    Ok(DeviceCode {
        user_code: reply.user_code,
        verification_uri: reply.verification_uri,
        device_code: reply.device_code,
        interval: reply.interval.unwrap_or(DEFAULT_INTERVAL),
        expires_in: reply.expires_in.unwrap_or_default(),
    })
}

fn poll_outcome(status: StatusCode, text: &str) -> Result<LoginPoll, LlmError> {
    if !status.is_success() {
        let detail = reason(text).unwrap_or_else(|| format!("GitHub answered HTTP {status}"));
        return Ok(LoginPoll::Failed(detail));
    }
    let reply: TokenReply = serde_json::from_str(text)
        .map_err(|error| LlmError::Decode(format!("GitHub access token: {error}")))?;
    if let Some(token) = reply.access_token.filter(|token| !token.is_empty()) {
        return Ok(LoginPoll::Done(token));
    }
    Ok(match reply.error.as_deref() {
        None | Some("authorization_pending") => LoginPoll::Pending,
        Some("slow_down") => LoginPoll::SlowDown {
            interval: reply
                .interval
                .filter(|interval| *interval > 0)
                .unwrap_or(DEFAULT_INTERVAL + SLOW_DOWN_STEP),
        },
        Some(_) => LoginPoll::Failed(reason(text).unwrap_or_default()),
    })
}

/// `error: description` from an OAuth error body, if it is one.
fn reason(text: &str) -> Option<String> {
    let reply: TokenReply = serde_json::from_str(text).unwrap_or_default();
    match (reply.error, reply.error_description) {
        (Some(error), Some(description)) => Some(format!("{error}: {description}")),
        (Some(error), None) => Some(error),
        (None, description) => description,
    }
}

fn clip(text: &str) -> String {
    text.chars().take(ERROR_BODY_LIMIT).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OK: StatusCode = StatusCode::OK;

    #[test]
    fn debug_output_hides_codes_and_tokens() {
        let code = DeviceCode {
            user_code: "ABCD-1234".into(),
            verification_uri: "https://github.com/login/device".into(),
            device_code: "dc-secret".into(),
            interval: 5,
            expires_in: 900,
        };
        let rendered = format!("{code:?}");
        assert!(rendered.contains("ABCD-1234") && !rendered.contains("dc-secret"));
        let done = format!("{:?}", LoginPoll::Done("gho_secret".into()));
        assert!(!done.contains("gho_secret"), "{done}");
    }

    #[test]
    fn device_codes_default_the_interval_and_surface_errors() {
        let body = r#"{"device_code":"d","user_code":"U","verification_uri":"https://github.com/login/device"}"#;
        let code = device_code(OK, body).unwrap();
        assert_eq!((code.interval, code.expires_in), (DEFAULT_INTERVAL, 0));
        let refused = r#"{"error":"unauthorized_client","error_description":"Device flow is off"}"#;
        assert_eq!(
            device_code(OK, refused),
            Err(LlmError::Http {
                status: 200,
                body: "unauthorized_client: Device flow is off".into()
            })
        );
        assert!(matches!(
            device_code(OK, "<html>"),
            Err(LlmError::Decode(_))
        ));
        assert!(matches!(
            device_code(StatusCode::NOT_FOUND, "Not Found"),
            Err(LlmError::Http { status: 404, body }) if body == "Not Found"
        ));
    }

    #[test]
    fn poll_replies_map_to_outcomes() {
        let poll = |text: &str| poll_outcome(OK, text).unwrap();
        assert_eq!(
            poll(r#"{"error":"authorization_pending"}"#),
            LoginPoll::Pending
        );
        assert_eq!(poll("{}"), LoginPoll::Pending);
        assert_eq!(
            poll(r#"{"error":"slow_down","interval":15}"#),
            LoginPoll::SlowDown { interval: 15 }
        );
        assert_eq!(
            poll(r#"{"error":"slow_down"}"#),
            LoginPoll::SlowDown { interval: 10 }
        );
        assert_eq!(
            poll(r#"{"access_token":"gho_x","token_type":"bearer"}"#),
            LoginPoll::Done("gho_x".into())
        );
        assert_eq!(
            poll(r#"{"error":"access_denied"}"#),
            LoginPoll::Failed("access_denied".into())
        );
        assert_eq!(
            poll_outcome(StatusCode::BAD_GATEWAY, "").unwrap(),
            LoginPoll::Failed("GitHub answered HTTP 502 Bad Gateway".into())
        );
        assert!(matches!(poll_outcome(OK, "nope"), Err(LlmError::Decode(_))));
    }
}
