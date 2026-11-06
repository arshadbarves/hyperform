//! A subagent still running when the app died reopens as `Failed` with a
//! note, so the agent tree never shows a run nothing drives any more.

mod support;

use support::Harness;
use z_engine_protocol::{AgentId, AgentInfo, AgentStatus, Event, Isolation, Usage, now_ms};
use z_engine_store::{LogRecord, SessionStore};
use z_engine_testkit::FixtureRepo;

#[tokio::test]
async fn running_agent_is_marked_failed_on_open() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    h.engine.close_session(&h.session).await.unwrap();

    let store = SessionStore::new(h.paths.sessions_dir.clone());
    let mut log = store.open_append(&h.session).unwrap();
    let agent = AgentId::new();
    log.append(&LogRecord::AgentUpdated {
        info: AgentInfo {
            agent_id: agent.clone(),
            parent_id: Some(AgentId::main()),
            call_id: None,
            agent_type: "general".into(),
            description: "orphan".into(),
            model: "test-model".into(),
            background: true,
            isolation: Isolation::Shared,
            worktree: None,
            status: AgentStatus::Running,
            depth: 1,
            started_at: now_ms(),
            finished_at: None,
            usage: Usage::default(),
            cost_usd: 0.0,
            tool_calls: 0,
            result_preview: None,
            read_web: false,
            task_id: None,
            error: None,
        },
    })
    .unwrap();
    log.sync().unwrap();
    drop(log);

    h.engine
        .open_session(h.repo.path(), Some(h.session.clone()))
        .await
        .unwrap();
    let snapshot = match h.wait(|e| matches!(e, Event::Snapshot { .. })).await {
        Event::Snapshot { snapshot } => snapshot,
        other => panic!("{other:?}"),
    };
    let info = snapshot
        .agents
        .iter()
        .find(|info| info.agent_id == agent)
        .unwrap();
    assert_eq!(info.status, AgentStatus::Failed);
    assert!(info.finished_at.is_some());
    assert!(
        info.error
            .as_deref()
            .is_some_and(|error| error.contains("interrupted"))
    );
}
