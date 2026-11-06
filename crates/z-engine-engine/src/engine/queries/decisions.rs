//! The Context tab's Decisions section (a session's running features and
//! decision trace) and Settings' "Test connection" for the decision model.

use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use tokio_util::sync::CancellationToken;
use z_engine_config::{DecisionRuntime, FeatureId, FeatureMode};
use z_engine_decisions::{DecisionRecord, DecisionSummary, ProbeReport, probe};
use z_engine_protocol::SessionId;

use crate::decisions::connect;
use crate::engine::Engine;
use crate::settings::load_session_settings;

/// A cold model can take seconds to answer its first question; the probe
/// waits this long and reports the latency against `timeout_ms`.
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningFeature {
    pub id: FeatureId,
    pub title: &'static str,
    pub mode: FeatureMode,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDecisions {
    /// Features in `shadow` or `on` mode for this session.
    pub features: Vec<RunningFeature>,
    /// `hybrid` when a model is connected, else `rules`.
    pub provider: &'static str,
    pub summary: DecisionSummary,
    /// Newest first.
    pub records: Vec<DecisionRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionModelTest {
    #[serde(flatten)]
    pub report: ProbeReport,
    /// Where the probe went; `None` when no connection was made.
    pub endpoint: Option<String>,
    /// The sidecar was started recently, or the native model is still
    /// loading, so the model may not answer yet.
    pub warming_up: bool,
    /// `decisions.timeout_ms`, to compare with the probe's latency.
    pub timeout_ms: u64,
}

impl Engine {
    /// `None` when the session is not open.
    pub fn session_decisions(
        &self,
        session_id: &SessionId,
        limit: usize,
    ) -> Option<SessionDecisions> {
        let handle = self.handle(session_id)?;
        let decisions = &handle.core.decisions;
        let service = decisions.service();
        let features = service
            .running()
            .into_iter()
            .map(|(id, mode)| RunningFeature {
                id,
                title: id.spec().title,
                mode,
            })
            .collect();
        Some(SessionDecisions {
            features,
            provider: service.provider_name(),
            summary: decisions.trace().summary(),
            records: decisions.trace().recent(limit),
        })
    }

    /// Asks the configured decision model one known question, starting its
    /// sidecar when one is set. Uses the effective settings of
    /// `project_root`, or the user's alone when `None`.
    pub async fn test_decision_model(&self, project_root: Option<&Path>) -> DecisionModelTest {
        let shared = self.shared();
        // The config directory is never a project, so it loads the user layer.
        let root = project_root.unwrap_or(&shared.paths.config_dir);
        let settings = load_session_settings(&shared.paths, root, &shared.env).settings;
        let timeout_ms = settings.settings.decisions.timeout_ms;
        let connection = match connect(shared, &settings, PROBE_TIMEOUT).await {
            Ok(connection) => connection,
            Err(error) => {
                let report = ProbeReport {
                    ok: false,
                    latency_ms: 0,
                    answer: None,
                    confidence: None,
                    error: Some(error),
                };
                return DecisionModelTest {
                    report,
                    endpoint: None,
                    warming_up: false,
                    timeout_ms,
                };
            }
        };
        let native = settings.settings.decisions.runtime == DecisionRuntime::Native;
        let loading = native && shared.sidecars.native().wait_loaded(PROBE_TIMEOUT).await;
        let report = probe(connection.provider.as_ref(), &CancellationToken::new()).await;
        let sidecar = connection.sidecar.as_ref();
        DecisionModelTest {
            report,
            endpoint: Some(connection.endpoint),
            warming_up: loading || sidecar.is_some_and(|sidecar| sidecar.warming_up()),
            timeout_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::engine;

    #[tokio::test]
    async fn a_new_session_has_no_decisions() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        let id = engine.open_session(&project, None).await.unwrap();
        let decisions = engine.session_decisions(&id, 50).unwrap();
        assert!(decisions.features.is_empty() && decisions.records.is_empty());
        assert_eq!(decisions.provider, "rules");
        assert_eq!(decisions.summary.count, 0);
        engine.shutdown().await;
    }

    #[tokio::test]
    async fn testing_an_unreachable_model_reports_why() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let settings = "schema = 2\n[decisions]\nendpoint = \"http://127.0.0.1:9\"\n";
        std::fs::write(&engine.paths().user_settings_file, settings).unwrap();
        let test = engine.test_decision_model(None).await;
        assert!(!test.report.ok);
        assert!(test.report.error.unwrap().contains("unreachable"));
        assert_eq!(
            test.endpoint.as_deref(),
            Some("http://127.0.0.1:9/v1/systemone")
        );
        let remote = "schema = 2\n[decisions]\nendpoint = \"https://laya.example.com\"\n";
        std::fs::write(&engine.paths().user_settings_file, remote).unwrap();
        let test = engine.test_decision_model(None).await;
        assert!(test.report.error.unwrap().contains("allow_remote"));
        assert_eq!(test.endpoint, None);
    }
}
