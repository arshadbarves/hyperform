//! Opening a session: create a new one or resume a stored one (repairing
//! a crash), assemble its core from the effective settings, start its
//! actor, then announce it with a `Snapshot` and run `SessionStart` hooks.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use serde_json::json;
use tokio_util::sync::CancellationToken;
use z_engine_config::FeatureId;
use z_engine_context::wrap_reminder;
use z_engine_host::PathLocks;
use z_engine_protocol::{AgentId, NoticeLevel, SessionId};
use z_engine_store::{LoadedSession, NewSession, ReplayState};

use super::actor::spawn_actor;
use super::checkpoint::Checkpoints;
use super::handle::SessionHandle;
use super::reload::git_info;
use super::resume::repair;
use super::snapshot::emit_snapshot;
use super::tools::builtin_registry;
use super::trust::request_trust;
use crate::broker::Broker;
use crate::decisions::seams::watch_notices;
use crate::decisions::{DecisionHub, build_service};
use crate::error::EngineError;
use crate::hooks::{HookEvent, HookInput, run_hooks};
use crate::lsp::LspHub;
use crate::mcp::{McpHub, sync_servers};
use crate::options::EventSink;
use crate::orchestration::{AgentRegistry, Orchestra, prune_stale_worktrees};
use crate::run::{RepoMapCache, prebuild_repo_map};
use crate::session::{
    AgentResources, Emitter, JobHub, Journal, ReminderBox, SessionCore, SessionState, Shared,
    StatusTracker,
};
use crate::settings::{
    SessionSettings, build_policy, load_session_settings, models, session_client,
};
use crate::sync::read;
use crate::verify::{CheckHub, ModeVerifier, discover_checks};
use crate::work::{WorkSession, reattach};

type Notices = Vec<(NoticeLevel, String)>;

pub(crate) async fn open_session(
    shared: &Shared,
    sink: EventSink,
    requested_root: &Path,
    id: Option<SessionId>,
) -> Result<SessionHandle, EngineError> {
    let stored = match &id {
        Some(id) => Some(shared.store.load(id)?),
        None => None,
    };
    let root = project_root(requested_root, stored.as_ref()).await?;
    let report = load_session_settings(&shared.paths, &root, &shared.env);
    let mut notices = report.notices;
    let settings = Arc::new(report.settings);
    let session_id = id.unwrap_or_default();
    let events = Arc::new(Emitter::new(session_id.clone(), sink));
    let journal_and_state = match stored {
        Some(loaded) => resume(
            shared,
            &events,
            &session_id,
            loaded,
            &settings,
            &mut notices,
        )?,
        None => create(shared, &events, &session_id, &root, &settings)?,
    };
    let (journal, state, fresh) = journal_and_state;
    let parts = Parts {
        id: session_id,
        events,
        journal,
        root,
        settings,
    };
    let core = assemble(shared, parts, state, &mut notices).await;
    reattach(&core, core.with_state(|state| state.attached_plan.take())).await;
    watch_notices(&core);
    prune_stale_worktrees(&core).await;
    let handle = spawn_actor(Arc::clone(&core));
    emit_snapshot(&core);
    for (level, text) in notices {
        core.events.notice(level, text);
    }
    request_trust(&core);
    sync_servers(&core);
    prebuild_repo_map(&core);
    session_start(&core, fresh).await;
    Ok(handle)
}

/// The root a stored session recorded (when it still exists), else the
/// requested one; canonical so every session of a project agrees.
async fn project_root(
    requested: &Path,
    stored: Option<&LoadedSession>,
) -> Result<PathBuf, EngineError> {
    let recorded = stored
        .and_then(|loaded| loaded.state.info.as_ref())
        .map(|(_, root, _)| PathBuf::from(root))
        .filter(|root| !root.as_os_str().is_empty() && root.is_dir());
    let root = recorded.unwrap_or_else(|| requested.to_path_buf());
    let invalid =
        |detail: String| EngineError::Invalid(format!("project root {}: {detail}", root.display()));
    let canonical = tokio::fs::canonicalize(&root)
        .await
        .map_err(|error| invalid(error.to_string()))?;
    if !canonical.is_dir() {
        return Err(invalid("not a directory".to_string()));
    }
    Ok(canonical)
}

fn create(
    shared: &Shared,
    events: &Arc<Emitter>,
    id: &SessionId,
    root: &Path,
    settings: &SessionSettings,
) -> Result<(Arc<Journal>, SessionState, bool), EngineError> {
    let model = settings.settings.model.main.clone();
    let mode = settings.settings.permissions.mode;
    let planning = settings.settings.permissions.planning;
    let (log, meta) = shared.store.create(NewSession {
        session_id: id.clone(),
        project_root: root.to_string_lossy().into_owned(),
        model: model.clone(),
        mode,
        planning,
    })?;
    let journal = Journal::new(Arc::clone(events), shared.store.clone(), id.clone(), log);
    let replay = ReplayState {
        info: Some((id.clone(), meta.project_root.clone(), meta.created_at)),
        mode,
        planning,
        model: Some(model.clone()),
        ..ReplayState::default()
    };
    let effort = settings.settings.model.effort;
    let state = SessionState::from_replay(replay, false, meta.created_at, &model, effort);
    Ok((Arc::new(journal), state, true))
}

