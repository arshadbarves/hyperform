//! A background agent returns a job id at once, `JobOutput` waits for its
//! report, and its completion reaches the parent as a reminder.

mod support;

use std::time::Duration;

use serde_json::json;
use support::{Harness, agent_call, last_user_text, results, route_task};
use z_engine_llm::{ModelEvent, StopReason};
use z_engine_protocol::{AgentStatus, Event, JobKind, JobStatus};
use z_engine_testkit::{FixtureRepo, Script, usage};

#[tokio::test]
async fn background_agent_reports_through_its_job() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    route_task(
        &h.model,
        "BG-TASK",
        vec![Script::Delayed(
            Duration::from_millis(800),
            vec![
                ModelEvent::TextDelta("BACKGROUND REPORT".into()),
                usage(40, 10),
                ModelEvent::Stop(StopReason::EndTurn),
            ],
        )],
    );
    let mut call = agent_call("general", "survey", "BG-TASK survey the tree");
    call["run_in_background"] = json!(true);
    h.model.push(Script::tool("Agent", call));
    h.model.push(Script::text("it runs"));
    h.run_turn("start a background survey").await;

    let started = results(h.main_requests()[1].messages.last().unwrap());
    let job = match h
        .expect(|e| matches!(e, Event::JobUpdated { job } if job.kind == JobKind::Agent))
        .await
    {
        Event::JobUpdated { job } => job,
        other => panic!("{other:?}"),
    };
    assert!(job.owner.is_main());
    assert_eq!(job.status, JobStatus::Running);
    assert!(
        started[0].2.contains(job.job_id.as_str()),
        "{}",
        started[0].2
    );
    let agent = job.agent_id.clone().expect("an agent job names its agent");
    assert!(h.agent_info(&agent).background);

    h.model.push(Script::tool(
        "JobOutput",
        json!({ "job_id": job.job_id, "wait_ms": 10_000 }),
    ));
    h.model.push(Script::text("read it"));
    h.run_turn("what did it find?").await;

    let requests = h.main_requests();
    let read = results(requests[3].messages.last().unwrap());
    assert!(read[0].2.contains("status: completed"), "{}", read[0].2);
    assert!(read[0].2.contains("BACKGROUND REPORT"), "{}", read[0].2);
    let reminder = last_user_text(&requests[3]);
    assert!(reminder.contains("has finished"), "{reminder}");
    assert!(reminder.contains(job.job_id.as_str()), "{reminder}");
    assert_eq!(
        h.agent_finished(&agent).await.status,
        AgentStatus::Completed
    );
    h.expect(|e| {
        matches!(e, Event::JobUpdated { job: done }
            if done.job_id == job.job_id && done.status == JobStatus::Completed)
    })
    .await;
}
