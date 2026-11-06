//! Settings' "Test connection": one known question, asked directly of the
//! model so its error is shown rather than hidden behind a fallback.

use std::time::Instant;

use serde::Serialize;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::provider::DecisionProvider;
use crate::question::{DecisionRequest, Question};

const PROBE: &str = "connection_probe";
const GREETING: &str = "Hello there, good morning!";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeReport {
    /// The server answered the probe with a usable answer.
    pub ok: bool,
    pub latency_ms: u64,
    /// The proposal label; a greeting should be answered `yes`.
    pub answer: Option<String>,
    pub confidence: Option<f64>,
    pub error: Option<String>,
}

pub async fn probe(provider: &dyn DecisionProvider, cancel: &CancellationToken) -> ProbeReport {
    let started = Instant::now();
    let question = match Question::yes_no(PROBE, z_engine_prompts::decisions::CONNECTION_PROBE) {
        Ok(question) => question,
        Err(error) => return failed(0, error.to_string()),
    };
    let request = DecisionRequest::new(json!({ "text": GREETING })).ask(question);
    let reply = provider.decide(&request, cancel).await;
    let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let answer = match reply {
        Ok(mut answers) if answers.len() == 1 => answers.remove(0),
        Ok(_) => return failed(latency_ms, "the server answered the wrong questions".into()),
        Err(error) => return failed(latency_ms, error.to_string()),
    };
    let label = answer.proposal.as_ref().map(|verdict| verdict.label());
    let error = answer
        .abstain
        .map(|reason| format!("the server's answer was not usable ({})", reason.label()));
    ProbeReport {
        ok: error.is_none(),
        latency_ms,
        answer: label,
        confidence: answer.confidence,
        error,
    }
}

fn failed(latency_ms: u64, error: String) -> ProbeReport {
    ProbeReport {
        ok: false,
        latency_ms,
        answer: None,
        confidence: None,
        error: Some(error),
    }
}
