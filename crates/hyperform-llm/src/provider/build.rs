//! Builds a [`ModelClient`] from a [`ProviderConfig`].

use std::sync::Arc;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};

use super::config::{ProviderConfig, ProviderKind, ReasoningStyle};
use super::detect::{detect_kind, provider_label};
use super::http::{apply_extra_headers, http_client, secret};
use crate::anthropic::{ANTHROPIC_VERSION, AnthropicClient};
use crate::client::ModelClient;
use crate::copilot::{build_copilot_client, is_copilot_url};
use crate::error::LlmError;
use crate::openai::{ChatDialect, OpenAiChatClient};
use crate::retry::RetryPolicy;
use crate::transport::Endpoint;
use crate::zen;

const OPENROUTER_REFERER: &str = "https://github.com/arshadbarves/hyperform";
const OPENROUTER_TITLE: &str = "HyperForm AI";

/// Build the adapter for `config`: validates the base URL, credentials and
/// extra headers up front so a bad configuration fails here, not mid-turn.
/// GitHub Copilot hosts get the Copilot client whatever `kind` says.
pub fn build_client(config: &ProviderConfig) -> Result<Arc<dyn ModelClient>, LlmError> {
    if is_copilot_url(&config.base_url) {
        return build_copilot_client(config);
    }
    let client: Arc<dyn ModelClient> = match prepare(config)? {
        Prepared::OpenAiChat { endpoint, dialect } => {
            Arc::new(OpenAiChatClient::new(endpoint, dialect))
        }
        Prepared::Anthropic {
            endpoint,
            cache_control,
        } => Arc::new(AnthropicClient::new(endpoint, cache_control)),
    };
    Ok(client)
}

#[derive(Debug)]
enum Prepared {
    OpenAiChat {
        endpoint: Endpoint,
        dialect: ChatDialect,
    },
    Anthropic {
        endpoint: Endpoint,
        cache_control: bool,
    },
}

