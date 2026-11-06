//! Two `Agent` calls in one message run at the same time: both agents
//! start before either finishes, and each report answers its own call.
//! With `agents.max_concurrent = 1` the second waits for the first.

mod support;

use std::time::Duration;

use support::{BASE_SETTINGS, Harness, agent_call, results, route_task};
use z_engine_llm::{ModelEvent, StopReason};
use z_engine_protocol::{AgentStatus, Event};
use z_engine_testkit::{FixtureRepo, Script, usage};

fn slow_report(text: &str) -> Script {
    Script::Delayed(
        Duration::from_millis(400),
        vec![
            ModelEvent::TextDelta(text.to_string()),
            usage(50, 10),
            ModelEvent::Stop(StopReason::EndTurn),
        ],
    )
}

#[tokio::test]
async fn agents_in_one_message_run_concurrently() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    route_task(&h.model, "TASK-ONE", vec![slow_report("report one")]);
    route_task(&h.model, "TASK-TWO", vec![slow_report("report two")]);
    h.model.push(Script::tools(&[
        (
            "Agent",
            agent_call("explore", "first", "TASK-ONE look around"),
        ),
        (
            "Agent",
            agent_call("explore", "second", "TASK-TWO look elsewhere"),
        ),
    ]));
    h.model.push(Script::text("both done"));
    h.run_turn("research two things").await;

    let order: Vec<&str> = h
        .events
        .seen()
        .iter()
        .filter_map(|event| match event {
            Event::AgentStarted { .. } => Some("started"),
            Event::AgentUpdated { info } if info.status == AgentStatus::Completed => {
                Some("completed")
            }
            _ => None,
        })
        .collect();
    assert_eq!(order, ["started", "started", "completed", "completed"]);

    let answered = results(h.main_requests()[1].messages.last().unwrap());
    assert_eq!(answered.len(), 2);
    assert!(answered[0].2.contains("report one"), "{}", answered[0].2);
    assert!(answered[1].2.contains("report two"), "{}", answered[1].2);
}

#[tokio::test]
async fn max_concurrent_queues_extra_agents() {
    let settings = format!("{BASE_SETTINGS}\n[agents]\nmax_concurrent = 1\n");
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    route_task(&h.model, "TASK-ONE", vec![slow_report("report one")]);
    route_task(&h.model, "TASK-TWO", vec![slow_report("report two")]);
    h.model.push(Script::tools(&[
        (
            "Agent",
            agent_call("explore", "first", "TASK-ONE look around"),
        ),
        (
            "Agent",
            agent_call("explore", "second", "TASK-TWO look elsewhere"),
        ),
    ]));
    h.model.push(Script::text("both done"));
    h.run_turn("research two things").await;

    let order: Vec<&str> = h
        .events
        .seen()
        .iter()
        .filter_map(|event| match event {
            Event::AssistantStarted { agent_id, .. } if !agent_id.is_main() => Some("round"),
            Event::AgentUpdated { info } if info.status == AgentStatus::Completed => {
                Some("completed")
            }
            _ => None,
        })
        .collect();
    assert_eq!(order, ["round", "completed", "round", "completed"]);
}
