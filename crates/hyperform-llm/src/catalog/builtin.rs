//! Built-in list prices for common model families, used when the models.dev
//! catalog is unavailable. Approximate USD per million tokens; the first
//! matching row wins, so specific variants precede their family.

use super::types::Pricing;

#[derive(Debug, Clone, Copy)]
enum Pattern {
    /// Substring of the lowercased id.
    Has(&'static str),
    /// Prefix of the model name (the id after its last `/`).
    Starts(&'static str),
}

use Pattern::{Has, Starts};

impl Pattern {
    fn matches(self, id: &str, name: &str) -> bool {
        match self {
            Has(needle) => id.contains(needle),
            Starts(prefix) => name.starts_with(prefix),
        }
    }
}

/// Cache prices relative to input: reads cost `input / read_divisor`,
/// writes `input * write_factor`.
#[derive(Debug, Clone, Copy)]
struct CacheRates {
    read_divisor: f64,
    write_factor: f64,
}

/// Anthropic: reads at 0.1x, five-minute writes at 1.25x.
const ANTHROPIC: CacheRates = rates(10.0, 1.25);
/// OpenAI bills no cache writes; the read discount depends on the family.
const OPENAI_HALF: CacheRates = rates(2.0, 1.0);
const OPENAI_QUARTER: CacheRates = rates(4.0, 1.0);
const OPENAI_TENTH: CacheRates = rates(10.0, 1.0);
const GOOGLE: CacheRates = rates(4.0, 1.0);
const GOOGLE_TENTH: CacheRates = rates(10.0, 1.0);
const DEEPSEEK: CacheRates = rates(10.0, 1.0);
/// Hosts of open-weight models commonly bill cached reads at half price.
const OPEN_WEIGHTS: CacheRates = rates(2.0, 1.0);
const NO_DISCOUNT: CacheRates = rates(1.0, 1.0);

const fn rates(read_divisor: f64, write_factor: f64) -> CacheRates {
    CacheRates {
        read_divisor,
        write_factor,
    }
}

/// `(patterns, input, output, cache rates)`.
type Row = (&'static [Pattern], f64, f64, CacheRates);

#[rustfmt::skip]
const TABLE: &[Row] = &[
    (&[Has("opus-4-5"), Has("opus-4.5")], 5.0, 25.0, ANTHROPIC),
    (&[Has("opus")], 15.0, 75.0, ANTHROPIC),
    (&[Has("haiku-4-5"), Has("haiku-4.5")], 1.0, 5.0, ANTHROPIC),
    (&[Has("claude-3-haiku")], 0.25, 1.25, ANTHROPIC),
    (&[Has("haiku")], 0.80, 4.0, ANTHROPIC),
    (&[Has("sonnet")], 3.0, 15.0, ANTHROPIC),
    (&[Has("gpt-5-nano")], 0.05, 0.40, OPENAI_TENTH),
    (&[Has("gpt-5-mini")], 0.25, 2.0, OPENAI_TENTH),
    (&[Has("gpt-5-pro")], 15.0, 120.0, NO_DISCOUNT),
    (&[Has("gpt-5")], 1.25, 10.0, OPENAI_TENTH),
    (&[Has("gpt-4.1-nano")], 0.10, 0.40, OPENAI_QUARTER),
    (&[Has("gpt-4.1-mini")], 0.40, 1.60, OPENAI_QUARTER),
    (&[Has("gpt-4.1")], 2.0, 8.0, OPENAI_QUARTER),
    (&[Has("gpt-4o-mini")], 0.15, 0.60, OPENAI_HALF),
    (&[Has("gpt-4o"), Has("chatgpt")], 2.50, 10.0, OPENAI_HALF),
    (&[Has("gpt-4-turbo")], 10.0, 30.0, NO_DISCOUNT),
    (&[Starts("o1-pro")], 150.0, 600.0, NO_DISCOUNT),
    (&[Starts("o1-mini"), Starts("o3-mini")], 1.10, 4.40, OPENAI_HALF),
    (&[Starts("o1")], 15.0, 60.0, OPENAI_HALF),
    (&[Starts("o3-pro")], 20.0, 80.0, NO_DISCOUNT),
    (&[Starts("o4-mini")], 1.10, 4.40, OPENAI_QUARTER),
    (&[Starts("o3")], 2.0, 8.0, OPENAI_QUARTER),
    (&[Has("gemini-3-pro")], 2.0, 12.0, GOOGLE_TENTH),
    (&[Has("gemini-2.5-pro")], 1.25, 10.0, GOOGLE),
    (&[Has("gemini-2.5-flash-lite")], 0.10, 0.40, GOOGLE),
    (&[Has("gemini-2.5-flash")], 0.30, 2.50, GOOGLE),
    (&[Has("gemini-2.0-flash-lite")], 0.075, 0.30, GOOGLE),
    (&[Has("gemini-2.0-flash")], 0.10, 0.40, GOOGLE),
    (&[Has("gemini-1.5-pro"), Has("gemini-2.0-pro")], 1.25, 5.0, GOOGLE),
    (&[Has("deepseek")], 0.28, 0.42, DEEPSEEK),
    (&[Has("llama-3"), Has("llama3")], 0.60, 0.70, OPEN_WEIGHTS),
    (&[Has("mistral"), Has("mixtral")], 0.50, 1.50, OPEN_WEIGHTS),
];

/// List price for a model id (OpenRouter-style or native), if its family
/// is known.
pub fn builtin_pricing(model_id: &str) -> Option<Pricing> {
    let id = model_id.trim().to_ascii_lowercase();
    let name = id.rsplit('/').next().unwrap_or_default();
    let (_, input, output, cache) = TABLE
        .iter()
        .find(|(patterns, ..)| patterns.iter().any(|pattern| pattern.matches(&id, name)))?;
    Some(Pricing {
        input: *input,
        output: *output,
        cache_read: input / cache.read_divisor,
        cache_write: input * cache.write_factor,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prices(model: &str) -> (f64, f64, f64, f64) {
        let pricing = builtin_pricing(model).unwrap_or_else(|| panic!("no price for {model}"));
        (
            pricing.input,
            pricing.output,
            pricing.cache_read,
            pricing.cache_write,
        )
    }

    #[test]
    fn anthropic_families_price_cache_reads_and_writes() {
        assert_eq!(prices("anthropic/claude-sonnet-4"), (3.0, 15.0, 0.3, 3.75));
        assert_eq!(prices("claude-opus-4-5-20251101"), (5.0, 25.0, 0.5, 6.25));
        assert_eq!(prices("claude-opus-4-1"), (15.0, 75.0, 1.5, 18.75));
        assert_eq!(prices("claude-haiku-4-5"), (1.0, 5.0, 0.1, 1.25));
        assert_eq!(prices("claude-3-5-haiku-20241022").0, 0.80);
        assert_eq!(prices("claude-3-haiku-20240307").0, 0.25);
    }

    #[test]
    fn openai_families_use_documented_cache_discounts() {
        assert_eq!(prices("openai/gpt-4o-mini"), (0.15, 0.60, 0.075, 0.15));
        assert_eq!(prices("gpt-4o-2024-08-06"), (2.50, 10.0, 1.25, 2.50));
        assert_eq!(prices("gpt-4.1"), (2.0, 8.0, 0.5, 2.0));
        assert_eq!(prices("gpt-5"), (1.25, 10.0, 0.125, 1.25));
        assert_eq!(prices("openai/o3-mini").0, 1.10);
        assert_eq!(prices("o3-2025-04-16").0, 2.0);
        assert_eq!(prices("openai/o1").0, 15.0);
        assert_eq!(prices("o4-mini").2, 0.275);
    }

    #[test]
    fn other_families_and_unknowns() {
        assert_eq!(prices("google/gemini-2.5-pro").0, 1.25);
        assert!((prices("deepseek/deepseek-chat").2 - 0.028).abs() < 1e-12);
        assert_eq!(prices("meta-llama/llama-3.3-70b-instruct").1, 0.70);
        assert_eq!(prices("mistralai/mixtral-8x7b").0, 0.50);
        for unknown in [
            "totally-unknown/model",
            "gpt-oss-120b",
            "qwen3-coder",
            "moonshotai/kimi-k2",
            "",
        ] {
            assert_eq!(builtin_pricing(unknown), None, "{unknown}");
        }
    }
}
