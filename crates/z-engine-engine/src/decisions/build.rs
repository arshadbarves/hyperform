//! Building a session's `DecisionService` from its settings: the running
//! features, the model connection (the sidecar, `decisions.endpoint` or the
//! native model), calibration, and the dataset. A model that cannot be
//! reached is not an error: the rules provider answers and features keep
//! today's behavior.

use std::sync::Arc;
use std::time::Duration;

use z_engine_config::{DecisionRuntime, DecisionSettings, FeatureGroup, Paths};
use z_engine_decisions::{
    Calibration, DecisionProvider, HybridProvider, QuestionCalibration, RulesProvider,
    SystemOneConfig, SystemOneProvider,
};
use z_engine_store::DecisionDataset;

use super::native;
use super::service::DecisionService;
use super::sidecar::{SidecarEndpoint, SidecarLaunch};
use crate::session::Shared;
use crate::settings::SessionSettings;

/// A model connection and, when the engine started it, its sidecar.
#[derive(Debug)]
pub(crate) struct Connection {
    pub provider: Arc<dyn DecisionProvider>,
    /// Where questions go: the SystemOne URL or the native model's folder.
    pub endpoint: String,
    pub sidecar: Option<SidecarEndpoint>,
}

/// The service plus a warning when the model is configured but unusable.
pub(crate) async fn build_service(
    shared: &Shared,
    settings: &SessionSettings,
) -> (DecisionService, Option<String>) {
    let mut running = settings.settings.running_features();
    running.retain(|(id, _)| id.spec().group == FeatureGroup::Decisions);
    if running.is_empty() {
        return (DecisionService::off(), None);
    }
    let decisions = &settings.settings.decisions;
    let dataset = decisions
        .record_dataset
        .then(|| DecisionDataset::new(dataset_dir(&shared.paths)));
    let timeout = Duration::from_millis(decisions.timeout_ms);
    let modes = running.into_iter().collect();
    match connect(shared, settings, timeout).await {
        Ok(connection) => {
            let hybrid = HybridProvider::new(connection.provider, calibration(decisions));
            let service = DecisionService::new(modes, Arc::new(hybrid), dataset, timeout);
            (service, None)
        }
        Err(reason) => {
            let warning = format!(
                "The decision model is not used: {reason}. Experimental decisions keep today's \
                 behavior."
            );
            let service = DecisionService::new(modes, Arc::new(RulesProvider), dataset, timeout);
            (service, Some(warning))
        }
    }
}

/// Connects to the configured model, starting the sidecar when one is set
/// and loading the native model in the background when it is chosen.
pub(crate) async fn connect(
    shared: &Shared,
    settings: &SessionSettings,
    timeout: Duration,
) -> Result<Connection, String> {
    let decisions = &settings.settings.decisions;
    let (endpoint, api_key, sidecar) = match decisions.runtime {
        DecisionRuntime::Sidecar => match &decisions.sidecar.command {
            Some(command) => {
                let launch = SidecarLaunch {
                    command: command.clone(),
                    checkpoint: decisions.checkpoint.clone(),
                    max_len: decisions.max_len,
                };
                let cwd = &shared.paths.data_dir;
                let started = shared
                    .sidecars
                    .ensure(&launch, cwd, &settings.shell, &settings.env)
                    .await
                    .map_err(|error| format!("its sidecar did not start ({error})"))?;
                let key = Some(started.api_key.clone());
                (started.endpoint.clone(), key, Some(started))
            }
            None => (decisions.endpoint.clone(), api_key(decisions)?, None),
        },
        DecisionRuntime::Native => return native::connect(shared, decisions, timeout).await,
    };
    let config = SystemOneConfig {
        endpoint,
        api_key,
        checkpoint: decisions.checkpoint.clone(),
        max_len: decisions.max_len,
        timeout,
        allow_remote: decisions.allow_remote,
        max_batch: decisions.max_batch as usize,
    };
    let provider = SystemOneProvider::new(config).map_err(|error| error.to_string())?;
    Ok(Connection {
        endpoint: provider.endpoint().to_string(),
        provider: Arc::new(provider),
        sidecar,
    })
}

fn api_key(decisions: &DecisionSettings) -> Result<Option<String>, String> {
    let Some(name) = &decisions.api_key_env else {
        return Ok(None);
    };
    match std::env::var(name) {
        Ok(key) if !key.trim().is_empty() => Ok(Some(key.trim().to_string())),
        _ => Err(format!(
            "decisions.api_key_env names {name}, which is not set"
        )),
    }
}

fn calibration(decisions: &DecisionSettings) -> Calibration {
    let questions = decisions.calibration.iter().map(|(question, entry)| {
        let calibration = QuestionCalibration {
            temperature: entry.temperature,
            threshold: entry.threshold,
        };
        (question.clone(), calibration)
    });
    let mut calibration = Calibration::new(decisions.threshold);
    calibration.questions.extend(questions);
    calibration
}

/// `<data dir>/decisions/dataset/`.
pub(crate) fn dataset_dir(paths: &Paths) -> std::path::PathBuf {
    paths.data_dir.join("decisions").join("dataset")
}

#[cfg(test)]
mod tests {
    use z_engine_config::CalibrationEntry;

    use super::*;

    #[test]
    fn calibration_comes_from_settings() {
        let mut decisions = DecisionSettings {
            threshold: 0.7,
            ..DecisionSettings::default()
        };
        let entry = CalibrationEntry {
            temperature: 2.0,
            threshold: Some(0.9),
        };
        decisions.calibration.insert("relevant".into(), entry);
        let calibration = calibration(&decisions);
        assert_eq!(calibration.threshold_for("relevant"), 0.9);
        assert_eq!(calibration.threshold_for("other"), 0.7);
    }

    #[test]
    fn a_missing_key_variable_is_reported() {
        let decisions = DecisionSettings {
            api_key_env: Some("HYPERFORM_TEST_SURELY_UNSET_KEY".into()),
            ..DecisionSettings::default()
        };
        let error = api_key(&decisions).unwrap_err();
        assert!(error.contains("HYPERFORM_TEST_SURELY_UNSET_KEY"));
        assert_eq!(api_key(&DecisionSettings::default()), Ok(None));
    }
}