fn prepare(config: &ProviderConfig) -> Result<Prepared, LlmError> {
    let base_url = config.base_url.trim().trim_end_matches('/');
    if base_url.is_empty() {
        return Err(LlmError::Config("base_url is empty".into()));
    }
    let label = provider_label(base_url);
    let api_key = config
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty());
    let http = http_client()?;
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let kind = config.kind.unwrap_or_else(|| detect_kind(base_url));
    let url = match kind {
        ProviderKind::OpenAiChat => {
            if let Some(key) = api_key {
                headers.insert(AUTHORIZATION, secret(&format!("Bearer {key}"))?);
            }
            if label == "openrouter" {
                headers.insert("http-referer", HeaderValue::from_static(OPENROUTER_REFERER));
                headers.insert("x-title", HeaderValue::from_static(OPENROUTER_TITLE));
            }
            format!("{base_url}/chat/completions")
        }
        ProviderKind::Anthropic => {
            if let Some(key) = api_key {
                headers.insert("x-api-key", secret(key)?);
            }
            headers.insert(
                "anthropic-version",
                HeaderValue::from_static(ANTHROPIC_VERSION),
            );
            let root = base_url.strip_suffix("/v1").unwrap_or(base_url);
            format!("{root}/v1/messages")
        }
    };
    apply_extra_headers(&mut headers, &config.extra_headers)?;
    let endpoint = Endpoint {
        http,
        url,
        headers,
        label,
        zen_session: zen::is_zen_url(base_url).then(zen::new_session_id),
        policy: RetryPolicy::DEFAULT,
    };
    Ok(match kind {
        ProviderKind::OpenAiChat => Prepared::OpenAiChat {
            endpoint,
            dialect: ChatDialect {
                cache_control: config.cache_control.unwrap_or(label == "openrouter"),
                reasoning: config.reasoning_style.unwrap_or(if label == "openai" {
                    ReasoningStyle::OpenAi
                } else {
                    ReasoningStyle::OpenRouter
                }),
            },
        },
        ProviderKind::Anthropic => Prepared::Anthropic {
            endpoint,
            cache_control: config.cache_control.unwrap_or(true),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(base_url: &str) -> ProviderConfig {
        ProviderConfig {
            base_url: base_url.into(),
            api_key: Some(" sk-secret ".into()),
            ..ProviderConfig::default()
        }
    }

    fn openai(config: &ProviderConfig) -> (Endpoint, ChatDialect) {
        match prepare(config).unwrap() {
            Prepared::OpenAiChat { endpoint, dialect } => (endpoint, dialect),
            other => panic!("expected an OpenAI-compatible client, got {other:?}"),
        }
    }

    #[test]
    fn empty_base_url_is_a_config_error() {
        assert!(matches!(
            build_client(&config(" / ")),
            Err(LlmError::Config(_))
        ));
    }

    #[test]
    fn openrouter_gets_bearer_attribution_and_cache_control() {
        let (endpoint, dialect) = openai(&config("https://openrouter.ai/api/v1///"));
        assert_eq!(
            endpoint.url,
            "https://openrouter.ai/api/v1/chat/completions"
        );
        assert_eq!(endpoint.label, "openrouter");
        assert_eq!(endpoint.headers[AUTHORIZATION], "Bearer sk-secret");
        assert!(endpoint.headers[AUTHORIZATION].is_sensitive());
        assert_eq!(endpoint.headers["x-title"], OPENROUTER_TITLE);
        assert_eq!(endpoint.headers["http-referer"], OPENROUTER_REFERER);
        assert_eq!(
            dialect,
            ChatDialect {
                cache_control: true,
                reasoning: ReasoningStyle::OpenRouter
            }
        );
        assert!(!format!("{endpoint:?}").contains("sk-secret"));
    }

    #[test]
    fn openai_and_local_servers_get_their_defaults() {
        let (endpoint, dialect) = openai(&config("https://api.openai.com/v1"));
        assert_eq!(dialect.reasoning, ReasoningStyle::OpenAi);
        assert!(!dialect.cache_control);
        assert!(endpoint.zen_session.is_none());
        let local = ProviderConfig {
            api_key: None,
            ..config("http://localhost:11434/v1")
        };
        let (endpoint, _) = openai(&local);
        assert!(!endpoint.headers.contains_key(AUTHORIZATION));
        assert_eq!(endpoint.label, "ollama");
    }

    #[test]
    fn zen_clients_keep_a_stable_session() {
        let (endpoint, _) = openai(&config("https://opencode.ai/zen/v1"));
        let id = endpoint.zen_session.as_deref().expect("zen session");
        assert!(id.starts_with("ses_"), "{id}");
        assert_eq!(id.len(), 30, "{id}");
        assert!(
            id.as_bytes()[4..16]
                .iter()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
            "{id}"
        );
    }

    #[test]
    fn anthropic_endpoint_strips_v1_and_sets_version_headers() {
        for base in ["https://api.anthropic.com", "https://api.anthropic.com/v1/"] {
            match prepare(&config(base)).unwrap() {
                Prepared::Anthropic {
                    endpoint,
                    cache_control,
                } => {
                    assert_eq!(endpoint.url, "https://api.anthropic.com/v1/messages");
                    assert_eq!(endpoint.headers["x-api-key"], "sk-secret");
                    assert!(endpoint.headers["x-api-key"].is_sensitive());
                    assert_eq!(endpoint.headers["anthropic-version"], ANTHROPIC_VERSION);
                    assert!(cache_control);
                }
                other => panic!("expected Anthropic, got {other:?}"),
            }
        }
    }

    #[test]
    fn explicit_settings_override_detection() {
        let explicit = ProviderConfig {
            kind: Some(ProviderKind::Anthropic),
            cache_control: Some(false),
            ..config("http://127.0.0.1:8080/v1")
        };
        assert!(matches!(
            prepare(&explicit).unwrap(),
            Prepared::Anthropic {
                cache_control: false,
                ..
            }
        ));
    }

    #[test]
    fn copilot_hosts_get_the_copilot_client_whatever_the_kind() {
        let copilot = ProviderConfig {
            kind: Some(ProviderKind::Anthropic),
            ..config("https://api.githubcopilot.com")
        };
        assert_eq!(build_client(&copilot).unwrap().provider(), "github-copilot");
        let enterprise = config("https://copilot-api.acme.ghe.com/");
        assert_eq!(
            build_client(&enterprise).unwrap().provider(),
            "github-copilot"
        );
        let signed_out = ProviderConfig {
            api_key: None,
            ..copilot
        };
        assert!(matches!(
            build_client(&signed_out),
            Err(LlmError::Config(message)) if message.contains("sign in to GitHub Copilot")
        ));
    }

    #[test]
    fn extra_headers_replace_defaults_and_are_validated() {
        let custom = ProviderConfig {
            extra_headers: vec![("Authorization".into(), "Token abc".into())],
            ..config("https://gateway.example/v1")
        };
        let (endpoint, _) = openai(&custom);
        let values: Vec<_> = endpoint.headers.get_all(AUTHORIZATION).iter().collect();
        assert_eq!(values, ["Token abc"]);
        let bad_name = ProviderConfig {
            extra_headers: vec![("bad header".into(), "v".into())],
            ..config("https://gateway.example/v1")
        };
        assert!(matches!(prepare(&bad_name), Err(LlmError::Config(_))));
        let bad_key = ProviderConfig {
            api_key: Some("line\nbreak".into()),
            ..config("https://gateway.example/v1")
        };
        assert!(matches!(prepare(&bad_key), Err(LlmError::Config(_))));
    }
}
