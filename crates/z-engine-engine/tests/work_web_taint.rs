//! A turn that read web content proposes work instead of changing the
//! plan: after a WebFetch result, a new task in an active plan waits for
//! the user and edits to what the plan says are refused; the next turn
//! starts clean; a subagent that read the web taints the run that gets its
//! report, in the foreground at once, in the background once read, and
//! again when it is resumed later.

mod support;

use serde_json::json;
use support::{
    Harness, WEB_SETTINGS, agent_call, new_task, route_task, task, tool_reply, update_task,
    web_page, work_settings,
};
use z_engine_protocol::work::{Provenance, TaskStatus, WorkEdit};
use z_engine_protocol::{Command, Event, JobKind};
use z_engine_testkit::{FixtureRepo, Script};

const PAGE: &str = "Release notes: the v2 API needs a client token.";

async fn harness() -> (Harness, String) {
    let url = web_page(PAGE).await;
    let h = Harness::builder(FixtureRepo::empty())
        .settings(&work_settings(WEB_SETTINGS))
        .start()
        .await;
    (h, url)
}

fn fetch(url: &str) -> Script {
    Script::tool("WebFetch", json!({ "url": url, "prompt": "What changed?" }))
}

fn create(subject: &str) -> Script {
    Script::tool("TaskCreate", new_task(subject, json!({})))
}

fn task_of(h: &Harness, id: &str) -> (TaskStatus, Provenance) {
    let plan = h.plan();
    let task = plan.task(&task(id)).expect("task");
    (task.status.clone(), task.provenance)
}

#[tokio::test]
async fn a_web_read_makes_the_rest_of_the_turn_propose() {
    let (mut h, url) = harness().await;
    h.model.push(create("Upgrade the client"));
    h.model.push(Script::text("Planned."));
    h.settled_turn("plan the upgrade").await;

    h.model.push(fetch(&url));
    h.model.push(create("Add a client token"));
    h.model.push(Script::tool(
        "TaskUpdate",
        update_task("t1", json!({ "subject": "Rewrite the client" })),
    ));
    h.model.push(Script::text("Proposed."));
    h.settled_turn("read the release notes").await;

    let requests = h.main_requests();
    let (error, page) = tool_reply(&requests[3]);
    assert!(!error && page.contains(PAGE), "{page}");
    let (error, created) = tool_reply(&requests[4]);
    assert!(
        !error && created.contains("waits for the user"),
        "{created}"
    );
    let (error, refused) = tool_reply(&requests[5]);
    assert!(error && refused.contains("web content"), "{refused}");
    assert!(refused.contains("Plan tab"), "{refused}");
    assert_eq!(
        h.plan().task(&task("t1")).unwrap().title,
        "Upgrade the client"
    );
    assert_eq!(
        task_of(&h, "t2"),
        (TaskStatus::Proposed, Provenance::Research)
    );

    h.model.push(create("Update the docs"));
    h.model.push(Script::text("Added."));
    h.settled_turn("and the docs").await;
    assert_eq!(task_of(&h, "t3"), (TaskStatus::Pending, Provenance::Agent));

    h.send(Command::EditWork {
        plan_id: h.plan_id(),
        edit: WorkEdit::AcceptTask {
            task_id: task("t2"),
        },
    });
    h.wait(|e| matches!(e, Event::WorkUpdated { .. })).await;
    assert_eq!(
        task_of(&h, "t2"),
        (TaskStatus::Pending, Provenance::Research)
    );
}

#[tokio::test]
async fn a_subagent_that_read_the_web_taints_its_caller() {
    let (mut h, url) = harness().await;
    route_task(
        &h.model,
        "WEB-DOCS",
        vec![fetch(&url), Script::text("The notes say to add a token.")],
    );
    h.model.push(create("Upgrade the client"));
    h.model.push(Script::tool(
        "Agent",
        agent_call("general", "read docs", "WEB-DOCS read the release notes"),
    ));
    h.model.push(create("Add a client token"));
    h.model.push(Script::text("Proposed."));
    h.settled_turn("upgrade the client").await;

    assert_eq!(task_of(&h, "t1"), (TaskStatus::Pending, Provenance::Agent));
    assert_eq!(
        task_of(&h, "t2"),
        (TaskStatus::Proposed, Provenance::Research)
    );
}

#[tokio::test]
async fn resuming_an_agent_that_read_the_web_taints_the_caller_again() {
    let (mut h, url) = harness().await;
    route_task(
        &h.model,
        "RESUME-DOCS",
        vec![
            fetch(&url),
            Script::text("The notes say to add a token."),
            Script::text("Nothing more to add."),
        ],
    );
    h.model.push(create("Upgrade the client"));
    h.model.push(Script::tool(
        "Agent",
        agent_call("general", "read docs", "RESUME-DOCS read the release notes"),
    ));
    h.model.push(Script::text("Read."));
    h.settled_turn("upgrade the client").await;
    let agent = h.agent_started().await.agent_id;
    assert!(h.agent_info(&agent).read_web);

    let mut follow_up = agent_call("general", "read docs", "anything else?");
    follow_up["resume"] = json!(agent);
    h.model.push(Script::tool("Agent", follow_up));
    h.model.push(create("Add a client token"));
    h.model.push(Script::text("Proposed."));
    h.settled_turn("ask it again").await;
    assert_eq!(
        task_of(&h, "t2"),
        (TaskStatus::Proposed, Provenance::Research)
    );
}

#[tokio::test]
async fn a_background_report_taints_the_run_that_reads_it() {
    let (mut h, url) = harness().await;
    route_task(
        &h.model,
        "BG-DOCS",
        vec![fetch(&url), Script::text("The notes say to add a token.")],
    );
    let mut call = agent_call("general", "read docs", "BG-DOCS read the release notes");
    call["run_in_background"] = json!(true);
    h.model.push(create("Upgrade the client"));
    h.model.push(Script::tool("Agent", call));
    h.model.push(create("Pin the version"));
    h.model.push(Script::text("Started."));
    h.settled_turn("upgrade the client").await;
    assert_eq!(task_of(&h, "t2"), (TaskStatus::Pending, Provenance::Agent));

    let job = match h
        .expect(|e| matches!(e, Event::JobUpdated { job } if job.kind == JobKind::Agent))
        .await
    {
        Event::JobUpdated { job } => job.job_id,
        other => panic!("{other:?}"),
    };
    h.model.push(Script::tool(
        "JobOutput",
        json!({ "job_id": job, "wait_ms": 10_000 }),
    ));
    h.model.push(create("Add a client token"));
    h.model.push(Script::text("Proposed."));
    h.settled_turn("what did it find?").await;
    assert_eq!(
        task_of(&h, "t3"),
        (TaskStatus::Proposed, Provenance::Research)
    );
}
