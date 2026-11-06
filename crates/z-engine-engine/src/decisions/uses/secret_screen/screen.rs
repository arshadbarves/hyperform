//! The screening use: detector hits become findings at once; assignments
//! the detectors only suspect go to the decision model, a few per request
//! and only while its endpoint is local.

use async_trait::async_trait;
use futures::future::join_all;
use serde_json::json;
use z_engine_config::FeatureId;
use z_engine_decisions::{Answer, DecisionRecord, DecisionRequest, Question, UNCHANGED};
use z_engine_prompts::decisions::SECRET_CANDIDATE;

use super::{detect, ledger};
use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};
use crate::decisions::seams::{Finding, ResultText};

const QUESTION: &str = "secret_candidate";
const DETECTOR: &str = "secret_detector";
const MODEL_KIND: &str = "possible credential";
/// Assignment lines the model judges per request; the rest stay unflagged.
const MAX_ASKED: usize = 8;
const LINE_CHARS: usize = 300;

#[derive(Debug)]
pub(crate) struct SecretScreen;

pub(crate) static SECRET_SCREEN: SecretScreen = SecretScreen;

#[async_trait]
impl DecisionUse for SecretScreen {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsSecretScreen
    }

    fn seams(&self) -> &'static [Seam] {
        &[Seam::BeforeRequest]
    }

    async fn before_request(&self, cx: &UseContext, results: &[ResultText]) -> Vec<Finding> {
        let mut found = Vec::new();
        let mut candidates = Vec::new();
        for result in results {
            let scan = detect::scan(&result.text);
            for secret in scan.secrets {
                cx.record(detected(cx, &secret.value, secret.kind));
                found.push(finding(result, secret.value, secret.kind));
            }
            candidates.extend(scan.candidates.into_iter().map(|line| (result, line)));
        }
        candidates.retain(|(_, (_, value))| !found.iter().any(|f| f.value.contains(value)));
        if !cx.core.settings().settings.decisions.allow_remote {
            found.extend(judge(cx, candidates).await);
        }
        found
    }
}

/// The candidates the model confidently calls real credentials.
async fn judge(cx: &UseContext, candidates: Vec<(&ResultText, (String, String))>) -> Vec<Finding> {
    let Ok(question) = Question::yes_no(QUESTION, SECRET_CANDIDATE) else {
        return Vec::new();
    };
    let asked: Vec<_> = candidates.into_iter().take(MAX_ASKED).collect();
    let requests: Vec<DecisionRequest> = asked
        .iter()
        .map(|(result, (line, _))| {
            let line: String = line.chars().take(LINE_CHARS).collect();
            let state = json!({ "tool": result.tool, "line": line });
            DecisionRequest::new(state).ask(question.clone())
        })
        .collect();
    let answers = join_all(requests.iter().map(|request| cx.ask_unrecorded(request))).await;
    let mut found = Vec::new();
    for ((result, (_, value)), answers) in asked.into_iter().zip(answers) {
        let print = ledger::fingerprint(&value);
        for answer in &answers {
            let flagged = answer.yes() == Some(true);
            let outcome = if flagged { "flagged" } else { UNCHANGED };
            cx.record(cx.record_of(answer, &print).outcome(outcome));
        }
        if answers.first().and_then(Answer::yes) == Some(true) {
            found.push(finding(result, value, MODEL_KIND));
        }
    }
    found
}

fn finding(result: &ResultText, value: String, kind: &'static str) -> Finding {
    Finding {
        call_id: result.call_id.clone(),
        tool: result.tool.clone(),
        value,
        kind,
    }
}

/// A detector hit as a trace record: the kind and the value's fingerprint.
fn detected(cx: &UseContext, value: &str, kind: &str) -> DecisionRecord {
    DecisionRecord {
        seq: 0,
        at_ms: 0,
        feature: cx.feature.as_str().to_string(),
        question: DETECTOR.to_string(),
        shadow: cx.shadow,
        provider: "rules".to_string(),
        input_fingerprint: ledger::fingerprint(value),
        answer: Some(kind.to_string()),
        confidence: None,
        latency_ms: 0,
        cached: false,
        fallback: None,
        outcome: "flagged".to_string(),
        override_reason: None,
        tokens_saved: None,
    }
}