fn resume(
    shared: &Shared,
    events: &Arc<Emitter>,
    id: &SessionId,
    loaded: LoadedSession,
    settings: &SessionSettings,
    notices: &mut Notices,
) -> Result<(Arc<Journal>, SessionState, bool), EngineError> {
    if loaded.torn_tail || loaded.corrupt_lines > 0 {
        let lines = loaded.corrupt_lines + usize::from(loaded.torn_tail);
        notices.push((
            NoticeLevel::Warn,
            format!(
                "{lines} damaged line(s) of this session's log were skipped; the rest was restored."
            ),
        ));
    }
    let log = shared.store.open_append(id)?;
    let journal = Journal::new(Arc::clone(events), shared.store.clone(), id.clone(), log);
    let replay = if repair(&journal, &loaded.state)? {
        shared.store.load(id)?.state
    } else {
        loaded.state
    };
    let model = &settings.settings.model;
    let state = SessionState::from_replay(
        replay,
        loaded.meta.legacy,
        loaded.meta.updated_at,
        &model.main,
        model.effort,
    );
    Ok((Arc::new(journal), state, false))
}

/// What `open_session` has built before the core exists.
struct Parts {
    id: SessionId,
    events: Arc<Emitter>,
    journal: Arc<Journal>,
    root: PathBuf,
    settings: Arc<SessionSettings>,
}

async fn assemble(
    shared: &Shared,
    parts: Parts,
    mut state: SessionState,
    notices: &mut Notices,
) -> Arc<SessionCore> {
    let Parts {
        id,
        events,
        journal,
        root,
        settings,
    } = parts;
    let home = shared.paths.home_dir.as_deref();
    let (policy, errors) = build_policy(&settings, &root, home, &[], &[]);
    notices.extend(errors.into_iter().map(|error| {
        (
            NoticeLevel::Warn,
            format!("ignored permission rule: {error}"),
        )
    }));
    let (client, warning) =
        session_client(&shared.paths, &settings.settings, shared.factory.as_ref());
    notices.extend(warning.map(|warning| (NoticeLevel::Warn, warning)));
    let catalog = read(&shared.catalog).clone();
    state.context_limit =
        models::context_window(&settings.settings, catalog.as_deref(), &state.model);
    let git = git_info(&root).await;
    let status = Arc::new(StatusTracker::new(Arc::clone(&events)));
    let broker = Broker::new(Arc::clone(&journal), Arc::clone(&status));
    let reminders = Arc::new(ReminderBox::default());
    let jobs = JobHub::new(Arc::clone(&events), Arc::clone(&reminders));
    let work_plans = settings.settings.feature(FeatureId::WorkPlans).acts();
    let registry = AgentRegistry::build(&settings.extensions.agents, work_plans);
    let tools = builtin_registry(registry.cards(), &settings.settings);
    let agents = Orchestra::new(registry, settings.settings.agents.max_concurrent);
    let checks = CheckHub::default();
    checks.set(discover_checks(&root, &settings.settings.verification.checks).await);
    let lsp = LspHub::default();
    lsp.configure(&root, &settings.settings.lsp);
    let (decisions, warning) = build_service(shared, &settings).await;
    notices.extend(warning.map(|warning| (NoticeLevel::Warn, warning)));
    Arc::new(SessionCore {
        id,
        root: root.clone(),
        shared: shared.clone(),
        events,
        journal,
        status,
        broker,
        reminders,
        jobs,
        agents,
        settings: RwLock::new(settings),
        client: RwLock::new(client),
        tools: RwLock::new(Arc::new(tools)),
        policy: Mutex::new(policy),
        state: Mutex::new(state),
        git: Mutex::new(git),
        main: AgentResources::new(root),
        locks: PathLocks::new(),
        checkpoints: Checkpoints::default(),
        verifier: Arc::new(ModeVerifier),
        checks,
        mcp: McpHub::default(),
        lsp,
        repo_map: RepoMapCache::default(),
        last_request: Mutex::new(None),
        decisions: DecisionHub::new(decisions),
        work: WorkSession::default(),
        cancel: CancellationToken::new(),
    })
}

/// `SessionStart` hook output becomes context for the next turn.
async fn session_start(core: &SessionCore, fresh: bool) {
    let source = if fresh { "startup" } else { "resume" };
    let input = HookInput::new()
        .target(source)
        .with("source", json!(source));
    let outcome = run_hooks(
        &core.hook_env(),
        &core.events,
        HookEvent::SessionStart,
        input,
        &core.cancel,
    )
    .await;
    for context in outcome.context {
        let note = format!("SessionStart hook context:\n{context}");
        core.reminders.push(&AgentId::main(), wrap_reminder(&note));
    }
}
