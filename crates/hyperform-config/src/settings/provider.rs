//! `[provider]`: how the model API is reached.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub enum ProviderKind {
    /// Pick the wire format from the base URL.
    #[default]
    Auto,
    /// OpenAI-compatible chat completions (OpenRouter, OpenCode Zen, local servers).
    #[serde(alias = "openai", alias = "openai-chat")]
    OpenaiChat,
    /// Anthropic Messages API.
    Anthropic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct ProviderSettings {
    pub kind: ProviderKind,
    /// API base URL, stored without a trailing slash.
    pub base_url: String,
    /// Extra HTTP headers sent with every model request.
    pub headers: BTreeMap<String, String>,
    /// Prompt caching; `None` enables it where the provider supports it.
    pub cache_control: Option<bool>,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            kind: ProviderKind::Auto,
            base_url: DEFAULT_BASE_URL.to_string(),
            headers: BTreeMap::new(),
            cache_control: None,
        }
    }
}
