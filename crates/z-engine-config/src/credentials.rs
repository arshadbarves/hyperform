//! API keys in `auth.json`, kept in the v1 format so both versions share
//! the file: `{ "<bucket>": { "type": "api", "key": "<secret>" } }`.
//! Entries this version does not understand are preserved on save.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ts_rs::TS;

use crate::error::ConfigError;
use crate::files::{read_data_file, write_atomic};
use crate::settings::SearchBackend;

pub const OPENROUTER: &str = "openrouter";
/// OpenCode Zen (`https://opencode.ai/zen/v1`).
pub const OPENCODE: &str = "opencode";
pub const ANTHROPIC: &str = "anthropic";
pub const OPENAI: &str = "openai";
/// The GitHub token from Copilot sign-in, for `api.githubcopilot.com`.
pub const GITHUB_COPILOT: &str = "github-copilot";
pub const BRAVE: &str = "brave";
pub const TAVILY: &str = "tavily";
pub const EXA: &str = "exa";

/// Stored keys by bucket. `Debug` lists bucket names only.
#[derive(Clone, Default, PartialEq)]
pub struct Credentials {
    entries: BTreeMap<String, Value>,
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("buckets", &self.entries.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// A stored key as the settings screen may show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "config/")]
pub struct KeyStatus {
    pub has_key: bool,
    /// The key's last four characters.
    pub hint: Option<String>,
}

impl Credentials {
    /// A missing or empty file holds no keys; a malformed one is an error so
    /// it is never overwritten.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let Some(text) = read_data_file(path)? else {
            return Ok(Self::default());
        };
        if text.trim().is_empty() {
            return Ok(Self::default());
        }
        let entries =
            serde_json::from_str(&text).map_err(|error| ConfigError::parse(path, error))?;
        Ok(Self { entries })
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let secret = self.entries.get(key)?.get("key")?.as_str()?.trim();
        (!secret.is_empty()).then(|| secret.to_string())
    }

    /// Stores a key; `None` or a blank value removes the bucket.
    pub fn set(&mut self, key: &str, value: Option<String>) {
        match value
            .as_deref()
            .map(str::trim)
            .filter(|secret| !secret.is_empty())
        {
            Some(secret) => {
                self.entries
                    .insert(key.to_string(), json!({ "type": "api", "key": secret }));
            }
            None => {
                self.entries.remove(key);
            }
        }
    }

    pub fn status(&self, key: &str) -> KeyStatus {
        let secret = self.get(key);
        KeyStatus {
            has_key: secret.is_some(),
            hint: secret.map(|secret| {
                let skip = secret.chars().count().saturating_sub(4);
                secret.chars().skip(skip).collect()
            }),
        }
    }

    /// Writes atomically; the file is owner-only (0600) on unix.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let text = serde_json::to_string_pretty(&self.entries).map_err(|error| {
            ConfigError::Serialize {
                what: "credentials",
                message: error.to_string(),
            }
        })?;
        write_atomic(path, format!("{text}\n").as_bytes(), true)
    }
}

/// The bucket holding the key for a model API: `openrouter`, `opencode`,
/// `anthropic`, `openai`, `github-copilot`, or `custom:<host[:port]>`. Keys
/// are never shared between hosts, so a Copilot Enterprise host
/// (`copilot-api.<domain>`) keeps its own `custom:` bucket.
pub fn credential_key(base_url: &str) -> String {
    let authority = authority(base_url);
    let host = host_name(&authority);
    let known = [
        ("openrouter.ai", OPENROUTER),
        ("opencode.ai", OPENCODE),
        ("anthropic.com", ANTHROPIC),
        ("githubcopilot.com", GITHUB_COPILOT),
    ];
    for (domain, bucket) in known {
        if host == domain || host.ends_with(&format!(".{domain}")) {
            return bucket.to_string();
        }
    }
    if host == "api.openai.com" {
        return OPENAI.to_string();
    }
    format!("custom:{authority}")
}

/// `HYPERFORM_API_KEY`, then provider variable, then stored key.
pub fn resolve_api_key(creds: &Credentials, base_url: &str) -> Option<String> {
    resolve_api_key_with(creds, base_url, |name| std::env::var(name).ok())
}

