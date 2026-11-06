//! Shared harness for engine integration tests: temp config/data roots, a
//! fixture project, a scripted model injected through `ClientFactory`
//! (side requests routed away from the main script queue), and an event
//! recorder.

// Each test binary uses a different subset of this module.
#![allow(dead_code, unused_imports)]

mod agents;
mod commands;
mod crash;
mod fakes;
mod hooks;
mod processes;
mod transcript;
mod web;
mod work;

use std::sync::Arc;

use z_engine_config::{EnvOverrides, Paths, Settings, TrustStore};
use z_engine_engine::{ClientFactory, Engine, EngineError, EngineOptions};
use z_engine_llm::{ModelClient, ModelRequest};
use z_engine_protocol::{ApprovalRequest, Command, Event, Message, SessionId, TurnRecord};
use z_engine_testkit::{EventRecorder, FixtureRepo, Script, ScriptedModel};

pub use agents::{
    SUBAGENT_NEEDLE, agent_call, first_user_text, is_subagent, route_task, task_requests,
    tool_names, write_agent,
};
pub use commands::user_blocks;
pub use crash::Remains;
pub use fakes::{fake_lsp_bin, fake_mcp_bin, fake_mcp_toml, py_mcp_toml, skip};
pub use hooks::{hook_script, hook_toml};
pub use processes::{PROCESS_WAIT, exec_recording_pid, group_gone, group_running, read_pid};
pub use transcript::{
    all_text, assert_valid_request, assert_valid_transcript, last_user_text, results,
};
pub use web::{WEB_SETTINGS, web_page};
pub use work::{
    dispatch, new_task, parallel_settings, rejections, report, slow, task, tool_reply, update_task,
    work_settings,
};

pub const BASE_SETTINGS: &str = "schema = 2\n\n[model]\nmain = \"test-model\"\n";
pub const TITLE_NEEDLE: &str = "You write the title shown for a coding session";
pub const SUMMARY_NEEDLE: &str = "You summarize a Z Engine coding session";

#[derive(Debug)]
struct Factory(ScriptedModel);

impl ClientFactory for Factory {
    fn build(
        &self,
        _settings: &Settings,
        _api_key: Option<String>,
    ) -> Result<Arc<dyn ModelClient>, EngineError> {
        Ok(Arc::new(self.0.clone()))
    }
}

/// An engine over `paths` answering from `model`, with side requests routed.
fn boot(paths: &Paths, model: &ScriptedModel) -> (Engine, EventRecorder) {
    model.route_system(TITLE_NEEDLE, Script::text("Test session"));
    model.route_system(SUMMARY_NEEDLE, Script::text("Summary of the earlier work."));
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let engine = Engine::new(EngineOptions {
        paths: paths.clone(),
        event_sink: Arc::new(move |envelope| {
            tx.send(envelope).ok();
        }),
        client_factory: Some(Arc::new(Factory(model.clone()))),
        env: EnvOverrides::default(),
    })
    .unwrap();
    (engine, EventRecorder::new(rx))
}

pub struct Builder {
    repo: FixtureRepo,
    settings: String,
    project_settings: Option<String>,
    trusted: bool,
    model: ScriptedModel,
}

impl Builder {
    pub fn settings(mut self, toml: &str) -> Self {
        self.settings = toml.to_string();
        self
    }

    pub fn project_settings(mut self, toml: &str) -> Self {
        self.project_settings = Some(toml.to_string());
        self
    }

    pub fn trusted(mut self) -> Self {
        self.trusted = true;
        self
    }

    /// Scripts queued before the session opens.
    pub fn model(&self) -> &ScriptedModel {
        &self.model
    }

