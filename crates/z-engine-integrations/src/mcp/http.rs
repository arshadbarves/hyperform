//! Streamable HTTP transport (MCP 2025-06-18). Every JSON-RPC message is a
//! POST; a request's reply is one JSON body or an SSE stream (which may
//! carry server requests and notifications first); notifications and
//! responses get `202 Accepted`. The server's `Mcp-Session-Id` is echoed on
//! every later request, and an optional GET stream carries messages the
//! server sends on its own (such as list changes).

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use reqwest::{Method, RequestBuilder, Response, StatusCode};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::client::PROTOCOL_VERSION;
use super::sse::sse_messages;
use crate::error::IntegrationError;
use crate::jsonrpc::{Inbound, Outbound, RpcMessage};
use crate::sync::lock;

const SESSION_HEADER: &str = "mcp-session-id";
const VERSION_HEADER: &str = "mcp-protocol-version";
const EVENT_STREAM: &str = "text/event-stream";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const LISTEN_OPEN_TIMEOUT: Duration = Duration::from_secs(5);
const DELETE_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_BODY_BYTES: usize = 64 * 1024 * 1024;
const ERROR_BODY_CHARS: usize = 2_000;

#[derive(Debug)]
pub(crate) struct HttpOutbound {
    client: reqwest::Client,
    url: String,
    headers: HeaderMap,
    session: Mutex<Option<String>>,
    version: Mutex<String>,
    closed: CancellationToken,
}

impl HttpOutbound {
    pub(crate) fn new(
        url: &str,
        headers: &BTreeMap<String, String>,
    ) -> Result<Self, IntegrationError> {
        // Configuration the client cannot use is reported as unsupported.
        let unusable = IntegrationError::Unsupported;
        let parsed = reqwest::Url::parse(url)
            .map_err(|e| unusable(format!("invalid MCP server URL `{url}`: {e}")))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(unusable(format!("MCP server URL `{url}` is not http(s)")));
        }
        let mut map = HeaderMap::new();
        for (name, value) in headers {
            let header = HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| unusable(format!("invalid HTTP header name `{name}`")))?;
            let value = HeaderValue::from_str(value)
                .map_err(|_| unusable(format!("invalid value for HTTP header `{name}`")))?;
            map.insert(header, value);
        }
        let client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .map_err(|e| {
                IntegrationError::io("building the HTTP client", std::io::Error::other(e))
            })?;
        Ok(Self {
            client,
            url: url.to_string(),
            headers: map,
            session: Mutex::new(None),
            version: Mutex::new(PROTOCOL_VERSION.to_string()),
            closed: CancellationToken::new(),
        })
    }

    /// The version negotiated by `initialize`, sent on later requests.
    pub(crate) fn set_protocol_version(&self, version: &str) {
        *lock(&self.version) = version.to_string();
    }

    pub(crate) fn session_id(&self) -> Option<String> {
        lock(&self.session).clone()
    }

    fn request(&self, method: Method) -> RequestBuilder {
        let version = lock(&self.version).clone();
        let mut builder = self
            .client
            .request(method, &self.url)
            .headers(self.headers.clone())
            .header(VERSION_HEADER, version);
        if let Some(session) = self.session_id() {
            builder = builder.header(SESSION_HEADER, session);
        }
        builder
    }

    /// Opens the optional listening stream; `None` when the server offers
    /// none (405) or does not answer promptly.
    pub(crate) async fn open_listener(&self) -> Option<Inbound> {
        let send = self
            .request(Method::GET)
            .header(ACCEPT, EVENT_STREAM)
            .send();
        let response = match tokio::time::timeout(LISTEN_OPEN_TIMEOUT, send).await {
            Ok(Ok(response)) => response,
            Ok(Err(e)) => {
                tracing::debug!(url = %self.url, error = %error_chain(&e), "no MCP listening stream");
                return None;
            }
            Err(_) => {
                tracing::debug!(url = %self.url, "the MCP listening stream did not open in time");
                return None;
            }
        };
        if response.status().is_success() && content_type(&response).starts_with(EVENT_STREAM) {
            Some(event_stream(response))
        } else {
            tracing::debug!(url = %self.url, status = %response.status(), "no MCP listening stream");
            None
        }
    }

    /// Ends the server-side session (best effort).
    pub(crate) async fn terminate_session(&self) {
        if self.session_id().is_none() {
            return;
        }
        match tokio::time::timeout(DELETE_TIMEOUT, self.request(Method::DELETE).send()).await {
            Ok(Ok(response)) => tracing::debug!(status = %response.status(), "MCP session ended"),
            Ok(Err(e)) => {
                tracing::debug!(error = %error_chain(&e), "could not end the MCP session")
            }
            Err(_) => tracing::debug!("ending the MCP session timed out"),
        }
    }
}

