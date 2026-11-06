//! The native runtime on a real downloaded model, against Laya itself.
//! Ignored: they need the model files (and laya-serve for the second test).
//!
//! ```text
//! ZENGINE_LAYA_MODEL_DIR=<data dir>/models/laya/<revision> \
//!   cargo test -p z-engine-decisions --features onnx --test onnx_parity -- --ignored
//! ```
//!
//! `ZENGINE_LAYA_CHECKPOINT` names the downloaded checkpoint (default
//! `multilingual`). The first test compares with `answers_<checkpoint>.json`,
//! Laya's own ONNXAgent on the same files (`generate_answers.py`); the second
//! asks the same questions of laya-serve at `ZENGINE_DECISIONS_URL` (optional
//! bearer `ZENGINE_DECISIONS_KEY`) through the SystemOne provider, so the wire
//! (`model`/`max_len`, option order, score scale) is checked against native.
#![cfg(feature = "onnx")]

use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Map, Value};
use tokio_util::sync::CancellationToken;
use z_engine_decisions::{
    Answer, Criterion, DecisionProvider, DecisionRequest, Form, OnnxConfig, OnnxProvider, Question,
    SystemOneConfig, SystemOneProvider, Verdict, native_model,
};

/// Same graph and inputs, a different ONNX Runtime build.
const ONNX_TOLERANCE: f64 = 2e-3;
/// laya-serve runs the PyTorch weights, not the ONNX export.
const SERVE_TOLERANCE: f64 = 0.02;

#[derive(Deserialize)]
struct Fixture {
    checkpoint: String,
    max_len: u32,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    state: Value,
    question: FixtureQuestion,
    answer: Map<String, Value>,
}

#[derive(Deserialize)]
struct FixtureQuestion {
    form: String,
    instructions: String,
    options: Vec<(String, String)>,
}

impl FixtureQuestion {
    fn question(&self) -> Question {
        let criteria = || {
            let options = self.options.iter().map(|(key, description)| Criterion {
                key: key.clone(),
                description: description.clone(),
            });
            options.collect()
        };
        let form = match self.form.as_str() {
            "yes_no" => Form::YesNo {
                yes: self.options[0].1.clone(),
                no: self.options[1].1.clone(),
            },
            "score" => Form::Score(criteria()),
            _ => Form::Choice(criteria()),
        };
        Question {
            name: "q".into(),
            instructions: self.instructions.clone(),
            form,
        }
    }
}

fn checkpoint() -> String {
    std::env::var("ZENGINE_LAYA_CHECKPOINT").unwrap_or_else(|_| "multilingual".into())
}

fn fixture() -> Fixture {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/onnx/answers_{}.json", checkpoint()));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_str(&text).unwrap()
}

async fn native(fixture: &Fixture) -> Option<OnnxProvider> {
    let Ok(dir) = std::env::var("ZENGINE_LAYA_MODEL_DIR") else {
        eprintln!("ZENGINE_LAYA_MODEL_DIR is not set; skipped");
        return None;
    };
    let model = native_model(&fixture.checkpoint).expect("a pinned checkpoint");
    let config = OnnxConfig::for_model(
        model,
        PathBuf::from(dir),
        fixture.max_len,
        Duration::from_secs(60),
        8,
    );
    let provider = OnnxProvider::open(config);
    provider.ready(Duration::from_secs(300)).await.unwrap();
    Some(provider)
}

