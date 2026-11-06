//! Model roles and per-model limits: the `fast`/`review` roles fall back to
//! the session's main model; windows, output ceilings and prices prefer
//! settings, then the catalog (the configured provider's entry first), then
//! built-in defaults.

use hyperform_config::Settings;
use hyperform_llm::{ModelCatalog, ModelInfo, Pricing, catalog_provider, price_for};

/// Used when neither settings nor the catalog know the window.
pub(crate) const DEFAULT_CONTEXT_WINDOW: u64 = 128_000;

pub(crate) fn fast_model(settings: &Settings, main: &str) -> String {
    settings
        .model
        .fast
        .clone()
        .filter(|model| !model.trim().is_empty())
        .unwrap_or_else(|| main.to_string())
}

pub(crate) fn context_window(
    settings: &Settings,
    catalog: Option<&ModelCatalog>,
    model: &str,
) -> u64 {
    if let Some(window) = settings.model.context_window.filter(|w| *w > 0) {
        return u64::from(window);
    }
    catalog_limit(settings, catalog, model, |info| info.context_window)
        .map_or(DEFAULT_CONTEXT_WINDOW, u64::from)
}

/// The configured ceiling, lowered to the model's own limit when known.
pub(crate) fn max_output_tokens(
    settings: &Settings,
    catalog: Option<&ModelCatalog>,
    model: &str,
) -> u32 {
    let configured = settings.model.max_output_tokens;
    catalog_limit(settings, catalog, model, |info| info.max_output)
        .map_or(configured, |limit| configured.min(limit))
}

/// A token limit of `model` from the configured provider's catalog entry
/// when it lists one, else from the catalog's preferred entry; 0 is unknown.
fn catalog_limit(
    settings: &Settings,
    catalog: Option<&ModelCatalog>,
    model: &str,
    limit: impl Fn(&ModelInfo) -> u32,
) -> Option<u32> {
    let catalog = catalog?;
    let known = |info: Option<&ModelInfo>| info.map(&limit).filter(|tokens| *tokens > 0);
    catalog_provider(&settings.provider.base_url)
        .and_then(|provider| known(catalog.lookup_in(provider, model)))
        .or_else(|| known(catalog.lookup(model)))
}

/// `[pricing]` overrides win; then the catalog, preferring the configured
/// provider's entry, and the built-in table.
pub(crate) fn pricing(
    settings: &Settings,
    catalog: Option<&ModelCatalog>,
    model: &str,
) -> Option<Pricing> {
    let bare = model.split_once('/').map_or(model, |(_, rest)| rest);
    let configured = settings.pricing.get(model).or_else(|| {
        settings
            .pricing
            .iter()
            .find(|(id, _)| id.split_once('/').map_or(id.as_str(), |(_, rest)| rest) == bare)
            .map(|(_, price)| price)
    });
    if let Some(price) = configured {
        return Some(Pricing {
            input: price.input,
            output: price.output,
            cache_read: price.cache_read.unwrap_or(price.input),
            cache_write: price.cache_write.unwrap_or(price.input),
        });
    }
    let provider = catalog_provider(&settings.provider.base_url);
    price_for(catalog, provider, model)
}

/// Unknown models are assumed to accept images.
pub(crate) fn supports_vision(catalog: Option<&ModelCatalog>, model: &str) -> bool {
    catalog
        .and_then(|catalog| catalog.lookup(model))
        .is_none_or(|info| info.vision)
}

#[cfg(test)]
mod tests {
    use hyperform_config::PricingOverride;

    use super::*;

    #[test]
    fn roles_and_windows_fall_back_in_order() {
        let mut settings = Settings::default();
        assert_eq!(fast_model(&settings, "main-model"), "main-model");
        settings.model.fast = Some("quick".into());
        assert_eq!(fast_model(&settings, "main-model"), "quick");
        assert_eq!(context_window(&settings, None, "x"), DEFAULT_CONTEXT_WINDOW);
        settings.model.context_window = Some(9_000);
        assert_eq!(context_window(&settings, None, "x"), 9_000);
    }

