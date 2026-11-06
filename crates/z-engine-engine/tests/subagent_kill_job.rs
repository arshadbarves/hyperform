//! `KillJob` on a background agent cancels it: the job ends `Killed` and
//! the agent `Cancelled`.

mod support;

use serde_json::json;
use support::{Harness, agent_call, route_task};
use z_engine_llm::ModelEvent;
use z_engine_protocol::{AgentStatus, Command, Event, JobKind, JobStatus};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn kill_job_stops_a_background_agent() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    route_task(
        &h.model,
        "LONG-TASK",
        vec![Script::Stall(vec![ModelEvent::TextDelta("working".into())])],
    );
    let mut call = agent_call("general", "long job", "LONG-TASK never ends");
    call["run_in_background"] = json!(true);
    h.model.push(Script::tool("Agent", call));
    h.model.push(Script::text("started"));
    h.run_turn("start a long agent").await;

    let job = match h
        .expect(|e| matches!(e, Event::JobUpdated { job } if job.kind == JobKind::Agent))
        .await
    {
        Event::JobUpdated { job } => job,
        other => panic!("{other:?}"),
    };
    let agent = job.agent_id.clone().unwrap();
    h.send(Command::KillJob {
        job_id: job.job_id.clone(),
    });
    h.expect(|e| {
        matches!(e, Event::JobUpdated { job: killed }
            if killed.job_id == job.job_id && killed.status == JobStatus::Killed)
    })
    .await;
    assert_eq!(
        h.agent_finished(&agent).await.status,
        AgentStatus::Cancelled
    );
}
