//! The GitHub Copilot [`ModelClient`]: one GitHub token, three APIs. Each
//! model's API comes from Copilot's `/models` list, fetched once per client.

use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::Value;
use tokio::sync::{OnceCell, mpsc};
use tokio_util::sync::CancellationToken;

use super::body;
use super::headers::{request_headers, static_headers};
use super::identity::LABEL;
use super::models::{Resolved, Route, RouteTable};
use crate::anthropic::{ANTHROPIC_VERSION, MessagesStreamParser};
use crate::client::ModelClient;
use crate::error::LlmError;
use crate::openai::ChatStreamParser;
use crate::provider::{ProviderConfig, http_client};
use crate::responses::ResponsesStreamParser;
use crate::retry::{RetryPolicy, describe};
use crate::transport::{CHANNEL_CAPACITY, Endpoint, StreamItem};
use crate::types::{ModelRequest, ModelStream};

/// A slow model list delays only the first request, by at most this.
const MODELS_TIMEOUT: Duration = Duration::from_secs(5);
const SIGNED_OUT: &str = "sign in to GitHub Copilot first (Settings > Providers)";
const SIGN_IN_AGAIN: &str =
    "GitHub Copilot refused the token: sign in to GitHub Copilot again (Settings > Providers)";

/// The GitHub Copilot client for `config`, whatever its host; [`crate::build_client`]
/// picks it for Copilot hosts. The API key is the GitHub token from sign-in.
pub fn build_copilot_client(config: &ProviderConfig) -> Result<Arc<dyn ModelClient>, LlmError> {
    Ok(Arc::new(CopilotClient::new(config)?))
}

#[derive(Debug, Clone)]
pub(crate) struct CopilotClient {
    chat: Endpoint,
    responses: Endpoint,
    messages: Endpoint,
    models_url: String,
    routes: Arc<OnceCell<RouteTable>>,
    cache_control: bool,
}

impl CopilotClient {
    pub(crate) fn new(config: &ProviderConfig) -> Result<Self, LlmError> {
        let base_url = config.base_url.trim().trim_end_matches('/');
        if base_url.is_empty() {
            return Err(LlmError::Config("base_url is empty".into()));
        }
        let token = config
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .ok_or_else(|| LlmError::Config(SIGNED_OUT.into()))?;
        let http = http_client()?;
        let headers = static_headers(token, &config.extra_headers)?;
        let endpoint = |path: &str, headers: HeaderMap| Endpoint {
            http: http.clone(),
            url: format!("{base_url}{path}"),
            headers,
            label: LABEL,
            zen_session: None,
            policy: RetryPolicy::DEFAULT,
        };
        let mut messages_headers = headers.clone();
        messages_headers.insert(
            "anthropic-version",
            HeaderValue::from_static(ANTHROPIC_VERSION),
        );
        Ok(Self {
            chat: endpoint("/chat/completions", headers.clone()),
            responses: endpoint("/responses", headers.clone()),
            messages: endpoint("/v1/messages", messages_headers),
            models_url: format!("{base_url}/models"),
            routes: Arc::new(OnceCell::new()),
            cache_control: config.cache_control.unwrap_or(true),
        })
    }

    async fn forward(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
        tx: &mpsc::Sender<StreamItem>,
    ) {
        let routes = self.routes.get_or_init(|| self.fetch_routes()).await;
        let mut inner = match routes.resolve(&request.model) {
            Ok(resolved) => self.open(request, &resolved, cancel),
            Err(error) => {
                if tx.send(Err(error)).await.is_err() {
                    tracing::debug!("route error dropped: consumer gone");
                }
                return;
            }
        };
        while let Some(item) = inner.recv().await {
            if tx.send(item.map_err(sign_in_hint)).await.is_err() {
                return;
            }
        }
    }

    fn open(
        &self,
        request: ModelRequest,
        resolved: &Resolved,
        cancel: CancellationToken,
    ) -> ModelStream {
        let request = body::prepare(request, resolved);
        let body = body::build(&request, resolved, self.cache_control);
        let headers = request_headers(&request, resolved.route);
        let session = request.session_key.as_deref();
        match resolved.route {
            Route::Chat => {
                self.chat
                    .stream(&body, headers, session, ChatStreamParser::default(), cancel)
            }
            Route::Responses => self.responses.stream(
                &body,
                headers,
                session,
                ResponsesStreamParser::default(),
                cancel,
            ),
            Route::Messages => self.messages.stream(
                &body,
                headers,
                session,
                MessagesStreamParser::default(),
                cancel,
            ),
        }
    }