async fn ask(provider: &dyn DecisionProvider, case: &Case) -> Answer {
    let request = DecisionRequest::new(case.state.clone()).ask(case.question.question());
    let mut answers = provider
        .decide(&request, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(answers.len(), 1, "{}", case.name);
    answers.remove(0)
}

/// Laya's answer (`a`/`b` for yes/no, level index for a score) against ours.
fn compare(name: &str, question: &Question, answer: &Answer, laya: &Map<String, Value>, tol: f64) {
    assert!(
        answer.abstain.is_none(),
        "{name}: abstained {:?}",
        answer.abstain
    );
    let ours = |key: &str| match (&question.form, key) {
        (Form::YesNo { .. }, "a") => "yes".to_string(),
        (Form::YesNo { .. }, "b") => "no".to_string(),
        (Form::Score(levels), index) => levels[index.parse::<usize>().unwrap()].key.clone(),
        (_, key) => key.to_string(),
    };
    let probabilities = laya["probabilities"].as_object().unwrap();
    for (key, p) in probabilities {
        let got = answer.probabilities[&ours(key)];
        let want = p.as_f64().unwrap();
        assert!(
            (got - want).abs() <= tol,
            "{name}: p[{key}] {got} vs {want}"
        );
    }
    let sum: f64 = answer.probabilities.values().sum();
    assert!(
        (sum - 1.0).abs() < 1e-3,
        "{name}: probabilities sum to {sum}"
    );
    match (answer.proposal.as_ref().unwrap(), &question.form) {
        (Verdict::Score(score), Form::Score(levels)) => {
            let want = laya["score"].as_f64().unwrap() / (levels.len() - 1) as f64;
            assert!(
                (score - want).abs() <= tol,
                "{name}: score {score} vs {want}"
            );
        }
        (verdict, _) => {
            let top = laya["choice"].as_str().unwrap();
            let margin = top_margin(probabilities);
            if margin > 2.0 * tol {
                assert_eq!(verdict.label(), ours(top), "{name}: choice");
            }
        }
    }
}

/// Gap between the two most likely options; a near tie may flip either way.
fn top_margin(probabilities: &Map<String, Value>) -> f64 {
    let mut ps: Vec<f64> = probabilities.values().filter_map(Value::as_f64).collect();
    ps.sort_by(|a, b| b.total_cmp(a));
    ps[0] - ps.get(1).copied().unwrap_or(0.0)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a downloaded model (ZENGINE_LAYA_MODEL_DIR)"]
async fn native_matches_laya_onnx_agent() {
    let fixture = fixture();
    let Some(provider) = native(&fixture).await else {
        return;
    };
    for case in &fixture.cases {
        let answer = ask(&provider, case).await;
        let question = case.question.question();
        compare(&case.name, &question, &answer, &case.answer, ONNX_TOLERANCE);
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a downloaded model and a running laya-serve (ZENGINE_DECISIONS_URL)"]
async fn native_matches_laya_serve() {
    let fixture = fixture();
    let Ok(endpoint) = std::env::var("ZENGINE_DECISIONS_URL") else {
        eprintln!("ZENGINE_DECISIONS_URL is not set; skipped");
        return;
    };
    let Some(native) = native(&fixture).await else {
        return;
    };
    let sidecar = SystemOneProvider::new(SystemOneConfig {
        endpoint,
        api_key: std::env::var("ZENGINE_DECISIONS_KEY").ok(),
        checkpoint: fixture.checkpoint.clone(),
        max_len: fixture.max_len,
        timeout: Duration::from_secs(60),
        allow_remote: true,
        max_batch: 8,
    })
    .unwrap();
    for case in &fixture.cases {
        let ours = ask(&native, case).await;
        let theirs = ask(&sidecar, case).await;
        same_answer(&case.name, &ours, &theirs);
    }
}

/// The native runtime and the SystemOne wire report Laya the same way.
fn same_answer(name: &str, ours: &Answer, theirs: &Answer) {
    assert!(
        theirs.abstain.is_none(),
        "{name}: laya-serve abstained {:?}",
        theirs.abstain
    );
    assert_eq!(
        ours.probabilities.len(),
        theirs.probabilities.len(),
        "{name}: options"
    );
    for (key, p) in &ours.probabilities {
        let q = theirs.probabilities[key];
        assert!(
            (p - q).abs() <= SERVE_TOLERANCE,
            "{name}: p[{key}] {p} vs {q}"
        );
    }
    match (
        ours.proposal.as_ref().unwrap(),
        theirs.proposal.as_ref().unwrap(),
    ) {
        (Verdict::Score(a), Verdict::Score(b)) => {
            assert!((a - b).abs() <= SERVE_TOLERANCE, "{name}: score {a} vs {b}");
        }
        (a, b) => {
            let mut ps: Vec<f64> = ours.probabilities.values().copied().collect();
            ps.sort_by(|x, y| y.total_cmp(x));
            if ps[0] - ps.get(1).copied().unwrap_or(0.0) > 2.0 * SERVE_TOLERANCE {
                assert_eq!(a, b, "{name}: proposal");
            }
        }
    }
}
