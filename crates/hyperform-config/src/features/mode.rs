//! How far an experimental feature may go: not at all, measure only, or act.

use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub enum FeatureMode {
    /// Today's behavior; the feature does nothing.
    #[default]
    Off,
    /// The feature runs and records what it would have done, but never acts.
    Shadow,
    /// The feature acts.
    On,
}

impl FeatureMode {
    pub fn parse(raw: &str) -> Option<FeatureMode> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "off" | "false" => Some(Self::Off),
            "shadow" => Some(Self::Shadow),
            "on" | "true" => Some(Self::On),
            _ => None,
        }
    }

    /// The feature runs at all (shadow or on).
    pub fn runs(self) -> bool {
        self != Self::Off
    }

    /// The feature may change what the engine does.
    pub fn acts(self) -> bool {
        self == Self::On
    }
}

/// Accepts `"off" | "shadow" | "on"` in any case, and `true` / `false`.
impl<'de> Deserialize<'de> for FeatureMode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ModeVisitor;

        impl Visitor<'_> for ModeVisitor {
            type Value = FeatureMode;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("\"off\", \"shadow\" or \"on\"")
            }

            fn visit_bool<E: de::Error>(self, value: bool) -> Result<FeatureMode, E> {
                Ok(if value {
                    FeatureMode::On
                } else {
                    FeatureMode::Off
                })
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<FeatureMode, E> {
                FeatureMode::parse(value)
                    .ok_or_else(|| E::custom(format!("unknown feature mode `{value}`")))
            }
        }

        deserializer.deserialize_any(ModeVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize)]
    struct Row {
        mode: FeatureMode,
    }

    fn parse(toml: &str) -> Result<FeatureMode, toml::de::Error> {
        toml::from_str::<Row>(toml).map(|row| row.mode)
    }

    #[test]
    fn accepts_names_and_booleans() {
        assert_eq!(parse("mode = \"shadow\"").unwrap(), FeatureMode::Shadow);
        assert_eq!(parse("mode = \"ON\"").unwrap(), FeatureMode::On);
        assert_eq!(parse("mode = false").unwrap(), FeatureMode::Off);
        assert_eq!(parse("mode = true").unwrap(), FeatureMode::On);
        assert!(parse("mode = \"maybe\"").is_err());
        assert_eq!(serde_json::to_value(FeatureMode::Shadow).unwrap(), "shadow");
    }

    #[test]
    fn only_on_acts() {
        assert!(!FeatureMode::Off.runs());
        assert!(FeatureMode::Shadow.runs() && !FeatureMode::Shadow.acts());
        assert!(FeatureMode::On.acts());
    }
}