    async fn fetch_routes(&self) -> RouteTable {
        let sent = self
            .chat
            .http
            .get(&self.models_url)
            .headers(self.chat.headers.clone())
            .timeout(MODELS_TIMEOUT)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status);
        let body = match sent {
            Ok(response) => response.text().await,
            Err(error) => Err(error),
        };
        match body.map(|text| serde_json::from_str::<Value>(&text)) {
            Ok(Ok(models)) => RouteTable::parse(&models),
            Ok(Err(error)) => {
                tracing::warn!(%error, "GitHub Copilot model list unreadable; routing by model name");
                RouteTable::default()
            }
            Err(error) => {
                tracing::warn!(error = %describe(&error), "GitHub Copilot model list unavailable; routing by model name");
                RouteTable::default()
            }
        }
    }
}

impl ModelClient for CopilotClient {
    fn stream(&self, request: ModelRequest, cancel: CancellationToken) -> ModelStream {
        let client = self.clone();
        let (tx, rx) = mpsc::channel(CHANNEL_CAPACITY);
        tokio::spawn(async move {
            // Cancellation wins over a forwarded item, so it is reported once.
            tokio::select! {
                biased;
                () = cancel.cancelled() => {
                    if tx.try_send(Err(LlmError::Cancelled)).is_err() {
                        tracing::debug!("stream consumer gone or full at cancellation");
                    }
                }
                () = tx.closed() => {}
                () = client.forward(request, cancel.child_token(), &tx) => {}
            }
        });
        rx
    }

    fn provider(&self) -> &str {
        LABEL
    }
}

/// A refused token most often means one issued to another app, or revoked.
fn sign_in_hint(error: LlmError) -> LlmError {
    match error {
        LlmError::Http {
            status: status @ (401 | 403),
            body,
        } => LlmError::Http {
            status,
            body: format!("{SIGN_IN_AGAIN}\n{body}"),
        },
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(base_url: &str) -> ProviderConfig {
        ProviderConfig {
            base_url: base_url.into(),
            api_key: Some(" gho_token ".into()),
            ..ProviderConfig::default()
        }
    }

    #[test]
    fn endpoints_share_bearer_auth_and_messages_has_no_api_key() {
        let client = CopilotClient::new(&config("https://api.githubcopilot.com/")).unwrap();
        assert_eq!(
            client.chat.url,
            "https://api.githubcopilot.com/chat/completions"
        );
        assert_eq!(
            client.responses.url,
            "https://api.githubcopilot.com/responses"
        );
        assert_eq!(
            client.messages.url,
            "https://api.githubcopilot.com/v1/messages"
        );
        assert_eq!(client.models_url, "https://api.githubcopilot.com/models");
        for endpoint in [&client.chat, &client.responses, &client.messages] {
            assert_eq!(endpoint.headers["authorization"], "Bearer gho_token");
            assert!(!endpoint.headers.contains_key("x-api-key"));
            assert_eq!(endpoint.label, LABEL);
        }
        assert_eq!(
            client.messages.headers["anthropic-version"],
            ANTHROPIC_VERSION
        );
        assert!(!client.chat.headers.contains_key("anthropic-version"));
        assert!(!format!("{client:?}").contains("gho_token"));
    }

    #[test]
    fn a_missing_token_or_base_is_a_config_error() {
        let signed_out = ProviderConfig {
            api_key: Some("  ".into()),
            ..config("https://api.githubcopilot.com")
        };
        assert_eq!(
            CopilotClient::new(&signed_out).unwrap_err(),
            LlmError::Config(SIGNED_OUT.into())
        );
        assert!(matches!(
            CopilotClient::new(&config(" ")),
            Err(LlmError::Config(_))
        ));
    }

    #[test]
    fn refused_tokens_suggest_signing_in_again() {
        let refused = sign_in_hint(LlmError::Http {
            status: 403,
            body: "forbidden".into(),
        });
        assert!(matches!(
            &refused,
            LlmError::Http { status: 403, body } if body.contains(SIGN_IN_AGAIN) && body.ends_with("forbidden")
        ));
        let other = LlmError::Http {
            status: 400,
            body: "bad".into(),
        };
        assert_eq!(sign_in_hint(other.clone()), other);
    }
}
