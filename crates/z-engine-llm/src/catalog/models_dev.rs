//! The models.dev catalog (<https://models.dev/api.json>): fetching and
//! parsing. Every provider's models are kept with the ids that provider
//! uses, so OpenRouter-style `vendor/model` ids and native ids both resolve.

use std::time::Duration;

use serde_json::Value;

use super::types::{ModelCatalog, ModelInfo, Pricing};
use crate::error::LlmError;
use crate::retry::{describe, error_body};

const MODELS_DEV_URL: &str = "https://models.dev/api.json";
const FETCH_TIMEOUT: Duration = Duration::from_secs(20);

/// Providers whose entries win lookups of ids that several providers
/// share: the aggregator first (its ids carry a vendor prefix), then
/// first-party APIs, so resellers and free tiers never shadow list prices.
const PREFERRED_PROVIDERS: &[&str] = &[
    "openrouter",
    "anthropic",
    "openai",
    "google",
    "deepseek",
    "mistral",
    "xai",
    "groq",
    "opencode",
];

impl ModelCatalog {
    /// Parse the raw models.dev payload. Malformed individual fields fall
    /// back to "unknown" values instead of rejecting the whole catalog.
    pub fn from_models_dev_json(json: &str) -> Result<Self, LlmError> {
        let root: Value = serde_json::from_str(json)
            .map_err(|error| LlmError::Decode(format!("models.dev catalog: {error}")))?;
        let providers = root
            .as_object()
            .ok_or_else(|| LlmError::Decode("models.dev catalog is not a JSON object".into()))?;
        let mut ordered: Vec<(&String, &Value)> = providers.iter().collect();
        ordered.sort_by(|(a, _), (b, _)| preference(a).cmp(&preference(b)).then_with(|| a.cmp(b)));
        let mut models = Vec::new();
        for (provider_id, provider) in ordered {
            let Some(entries) = provider.get("models").and_then(Value::as_object) else {
                continue;
            };
            let mut entries: Vec<(&String, &Value)> = entries.iter().collect();
            entries.sort_by_key(|(id, _)| *id);
            for (model_id, entry) in entries {
                let mut info = unknown_model(provider_id, model_id, false);
                apply_fields(&mut info, entry);
                models.push(info);
            }
        }
        Ok(Self { models })
    }
}

/// Download the raw models.dev catalog JSON.
pub async fn fetch_models_dev(http: &reqwest::Client) -> Result<String, LlmError> {
    fetch_catalog(http, MODELS_DEV_URL).await
}

async fn fetch_catalog(http: &reqwest::Client, url: &str) -> Result<String, LlmError> {
    let response = http
        .get(url)
        .timeout(FETCH_TIMEOUT)
        .send()
        .await
        .map_err(|error| LlmError::Connect {
            attempts: 1,
            cause: describe(&error),
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(LlmError::Http {
            status: status.as_u16(),
            body: error_body(response).await,
        });
    }
    response
        .text()
        .await
        .map_err(|error| LlmError::Stream(describe(&error)))
}

/// A model with nothing known beyond its identity.
pub(super) fn unknown_model(provider: &str, id: &str, tools: bool) -> ModelInfo {
    ModelInfo {
        id: id.to_string(),
        name: id.to_string(),
        provider: provider.to_string(),
        context_window: 0,
        max_output: 0,
        pricing: None,
        tools,
        vision: false,
        reasoning: false,
        deprecated: false,
    }
}

/// Overwrite the fields present in a models.dev-shaped `entry`. Also reads
/// the flat `context`/`output` and `attachment` fields of v1 overrides.
pub(super) fn apply_fields(info: &mut ModelInfo, entry: &Value) {
    let flag = |name: &str| entry.get(name).and_then(Value::as_bool);
    if let Some(name) = entry
        .get("name")
        .and_then(Value::as_str)
        .filter(|n| !n.is_empty())
    {
        info.name = name.to_string();
    }
    if let Some(reasoning) = flag("reasoning") {
        info.reasoning = reasoning;
    }
    if let Some(tools) = flag("tool_call") {
        info.tools = tools;
    }
    if let Some(status) = entry.get("status").and_then(Value::as_str) {
        info.deprecated = status == "deprecated";
    }
    let image_input = entry
        .pointer("/modalities/input")
        .and_then(Value::as_array)
        .map(|inputs| inputs.iter().any(|input| input == "image"));
    if let Some(vision) = image_input.or_else(|| flag("attachment")) {
        info.vision = vision;
    }
    let limit = |name: &str| {
        count(entry.get("limit").and_then(|limit| limit.get(name)))
            .or_else(|| count(entry.get(name)))
    };
    if let Some(tokens) = limit("context") {
        info.context_window = tokens;
    }
    if let Some(tokens) = limit("output") {
        info.max_output = tokens;
    }
    if let Some(cost) = entry.get("cost").filter(|cost| cost.is_object()) {
        info.pricing = Some(pricing(cost, info.pricing));
    }
}

/// Listed prices win; missing ones keep `previous`, and cache prices with
/// no previous value assume no discount (the input price).
fn pricing(cost: &Value, previous: Option<Pricing>) -> Pricing {
    let price = |name: &str| {
        cost.get(name)
            .and_then(Value::as_f64)
            .filter(|usd| usd.is_finite() && *usd >= 0.0)
    };
    let input = price("input").or(previous.map(|p| p.input)).unwrap_or(0.0);
    let output = price("output")
        .or(previous.map(|p| p.output))
        .unwrap_or(0.0);
    Pricing {
        input,
        output,
        cache_read: price("cache_read")
            .or(previous.map(|p| p.cache_read))
            .unwrap_or(input),
        cache_write: price("cache_write")
            .or(previous.map(|p| p.cache_write))
            .unwrap_or(input),
    }
}

fn count(value: Option<&Value>) -> Option<u32> {
    let value = value?;
    let tokens = value.as_u64().or_else(|| {
        value
            .as_f64()
            .filter(|tokens| tokens.is_finite() && *tokens >= 0.0)
            .map(|tokens| tokens as u64)
    })?;
    Some(u32::try_from(tokens).unwrap_or(u32::MAX))
}

fn preference(provider: &str) -> usize {
    PREFERRED_PROVIDERS
        .iter()
        .position(|preferred| *preferred == provider)
        .unwrap_or(PREFERRED_PROVIDERS.len())
}

#[cfg(test)]
mod tests;
