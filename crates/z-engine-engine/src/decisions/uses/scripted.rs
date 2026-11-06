//! Test support for uses: a provider answering each question by name from
//! a script (confidence 0.99; unscripted questions come back unsure), a
//! session, and installing one feature in a chosen mode.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use z_engine_config::{EnvOverrides, FeatureId, FeatureMode, Paths};
use z_engine_decisions::{
    Answer, Calibration, DecisionError, DecisionProvider, DecisionRecord, DecisionRequest,
    HybridProvider, Verdict,
};
use z_engine_host::WebClient;
use z_engine_protocol::{Event, EventEnvelope};
use z_engine_store::SessionStore;

use crate::EventSink;
use crate::decisions::service::DecisionService;
use crate::decisions::sidecar::Sidecars;
use crate::run::{AgentSpec, RunContext};
use crate::session::{SessionHandle, Shared, open_session};

#[derive(Debug, Clone, Default)]
pub(crate) struct Scripted {
    answers: BTreeMap<String, Verdict>,
    down: bool,
}

impl Scripted {
    /// Every request fails as if the model were unreachable.
    pub(crate) fn down() -> Self {
        Self {
            down: true,
            ..Self::default()
        }
    }

    pub(crate) fn yes(self, question: &str, yes: bool) -> Self {
        self.answer(question, Verdict::YesNo(yes))
    }

    pub(crate) fn choice(self, question: &str, key: &str) -> Self {
        self.answer(question, Verdict::Choice(key.to_string()))
    }

    fn answer(mut self, question: &str, verdict: Verdict) -> Self {
        self.answers.insert(question.to_string(), verdict);
        self
    }
}

#[async_trait]
impl DecisionProvider for Scripted {
    fn name(&self) -> &'static str {
        "scripted"
    }

    fn revision(&self) -> String {
        format!("{:?}", self.answers)
    }

    async fn decide(
        &self,
        request: &DecisionRequest,
        _cancel: &CancellationToken,
    ) -> Result<Vec<Answer>, DecisionError> {
        if self.down {
            return Err(DecisionError::Unavailable("refused".into()));
        }
        let answers = request.questions.iter().map(|question| {
            let proposal = self.answers.get(&question.name).cloned();
            Answer {
                question: question.name.clone(),
                confidence: Some(if proposal.is_some() { 0.99 } else { 0.4 }),
                proposal: proposal.or(Some(Verdict::YesNo(false))),
                probabilities: BTreeMap::new(),
                abstain: None,
                provider: "scripted".into(),
                latency_ms: 1,
                cached: false,
            }
        });
        Ok(answers.collect())
    }
}

/// Every event a test session emitted.
pub(crate) type Events = Arc<Mutex<Vec<Event>>>;

/// Texts of the notices among `events`.
pub(crate) fn notices(events: &Events) -> Vec<String> {
    let events = events.lock().unwrap();
    let notices = events.iter().filter_map(|event| match event {
        Event::Notice { text, .. } => Some(text.clone()),
        _ => None,
    });
    notices.collect()
}

/// A session over `<dir>/project` with its own config and data roots.
pub(crate) async fn session(dir: &Path) -> (SessionHandle, Events) {
    session_with(dir, None).await
}

/// `session` with `settings` as the user's `settings.toml`.
pub(crate) async fn session_with(dir: &Path, settings: Option<&str>) -> (SessionHandle, Events) {
    let events: Events = Arc::default();
    let seen = Arc::clone(&events);
    let sink: EventSink = Arc::new(move |envelope: EventEnvelope| {
        seen.lock().unwrap().push(envelope.event);
    });
    let paths = Paths::with_roots(dir.join("config"), dir.join("data"));
    if let Some(settings) = settings {
        std::fs::create_dir_all(dir.join("config")).unwrap();
        std::fs::write(&paths.user_settings_file, settings).unwrap();
    }
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
    let handle = open_session(&shared, sink, &project, None).await.unwrap();
    (handle, events)
}

/// Runs only `feature`, in `mode`, against `model` behind the hybrid gate.
pub(crate) fn install(
    handle: &SessionHandle,
    feature: FeatureId,
    mode: FeatureMode,
    model: Scripted,
) {
    let modes = BTreeMap::from([(feature, mode)]);
    let provider = Arc::new(HybridProvider::new(Arc::new(model), Calibration::new(0.8)));
    let service = DecisionService::new(modes, provider, None, Duration::from_millis(250));
    handle.core.decisions.replace(service);
}

/// The main agent's run context in `handle`.
pub(crate) fn main_run(handle: &SessionHandle) -> RunContext {
    let core = Arc::clone(&handle.core);
    let spec = AgentSpec::main(core.root.clone(), 10);
    RunContext::new(
        core,
        spec,
        handle.core.main.clone(),
        CancellationToken::new(),
    )
}

/// Waits until the trace holds a `wanted` record (shadow work runs in the
/// background).
pub(crate) async fn traced(
    handle: &SessionHandle,
    wanted: impl Fn(&DecisionRecord) -> bool,
) -> bool {
    for _ in 0..200 {
        let records = handle.core.decisions.trace().recent(100);
        if records.iter().any(&wanted) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    false
}
