//! `[decisions.loop_guard]`: how often `decisions_loop_guard` reminds the
//! model before it asks you.

use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The range `max_reminders` is held to where it is used.
pub const LOOP_GUARD_MAX_REMINDERS: RangeInclusive<u32> = 1..=10;

/// `[decisions.loop_guard]`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct DecisionLoopGuardSettings {
    /// Reminders per turn before the loop guard asks you what to do,
    /// 1 to 10.
    pub max_reminders: u32,
}

impl Default for DecisionLoopGuardSettings {
    fn default() -> Self {
        Self { max_reminders: 3 }
    }
}

impl DecisionLoopGuardSettings {
    /// `max_reminders` held to [`LOOP_GUARD_MAX_REMINDERS`].
    pub fn reminders(&self) -> u32 {
        let range = LOOP_GUARD_MAX_REMINDERS;
        self.max_reminders.clamp(*range.start(), *range.end())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_reminders_by_default_and_held_to_the_range() {
        assert_eq!(DecisionLoopGuardSettings::default().reminders(), 3);
        let parsed: DecisionLoopGuardSettings = toml::from_str("max_reminders = 0").unwrap();
        assert_eq!(parsed.reminders(), 1);
        let parsed: DecisionLoopGuardSettings = toml::from_str("max_reminders = 99").unwrap();
        assert_eq!(parsed.reminders(), 10);
    }
}
