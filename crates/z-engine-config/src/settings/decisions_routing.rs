//! `[decisions.routing]`: what `decisions_routing` may choose when a new
//! task starts.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// `[decisions.routing]`
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct DecisionRoutingSettings {
    /// Let a new task (and a subagent whose model is `inherit`) run on
    /// the fast model instead of the main one. Off: only the reasoning
    /// effort is suggested.
    pub allow_model_switch: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_switching_is_off_unless_asked_for() {
        assert!(!DecisionRoutingSettings::default().allow_model_switch);
        let parsed: DecisionRoutingSettings = toml::from_str("allow_model_switch = true").unwrap();
        assert!(parsed.allow_model_switch);
    }
}
