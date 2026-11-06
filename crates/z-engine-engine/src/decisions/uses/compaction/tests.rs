use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;
use tokio_util::sync::CancellationToken;
use z_engine_config::{FeatureId, FeatureMode};
use z_engine_context::{ClearTarget, plan_microcompact};
use z_engine_decisions::{
    Answer, Calibration, DecisionError, DecisionProvider, DecisionRequest, HybridProvider, Verdict,
};
use z_engine_protocol::{CallId, ContentBlock, Message, Role, ToolResultPart, ToolStatus};

use crate::batch::ToolCall;
use crate::decisions::seams::{annotate_result, review_clears};
use crate::decisions::service::DecisionService;
use crate::decisions::uses::scripted::{main_run, session, traced};
use crate::run::MIN_CLEAR_CHARS;
use crate::session::SessionHandle;

/// Answers "needed" when the output holds `NEEDED`; counts requests.
#[derive(Debug, Default)]
struct ByOutput {
    asked: AtomicUsize,
    down: bool,
}

#[async_trait]
impl DecisionProvider for ByOutput {
    fn name(&self) -> &'static str {
        "by-output"
    }

    fn revision(&self) -> String {
        "r1".into()
    }

    async fn decide(
        &self,
        request: &DecisionRequest,
        _cancel: &CancellationToken,
    ) -> Result<Vec<Answer>, DecisionError> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        if self.down {
            return Err(DecisionError::Unavailable("refused".into()));
        }
        let output = request.state["output"].as_str().unwrap_or_default();
        let answers = request.questions.iter().map(|question| Answer {
            question: question.name.clone(),
            proposal: Some(Verdict::YesNo(output.contains("NEEDED"))),
            probabilities: BTreeMap::new(),
            confidence: Some(0.99),
            abstain: None,
            provider: "by-output".into(),
            latency_ms: 1,
            cached: false,
        });
        Ok(answers.collect())
    }
}

fn install(handle: &SessionHandle, mode: FeatureMode, model: Arc<ByOutput>) {
    let modes = BTreeMap::from([(FeatureId::DecisionsCompaction, mode)]);
    let provider = Arc::new(HybridProvider::new(model, Calibration::new(0.8)));
    let service = DecisionService::new(modes, provider, None, Duration::from_millis(250));
    handle.core.decisions.replace(service);
}

fn round(
    id: &str,
    name: &str,
    input: serde_json::Value,
    output: String,
    error: bool,
) -> [Message; 2] {
    let call = Message::new(
        Role::Assistant,
        vec![ContentBlock::ToolUse {
            id: CallId::from(id),
            name: name.into(),
            input,
        }],
    );
    let result = Message::new(
        Role::User,
        vec![ContentBlock::tool_result(CallId::from(id), output, error)],
    );
    [call, result]
}

fn read(id: &str, path: &str, output: String) -> [Message; 2] {
    round(id, "Read", json!({ "file_path": path }), output, false)
}

/// A request naming auth.rs, then: auth.rs (2,000 chars), a needed file
/// (2,000), a noisy one (500, below today's floor), a large unrelated one
/// (2,000), a failing check (2,000), and eight small recent results.
fn working() -> Vec<Message> {
    let mut working = vec![Message::user_text("Fix the login bug in auth.rs")];
    working.extend(read("auth", "/p/src/auth.rs", "a".repeat(2_000)));
    working.extend(read(
        "needed",
        "/p/src/db.rs",
        format!("NEEDED {}", "d".repeat(2_000)),
    ));
    working.extend(read("noise", "/p/src/noise.rs", "n".repeat(500)));
    working.extend(read("big", "/p/src/big.rs", "b".repeat(2_000)));
    let failing = round(
        "check",
        "Bash",
        json!({"command": "cargo test"}),
        "f".repeat(2_000),
        true,
    );
    working.extend(failing);
    for index in 0..8 {
        working.extend(read(
            &format!("r{index}"),
            &format!("/p/r{index}.rs"),
            "r".into(),
        ));
    }
    working
}

