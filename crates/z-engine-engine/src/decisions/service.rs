//! `DecisionService`: what one settings snapshot of a session decides with,
//! the running features and their modes, the provider (hybrid over the
//! model, or rules), and the opt-in dataset. Rebuilt whole on reload.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio_util::sync::CancellationToken;
use z_engine_config::{FeatureId, FeatureMode};
use z_engine_decisions::{Answer, DecisionProvider, DecisionRequest, RulesProvider};
use z_engine_protocol::now_ms;
use z_engine_store::DecisionDataset;

#[derive(Debug)]
pub(crate) struct DecisionService {
    /// Only features whose mode runs (`shadow` or `on`).
    modes: BTreeMap<FeatureId, FeatureMode>,
    provider: Arc<dyn DecisionProvider>,
    dataset: Option<DecisionDataset>,
    /// `decisions.timeout_ms`: how long one answer may take.
    timeout: Duration,
}

impl DecisionService {
    pub(crate) fn new(
        modes: BTreeMap<FeatureId, FeatureMode>,
        provider: Arc<dyn DecisionProvider>,
        dataset: Option<DecisionDataset>,
        timeout: Duration,
    ) -> Self {
        let modes = modes.into_iter().filter(|(_, mode)| mode.runs()).collect();
        Self {
            modes,
            provider,
            dataset,
            timeout,
        }
    }

    /// Nothing runs: every feature is off.
    pub(crate) fn off() -> Self {
        Self::new(
            BTreeMap::new(),
            Arc::new(RulesProvider),
            None,
            Duration::ZERO,
        )
    }

    pub(crate) fn timeout(&self) -> Duration {
        self.timeout
    }

    pub(crate) fn mode(&self, feature: FeatureId) -> FeatureMode {
        self.modes.get(&feature).copied().unwrap_or_default()
    }

    pub(crate) fn running(&self) -> Vec<(FeatureId, FeatureMode)> {
        self.modes.iter().map(|(id, mode)| (*id, *mode)).collect()
    }

    pub(crate) fn provider_name(&self) -> &'static str {
        self.provider.name()
    }

    /// Changes whenever the provider could answer differently (model,
    /// checkpoint, calibration); verdicts kept across seams key on it.
    pub(crate) fn revision(&self) -> String {
        self.provider.revision()
    }

    /// One answer per question; never fails (failures abstain). Recorded
    /// to the dataset when `decisions.record_dataset` is on.
    pub(crate) async fn ask(
        &self,
        feature: FeatureId,
        shadow: bool,
        request: &DecisionRequest,
        cancel: &CancellationToken,
    ) -> Vec<Answer> {
        let answers = self.ask_unrecorded(request, cancel).await;
        if let Some(dataset) = &self.dataset {
            record(dataset, feature, shadow, request, &answers);
        }
        answers
    }

    /// Like `ask`, but never written to the dataset: for states that may
    /// hold credentials.
    pub(crate) async fn ask_unrecorded(
        &self,
        request: &DecisionRequest,
        cancel: &CancellationToken,
    ) -> Vec<Answer> {
        match self.provider.decide(request, cancel).await {
            Ok(answers) => answers,
            Err(error) => {
                tracing::debug!(%error, "decision provider failed; keeping rules");
                let reason = z_engine_decisions::AbstainReason::from_error(&error);
                let name = self.provider.name();
                let all = request.questions.iter();
                all.map(|question| Answer::abstained(&question.name, reason, name))
                    .collect()
            }
        }
    }
}

/// One line per answer: the state (it may hold source text, which is why
/// the dataset is opt-in and stays on this machine), question and answer.
fn record(
    dataset: &DecisionDataset,
    feature: FeatureId,
    shadow: bool,
    request: &DecisionRequest,
    answers: &[Answer],
) {
    for answer in answers {
        let line = json!({
            "atMs": now_ms(),
            "feature": feature.as_str(),
            "shadow": shadow,
            "state": request.state,
            "answer": answer,
        });
        if let Err(error) = dataset.append(&answer.question, &line) {
            tracing::warn!(%error, "could not record a decision to the dataset");
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use z_engine_decisions::{AbstainReason, Question};

    use super::*;

    #[tokio::test]
    async fn off_runs_nothing_and_rules_abstain() {
        let service = DecisionService::off();
        assert!(service.running().is_empty());
        assert_eq!(service.mode(FeatureId::DecisionsRisk), FeatureMode::Off);
        let question = Question::yes_no("q", "Q?\n- yes: y\n- no: n\n").unwrap();
        let request = DecisionRequest::new(json!({})).ask(question);
        let feature = FeatureId::DecisionsRisk;
        let answers = service
            .ask(feature, false, &request, &CancellationToken::new())
            .await;
        assert_eq!(answers[0].abstain, Some(AbstainReason::Rules));
    }

    #[tokio::test]
    async fn the_dataset_gets_one_line_per_answer() {
        let tmp = tempfile::tempdir().unwrap();
        let dataset = DecisionDataset::new(tmp.path());
        let modes = BTreeMap::from([
            (FeatureId::DecisionsRisk, FeatureMode::Shadow),
            (FeatureId::DecisionsHints, FeatureMode::Off),
        ]);
        let provider = Arc::new(RulesProvider);
        let service = DecisionService::new(modes, provider, Some(dataset), Duration::ZERO);
        assert_eq!(
            service.running(),
            [(FeatureId::DecisionsRisk, FeatureMode::Shadow)]
        );
        let question = Question::yes_no("risky", "Risky?\n- yes: y\n- no: n\n").unwrap();
        let request = DecisionRequest::new(json!({ "command": "ls" })).ask(question);
        let cancel = CancellationToken::new();
        service
            .ask(FeatureId::DecisionsRisk, true, &request, &cancel)
            .await;
        let text = std::fs::read_to_string(tmp.path().join("risky.jsonl")).unwrap();
        let line: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        assert_eq!(line["feature"], "decisions_risk");
        assert_eq!(line["state"]["command"], "ls");
        assert_eq!(line["answer"]["abstain"], "rules");
    }
}
