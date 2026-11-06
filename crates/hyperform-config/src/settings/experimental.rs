//! `[experimental]`: one mode per feature id. `Settings::feature` is the
//! only way code asks whether a feature runs.

use super::Settings;
use crate::features::{FeatureId, FeatureMode, FeatureSpec, FeatureStage};

impl Settings {
    /// The effective mode of `id`. Stable features are always on; features
    /// not built in this version, `shadow` on a feature without shadow
    /// support, and features whose prerequisite does not run are off;
    /// otherwise the configured mode, default off.
    pub fn feature(&self, id: FeatureId) -> FeatureMode {
        let spec = id.spec();
        let mode = resolve(spec, self.experimental.get(id.as_str()).copied()).0;
        match spec.requires {
            Some(base) if mode.runs() && !self.feature(base).runs() => FeatureMode::Off,
            _ => mode,
        }
    }

    /// Every feature that runs (shadow or on), in registry order.
    pub fn running_features(&self) -> Vec<(FeatureId, FeatureMode)> {
        FeatureId::ALL
            .into_iter()
            .map(|id| (id, self.feature(id)))
            .filter(|(_, mode)| mode.runs())
            .collect()
    }
}

/// The mode a feature runs in, and why a configured value is not used.
fn resolve(
    spec: &FeatureSpec,
    configured: Option<FeatureMode>,
) -> (FeatureMode, Option<&'static str>) {
    match (spec.stage, configured) {
        (FeatureStage::Stable, None) => (FeatureMode::On, None),
        (FeatureStage::Stable, Some(_)) => (
            FeatureMode::On,
            Some("is stable and always on; the setting is ignored"),
        ),
        (FeatureStage::Experimental, None) => (FeatureMode::Off, None),
        (FeatureStage::Experimental, Some(mode)) if mode.runs() && !spec.available => (
            FeatureMode::Off,
            Some("is not available in this version; it stays off"),
        ),
        (FeatureStage::Experimental, Some(FeatureMode::Shadow)) if !spec.supports_shadow => (
            FeatureMode::Off,
            Some("does not support shadow mode; it stays off"),
        ),
        (FeatureStage::Experimental, Some(mode)) => (mode, None),
    }
}

/// Drops unknown ids (each layer already reported them as unknown keys) and
/// entries whose value is not used, with a warning for the latter. A
/// feature waiting on its prerequisite keeps its entry and is warned about.
pub(super) fn normalize_experimental(settings: &mut Settings, w: &mut Vec<String>) {
    settings.experimental.retain(|key, mode| {
        let Some(id) = FeatureId::parse(key) else {
            return false;
        };
        match resolve(id.spec(), Some(*mode)) {
            (_, Some(problem)) => {
                w.push(format!("experimental.{key} {problem}"));
                false
            }
            (_, None) => true,
        }
    });
    for id in FeatureId::ALL {
        let configured = settings.experimental.get(id.as_str()).copied();
        if let Some(base) = id.spec().requires
            && configured.is_some_and(FeatureMode::runs)
            && !settings.feature(base).runs()
        {
            w.push(format!(
                "experimental.{id} needs {base}; it stays off until {base} is on"
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::FEATURES;

    fn spec() -> FeatureSpec {
        FEATURES[0].available()
    }

    #[test]
    fn experimental_features_default_off_and_take_the_configured_mode() {
        let spec = spec();
        assert_eq!(resolve(&spec, None), (FeatureMode::Off, None));
        assert_eq!(
            resolve(&spec, Some(FeatureMode::Shadow)),
            (FeatureMode::Shadow, None)
        );
        assert_eq!(
            resolve(&spec, Some(FeatureMode::On)),
            (FeatureMode::On, None)
        );
    }

    #[test]
    fn stable_features_are_always_on() {
        let stable = FeatureSpec {
            stage: FeatureStage::Stable,
            ..spec()
        };
        assert_eq!(resolve(&stable, None), (FeatureMode::On, None));
        let (mode, problem) = resolve(&stable, Some(FeatureMode::Off));
        assert_eq!(mode, FeatureMode::On);
        assert!(problem.is_some());
    }

    #[test]
    fn shadow_without_support_and_unbuilt_features_stay_off() {
        let no_shadow = FeatureSpec {
            supports_shadow: false,
            ..spec()
        };
        assert_eq!(
            resolve(&no_shadow, Some(FeatureMode::Shadow)).0,
            FeatureMode::Off
        );
        assert_eq!(
            resolve(&no_shadow, Some(FeatureMode::On)).0,
            FeatureMode::On
        );
        let unbuilt = FeatureSpec {
            available: false,
            ..FEATURES[0]
        };
        let (mode, problem) = resolve(&unbuilt, Some(FeatureMode::On));
        assert_eq!(mode, FeatureMode::Off);
        assert!(problem.is_some());
        assert_eq!(
            resolve(&unbuilt, Some(FeatureMode::Off)),
            (FeatureMode::Off, None)
        );
    }

    #[test]
    fn normalize_drops_unknown_and_unused_entries() {
        let mut settings = Settings::default();
        settings
            .experimental
            .insert("decisions_nope".into(), FeatureMode::On);
        settings
            .experimental
            .insert("decisions_risk".into(), FeatureMode::Off);
        let mut warnings = Vec::new();
        normalize_experimental(&mut settings, &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(settings.experimental.len(), 1);
        assert_eq!(settings.feature(FeatureId::DecisionsRisk), FeatureMode::Off);
        assert!(settings.running_features().is_empty());
    }

    #[test]
    fn a_feature_stays_off_until_its_prerequisite_runs() {
        let mut settings = Settings::default();
        settings
            .experimental
            .insert("work_parallel".into(), FeatureMode::On);
        let mut warnings = Vec::new();
        normalize_experimental(&mut settings, &mut warnings);
        assert_eq!(settings.feature(FeatureId::WorkParallel), FeatureMode::Off);
        assert_eq!(
            warnings,
            ["experimental.work_parallel needs work_plans; it stays off until work_plans is on"]
        );
        assert_eq!(settings.experimental.len(), 1, "the entry is kept");
        settings
            .experimental
            .insert("work_plans".into(), FeatureMode::On);
        warnings.clear();
        normalize_experimental(&mut settings, &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(settings.feature(FeatureId::WorkParallel), FeatureMode::On);
        assert_eq!(
            settings.running_features(),
            [
                (FeatureId::WorkPlans, FeatureMode::On),
                (FeatureId::WorkParallel, FeatureMode::On)
            ]
        );
    }

    #[test]
    fn the_toml_table_maps_ids_to_modes() {
        let settings: Settings =
            toml::from_str("[experimental]\ndecisions_compaction = \"shadow\"\n").unwrap();
        assert_eq!(
            settings.experimental.get("decisions_compaction"),
            Some(&FeatureMode::Shadow)
        );
    }
}
