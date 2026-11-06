//! The SystemOne provider: one `POST /v1/systemone` per batch of questions
//! to laya-serve or Jev, bounded by the configured timeout.

use std::fmt;
use std::time::Duration;

use async_trait::async_trait;
use futures::future::try_join_all;
use tokio_util::sync::CancellationToken;

use super::endpoint::{self, Endpoint};
use super::wire::{self, Controls, WireResponse};
use crate::answer::Answer;
use crate::error::DecisionError;
use crate::provider::DecisionProvider;
use crate::question::{DecisionRequest, Question};

const NAME: &str = "systemone";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_ERROR_BODY: usize = 300;

#[derive(Clone, PartialEq)]
pub struct SystemOneConfig {
    /// Server root, e.g. `http://127.0.0.1:8000`.
    pub endpoint: String,
    /// Sent as `Authorization: Bearer <key>`.
    pub api_key: Option<String>,
    /// Sent as the body's `model`; laya-serve answers with that checkpoint.
    /// Also part of the provider revision (cache key).
    pub checkpoint: String,
    /// Sent as the body's `max_len` (tokens per question); 0 leaves it to the server.
    pub max_len: u32,
    /// The whole call, all batches included.
    pub timeout: Duration,
    pub allow_remote: bool,
    /// Questions per request; more are split into parallel requests.
    pub max_batch: usize,
}

impl fmt::Debug for SystemOneConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SystemOneConfig")
            .field("endpoint", &self.endpoint)
            .field("api_key", &self.api_key.as_ref().map(|_| "<set>"))
            .field("checkpoint", &self.checkpoint)
            .field("max_len", &self.max_len)
            .field("timeout", &self.timeout)
            .field("allow_remote", &self.allow_remote)
            .field("max_batch", &self.max_batch)
            .finish()
    }
}

#[derive(Debug)]
pub struct SystemOneProvider {
    config: SystemOneConfig,
    endpoint: Endpoint,
    http: reqwest::Client,
}

impl SystemOneProvider {
    /// Fails on a malformed endpoint, or a remote one without `allow_remote`.
    pub fn new(config: SystemOneConfig) -> Result<Self, DecisionError> {
        let endpoint = endpoint::parse(&config.endpoint, config.allow_remote)?;
        let mut builder = reqwest::Client::builder().connect_timeout(CONNECT_TIMEOUT);
        if endpoint.loopback {
            builder = builder.no_proxy();
        }
        let http = builder
            .build()
            .map_err(|error| DecisionError::Unavailable(error.to_string()))?;
        Ok(Self {
            config,
            endpoint,
            http,
        })
    }

    /// The full request URL (`.../v1/systemone`).
    pub fn endpoint(&self) -> &str {
        self.endpoint.url.as_str()
    }

    async fn post(
        &self,
        request: &DecisionRequest,
        batch: &[&Question],
    ) -> Result<Vec<Answer>, DecisionError> {
        let controls = Controls {
            checkpoint: &self.config.checkpoint,
            max_len: self.config.max_len,
        };
        let body = wire::encode(&request.state, batch, controls);
        let mut call = self.http.post(self.endpoint.url.clone()).json(&body);
        if let Some(key) = &self.config.api_key {
            call = call.bearer_auth(key);
        }
        let response = call
            .send()
            .await
            .map_err(|error| DecisionError::Unavailable(without_url(&error)))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|error| DecisionError::Unavailable(without_url(&error)))?;
        if !status.is_success() {
            let body: String = text.chars().take(MAX_ERROR_BODY).collect();
            return Err(DecisionError::Status {
                status: status.as_u16(),
                body,
            });
        }
        let parsed: WireResponse = serde_json::from_str(&text)
            .map_err(|error| DecisionError::Malformed(error.to_string()))?;
        Ok(wire::decode(batch, &parsed, NAME))
    }
}

#[async_trait]
impl DecisionProvider for SystemOneProvider {
    fn name(&self) -> &'static str {
        NAME
    }

    fn revision(&self) -> String {
        let config = &self.config;
        format!(
            "{NAME}:{}:{}:{}",
            self.endpoint.url, config.checkpoint, config.max_len
        )
    }

    async fn decide(
        &self,
        request: &DecisionRequest,
        cancel: &CancellationToken,
    ) -> Result<Vec<Answer>, DecisionError> {
        request.validate()?;
        let questions: Vec<&Question> = request.questions.iter().collect();
        let batches = questions.chunks(self.config.max_batch.max(1));
        let calls = try_join_all(batches.map(|batch| self.post(request, batch)));
        let millis = u64::try_from(self.config.timeout.as_millis()).unwrap_or(u64::MAX);
        tokio::select! {
            () = cancel.cancelled() => Err(DecisionError::Cancelled),
            result = tokio::time::timeout(self.config.timeout, calls) => match result {
                Ok(answers) => Ok(answers?.into_iter().flatten().collect()),
                Err(_) => Err(DecisionError::Timeout(millis)),
            },
        }
    }
}

/// reqwest errors embed the URL; the endpoint is already known to the user.
fn without_url(error: &reqwest::Error) -> String {
    let kind = if error.is_connect() {
        "could not connect"
    } else if error.is_timeout() {
        "timed out"
    } else {
        "request failed"
    };
    let source = std::error::Error::source(error).map(ToString::to_string);
    match source {
        Some(source) => format!("{kind}: {source}"),
        None => kind.to_string(),
    }
}
