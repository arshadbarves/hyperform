//! Invariants of the seams, with a probe use that acts on every seam when
//! the model says yes: a model that is down, times out, answers garbage or
//! is unsure leaves every seam exactly as with the feature off; shadow
//! mode never waits for the model and never acts.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;
use tokio_util::sync::CancellationToken;
use z_engine_config::{EnvOverrides, FeatureId, FeatureMode, Paths};
use z_engine_context::ClearTarget;
use z_engine_decisions::{
    Answer, Calibration, DecisionError, DecisionProvider, DecisionRequest, HybridProvider,
    Question, Verdict,
};
use z_engine_host::WebClient;
use z_engine_policy::Decision;
use z_engine_protocol::{CallId, Message, ToolResultPart, ToolStatus};
use z_engine_store::SessionStore;

use super::{after_call, pressure, stop, tool_gate, turn_start};
use crate::batch::ToolCall;
use crate::decisions::context::UseContext;
use crate::decisions::registry::{DecisionUse, Seam};
use crate::decisions::seams::ClearAdvice;
use crate::decisions::service::DecisionService;
use crate::decisions::sidecar::Sidecars;
use crate::run::{AgentSpec, RunContext};
use crate::session::{SessionHandle, Shared, open_session};

#[derive(Debug, Clone, Copy)]
enum Model {
    Yes,
    Down,
    TimedOut,
    Garbage,
    Unsure,
    /// Never answers until cancelled.
    Hang,
}

#[async_trait]
impl DecisionProvider for Model {
    fn name(&self) -> &'static str {
        "fake"
    }

    fn revision(&self) -> String {
        format!("{self:?}")
    }

    async fn decide(
        &self,
        request: &DecisionRequest,
        cancel: &CancellationToken,
    ) -> Result<Vec<Answer>, DecisionError> {
        let confidence = match self {
            Self::Yes => 0.99,
            Self::Unsure => 0.55,
            Self::Down => return Err(DecisionError::Unavailable("refused".into())),
            Self::TimedOut => return Err(DecisionError::Timeout(250)),
            Self::Garbage => return Err(DecisionError::Malformed("not json".into())),
            Self::Hang => {
                cancel.cancelled().await;
                return Err(DecisionError::Cancelled);
            }
        };
        let answers = request.questions.iter().map(|question| Answer {
            question: question.name.clone(),
            proposal: Some(Verdict::YesNo(true)),
            probabilities: BTreeMap::new(),
            confidence: Some(confidence),
            abstain: None,
            provider: "fake".into(),
            latency_ms: 1,
            cached: false,
        });
        Ok(answers.collect())
    }
}

/// Acts on every seam when the model says yes.
#[derive(Debug)]
struct Probe;

static PROBE: Probe = Probe;
static USES: &[&dyn DecisionUse] = &[&PROBE];

impl Probe {
    async fn yes(cx: &UseContext, seam: &str) -> bool {
        let question = Question::yes_no(seam, "Act?\n- yes: Act.\n- no: Keep.\n").unwrap();
        let request = DecisionRequest::new(json!({ "seam": seam })).ask(question);
        let answer = cx.ask(&request).await.remove(0);
        cx.record(cx.record_of(&answer, &request.fingerprint()));
        answer.yes() == Some(true)
    }
}

#[async_trait]
impl DecisionUse for Probe {
    fn feature(&self) -> FeatureId {
        FeatureId::DecisionsRisk
    }

    fn seams(&self) -> &'static [Seam] {
        &[
            Seam::TurnStart,
            Seam::Pressure,
            Seam::ToolGate,
            Seam::AfterCall,
            Seam::Stop,
        ]
    }

    async fn turn_start(&self, cx: &UseContext, _text: &str) -> Vec<String> {
        let act = Self::yes(cx, "turn_start").await;
        act.then(|| "probe note".to_string()).into_iter().collect()
    }

    async fn pressure(
        &self,
        cx: &UseContext,
        _: &[Message],
        planned: &[ClearTarget],
    ) -> ClearAdvice {
        let keep = planned
            .iter()
            .map(|target| target.call_id.clone())
            .collect();
        if !Self::yes(cx, "pressure").await {
            return ClearAdvice::default();
        }
        ClearAdvice {
            keep,
            clear: Vec::new(),
        }
    }

    async fn tool_gate(&self, cx: &UseContext, _call: &ToolCall) -> Option<String> {
        Self::yes(cx, "tool_gate")
            .await
            .then(|| "probe asks".to_string())
    }

    async fn after_call(
        &self,
        cx: &UseContext,
        _: &ToolCall,
        _: ToolStatus,
        _: &[ToolResultPart],
    ) -> Vec<String> {
        let act = Self::yes(cx, "after_call").await;
        act.then(|| "probe note".to_string()).into_iter().collect()
    }

    async fn stop(&self, cx: &UseContext, _changed: &[PathBuf]) -> Option<String> {
        Self::yes(cx, "stop")
            .await
            .then(|| "probe continue".to_string())
    }
}

/// What every seam produced for the same inputs.
#[derive(Debug, PartialEq)]
struct Outcome {
    reminders: Vec<String>,
    clears: Vec<ClearTarget>,
    decision: Decision,
    content: Vec<ToolResultPart>,
    stop: Option<String>,
}

async fn session(dir: &Path) -> SessionHandle {
    let paths = Paths::with_roots(dir.join("config"), dir.join("data"));
    paths.ensure().unwrap();
    let shared = Shared {
        store: SessionStore::new(paths.sessions_dir.clone()),
        plans: crate::work::PlanHub::new(paths.plans_dir.clone()),
        paths,
        env: EnvOverrides::default(),
        factory: None,
        catalog: Arc::default(),
        web: WebClient::new().unwrap(),
        sidecars: Sidecars::default(),
    };
    let project = dir.join("project");
    std::fs::create_dir_all(&project).unwrap();
    open_session(&shared, Arc::new(|_| {}), &project, None)
        .await
        .unwrap()
}