fn planned(working: &[Message]) -> Vec<ClearTarget> {
    plan_microcompact(working, 8, MIN_CLEAR_CHARS)
}

fn ids(targets: &[ClearTarget]) -> Vec<&str> {
    targets
        .iter()
        .map(|target| target.call_id.as_str())
        .collect()
}

#[tokio::test]
async fn off_leaves_todays_plan() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    let working = working();
    let cleared = review_clears(&main_run(&handle), &working, planned(&working)).await;
    assert_eq!(ids(&cleared), ["auth", "needed", "big", "check"]);
}

#[tokio::test]
async fn on_keeps_needed_and_protected_results_and_clears_unrelated_ones() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    install(&handle, FeatureMode::On, Arc::default());
    let working = working();
    let cleared = review_clears(&main_run(&handle), &working, planned(&working)).await;
    assert_eq!(ids(&cleared), ["noise", "big"]);
    let records = handle.core.decisions.trace().recent(50);
    let outcome = |outcome: &str| records.iter().filter(|r| r.outcome == outcome).count();
    assert_eq!(outcome("cleared"), 1, "noise is a new clear");
    assert_eq!(
        outcome("kept"),
        3,
        "needed by the model, auth and check by rule"
    );
    let reasons: Vec<_> = records
        .iter()
        .filter_map(|r| r.override_reason.clone())
        .collect();
    assert!(reasons.contains(&"file named by the user".to_string()));
    assert!(reasons.contains(&"latest failing check output".to_string()));
    let saved: u64 = records.iter().filter_map(|r| r.tokens_saved).sum();
    assert_eq!(saved, 125);
}

#[tokio::test]
async fn shadow_records_but_leaves_todays_plan() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    install(&handle, FeatureMode::Shadow, Arc::default());
    let working = working();
    let cleared = review_clears(&main_run(&handle), &working, planned(&working)).await;
    assert_eq!(ids(&cleared), ["auth", "needed", "big", "check"]);
    assert!(traced(&handle, |r| r.shadow && r.outcome == "cleared").await);
}

#[tokio::test]
async fn an_unavailable_model_leaves_todays_plan() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    let model = Arc::new(ByOutput {
        down: true,
        ..ByOutput::default()
    });
    install(&handle, FeatureMode::On, model);
    let working = working();
    let cleared = review_clears(&main_run(&handle), &working, planned(&working)).await;
    assert_eq!(ids(&cleared), ["auth", "needed", "big", "check"]);
}

#[tokio::test]
async fn verdicts_are_reused_by_later_pressure() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    let model = Arc::new(ByOutput::default());
    install(&handle, FeatureMode::On, Arc::clone(&model));
    let working = working();
    let ctx = main_run(&handle);
    let first = review_clears(&ctx, &working, planned(&working)).await;
    let asked = model.asked.load(Ordering::SeqCst);
    assert_eq!(asked, 3, "auth and check are protected, so never asked");
    let second = review_clears(&ctx, &working, planned(&working)).await;
    assert_eq!(first, second);
    assert_eq!(model.asked.load(Ordering::SeqCst), asked);
}

#[tokio::test]
async fn reading_a_model_cleared_result_again_is_a_reread() {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = session(dir.path()).await;
    install(&handle, FeatureMode::On, Arc::default());
    let working = working();
    let ctx = main_run(&handle);
    review_clears(&ctx, &working, planned(&working)).await;
    let call = ToolCall {
        id: CallId::from("again"),
        name: "Read".into(),
        input: json!({ "file_path": "/p/src/noise.rs", "offset": 1 }),
    };
    let mut content = vec![ToolResultPart::Text { text: "n".into() }];
    annotate_result(&ctx, &call, ToolStatus::Ok, &mut content).await;
    annotate_result(&ctx, &call, ToolStatus::Ok, &mut content).await;
    assert_eq!(content.len(), 1, "no note for the model");
    let records = handle.core.decisions.trace().recent(50);
    let rereads: Vec<_> = records.iter().filter(|r| r.outcome == "reread").collect();
    assert_eq!(rereads.len(), 1);
    assert_eq!(rereads[0].input_fingerprint, "noise");
}
