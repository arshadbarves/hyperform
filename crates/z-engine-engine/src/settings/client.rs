//! The session's model client: the injected factory or a real provider
//! adapter, wrapped in [`FallbackClient`] when fallbacks are configured. A
//! client that cannot be built is replaced by one that fails every request
//! with the reason, so the session opens and turns report the problem.

use std::sync::Arc;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use z_engine_config::{
    Credentials, Paths, ProviderKind as SettingsKind, Settings, resolve_api_key,
};
use z_engine_llm::{
    FallbackClient, LlmError, ModelClient, ModelRequest, ModelStream, ProviderConfig, ProviderKind,
    build_client,
};

use crate::error::EngineError;
use crate::options::ClientFactory;

/// The client plus a warning when credentials could not be read.
pub(crate) fn session_client(
    paths: &Paths,
    settings: &Settings,
    factory: Option<&Arc<dyn ClientFactory>>,
) -> (Arc<dyn ModelClient>, Option<String>) {
    let (api_key, warning) = match Credentials::load(&paths.auth_file) {
        Ok(credentials) => (
            resolve_api_key(&credentials, &settings.provider.base_url),
            None,
        ),
        Err(error) => (
            None,
            Some(format!("stored API keys are unreadable: {error}")),
        ),
    };
    match build(settings, api_key, factory) {
        Ok(client) => (client, warning),
        Err(error) => {
            let reason = error.to_string();
            let client: Arc<dyn ModelClient> = Arc::new(UnavailableClient {
                reason: reason.clone(),
            });
            let warning = warning.map_or(reason.clone(), |w| format!("{w}; {reason}"));
            (client, Some(warning))
        }
    }
}

fn build(
    settings: &Settings,
    api_key: Option<String>,
    factory: Option<&Arc<dyn ClientFactory>>,
) -> Result<Arc<dyn ModelClient>, EngineError> {
    let base = match factory {
        Some(factory) => factory.build(settings, api_key)?,
        None => build_client(&provider_config(settings, api_key))?,
    };
    if settings.model.fallbacks.is_empty() {
        return Ok(base);
    }
    Ok(Arc::new(FallbackClient::new(
        base,
        settings.model.fallbacks.clone(),
    )))
}

fn provider_config(settings: &Settings, api_key: Option<String>) -> ProviderConfig {
    let provider = &settings.provider;
    ProviderConfig {
        kind: match provider.kind {
            SettingsKind::Auto => None,
            SettingsKind::OpenaiChat => Some(ProviderKind::OpenAiChat),
            SettingsKind::Anthropic => Some(ProviderKind::Anthropic),
        },
        base_url: provider.base_url.clone(),
        api_key,
        extra_headers: provider
            .headers
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect(),
        cache_control: provider.cache_control,
        reasoning_style: None,
    }
}

/// Fails every request with the configuration problem.
#[derive(Debug)]
pub(crate) struct UnavailableClient {
    reason: String,
}

impl ModelClient for UnavailableClient {
    fn stream(&self, _request: ModelRequest, _cancel: CancellationToken) -> ModelStream {
        let (tx, rx) = mpsc::channel(1);
        if tx
            .try_send(Err(LlmError::Config(self.reason.clone())))
            .is_err()
        {
            tracing::debug!("unavailable client: receiver dropped before the error");
        }
        rx
    }

    fn provider(&self) -> &str {
        "unavailable"
    }
}