fn install(handle: &SessionHandle, model: Model, mode: FeatureMode, timeout_ms: u64) {
    let modes = BTreeMap::from([(FeatureId::DecisionsRisk, mode)]);
    let provider = Arc::new(HybridProvider::new(Arc::new(model), Calibration::new(0.8)));
    let timeout = Duration::from_millis(timeout_ms);
    let service = DecisionService::new(modes, provider, None, timeout);
    handle.core.decisions.replace(service);
}

async fn run_seams(handle: &SessionHandle) -> Outcome {
    let core = Arc::clone(&handle.core);
    let spec = AgentSpec::main(core.root.clone(), 10);
    let ctx = RunContext::new(
        core,
        spec,
        handle.core.main.clone(),
        CancellationToken::new(),
    );
    let call = ToolCall {
        id: CallId::from("call_1"),
        name: "Bash".into(),
        input: json!({ "command": "rm -rf build" }),
    };
    let planned = vec![ClearTarget {
        message: 1,
        block: 0,
        call_id: CallId::from("call_0"),
        chars: 4_000,
    }];
    let allow = Decision::Allow {
        reason: "allow rule".into(),
    };
    let mut content = vec![ToolResultPart::Text { text: "ok".into() }];
    turn_start::at_turn_start_with(USES, &ctx, "clean the build").await;
    let reminders = ctx.core.reminders.take(&ctx.spec.agent_id);
    let clears = pressure::review_clears_with(USES, &ctx, &[], planned).await;
    let decision = tool_gate::review_call_with(USES, &ctx, &call, allow).await;
    after_call::annotate_result_with(USES, &ctx, &call, ToolStatus::Ok, &mut content).await;
    let stop = stop::review_stop_with(USES, &ctx, &[]).await;
    Outcome {
        reminders,
        clears,
        decision,
        content,
        stop,
    }
}

async fn bounded(handle: &SessionHandle) -> Outcome {
    let limit = Duration::from_secs(5);
    tokio::time::timeout(limit, run_seams(handle))
        .await
        .expect("a seam waited on the model")
}

#[tokio::test]
async fn every_model_failure_behaves_exactly_like_off() {
    let dir = tempfile::tempdir().unwrap();
    let handle = session(dir.path()).await;
    install(&handle, Model::Yes, FeatureMode::Off, 250);
    let off = bounded(&handle).await;
    assert!(off.reminders.is_empty() && off.stop.is_none() && off.clears.len() == 1);
    for model in [Model::Down, Model::TimedOut, Model::Garbage, Model::Unsure] {
        install(&handle, model, FeatureMode::On, 250);
        assert_eq!(bounded(&handle).await, off, "{model:?}");
    }
    let records = handle.core.decisions.trace().recent(100);
    assert_eq!(records.len(), 20, "each failure is traced");
    assert!(records.iter().all(|record| record.fallback.is_some()));
    handle.close("test").await;
}

#[tokio::test]
async fn a_confident_model_acts_on_every_seam_when_on() {
    let dir = tempfile::tempdir().unwrap();
    let handle = session(dir.path()).await;
    install(&handle, Model::Yes, FeatureMode::On, 250);
    let on = bounded(&handle).await;
    assert_eq!(on.reminders.len(), 1);
    assert!(on.clears.is_empty(), "the planned clear was kept");
    assert!(matches!(on.decision, Decision::Ask { ref reason, .. } if reason == "probe asks"));
    assert_eq!(on.content.len(), 2, "a note was prepended");
    assert!(on.stop.unwrap().contains("probe continue"));
    handle.close("test").await;
}

#[tokio::test]
async fn shadow_never_waits_and_never_acts() {
    let dir = tempfile::tempdir().unwrap();
    let hanging = session(&dir.path().join("a")).await;
    install(&hanging, Model::Yes, FeatureMode::Off, 250);
    let off = bounded(&hanging).await;
    install(&hanging, Model::Hang, FeatureMode::Shadow, 250);
    assert_eq!(bounded(&hanging).await, off, "a hanging model in shadow");
    hanging.close("test").await;
    let confident = session(&dir.path().join("b")).await;
    install(&confident, Model::Yes, FeatureMode::Shadow, 250);
    assert_eq!(
        bounded(&confident).await,
        off,
        "a confident model in shadow"
    );
    let trace = confident.core.decisions.trace();
    let answered = |records: &[z_engine_decisions::DecisionRecord]| {
        records
            .iter()
            .any(|record| record.answer.as_deref() == Some("yes"))
    };
    for _ in 0..200 {
        if answered(&trace.recent(100)) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let records = trace.recent(100);
    assert!(answered(&records), "shadow records what it would have done");
    assert!(records.iter().all(|record| record.shadow));
    confident.close("test").await;
}

#[tokio::test]
async fn a_hanging_model_is_cut_off_at_the_budget() {
    let dir = tempfile::tempdir().unwrap();
    let handle = session(dir.path()).await;
    install(&handle, Model::Yes, FeatureMode::Off, 50);
    let off = bounded(&handle).await;
    install(&handle, Model::Hang, FeatureMode::On, 50);
    assert_eq!(bounded(&handle).await, off);
    let records = handle.core.decisions.trace().recent(100);
    assert_eq!(records.len(), 5);
    assert!(
        records
            .iter()
            .all(|record| record.fallback.as_deref() == Some("timeout"))
    );
    handle.close("test").await;
}
