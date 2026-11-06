//! Cancelling the turn stops its foreground agents (their token hangs off
//! the turn) but not background agents (their token hangs off the
//! session).

mod support;

use std::time::Duration;

use serde_json::json;
use support::{Harness, agent_call, assert_valid_transcript, route_task};
use z_engine_llm::{ModelEvent, StopReason};
use z_engine_protocol::{AgentStatus, Command, Event, TurnOutcome};
use z_engine_testkit::{FixtureRepo, Script, usage};

#[tokio::test]
async fn cancel_stops_foreground_agents_only() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    route_task(
        &h.model,
        "FG-TASK",
        vec![Script::Stall(vec![ModelEvent::TextDelta(
            "thinking".into(),
        )])],
    );
    route_task(
        &h.model,
        "BG-TASK",
        vec![Script::Delayed(
            Duration::from_millis(1_200),
            vec![
                ModelEvent::TextDelta("background finished".into()),
                usage(10, 5),
                ModelEvent::Stop(StopReason::EndTurn),
            ],
        )],
    );
    let mut background = agent_call("general", "bg", "BG-TASK keep going");
    background["run_in_background"] = json!(true);
    h.model.push(Script::tools(&[
        ("Agent", background),
        ("Agent", agent_call("explore", "fg", "FG-TASK look")),
    ]));
    h.submit("start two agents");

    let started = h.agents_started(2).await;
    let (bg, fg) = if started[0].background {
        (&started[0], &started[1])
    } else {
        (&started[1], &started[0])
    };
    let fg_id = fg.agent_id.clone();
    h.expect(|e| matches!(e, Event::TextDelta { agent_id, .. } if *agent_id == fg_id))
        .await;
    h.send(Command::Cancel);
    let turn = h.turn_finished().await;
    assert_eq!(turn.outcome, TurnOutcome::Cancelled);

    assert_eq!(
        h.agent_finished(&fg.agent_id).await.status,
        AgentStatus::Cancelled
    );
    let bg_done = h.agent_finished(&bg.agent_id).await;
    assert_eq!(bg_done.status, AgentStatus::Completed);
    assert!(
        bg_done
            .result_preview
            .as_deref()
            .is_some_and(|text| text.contains("background finished"))
    );
    assert_valid_transcript(&h.transcript());
    let fg_transcript = h.engine.agent_transcript(&h.session, &fg.agent_id).unwrap();
    assert_valid_transcript(&fg_transcript);
}
