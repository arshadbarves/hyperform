//! Cost accounting: token usage priced per model.

use hyperform_protocol::Usage;

use crate::catalog::{ModelCatalog, Pricing, builtin_pricing};

const TOKENS_PER_PRICE_UNIT: f64 = 1_000_000.0;

/// USD for `usage` at `pricing`. Reasoning tokens are already counted in
/// `output_tokens`, so they are not charged again.
pub fn cost_usd(pricing: &Pricing, usage: &Usage) -> f64 {
    let charge =
        |tokens: u64, usd_per_million: f64| tokens as f64 * usd_per_million / TOKENS_PER_PRICE_UNIT;
    charge(usage.input_tokens, pricing.input)
        + charge(usage.output_tokens, pricing.output)
        + charge(usage.cache_read_tokens, pricing.cache_read)
        + charge(usage.cache_write_tokens, pricing.cache_write)
}

/// The price of `model_id` on `provider` (a models.dev provider id, see
/// [`crate::catalog_provider`]): that provider's catalog entry when it
/// lists a price, else the catalog's preferred entry for the id, else the
/// built-in table.
pub fn price_for(
    catalog: Option<&ModelCatalog>,
    provider: Option<&str>,
    model_id: &str,
) -> Option<Pricing> {
    catalog
        .and_then(|catalog| {
            provider
                .and_then(|provider| catalog.lookup_in(provider, model_id))
                .and_then(|model| model.pricing)
                .or_else(|| catalog.lookup(model_id).and_then(|model| model.pricing))
        })
        .or_else(|| builtin_pricing(model_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn catalog() -> ModelCatalog {
        ModelCatalog::from_models_dev_json(include_str!("../tests/fixtures/models_dev.json"))
            .unwrap()
    }

    #[test]
    fn charges_every_token_class_once() {
        let pricing = Pricing {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        };
        let usage = Usage {
            input_tokens: 1_000_000,
            output_tokens: 100_000,
            cache_read_tokens: 2_000_000,
            cache_write_tokens: 400_000,
            reasoning_tokens: 50_000,
        };
        assert!(close(cost_usd(&pricing, &usage), 3.0 + 1.5 + 0.6 + 1.5));
        assert_eq!(cost_usd(&pricing, &Usage::default()), 0.0);
    }

    #[test]
    fn catalog_prices_win_over_the_builtin_table() {
        let free = price_for(Some(&catalog()), None, "big-pickle").unwrap();
        assert_eq!((free.input, free.output), (0.0, 0.0));
        let mut discounted = catalog();
        discounted.merge_overrides(
            r#"{"anthropic": {"models": {"claude-sonnet-4": {"cost": {"input": 1, "output": 2}}}}}"#,
        )
        .unwrap();
        let listed = price_for(Some(&discounted), None, "anthropic/claude-sonnet-4").unwrap();
        assert_eq!((listed.input, listed.output), (1.0, 2.0));
        assert_eq!(
            builtin_pricing("anthropic/claude-sonnet-4").unwrap().input,
            3.0
        );
    }

    #[test]
    fn the_active_providers_entry_prices_a_shared_model_id() {
        let catalog = catalog();
        let price = |provider: Option<&str>| {
            price_for(Some(&catalog), provider, "gpt-4o").map(|p| (p.input, p.output))
        };
        assert_eq!(price(Some("github-copilot")), Some((0.0, 0.0)));
        assert_eq!(price(Some("openai")), Some((2.5, 10.0)));
        assert_eq!(price(None), Some((2.5, 10.0)));
    }

    #[test]
    fn a_provider_without_the_model_falls_back_to_the_preferred_entry() {
        let catalog = catalog();
        let input = |provider: &str, model: &str| {
            price_for(Some(&catalog), Some(provider), model).map(|p| p.input)
        };
        assert_eq!(
            input("github-copilot", "claude-sonnet-4-5-20250929"),
            Some(3.0)
        );
        assert_eq!(input("openrouter", "gpt-4o"), Some(2.5));
        assert_eq!(input("unlisted-provider", "big-pickle"), Some(0.0));
        assert!(close(
            input("github-copilot", "deepseek/deepseek-chat").unwrap(),
            0.28
        ));
    }

    #[test]
    fn falls_back_to_builtin_prices_then_none() {
        let catalog = catalog();
        let unpriced = price_for(Some(&catalog), None, "moonshotai/kimi-k2");
        assert_eq!(unpriced, None);
        let missing = price_for(Some(&catalog), None, "deepseek/deepseek-chat").unwrap();
        assert!(close(missing.input, 0.28));
        assert_eq!(
            price_for(None, Some("github-copilot"), "anthropic/claude-sonnet-4")
                .unwrap()
                .input,
            3.0
        );
        assert_eq!(price_for(None, None, "totally-unknown"), None);
    }
}