/// [`resolve_api_key`] with an injected environment lookup.
pub fn resolve_api_key_with(
    creds: &Credentials,
    base_url: &str,
    env: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let var = |name: &str| {
        env(name)
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    if let Some(key) = var("HYPERFORM_API_KEY") {
        return Some(key);
    }
    let bucket = credential_key(base_url);
    let provider_var = match bucket.as_str() {
        ANTHROPIC => Some("ANTHROPIC_API_KEY"),
        OPENROUTER => Some("OPENROUTER_API_KEY"),
        OPENAI => Some("OPENAI_API_KEY"),
        OPENCODE => Some("OPENCODE_API_KEY"),
        _ => None,
    };
    provider_var.and_then(var).or_else(|| creds.get(&bucket))
}

/// The bucket of a web search backend's key; SearXNG and none need no key.
pub fn search_key_bucket(backend: SearchBackend) -> Option<&'static str> {
    match backend {
        SearchBackend::Brave => Some(BRAVE),
        SearchBackend::Tavily => Some(TAVILY),
        SearchBackend::Exa => Some(EXA),
        SearchBackend::Disabled | SearchBackend::Searxng => None,
    }
}

/// `BRAVE_API_KEY` / `TAVILY_API_KEY` / `EXA_API_KEY`, then the stored key.
pub fn resolve_search_key(creds: &Credentials, backend: SearchBackend) -> Option<String> {
    resolve_search_key_with(creds, backend, |name| std::env::var(name).ok())
}

pub fn resolve_search_key_with(
    creds: &Credentials,
    backend: SearchBackend,
    env: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let bucket = search_key_bucket(backend)?;
    let var = format!("{}_API_KEY", bucket.to_ascii_uppercase());
    env(&var)
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| creds.get(bucket))
}

/// `host[:port]` of a URL, lowercased and without credentials.
fn authority(url: &str) -> String {
    let url = url.trim();
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    authority.to_ascii_lowercase()
}

fn host_name(authority: &str) -> &str {
    if authority.starts_with('[') {
        return authority
            .split(']')
            .next()
            .map_or(authority, |host| &host[1..]);
    }
    authority.split(':').next().unwrap_or(authority)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_follow_the_host() {
        assert_eq!(credential_key("https://openrouter.ai/api/v1"), OPENROUTER);
        assert_eq!(credential_key("https://opencode.ai/zen/v1"), OPENCODE);
        assert_eq!(credential_key("https://api.anthropic.com/v1/"), ANTHROPIC);
        assert_eq!(credential_key("https://api.openai.com/v1"), OPENAI);
        assert_eq!(
            credential_key("http://user:pw@LocalHost:11434/v1"),
            "custom:localhost:11434"
        );
        assert_eq!(
            credential_key("https://evil.example/?openrouter.ai"),
            "custom:evil.example"
        );
        assert_eq!(credential_key("http://[::1]:8080/v1"), "custom:[::1]:8080");
    }

    #[test]
    fn copilot_hosts_share_a_bucket_and_enterprise_keeps_its_own() {
        for url in [
            "https://api.githubcopilot.com",
            "https://githubcopilot.com/",
            "https://api.individual.githubcopilot.com/chat",
        ] {
            assert_eq!(credential_key(url), GITHUB_COPILOT, "{url}");
        }
        assert_eq!(
            credential_key("https://notgithubcopilot.com"),
            "custom:notgithubcopilot.com"
        );
        assert_eq!(
            credential_key("https://copilot-api.acme.ghe.com"),
            "custom:copilot-api.acme.ghe.com"
        );
    }

    #[test]
    fn copilot_reads_no_provider_variable() {
        let mut creds = Credentials::default();
        creds.set(GITHUB_COPILOT, Some("gho_saved".into()));
        let url = "https://api.githubcopilot.com";
        let github_token = |name: &str| (name == "GITHUB_TOKEN").then(|| "ghp_other".to_string());
        assert_eq!(
            resolve_api_key_with(&creds, url, github_token).as_deref(),
            Some("gho_saved")
        );
        let override_hyperform = |name: &str| (name == "HYPERFORM_API_KEY").then(|| "hk".to_string());
        assert_eq!(
            resolve_api_key_with(&creds, url, override_hyperform).as_deref(),
            Some("hk")
        );
    }

    #[test]
    fn debug_output_hides_secrets() {
        let mut creds = Credentials::default();
        creds.set(OPENROUTER, Some("sk-or-secret".into()));
        let shown = format!("{creds:?}");
        assert!(shown.contains("openrouter") && !shown.contains("sk-or-secret"));
    }
}
