//! Provider selection and the switches that tune each wire dialect.

use std::fmt;

/// Default base URL of Anthropic's native API.
pub const ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com";

/// Wire protocol spoken by an endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderKind {
    /// OpenAI-compatible Chat Completions (OpenRouter, OpenAI, Ollama, ...).
    OpenAiChat,
    /// Anthropic's native Messages API.
    Anthropic,
}

/// How an OpenAI-compatible endpoint receives the reasoning effort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReasoningStyle {
    /// OpenRouter's unified `reasoning: {effort}` object.
    OpenRouter,
    /// OpenAI's `reasoning_effort` (with `max_completion_tokens`).
    OpenAi,
    /// Never send a reasoning parameter.
    None,
}

/// Everything needed to build a [`crate::ModelClient`] for one endpoint.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct ProviderConfig {
    /// `None` detects the protocol from `base_url`.
    pub kind: Option<ProviderKind>,
    pub base_url: String,
    pub api_key: Option<String>,
    /// Sent with every request; replaces a default header of the same name.
    pub extra_headers: Vec<(String, String)>,
    /// Prompt-cache breakpoints. `None`: on for OpenRouter and Anthropic.
    pub cache_control: Option<bool>,
    /// `None`: `OpenAi` for api.openai.com, `OpenRouter` otherwise.
    pub reasoning_style: Option<ReasoningStyle>,
}

impl fmt::Debug for ProviderConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let header_names: Vec<&str> = self
            .extra_headers
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        f.debug_struct("ProviderConfig")
            .field("kind", &self.kind)
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("extra_headers", &header_names)
            .field("cache_control", &self.cache_control)
            .field("reasoning_style", &self.reasoning_style)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_the_key_and_header_values() {
        let config = ProviderConfig {
            base_url: "https://openrouter.ai/api/v1".into(),
            api_key: Some("sk-or-secret".into()),
            extra_headers: vec![("x-proxy-token".into(), "proxy-secret".into())],
            ..ProviderConfig::default()
        };
        let rendered = format!("{config:?}");
        assert!(!rendered.contains("sk-or-secret"));
        assert!(!rendered.contains("proxy-secret"));
        assert!(rendered.contains("<redacted>"));
        assert!(rendered.contains("x-proxy-token"));
    }
}
