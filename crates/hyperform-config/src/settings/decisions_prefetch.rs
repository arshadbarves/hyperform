//! `[decisions.prefetch]`: how much `decisions_prefetch` may attach to the
//! opening message of a task.

use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const PREFETCH_MAX_FILES: RangeInclusive<u32> = 1..=10;
pub const PREFETCH_MAX_TOKENS: RangeInclusive<u32> = 500..=50_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct DecisionPrefetchSettings {
    /// Most files attached at one task start.
    pub max_files: u32,
    /// Estimated tokens all attached files may take together; a file that
    /// does not fit is skipped.
    pub max_tokens: u32,
}

impl Default for DecisionPrefetchSettings {
    fn default() -> Self {
        Self {
            max_files: 3,
            max_tokens: 6_000,
        }
    }
}

/// Clamps both limits into range, warning about each change.
pub(super) fn normalize_prefetch(p: &mut DecisionPrefetchSettings, w: &mut Vec<String>) {
    for (key, value, range) in [
        ("max_files", &mut p.max_files, PREFETCH_MAX_FILES),
        ("max_tokens", &mut p.max_tokens, PREFETCH_MAX_TOKENS),
    ] {
        let clamped = (*value).clamp(*range.start(), *range.end());
        if clamped != *value {
            w.push(format!(
                "decisions.prefetch.{key} = {value} is out of range; using {clamped}"
            ));
            *value = clamped;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_tables_keep_defaults_and_limits_are_clamped() {
        let parsed: DecisionPrefetchSettings = toml::from_str("max_files = 5\n").unwrap();
        assert_eq!((parsed.max_files, parsed.max_tokens), (5, 6_000));
        let mut p = DecisionPrefetchSettings {
            max_files: 0,
            max_tokens: 1_000_000,
        };
        let mut w = Vec::new();
        normalize_prefetch(&mut p, &mut w);
        assert_eq!((p.max_files, p.max_tokens), (1, 50_000));
        assert_eq!(w.len(), 2, "{w:?}");
        let mut defaults = DecisionPrefetchSettings::default();
        normalize_prefetch(&mut defaults, &mut w);
        assert_eq!(w.len(), 2, "defaults need no adjustment");
    }
}
