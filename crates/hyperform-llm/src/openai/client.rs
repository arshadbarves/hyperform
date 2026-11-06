//! The OpenAI-compatible [`ModelClient`]: one adapter for OpenRouter,
//! OpenAI, Ollama, LM Studio, Groq and OpenCode Zen.

use reqwest::header::HeaderMap;
use tokio_util::sync::CancellationToken;

use super::request::{ChatDialect, build_body};
use super::stream::ChatStreamParser;
use crate::client::ModelClient;
use crate::transport::Endpoint;
use crate::types::{ModelRequest, ModelStream};
use crate::zen;

#[derive(Debug, Clone)]
pub(crate) struct OpenAiChatClient {
    endpoint: Endpoint,
    dialect: ChatDialect,
}

impl OpenAiChatClient {
    pub(crate) fn new(endpoint: Endpoint, dialect: ChatDialect) -> Self {
        Self { endpoint, dialect }
    }
}

impl ModelClient for OpenAiChatClient {
    fn stream(&self, request: ModelRequest, cancel: CancellationToken) -> ModelStream {
        let mut body = build_body(&request, self.dialect);
        if self.endpoint.zen_session.is_some() {
            zen::apply_free_tier_body(&mut body);
        }
        self.endpoint.stream(
            &body,
            HeaderMap::new(),
            request.session_key.as_deref(),
            ChatStreamParser::default(),
            cancel,
        )
    }

    fn provider(&self) -> &str {
        self.endpoint.label
    }
}
