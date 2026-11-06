//! The decision benchmark: the hand-labeled fixtures (always validated)
//! and, when `ZENGINE_DECISIONS_URL` points at a laya-serve, an ignored
//! run comparing rules, the raw model and the hybrid gate per kind. Run it
//! with
//!
//! ```text
//! ZENGINE_DECISIONS_URL=http://127.0.0.1:8000 \
//!   cargo test -p z-engine-decisions --test eval -- --ignored --nocapture
//! ```
//!
//! Optional: `ZENGINE_DECISIONS_KEY` (bearer key), `ZENGINE_DECISIONS_CHECKPOINT`,
//! `ZENGINE_DECISIONS_ALLOW_REMOTE=1`, `ZENGINE_DECISIONS_THRESHOLD` (default
//! 0.8) and `ZENGINE_DECISIONS_CALIBRATION` (`question=temperature[:threshold],...`)
//! to score the hybrid with a fit. The run prints a fitted
//! `[decisions.calibration.<question>]` block per kind to paste into
//! `settings.toml`.

mod eval {
    pub mod fixtures;
    pub mod metrics;
}

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eval::fixtures::{KINDS, Kind, load, options, question};
use eval::metrics::{
    Report, Scored, calibration_toml, fit_temperature, fit_threshold, parse_calibration, report,
};
use tokio_util::sync::CancellationToken;
use z_engine_decisions::{
    Calibration, DecisionProvider, DecisionRequest, HybridProvider, QuestionCalibration,
    RulesProvider, SystemOneConfig, SystemOneProvider,
};

const MIN_EXAMPLES: usize = 10;
/// Shapes of real secrets that must never appear in a fixture.
const SECRET_SHAPES: &[&str] = &["sk-", "AKIA", "ghp_", "xoxb-", "-----BEGIN"];

#[test]
fn fixtures_are_valid_and_balanced() {
    for kind in KINDS {
        let examples = load(kind);
        assert!(
            examples.len() >= MIN_EXAMPLES,
            "{}: too few examples",
            kind.file
        );
        let question = question(kind.question);
        let keys = options(&question);
        let mut ids = BTreeSet::new();
        let mut acting = 0;
        for example in &examples {
            assert!(
                ids.insert(example.id.clone()),
                "{}: duplicate id {}",
                kind.file,
                example.id
            );
            assert_eq!(example.question, kind.question, "{}", example.id);
            assert!(
                keys.contains(&example.label),
                "{}: label {}",
                example.id,
                example.label
            );
            assert!(example.state.is_object(), "{}: state", example.id);
            let text = example.state.to_string();
            for shape in SECRET_SHAPES {
                assert!(!text.contains(shape), "{}: looks like a secret", example.id);
            }
            acting += usize::from(kind.act.contains(&example.label.as_str()));
        }
        assert!(
            acting > 0 && acting < examples.len(),
            "{}: one-sided labels",
            kind.file
        );
    }
}

#[test]
fn the_rules_baseline_abstains_on_every_fixture() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    for kind in KINDS {
        let scored = runtime.block_on(score(&RulesProvider, kind));
        let report = report(&scored, kind.act);
        assert_eq!(report.abstain_rate, 1.0, "{}", kind.file);
        assert_eq!(report.precision, None);
    }
}

