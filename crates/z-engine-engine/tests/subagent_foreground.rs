//! A foreground `explore` agent: its report is the `Agent` tool result,
//! its messages live in its own transcript (not the main one), its
//! `AgentInfo` names the calling tool call, and both survive a reopen.

mod support;

use support::{Harness, agent_call, results, route_task, task_requests, tool_names};
use z_engine_protocol::{AgentStatus, Event, Role, TurnOutcome};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn explore_report_returns_as_the_tool_result() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    route_task(
        &h.model,
        "SEARCH-TASK",
        vec![Script::text("REPORT: the loader is in src/config.rs")],
    );
    h.model.push(Script::tool(
        "Agent",
        agent_call(
            "explore",
            "find config",
            "SEARCH-TASK find the config loader",
        ),
    ));
    h.model.push(Script::text("relayed"));
    let turn = h.run_turn("where is the config loaded?").await;
    assert_eq!(turn.outcome, TurnOutcome::Completed);

    let started = h.agent_started().await;
    assert_eq!(started.agent_type, "explore");
    assert_eq!(started.depth, 1);
    assert!(started.parent_id.as_ref().is_some_and(|id| id.is_main()));
    assert!(!started.background);
    let call = h
        .expect(|e| matches!(e, Event::ToolStarted { tool, .. } if tool == "Agent"))
        .await;
    let Event::ToolStarted { call_id, .. } = call else {
        unreachable!()
    };
    assert_eq!(started.call_id.as_ref(), Some(&call_id));

    let finished = h.agent_finished(&started.agent_id).await;
    assert_eq!(finished.status, AgentStatus::Completed);
    assert!(finished.finished_at.is_some());
    assert!(
        finished
            .result_preview
            .as_deref()
            .is_some_and(|text| text.contains("REPORT"))
    );
    let sub = started.agent_id.clone();
    assert!(h.events.seen().iter().any(|e| matches!(
        e,
        Event::AssistantFinished { agent_id, .. } if *agent_id == sub
    )));

    let answered = results(h.main_requests()[1].messages.last().unwrap());
    assert!(!answered[0].1, "{}", answered[0].2);
    assert!(
        answered[0].2.contains("REPORT: the loader"),
        "{}",
        answered[0].2
    );
    assert!(answered[0].2.contains(sub.as_str()), "{}", answered[0].2);

    let offered = tool_names(&task_requests(&h.model, "SEARCH-TASK")[0]);
    for hidden in ["AskUserQuestion", "ExitPlanMode", "Write", "Edit", "Agent"] {
        assert!(!offered.iter().any(|name| name == hidden), "{offered:?}");
    }
    assert!(offered.iter().any(|name| name == "Read"), "{offered:?}");

    let transcript = h.engine.agent_transcript(&h.session, &sub).unwrap();
    assert_eq!(transcript.len(), 2);
    assert!(support::all_text(&transcript[0]).contains("SEARCH-TASK"));
    assert_eq!(transcript[1].role, Role::Assistant);
    assert!(
        h.transcript().iter().all(|message| {
            message.role != Role::Assistant || !message.text().contains("REPORT")
        })
    );

    h.reopen().await;
    let snapshot = h.wait(|e| matches!(e, Event::Snapshot { .. })).await;
    let Event::Snapshot { snapshot } = snapshot else {
        unreachable!()
    };
    let restored = snapshot
        .agents
        .iter()
        .find(|info| info.agent_id == sub)
        .expect("the agent is in the snapshot");
    assert_eq!(restored.status, AgentStatus::Completed);
    assert_eq!(
        h.engine.agent_transcript(&h.session, &sub).unwrap().len(),
        2
    );
}
