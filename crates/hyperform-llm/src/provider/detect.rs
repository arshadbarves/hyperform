//! Endpoint identification from a base URL.

use super::config::ProviderKind;
use crate::copilot::is_copilot_host;

/// The protocol a base URL speaks: Anthropic's own hosts use the native
/// Messages API, everything else is treated as OpenAI-compatible.
pub fn detect_kind(base_url: &str) -> ProviderKind {
    if authority(base_url).contains("anthropic.com") {
        ProviderKind::Anthropic
    } else {
        ProviderKind::OpenAiChat
    }
}

/// Short provider label for diagnostics and provider-specific defaults.
pub fn provider_label(base_url: &str) -> &'static str {
    let authority = authority(base_url);
    let (host, port) = split_port(&authority);
    if host.contains("openrouter.ai") {
        "openrouter"
    } else if is_copilot_host(host) {
        "github-copilot"
    } else if host.contains("anthropic.com") {
        "anthropic"
    } else if host == "openai.com" || host.ends_with(".openai.com") {
        "openai"
    } else if host.contains("opencode.ai") {
        "opencode"
    } else if host.contains("groq.com") {
        "groq"
    } else if port == Some("11434") {
        "ollama"
    } else if port == Some("1234") && matches!(host, "localhost" | "127.0.0.1" | "[::1]") {
        "lmstudio"
    } else {
        "custom"
    }
}

/// The models.dev provider id whose catalog entries describe this
/// endpoint's models, when its label is one; `None` for a local Ollama
/// server (models.dev lists only Ollama's cloud) and custom endpoints.
pub fn catalog_provider(base_url: &str) -> Option<&'static str> {
    match provider_label(base_url) {
        label @ ("openrouter" | "github-copilot" | "anthropic" | "openai" | "opencode" | "groq"
        | "lmstudio") => Some(label),
        _ => None,
    }
}

/// Lowercased `host[:port]` of a URL, tolerating a missing scheme.
pub(crate) fn authority(base_url: &str) -> String {
    let lower = base_url.trim().to_ascii_lowercase();
    let rest = lower
        .split_once("://")
        .map_or(lower.as_str(), |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    authority.rsplit('@').next().unwrap_or_default().to_string()
}

pub(crate) fn split_port(authority: &str) -> (&str, Option<&str>) {
    match authority.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => {
            (host, Some(port))
        }
        _ => (authority, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anthropic_hosts_use_the_native_protocol() {
        assert_eq!(
            detect_kind("https://api.anthropic.com"),
            ProviderKind::Anthropic
        );
        assert_eq!(
            detect_kind("https://API.ANTHROPIC.COM/v1/"),
            ProviderKind::Anthropic
        );
        assert_eq!(
            detect_kind("https://openrouter.ai/api/v1"),
            ProviderKind::OpenAiChat
        );
        assert_eq!(
            detect_kind("https://proxy.example/anthropic.com/v1"),
            ProviderKind::OpenAiChat
        );
    }

    #[test]
    fn labels_known_providers() {
        let cases = [
            ("https://openrouter.ai/api/v1", "openrouter"),
            ("https://api.anthropic.com", "anthropic"),
            ("https://api.openai.com/v1", "openai"),
            ("https://opencode.ai/zen/v1", "opencode"),
            ("https://api.githubcopilot.com", "github-copilot"),
            ("https://copilot-api.acme.ghe.com", "github-copilot"),
            ("https://api.groq.com/openai/v1", "groq"),
            ("http://localhost:11434/v1", "ollama"),
            ("http://gpu-box:11434/v1", "ollama"),
            ("localhost:1234/v1", "lmstudio"),
            ("http://127.0.0.1:1234/v1", "lmstudio"),
            ("http://[::1]:1234/v1", "lmstudio"),
            ("http://10.0.0.5:1234/v1", "custom"),
            ("https://user:pw@my-proxy.example/v1", "custom"),
            ("https://example.openai.azure.com/v1", "custom"),
            ("", "custom"),
        ];
        for (url, label) in cases {
            assert_eq!(provider_label(url), label, "{url}");
        }
    }

    #[test]
    fn catalog_providers_are_models_dev_ids() {
        let cases = [
            ("https://api.githubcopilot.com", Some("github-copilot")),
            ("https://copilot-api.acme.ghe.com", Some("github-copilot")),
            ("https://openrouter.ai/api/v1", Some("openrouter")),
            ("https://api.anthropic.com", Some("anthropic")),
            ("https://api.openai.com/v1", Some("openai")),
            ("https://opencode.ai/zen/v1", Some("opencode")),
            ("https://api.groq.com/openai/v1", Some("groq")),
            ("http://localhost:1234/v1", Some("lmstudio")),
            ("http://localhost:11434/v1", None),
            ("https://my-proxy.example/v1", None),
        ];
        for (url, provider) in cases {
            assert_eq!(catalog_provider(url), provider, "{url}");
        }
    }
}