#[async_trait]
impl Outbound for HttpOutbound {
    async fn send(&self, message: Value) -> Result<Option<Inbound>, IntegrationError> {
        if self.closed.is_cancelled() {
            return Err(IntegrationError::NotSent("the connection is closed".into()));
        }
        let expects_reply =
            message.get("method").is_some() && message.get("id").is_some_and(|id| !id.is_null());
        let body = serde_json::to_vec(&message)
            .map_err(|e| IntegrationError::Protocol(format!("could not encode a message: {e}")))?;
        let had_session = self.session_id().is_some();
        let response = self
            .request(Method::POST)
            .header(ACCEPT, format!("application/json, {EVENT_STREAM}"))
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| send_error(&self.url, &e))?;
        if let Some(session) = response
            .headers()
            .get(SESSION_HEADER)
            .and_then(|v| v.to_str().ok())
        {
            *lock(&self.session) = Some(session.to_string());
        }
        let status = response.status();
        if status == StatusCode::NOT_FOUND && had_session {
            *lock(&self.session) = None;
            return Err(IntegrationError::NotSent("the MCP session expired".into()));
        }
        if !status.is_success() {
            return Err(http_error(response).await);
        }
        if !expects_reply || status == StatusCode::ACCEPTED {
            return Ok(None);
        }
        let kind = content_type(&response);
        if kind.starts_with(EVENT_STREAM) {
            Ok(Some(event_stream(response)))
        } else if kind.starts_with("application/json") {
            Ok(Some(json_body(response)))
        } else {
            Err(IntegrationError::Protocol(format!(
                "unexpected response content type `{kind}`"
            )))
        }
    }

    async fn close(&self) {
        self.closed.cancel();
    }
}

fn content_type(response: &Response) -> String {
    response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn send_error(url: &str, error: &reqwest::Error) -> IntegrationError {
    let detail = error_chain(error);
    if error.is_connect() {
        IntegrationError::NotSent(format!("could not connect to {url}: {detail}"))
    } else {
        IntegrationError::Disconnected(format!("the request to {url} failed: {detail}"))
    }
}

/// An error with its sources, which carry the useful detail for reqwest.
fn error_chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

async fn http_error(response: Response) -> IntegrationError {
    let status = response.status().as_u16();
    let body = match response.text().await {
        Ok(body) => body.chars().take(ERROR_BODY_CHARS).collect(),
        Err(e) => format!("(unreadable body: {e})"),
    };
    IntegrationError::Http { status, body }
}

fn json_body(response: Response) -> Inbound {
    futures::stream::once(async move {
        match read_capped(response).await {
            Ok(bytes) => RpcMessage::parse_frame(&bytes)
                .into_iter()
                .map(Ok)
                .collect(),
            Err(e) => vec![Err(e)],
        }
    })
    .flat_map(futures::stream::iter)
    .boxed()
}

async fn read_capped(response: Response) -> Result<Vec<u8>, IntegrationError> {
    let mut body = Vec::new();
    let mut chunks = response.bytes_stream();
    while let Some(chunk) = chunks.next().await {
        let chunk = chunk.map_err(|e| {
            IntegrationError::Disconnected(format!(
                "reading the response failed: {}",
                error_chain(&e)
            ))
        })?;
        if body.len() + chunk.len() > MAX_BODY_BYTES {
            return Err(IntegrationError::Protocol(format!(
                "the response body exceeds {MAX_BODY_BYTES} bytes"
            )));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// The JSON-RPC messages of an SSE response body.
fn event_stream(response: Response) -> Inbound {
    sse_messages(response.bytes_stream().map(|chunk| {
        chunk.map_err(|e| {
            IntegrationError::Disconnected(format!("the event stream failed: {}", error_chain(&e)))
        })
    }))
}