    #[test]
    fn pricing_overrides_match_with_or_without_vendor() {
        let mut settings = Settings::default();
        settings.pricing.insert(
            "acme/model".into(),
            PricingOverride {
                input: 1.0,
                output: 2.0,
                cache_read: None,
                cache_write: Some(4.0),
            },
        );
        let price = pricing(&settings, None, "model").unwrap();
        assert_eq!(
            (price.input, price.cache_read, price.cache_write),
            (1.0, 1.0, 4.0)
        );
        assert!(pricing(&settings, None, "unknown-model").is_none());
    }

    fn copilot_catalog() -> ModelCatalog {
        ModelCatalog::from_models_dev_json(
            r#"{
                "openai": {"models": {
                    "gpt-5-mini": {"cost": {"input": 0.25, "output": 2},
                                   "limit": {"context": 400000, "output": 128000}},
                    "gpt-4o": {"limit": {"context": 128000, "output": 16384}}
                }},
                "github-copilot": {"models": {
                    "gpt-5-mini": {"cost": {"input": 0, "output": 0}},
                    "gpt-4o": {"limit": {"context": 64000, "output": 4096}}
                }}
            }"#,
        )
        .unwrap()
    }

    fn input_price(base_url: &str, settings: &mut Settings) -> Option<f64> {
        settings.provider.base_url = base_url.into();
        pricing(settings, Some(&copilot_catalog()), "gpt-5-mini").map(|price| price.input)
    }

    #[test]
    fn the_configured_providers_catalog_entry_prices_its_models() {
        let mut settings = Settings::default();
        for copilot in [
            "https://api.githubcopilot.com",
            "https://copilot-api.acme.ghe.com",
        ] {
            assert_eq!(input_price(copilot, &mut settings), Some(0.0), "{copilot}");
        }
        assert_eq!(
            input_price("https://api.openai.com/v1", &mut settings),
            Some(0.25)
        );
        assert_eq!(
            input_price("https://openrouter.ai/api/v1", &mut settings),
            Some(0.25),
            "a provider without the model falls back"
        );
        assert_eq!(
            input_price("https://my-proxy.example/v1", &mut settings),
            Some(0.25)
        );
    }

    #[test]
    fn pricing_overrides_win_over_the_providers_entry() {
        let mut settings = Settings::default();
        settings.pricing.insert(
            "gpt-5-mini".into(),
            PricingOverride {
                input: 9.0,
                output: 9.0,
                cache_read: None,
                cache_write: None,
            },
        );
        assert_eq!(
            input_price("https://api.githubcopilot.com", &mut settings),
            Some(9.0)
        );
    }

    /// `(context window, max output tokens)` for `model` behind `base_url`.
    fn limits(settings: &mut Settings, base_url: &str, model: &str) -> (u64, u32) {
        settings.provider.base_url = base_url.into();
        let catalog = copilot_catalog();
        (
            context_window(settings, Some(&catalog), model),
            max_output_tokens(settings, Some(&catalog), model),
        )
    }

    #[test]
    fn limits_prefer_the_configured_providers_entry() {
        let mut settings = Settings::default();
        settings.model.max_output_tokens = 32_000;
        let copilot = "https://api.githubcopilot.com";
        assert_eq!(limits(&mut settings, copilot, "gpt-4o"), (64_000, 4_096));
        assert_eq!(
            limits(&mut settings, "https://copilot-api.acme.ghe.com", "gpt-4o"),
            (64_000, 4_096)
        );
        assert_eq!(
            limits(&mut settings, copilot, "gpt-5-mini"),
            (400_000, 32_000),
            "an entry without limits falls back to the preferred entry"
        );
        for other in ["https://api.openai.com/v1", "https://openrouter.ai/api/v1"] {
            assert_eq!(
                limits(&mut settings, other, "gpt-4o"),
                (128_000, 16_384),
                "{other}"
            );
        }
        assert_eq!(
            limits(&mut settings, copilot, "unknown-model"),
            (DEFAULT_CONTEXT_WINDOW, 32_000)
        );
    }

    #[test]
    fn configured_limits_win_over_the_providers_entry() {
        let mut settings = Settings::default();
        settings.model.context_window = Some(9_000);
        settings.model.max_output_tokens = 1_000;
        assert_eq!(
            limits(&mut settings, "https://api.githubcopilot.com", "gpt-4o"),
            (9_000, 1_000)
        );
    }
}
