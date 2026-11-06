//! `[ui]`: presentation preferences.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The pet's name when none is set.
pub const DEFAULT_PET_NAME: &str = "Zen";
/// Longest pet name, in characters.
pub const MAX_PET_NAME_CHARS: usize = 24;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub enum TaskReportView {
    #[default]
    Quiet,
    Compact,
    Detailed,
}

/// How much the pet reacts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub enum CompanionLevel {
    /// Reacts to the agent and to what you do, and roams if `ui.pet.roam`.
    #[default]
    Lively,
    /// Reacts only to the agent and stays in the title bar.
    Calm,
    /// No pet; the status line keeps a small dot.
    Off,
}

/// The pet's body color.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub enum PetLook {
    #[default]
    Pearl,
    Mint,
    Sky,
    Lilac,
    Peach,
    Graphite,
}

/// `[ui.pet]`: the pet's name, its look, and whether it walks around the window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct PetSettings {
    pub name: String,
    pub look: PetLook,
    /// At the lively level, the pet leaves the title bar to sit on the
    /// composer and the panels while nothing needs it there.
    pub roam: bool,
}

impl Default for PetSettings {
    fn default() -> Self {
        Self {
            name: DEFAULT_PET_NAME.to_string(),
            look: PetLook::Pearl,
            roam: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "snake_case")]
#[ts(export, export_to = "config/")]
pub struct UiSettings {
    pub task_report_view: TaskReportView,
    /// Output style name (from `output-styles/`); `None` is the default style.
    pub output_style: Option<String>,
    pub companion: CompanionLevel,
    pub pet: PetSettings,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pet_settings_default_and_round_trip() {
        let ui = UiSettings::default();
        assert_eq!(ui.pet.name, "Zen");
        assert_eq!(ui.pet.look, PetLook::Pearl);
        assert!(ui.pet.roam);

        let parsed: UiSettings = toml::from_str(
            "companion = \"calm\"\n[pet]\nname = \"Pip\"\nlook = \"lilac\"\nroam = false\n",
        )
        .unwrap();
        assert_eq!(parsed.companion, CompanionLevel::Calm);
        assert_eq!(
            parsed.pet,
            PetSettings {
                name: "Pip".into(),
                look: PetLook::Lilac,
                roam: false,
            }
        );
        let text = toml::to_string(&parsed).unwrap();
        assert_eq!(toml::from_str::<UiSettings>(&text).unwrap(), parsed);
    }

    #[test]
    fn a_partial_pet_table_keeps_the_other_defaults() {
        let parsed: UiSettings = toml::from_str("[pet]\nlook = \"mint\"\n").unwrap();
        assert_eq!(parsed.pet.name, DEFAULT_PET_NAME);
        assert_eq!(parsed.pet.look, PetLook::Mint);
        assert!(parsed.pet.roam);
    }
}
