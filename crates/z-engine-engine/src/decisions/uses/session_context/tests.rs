use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use z_engine_config::{FeatureId, FeatureMode};
use z_engine_decisions::{
    Answer, Calibration, DecisionError, DecisionProvider, DecisionRequest, HybridProvider, Verdict,
};
use z_engine_protocol::{MessageId, TurnId, TurnOutcome, TurnRecord, VerificationOutcome};

use crate::decisions::seams::at_turn_start;
use crate::decisions::service::DecisionService;
use crate::decisions::uses::scripted::{main_run, session, traced};
use crate::run::repo_map_files;
use crate::session::SessionHandle;

const REQUEST: &str = "fix the login form validation";

/// Calls a file relevant when its path mentions `login`.
#[derive(Debug)]
struct ByPath {
    down: bool,
}

#[async_trait]
impl DecisionProvider for ByPath {
    fn name(&self) -> &'static str {
        "by-path"
    }

    fn revision(&self) -> String {
        "r1".into()
    }

    async fn decide(
        &self,
        request: &DecisionRequest,
        _cancel: &CancellationToken,
    ) -> Result<Vec<Answer>, DecisionError> {
        if self.down {
            return Err(DecisionError::Unavailable("refused".into()));
        }
        let file = request.state["file"].as_str().unwrap_or_default();
        let answers = request.questions.iter().map(|question| Answer {
            question: question.name.clone(),
            proposal: Some(Verdict::YesNo(file.contains("login"))),
            probabilities: BTreeMap::new(),
            confidence: Some(0.99),
            abstain: None,
            provider: "by-path".into(),
            latency_ms: 1,
            cached: false,
        });
        Ok(answers.collect())
    }
}

fn install(handle: &SessionHandle, mode: FeatureMode, down: bool) {
    let modes = BTreeMap::from([(FeatureId::DecisionsSessionContext, mode)]);
    let model = Arc::new(ByPath { down });
    let provider = Arc::new(HybridProvider::new(model, Calibration::new(0.8)));
    let service = DecisionService::new(modes, provider, None, Duration::from_millis(250));
    handle.core.decisions.replace(service);
}

/// A project whose newest file is unrelated, so it would list first.
async fn project(dir: &Path) -> SessionHandle {
    let src = dir.join("project/src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("login_form.rs"), "pub fn validate() {}\n").unwrap();
    std::thread::sleep(Duration::from_millis(20));
    std::fs::write(src.join("zebra.rs"), "pub fn stripes() {}\n").unwrap();
    session(dir).await.0
}

/// The first file the repository map lists.
async fn first_listed(handle: &SessionHandle) -> String {
    repo_map_files(&handle.core).await;
    let map = handle.core.repo_map.peek().unwrap();
    let header = map.lines().find(|line| line.ends_with(".rs:")).unwrap();
    header.trim_end_matches(':').to_string()
}

#[tokio::test]
async fn on_ranks_relevant_files_first_for_the_first_request() {
    let dir = tempfile::tempdir().unwrap();
    let handle = project(dir.path()).await;
    install(&handle, FeatureMode::On, false);
    at_turn_start(&main_run(&handle), REQUEST).await;
    assert_eq!(first_listed(&handle).await, "src/login_form.rs");
    let records = handle.core.decisions.trace().recent(10);
    assert!(records.iter().any(|r| r.outcome == "ranked first"));
}

#[tokio::test]
async fn later_turns_and_failures_leave_the_map_alone() {
    let dir = tempfile::tempdir().unwrap();
    let handle = project(dir.path()).await;
    let before = first_listed(&handle).await;
    install(&handle, FeatureMode::On, true);
    at_turn_start(&main_run(&handle), REQUEST).await;
    assert_eq!(first_listed(&handle).await, before);
    install(&handle, FeatureMode::On, false);
    handle.core.with_state(|state| {
        state.turns.push(TurnRecord {
            turn_id: TurnId::new(),
            message_id: MessageId::new(),
            outcome: TurnOutcome::Completed,
            verification: VerificationOutcome::NotApplicable,
            usage: Default::default(),
            cost_usd: 0.0,
            started_at: 0,
            finished_at: 0,
        });
    });
    at_turn_start(&main_run(&handle), REQUEST).await;
    assert_eq!(first_listed(&handle).await, before);
    assert!(
        handle
            .core
            .decisions
            .trace()
            .recent(10)
            .iter()
            .all(|r| r.outcome != "ranked first")
    );
}

#[tokio::test]
async fn shadow_records_but_leaves_the_map_alone() {
    let dir = tempfile::tempdir().unwrap();
    let handle = project(dir.path()).await;
    let before = first_listed(&handle).await;
    install(&handle, FeatureMode::Shadow, false);
    at_turn_start(&main_run(&handle), REQUEST).await;
    assert!(traced(&handle, |r| r.shadow && r.outcome == "ranked first").await);
    assert_eq!(first_listed(&handle).await, before);
}

#[tokio::test]
async fn a_served_map_stays_byte_stable() {
    let dir = tempfile::tempdir().unwrap();
    let handle = project(dir.path()).await;
    repo_map_files(&handle.core).await;
    let served = handle.core.repo_map.current().unwrap();
    install(&handle, FeatureMode::On, false);
    at_turn_start(&main_run(&handle), REQUEST).await;
    assert_eq!(handle.core.repo_map.peek().unwrap(), served);
}
