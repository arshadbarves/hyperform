//! The native Anthropic Messages [`ModelClient`].

use reqwest::header::{HeaderMap, HeaderValue};
use tokio_util::sync::CancellationToken;

use super::request::{build_body, thinking_enabled};
use super::stream::MessagesStreamParser;
use crate::client::ModelClient;
use crate::transport::Endpoint;
use crate::types::{ModelRequest, ModelStream};

/// The `anthropic-version` every Messages request carries.
pub(crate) const ANTHROPIC_VERSION: &str = "2023-06-01";
/// Lets the model think between tool calls within one response.
const INTERLEAVED_THINKING_BETA: &str = "interleaved-thinking-2025-05-14";

#[derive(Debug, Clone)]
pub(crate) struct AnthropicClient {
    endpoint: Endpoint,
    cache_control: bool,
}

impl AnthropicClient {
    pub(crate) fn new(endpoint: Endpoint, cache_control: bool) -> Self {
        Self {
            endpoint,
            cache_control,
        }
    }
}

/// Per-request headers: the interleaved-thinking beta when thinking meets
/// tools.
pub(crate) fn beta_headers(request: &ModelRequest) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if thinking_enabled(request) && !request.tools.is_empty() {
        headers.insert(
            "anthropic-beta",
            HeaderValue::from_static(INTERLEAVED_THINKING_BETA),
        );
    }
    headers
}

impl ModelClient for AnthropicClient {
    fn stream(&self, request: ModelRequest, cancel: CancellationToken) -> ModelStream {
        let body = build_body(&request, self.cache_control);
        self.endpoint.stream(
            &body,
            beta_headers(&request),
            request.session_key.as_deref(),
            MessagesStreamParser::default(),
            cancel,
        )
    }

    fn provider(&self) -> &str {
        self.endpoint.label
    }
}
