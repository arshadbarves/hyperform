//! The completion gate through the tools: a task with a check criterion is
//! refused as done until a passing check of that kind ran on the current
//! tree. Check runs are linked to the running agent's task by the engine
//! (the model cannot mark a check criterion met, and a note is no check),
//! a failing run fails the criterion, and a passing run goes stale as soon
//! as the tree changes, even while the criterion still reads Met. A task
//! left Verifying because its check could not run finishes when the user
//! runs its checks again, or when the main agent's check of that kind runs.

mod support;

use hyperform_protocol::Event;
use hyperform_protocol::work::{
    Actor, CriterionStatus, EvidenceRef, TaskStatus, WorkEdit, WorkEvent,
};
use hyperform_testkit::{FixtureRepo, Script};
use serde_json::{Value, json};
use support::{
    BASE_SETTINGS, Harness, new_task, parallel_settings, report, route_task, task, tool_reply,
    update_task,
};

const CHECKS: &str = "[permissions]\nmode = \"bypass\"\n\n\
    [[verification.checks]]\nid = \"unit\"\nlabel = \"Unit tests\"\ncommand = \"echo all good\"\nkind = \"test\"\n\n\
    [[verification.checks]]\nid = \"broken\"\nlabel = \"Broken tests\"\ncommand = \"echo boom; exit 1\"\nkind = \"test\"\n";

async fn harness() -> Harness {
    let settings = format!("{BASE_SETTINGS}\n[experimental]\nwork_plans = \"on\"\n\n{CHECKS}");
    let repo = FixtureRepo::git(&[("src/lib.rs", "pub fn retry() {}\n")]);
    Harness::builder(repo).settings(&settings).start().await
}

