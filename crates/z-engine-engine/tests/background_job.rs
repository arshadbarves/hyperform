//! A background Bash job reports through `JobUpdated`, and its completion
//! reaches the model as a reminder in the next request.

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness, last_user_text, results};
use z_engine_protocol::{Event, JobStatus};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn finished_job_is_reported_in_the_next_request() {
    let settings = format!("{BASE_SETTINGS}\n[permissions]\nmode = \"bypass\"\n");
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    h.model.push(Script::tool(
        "Bash",
        json!({
            "command": "sleep 0.5; echo job-output",
            "description": "print later",
            "run_in_background": true
        }),
    ));
    h.model.push(Script::text("started it"));
    h.run_turn("start a job").await;

    let started = results(h.main_requests()[1].messages.last().unwrap());
    assert!(!started[0].1, "{}", started[0].2);
    let job = match h
        .expect(|e| matches!(e, Event::JobUpdated { job } if job.status == JobStatus::Completed))
        .await
    {
        Event::JobUpdated { job } => job,
        other => panic!("{other:?}"),
    };
    assert!(job.owner.is_main());
    assert!(job.output_tail.contains("job-output"));
    assert!(
        started[0].2.contains(job.job_id.as_str()),
        "{}",
        started[0].2
    );

    h.model.push(Script::text("it finished"));
    h.run_turn("how is the job").await;
    let request = h.main_requests().pop().unwrap();
    let prompt = last_user_text(&request);
    assert!(prompt.contains("has finished"), "{prompt}");
    assert!(prompt.contains(job.job_id.as_str()), "{prompt}");
}
