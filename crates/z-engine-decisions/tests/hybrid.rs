//! The hybrid provider's fallback semantics: every model failure and every
//! low-confidence answer abstains and counts as a fallback; good answers
//! are cached and calibrated.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use serde_json::json;
use tokio_util::sync::CancellationToken;
use z_engine_decisions::{
    AbstainReason, Answer, Calibration, DecisionError, DecisionProvider, DecisionRequest,
    HybridProvider, Question, QuestionCalibration, Verdict,
};

#[derive(Debug)]
enum Script {
    /// Answers yes with this confidence.
    Confident(f64),
    Fail(fn() -> DecisionError),
    /// Answers fewer questions than asked.
    Short,
}

#[derive(Debug)]
struct Scripted {
    script: Script,
    calls: AtomicUsize,
}

impl Scripted {
    fn new(script: Script) -> Arc<Self> {
        Arc::new(Self {
            script,
            calls: AtomicUsize::new(0),
        })
    }
}

#[async_trait]
impl DecisionProvider for Scripted {
    fn name(&self) -> &'static str {
        "scripted"
    }

    fn revision(&self) -> String {
        "scripted-1".into()
    }

    async fn decide(
        &self,
        request: &DecisionRequest,
        _cancel: &CancellationToken,
    ) -> Result<Vec<Answer>, DecisionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let answer = |question: &Question, confidence: f64| Answer {
            question: question.name.clone(),
            proposal: Some(Verdict::YesNo(true)),
            probabilities: [("yes".into(), confidence), ("no".into(), 1.0 - confidence)].into(),
            confidence: Some(confidence),
            abstain: None,
            provider: "scripted".into(),
            latency_ms: 0,
            cached: false,
        };
        match &self.script {
            Script::Confident(confidence) => Ok(request
                .questions
                .iter()
                .map(|q| answer(q, *confidence))
                .collect()),
            Script::Fail(error) => Err(error()),
            Script::Short => Ok(Vec::new()),
        }
    }
}

fn request(text: &str) -> DecisionRequest {
    let question = Question::yes_no("relevant", "Relevant?\n- yes: Needed.\n- no: Not.\n").unwrap();
    DecisionRequest::new(json!({ "text": text })).ask(question)
}

async fn ask(hybrid: &HybridProvider, text: &str) -> Answer {
    let mut answers = hybrid
        .decide(&request(text), &CancellationToken::new())
        .await
        .unwrap();
    answers.remove(0)
}

#[tokio::test]
async fn every_failure_abstains_and_counts_as_a_fallback() {
    let failures: [(Script, AbstainReason); 5] = [
        (
            Script::Fail(|| DecisionError::Timeout(250)),
            AbstainReason::Timeout,
        ),
        (
            Script::Fail(|| DecisionError::Unavailable("down".into())),
            AbstainReason::Unavailable,
        ),
        (
            Script::Fail(|| DecisionError::Malformed("junk".into())),
            AbstainReason::Malformed,
        ),
        (Script::Short, AbstainReason::Malformed),
        (Script::Confident(0.6), AbstainReason::LowConfidence),
    ];
    for (script, reason) in failures {
        let hybrid = HybridProvider::new(Scripted::new(script), Calibration::new(0.8));
        let answer = ask(&hybrid, "x").await;
        assert_eq!(answer.abstain, Some(reason));
        assert_eq!(answer.yes(), None, "{reason:?} must not be acted on");
        assert_eq!(hybrid.fallbacks(), 1, "{reason:?}");
    }
}

#[tokio::test]
async fn confident_answers_are_cached_per_state() {
    let model = Scripted::new(Script::Confident(0.95));
    let hybrid = HybridProvider::new(Arc::clone(&model) as _, Calibration::new(0.8));
    let first = ask(&hybrid, "same").await;
    assert_eq!(first.yes(), Some(true));
    assert!(!first.cached);
    let second = ask(&hybrid, "same").await;
    assert!(second.cached && second.yes() == Some(true));
    ask(&hybrid, "other").await;
    assert_eq!(model.calls.load(Ordering::SeqCst), 2);
    assert_eq!(hybrid.fallbacks(), 0);
}

#[tokio::test]
async fn calibration_can_push_an_answer_below_its_threshold() {
    let soft = QuestionCalibration {
        temperature: 3.0,
        threshold: Some(0.85),
    };
    let calibration = Calibration::new(0.8).with("relevant", soft);
    let hybrid = HybridProvider::new(Scripted::new(Script::Confident(0.95)), calibration);
    let answer = ask(&hybrid, "x").await;
    assert_eq!(answer.abstain, Some(AbstainReason::LowConfidence));
    assert!(answer.confidence.unwrap() < 0.85);
    assert_eq!(
        answer.proposal,
        Some(Verdict::YesNo(true)),
        "the proposal stays for the trace"
    );
}

#[tokio::test]
async fn invalid_requests_abstain_without_asking() {
    let model = Scripted::new(Script::Confident(0.95));
    let hybrid = HybridProvider::new(Arc::clone(&model) as _, Calibration::new(0.8));
    let twice = request("x").ask(request("y").questions.remove(0));
    let answers = hybrid
        .decide(&twice, &CancellationToken::new())
        .await
        .unwrap();
    assert!(
        answers
            .iter()
            .all(|a| a.abstain == Some(AbstainReason::Invalid))
    );
    assert_eq!(model.calls.load(Ordering::SeqCst), 0);
}