fn started_task() -> [(&'static str, Value); 2] {
    let acceptance = json!({
        "acceptance": [{ "text": "Unit tests pass", "method": "test" }],
        "scope": ["src/lib.rs"]
    });
    [
        ("TaskCreate", new_task("Fix retries", acceptance)),
        (
            "TaskUpdate",
            update_task("t1", json!({ "status": "in_progress" })),
        ),
    ]
}

fn run(check: &str) -> (&'static str, Value) {
    (
        "Verify",
        json!({ "action": "run", "check": format!("custom:{check}") }),
    )
}

fn complete() -> (&'static str, Value) {
    (
        "TaskUpdate",
        update_task("t1", json!({ "status": "completed" })),
    )
}

async fn script(h: &mut Harness, calls: Vec<(&'static str, Value)>) {
    for (tool, input) in calls {
        h.model.push(Script::tool(tool, input));
    }
    h.model.push(Script::text("Done."));
    h.run_turn("fix the retries").await;
}

#[tokio::test]
async fn done_waits_for_a_passing_check_the_engine_links() {
    let mut h = harness().await;
    let mut calls = started_task().to_vec();
    calls.extend([
        complete(),
        (
            "TaskUpdate",
            update_task("t1", json!({ "metCriteria": ["c1"] })),
        ),
        (
            "PlanRecord",
            json!({ "kind": "evidence", "text": "I ran the tests myself", "taskIds": ["t1"], "criterionId": "c1" }),
        ),
        complete(),
        run("broken"),
        complete(),
        run("unit"),
        complete(),
    ]);
    script(&mut h, calls).await;

    let requests = h.main_requests();
    let reply = |call: usize| tool_reply(&requests[call + 1]);
    let (error, early) = reply(2);
    assert!(error, "{early}");
    assert!(
        early.contains("needs a passing test check") && early.contains("no check"),
        "{early}"
    );
    let (error, claimed) = reply(3);
    assert!(error && claimed.contains("done by the engine"), "{claimed}");
    let (error, noted) = reply(4);
    assert!(!error, "a note is recorded: {noted}");
    let (error, still) = reply(5);
    assert!(
        error && still.contains("no check"),
        "a note is no check: {still}"
    );
    let (error, failed) = reply(7);
    assert!(error && failed.contains("failed"), "{failed}");
    let (error, done) = reply(9);
    assert!(!error, "{done}");

    let passed = h
        .events
        .seen()
        .iter()
        .filter_map(|e| match e {
            Event::CheckRecorded { record } if record.passed => Some(record.record_id.clone()),
            _ => None,
        })
        .next_back()
        .expect("a passing check");
    let plan = h.plan();
    let t1 = plan.task(&task("t1")).unwrap();
    assert_eq!(t1.status, TaskStatus::Done);
    let criterion = &t1.acceptance[0];
    assert_eq!(criterion.status, CriterionStatus::Met);
    let checks: Vec<(bool, &str)> = criterion
        .evidence
        .iter()
        .filter_map(|evidence| match evidence {
            EvidenceRef::Check {
                passed, record_id, ..
            } => Some((*passed, record_id.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(checks.len(), 2, "both runs linked: {checks:?}");
    assert_eq!(checks[1], (true, passed.as_str()));
    let log = h.plan_log().records;
    let linked: Vec<&Actor> = log
        .iter()
        .filter(|record| {
            matches!(
                &record.event,
                WorkEvent::EvidenceLinked {
                    evidence: EvidenceRef::Check { .. },
                    ..
                }
            )
        })
        .map(|record| &record.actor)
        .collect();
    assert_eq!(
        linked,
        [&Actor::Kernel, &Actor::Kernel],
        "the engine links checks"
    );
}

#[tokio::test]
async fn a_change_to_the_tree_makes_the_passing_check_stale() {
    let mut h = harness().await;
    let write =
        json!({ "file_path": h.path("src/backoff.rs"), "content": "pub fn backoff() {}\n" });
    let mut calls = started_task().to_vec();
    calls.extend([run("unit"), ("Write", write), complete()]);
    script(&mut h, calls).await;

    let requests = h.main_requests();
    let (error, wrote) = tool_reply(&requests[requests.len() - 2]);
    assert!(!error, "{wrote}");
    let (error, stale) = tool_reply(requests.last().unwrap());
    assert!(error, "{stale}");
    assert!(stale.contains("older tree"), "{stale}");
    let plan = h.plan();
    let t1 = plan.task(&task("t1")).unwrap();
    assert_eq!(t1.status, TaskStatus::Active);
    assert_eq!(
        t1.acceptance[0].status,
        CriterionStatus::Met,
        "the criterion still reads Met, yet the gate refused"
    );

    script(&mut h, vec![run("unit"), complete()]).await;
    let (error, done) = tool_reply(h.main_requests().last().unwrap());
    assert!(!error, "{done}");
    assert_eq!(h.plan().task(&task("t1")).unwrap().status, TaskStatus::Done);
}

const UNIT: &str = "\n[[verification.checks]]\nid = \"unit\"\nlabel = \"Unit tests\"\ncommand = \"echo all good\"\nkind = \"test\"\n";
const MERGE_AUTO: &str = "[permissions]\nmode = \"bypass\"\n\n[work]\nmerge = \"auto\"\n";

/// A helper's merged change left `t1` Verifying: the project had no test
/// check when it came back. The check is then added.
async fn stuck_verifying() -> Harness {
    let repo = FixtureRepo::git(&[("src/lib.rs", "pub fn retry() {}\n")]);
    let mut h = Harness::builder(repo)
        .settings(&parallel_settings(MERGE_AUTO))
        .start()
        .await;
    let fields = json!({
        "acceptance": [{ "text": "Unit tests pass", "method": "test" }],
        "scope": ["src"]
    });
    h.model
        .push(Script::tool("TaskCreate", new_task("Fix retries", fields)));
    h.model.push(Script::text("Planned."));
    h.settled_turn("plan it").await;
    h.approve_parallel().await;
    let rounds = vec![
        Script::tools(&[
            (
                "Write",
                json!({ "file_path": "src/backoff.rs", "content": "pub fn backoff() {}\n" }),
            ),
            (
                "TaskUpdate",
                report("t1", json!({ "summary": "Added backoff" })),
            ),
        ]),
        Script::text("Reported."),
    ];
    route_task(&h.model, "STUCK-HELPER", rounds);
    let replies = h.dispatch_turn(&[("t1", "STUCK-HELPER")]).await;
    assert!(!replies[0].0, "{replies:?}");
    h.task_until("t1", |t| t.status == TaskStatus::Verifying)
        .await;
    h.expect(|e| matches!(e, Event::Notice { text, .. } if text.contains("has no test check")))
        .await;
    let with_check = format!("{}{UNIT}", parallel_settings(MERGE_AUTO));
    std::fs::write(&h.paths.user_settings_file, with_check).unwrap();
    h.events.drain();
    h.engine.reload_settings(None);
    h.wait(|e| matches!(e, Event::Snapshot { .. })).await;
    let stuck = h.plan().task(&task("t1")).unwrap().clone();
    assert_eq!(stuck.status, TaskStatus::Verifying, "{stuck:?}");
    h
}

#[tokio::test]
async fn the_users_recheck_runs_a_verifying_tasks_checks_again() {
    let mut h = stuck_verifying().await;
    h.edit_plan(WorkEdit::RecheckTask {
        task_id: task("t1"),
    });
    let done = h.finished("t1").await;
    assert_eq!(done.status, TaskStatus::Done, "{done:?}");
    assert_eq!(done.acceptance[0].status, CriterionStatus::Met);
}

#[tokio::test]
async fn the_main_agents_check_counts_for_a_verifying_task() {
    let mut h = stuck_verifying().await;
    script(&mut h, vec![run("unit")]).await;
    let done = h.finished("t1").await;
    assert_eq!(done.status, TaskStatus::Done, "{done:?}");
    assert_eq!(done.acceptance[0].status, CriterionStatus::Met);
}

#[tokio::test]
async fn a_recheck_of_a_task_that_is_not_verifying_is_refused() {
    let mut h = harness().await;
    script(&mut h, started_task().to_vec()).await;
    h.edit_plan(WorkEdit::RecheckTask {
        task_id: task("t1"),
    });
    h.expect(|e| matches!(e, Event::Notice { text, .. } if text.contains("not verifying")))
        .await;
    assert_eq!(
        h.plan().task(&task("t1")).unwrap().status,
        TaskStatus::Active
    );
}
