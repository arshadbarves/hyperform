//! `[model]` (which models run) and `[pricing]` (per-model cost overrides).

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use z_engine_protocol::Effort;

use super::lenient;

pub const DEFAULT_MODEL: &str = "anthropic/claude-sonnet-4.5";
pub const MIN_OUTPUT_TOKENS: u32 = 256;
pub const MAX_OUTPUT_TOKENS: u32 = 200_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct ModelSettings {
    /// Model id of the main agent.
    pub main: String,
    /// Model for titles, summaries, and quick subagents; `None` uses `main`.
    pub fast: Option<String>,
    /// Model for review agents; `None` uses `main`.
    pub review: Option<String>,
    /// Tried in order when the main model fails with a retryable error.
    pub fallbacks: Vec<String>,
    #[serde(deserialize_with = "lenient::effort")]
    pub effort: Option<Effort>,
    /// Per-request output ceiling, clamped to 256..=200000.
    pub max_output_tokens: u32,
    /// Overrides the catalog's context window when set.
    pub context_window: Option<u32>,
}

impl Default for ModelSettings {
    fn default() -> Self {
        Self {
            main: DEFAULT_MODEL.to_string(),
            fast: None,
            review: None,
            fallbacks: Vec::new(),
            effort: None,
            max_output_tokens: 16_384,
            context_window: None,
        }
    }
}

impl ModelSettings {
    pub fn fast_model(&self) -> &str {
        self.fast.as_deref().unwrap_or(&self.main)
    }

    pub fn review_model(&self) -> &str {
        self.review.as_deref().unwrap_or(&self.main)
    }
}

/// USD per million tokens for one model id, overriding the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct PricingOverride {
    #[serde(alias = "usd_per_mtok_input")]
    pub input: f64,
    #[serde(alias = "usd_per_mtok_output")]
    pub output: f64,
    pub cache_read: Option<f64>,
    pub cache_write: Option<f64>,
}
