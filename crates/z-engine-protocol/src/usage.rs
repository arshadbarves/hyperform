//! Token accounting normalized across providers.
//!
//! `input_tokens` counts only uncached prompt tokens; cache reads/writes are
//! separate so cost can price them. `reasoning_tokens` is a subset of
//! `output_tokens`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Usage {
    #[ts(type = "number")]
    pub input_tokens: u64,
    #[ts(type = "number")]
    pub output_tokens: u64,
    #[ts(type = "number")]
    pub cache_read_tokens: u64,
    #[ts(type = "number")]
    pub cache_write_tokens: u64,
    #[ts(type = "number")]
    pub reasoning_tokens: u64,
}

impl Usage {
    /// Every prompt token the model saw, cached or not.
    pub fn prompt_tokens(&self) -> u64 {
        self.input_tokens + self.cache_read_tokens + self.cache_write_tokens
    }

    pub fn total_tokens(&self) -> u64 {
        self.prompt_tokens() + self.output_tokens
    }

    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

impl std::ops::AddAssign for Usage {
    fn add_assign(&mut self, other: Self) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_read_tokens += other.cache_read_tokens;
        self.cache_write_tokens += other.cache_write_tokens;
        self.reasoning_tokens += other.reasoning_tokens;
    }
}

impl std::ops::Add for Usage {
    type Output = Self;

    fn add(mut self, other: Self) -> Self {
        self += other;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_tokens_include_cache_traffic() {
        let usage = Usage {
            input_tokens: 10,
            output_tokens: 5,
            cache_read_tokens: 100,
            cache_write_tokens: 7,
            reasoning_tokens: 2,
        };
        assert_eq!(usage.prompt_tokens(), 117);
        assert_eq!(usage.total_tokens(), 122);
        assert_eq!((usage + usage).cache_read_tokens, 200);
    }
}
