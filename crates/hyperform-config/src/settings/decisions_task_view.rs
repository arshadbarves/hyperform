//! `[decisions.task_view]`: what `decisions_task_view` always keeps when a
//! new task sets earlier exchanges aside.

use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The range `keep_recent_exchanges` is held to where it is used.
pub const TASK_VIEW_KEEP_RECENT: RangeInclusive<u32> = 1..=20;

/// `[decisions.task_view]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct DecisionTaskViewSettings {
    /// The newest exchanges before a new task that always stay in view,
    /// 1 to 20.
    pub keep_recent_exchanges: u32,
}

impl Default for DecisionTaskViewSettings {
    fn default() -> Self {
        Self {
            keep_recent_exchanges: 2,
        }
    }
}

impl DecisionTaskViewSettings {
    /// `keep_recent_exchanges` held to [`TASK_VIEW_KEEP_RECENT`].
    pub fn keep_recent(&self) -> usize {
        let range = TASK_VIEW_KEEP_RECENT;
        self.keep_recent_exchanges
            .clamp(*range.start(), *range.end()) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_exchanges_by_default_and_held_to_the_range() {
        assert_eq!(DecisionTaskViewSettings::default().keep_recent(), 2);
        let parsed: DecisionTaskViewSettings = toml::from_str("keep_recent_exchanges = 0").unwrap();
        assert_eq!(parsed.keep_recent(), 1);
        let parsed: DecisionTaskViewSettings =
            toml::from_str("keep_recent_exchanges = 99").unwrap();
        assert_eq!(parsed.keep_recent(), 20);
    }
}