    pub async fn start(self) -> Harness {
        let dirs = tempfile::tempdir().unwrap();
        let paths = Paths::with_roots(dirs.path().join("config"), dirs.path().join("data"));
        std::fs::create_dir_all(&paths.config_dir).unwrap();
        std::fs::write(&paths.user_settings_file, &self.settings).unwrap();
        if let Some(toml) = &self.project_settings {
            self.repo.write(".z-engine/settings.toml", toml);
        }
        if self.trusted {
            let mut trust = TrustStore::default();
            trust.trust(self.repo.path());
            trust.save(&paths.trust_file).unwrap();
        }
        let model = self.model;
        let (engine, mut events) = boot(&paths, &model);
        let session = engine.open_session(self.repo.path(), None).await.unwrap();
        events
            .wait_for(|event| matches!(event, Event::Snapshot { .. }))
            .await;
        Harness {
            engine,
            model,
            events,
            repo: self.repo,
            session,
            paths,
            _dirs: dirs,
        }
    }
}

pub struct Harness {
    pub engine: Engine,
    pub model: ScriptedModel,
    pub events: EventRecorder,
    pub repo: FixtureRepo,
    pub session: SessionId,
    pub paths: Paths,
    _dirs: tempfile::TempDir,
}

impl Harness {
    pub fn builder(repo: FixtureRepo) -> Builder {
        Builder {
            repo,
            settings: BASE_SETTINGS.to_string(),
            project_settings: None,
            trusted: false,
            model: ScriptedModel::new(),
        }
    }

    pub async fn start(repo: FixtureRepo) -> Harness {
        Self::builder(repo).start().await
    }

    pub fn send(&self, command: Command) {
        self.engine.send(&self.session, command).unwrap();
    }

    pub fn submit(&self, text: &str) {
        self.send(Command::Submit {
            text: text.to_string(),
            attachments: Vec::new(),
        });
    }

    /// The next matching event not seen yet.
    pub async fn wait(&mut self, predicate: impl Fn(&Event) -> bool) -> Event {
        self.events.wait_for(predicate).await
    }

    /// A matching event already seen, else the next one.
    pub async fn expect(&mut self, predicate: impl Fn(&Event) -> bool) -> Event {
        self.events.drain();
        if let Some(event) = self.events.seen().iter().find(|event| predicate(event)) {
            return event.clone();
        }
        self.events.wait_for(predicate).await
    }

    pub async fn turn_finished(&mut self) -> TurnRecord {
        match self.events.wait_turn_finished().await {
            Event::TurnFinished { turn } => turn,
            other => panic!("expected TurnFinished, got {other:?}"),
        }
    }

    /// Submits `text` and waits for its turn to finish.
    pub async fn run_turn(&mut self, text: &str) -> TurnRecord {
        self.submit(text);
        self.turn_finished().await
    }

    pub async fn approval(&mut self) -> ApprovalRequest {
        match self
            .wait(|event| matches!(event, Event::ApprovalRequested { .. }))
            .await
        {
            Event::ApprovalRequested { request } => request,
            other => panic!("expected an approval, got {other:?}"),
        }
    }

    /// Requests of the main agent (side requests carry no tools).
    pub fn main_requests(&self) -> Vec<ModelRequest> {
        self.model
            .requests()
            .into_iter()
            .filter(|request| !request.tools.is_empty() && !is_subagent(request))
            .collect()
    }

    /// `relative` under the canonical project root, as the model sees it.
    pub fn path(&self, relative: &str) -> String {
        std::fs::canonicalize(self.repo.path())
            .unwrap()
            .join(relative)
            .to_string_lossy()
            .into_owned()
    }

    pub fn transcript(&self) -> Vec<Message> {
        self.engine
            .agent_transcript(&self.session, &z_engine_protocol::AgentId::main())
            .unwrap()
    }

    /// Closes the session and opens it again from disk.
    pub async fn reopen(&mut self) {
        self.engine.close_session(&self.session).await.unwrap();
        let id = self
            .engine
            .open_session(self.repo.path(), Some(self.session.clone()))
            .await
            .unwrap();
        assert_eq!(id, self.session);
    }
}
