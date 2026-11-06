//! Usage roll-up: a foreground agent's usage counts toward its own
//! `AgentInfo`, the spawning turn's record and the session totals; a
//! background agent's usage survives a reopen through its own record.

mod support;

use serde_json::json;
use support::{Harness, agent_call, route_task};
use z_engine_protocol::{AgentStatus, Event};
use z_engine_testkit::{FixtureRepo, Script};

/// Input tokens: `Script::tool` rounds report 120, `Script::text` 100.
#[tokio::test]
async fn foreground_usage_rolls_into_the_turn_and_session() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    route_task(&h.model, "COST-TASK", vec![Script::text("cheap report")]);
    h.model.push(Script::tool(
        "Agent",
        agent_call("general", "costly", "COST-TASK do it"),
    ));
    h.model.push(Script::text("done"));
    let turn = h.run_turn("delegate").await;
    assert_eq!(turn.usage.input_tokens, 120 + 100 + 100);
    assert_eq!(turn.usage.output_tokens, 30 + 20 + 20);

    let agent = h.agent_started().await.agent_id;
    let info = h.agent_finished(&agent).await;
    assert_eq!(info.usage.input_tokens, 100);
    assert_eq!(info.usage.output_tokens, 20);
    let sub = agent.clone();
    let usage = h
        .expect(|e| matches!(e, Event::UsageUpdated { agent_id, .. } if *agent_id == sub))
        .await;
    let Event::UsageUpdated { session_usage, .. } = usage else {
        unreachable!()
    };
    assert!(session_usage.input_tokens >= 120 + 100, "{session_usage:?}");

    h.expect(|e| matches!(e, Event::TitleChanged { .. })).await;
    h.reopen().await;
    let snapshot = h.wait(|e| matches!(e, Event::Snapshot { .. })).await;
    let Event::Snapshot { snapshot } = snapshot else {
        unreachable!()
    };
    let title = 100;
    assert_eq!(snapshot.usage.input_tokens, turn.usage.input_tokens + title);
}

#[tokio::test]
async fn background_usage_is_kept_in_the_session_log() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    route_task(&h.model, "BG-COST", vec![Script::text("bg report")]);
    let mut call = agent_call("general", "bg", "BG-COST work");
    call["run_in_background"] = json!(true);
    h.model.push(Script::tool("Agent", call));
    h.model.push(Script::text("started"));
    let turn = h.run_turn("start it").await;
    let agent = h.agent_started().await.agent_id;
    assert_eq!(
        h.agent_finished(&agent).await.status,
        AgentStatus::Completed
    );
    assert_eq!(turn.usage.input_tokens, 120 + 100);
    h.expect(|e| matches!(e, Event::TitleChanged { .. })).await;

    h.reopen().await;
    let snapshot = h.wait(|e| matches!(e, Event::Snapshot { .. })).await;
    let Event::Snapshot { snapshot } = snapshot else {
        unreachable!()
    };
    let title = 100;
    let background = 100;
    assert_eq!(
        snapshot.usage.input_tokens,
        turn.usage.input_tokens + title + background
    );
}