#[tokio::test]
#[ignore = "needs a decision model at ZENGINE_DECISIONS_URL"]
async fn eval_against_the_decision_model() {
    let Ok(endpoint) = std::env::var("ZENGINE_DECISIONS_URL") else {
        eprintln!("ZENGINE_DECISIONS_URL is not set; skipping");
        return;
    };
    let env = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
    let config = SystemOneConfig {
        endpoint,
        api_key: env("ZENGINE_DECISIONS_KEY"),
        checkpoint: env("ZENGINE_DECISIONS_CHECKPOINT").unwrap_or_else(|| "bench".into()),
        max_len: 1024,
        timeout: Duration::from_secs(5),
        allow_remote: env("ZENGINE_DECISIONS_ALLOW_REMOTE").as_deref() == Some("1"),
        max_batch: 8,
    };
    let model = Arc::new(SystemOneProvider::new(config).expect("endpoint"));
    let threshold = env("ZENGINE_DECISIONS_THRESHOLD")
        .and_then(|t| t.parse().ok())
        .unwrap_or(0.8);
    let mut calibration = Calibration::new(threshold);
    for (name, temperature, threshold) in
        parse_calibration(&env("ZENGINE_DECISIONS_CALIBRATION").unwrap_or_default())
    {
        calibration = calibration.with(
            &name,
            QuestionCalibration {
                temperature,
                threshold,
            },
        );
    }
    let hybrid = HybridProvider::new(model.clone(), calibration);
    println!(
        "{:<22} {:<7} {:>9} {:>7} {:>6} {:>7} {:>7} {:>8}",
        "kind", "provider", "precision", "recall", "ece", "p50 ms", "p95 ms", "abstain"
    );
    let mut fits = Vec::new();
    for kind in KINDS {
        let rules = score(&RulesProvider, kind).await;
        let raw = score(model.as_ref(), kind).await;
        let gated = score(&hybrid, kind).await;
        for (name, scored) in [("rules", &rules), ("model", &raw), ("hybrid", &gated)] {
            print_row(kind.file, name, &report(scored, kind.act));
        }
        let temperature = fit_temperature(&raw);
        fits.push(calibration_toml(
            kind.question,
            temperature,
            fit_threshold(&raw, temperature, kind.act),
        ));
    }
    println!(
        "\n# Fitted calibration (paste into settings.toml):\n{}",
        fits.join("\n")
    );
}

async fn score(provider: &dyn DecisionProvider, kind: &Kind) -> Vec<Scored> {
    let question = question(kind.question);
    let cancel = CancellationToken::new();
    let mut scored = Vec::new();
    for example in load(kind) {
        let request = DecisionRequest::new(example.state.clone()).ask(question.clone());
        let started = Instant::now();
        let answers = provider.decide(&request, &cancel).await.unwrap_or_default();
        let latency = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        scored.push(Scored::of(&example.label, answers.first(), latency));
    }
    scored
}

fn print_row(kind: &str, provider: &str, report: &Report) {
    let opt = |value: Option<f64>| value.map_or_else(|| "-".to_string(), |v| format!("{v:.2}"));
    println!(
        "{kind:<22} {provider:<7} {:>9} {:>7.2} {:>6} {:>7} {:>7} {:>8.2}",
        opt(report.precision),
        report.recall,
        opt(report.ece),
        report.p50_ms,
        report.p95_ms,
        report.abstain_rate,
    );
}

#[test]
fn metrics_score_precision_recall_and_calibration() {
    let s = |label: &str, predicted: Option<&str>, confidence: f64, ms: u64| Scored {
        label: label.into(),
        predicted: predicted.map(Into::into),
        confidence: predicted.map(|_| confidence),
        latency_ms: ms,
    };
    let scored = vec![
        s("no", Some("no"), 0.9, 10),
        s("no", Some("yes"), 0.6, 20),
        s("yes", Some("no"), 0.9, 30),
        s("yes", Some("yes"), 0.95, 40),
        s("no", None, 0.0, 50),
    ];
    let report = report(&scored, &["no"]);
    assert_eq!(report.precision, Some(0.5));
    assert!((report.recall - 1.0 / 3.0).abs() < 1e-9);
    assert_eq!((report.p50_ms, report.p95_ms), (30, 50));
    assert!((report.abstain_rate - 0.2).abs() < 1e-9);
    assert!(report.ece.unwrap() > 0.0);
    let overconfident: Vec<Scored> = (0..20)
        .map(|i| s("yes", Some(if i % 2 == 0 { "yes" } else { "no" }), 0.99, 1))
        .collect();
    assert!(
        fit_temperature(&overconfident) > 2.0,
        "spread an over-confident model"
    );
    assert_eq!(
        calibration_toml("exchange_needed", 1.7, Some(0.85)),
        "[decisions.calibration.exchange_needed]\ntemperature = 1.7\nthreshold = 0.85\n"
    );
    assert_eq!(
        parse_calibration("a=1.5, b=2.0:0.9, bad"),
        vec![("a".into(), 1.5, None), ("b".into(), 2.0, Some(0.9))]
    );
}
