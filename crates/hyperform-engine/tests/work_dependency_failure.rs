//! A failed dependency: when a task's check fails in the project tree after
//! its helper's change came back, the task fails and its dependents are
//! blocked with the dependency as the reason (a reconciliation finding, so
//! the plan waits in Replanning for the user's approval). The main agent
//! may retry it while `work.max_attempts` allows; past the budget only the
//! user's retry runs it again, with a fresh budget, and a passing run
//! unblocks the dependents.

mod support;

use hyperform_protocol::work::{
    BlockReason, PlanStatus, ReconcileFinding, Task, TaskStatus, WorkEdit,
};
use hyperform_testkit::{FixtureRepo, Script};
use serde_json::json;
use support::{
    Harness, new_task, report, route_task, task, tool_reply, update_task, work_settings,
};

const CHECKED: &str = "[permissions]\nmode = \"bypass\"\n\n[work]\nmax_attempts = 2\n\n\
    [[verification.checks]]\nid = \"marker\"\nlabel = \"Marker test\"\ncommand = \"test -f src/ok.txt\"\nkind = \"test\"\n";

/// `t1` (checked by `test -f src/ok.txt`) and `t2`, which needs `t1`.
async fn planned() -> Harness {
    let repo = FixtureRepo::git(&[("src/lib.rs", "lib\n")]);
    let mut h = Harness::builder(repo)
        .settings(&work_settings(CHECKED))
        .start()
        .await;
    let checked = json!({
        "scope": ["src"],
        "acceptance": [{ "text": "The marker test passes", "method": "test" }]
    });
    let dependent = json!({ "scope": ["docs"], "blockedBy": ["t1"] });
    h.model
        .push(Script::tool("TaskCreate", new_task("Build lib", checked)));
    h.model
        .push(Script::tool("TaskCreate", new_task("Use lib", dependent)));
    h.model.push(Script::text("Planned."));
    h.settled_turn("plan the lib").await;
    h
}

/// One helper run of `t1` that writes `file`; `t1` once it finished.
async fn attempt(h: &mut Harness, marker: &'static str, file: &str) -> Task {
    let rounds = vec![
        Script::tools(&[
            ("Write", json!({ "file_path": file, "content": "x\n" })),
            (
                "TaskUpdate",
                report("t1", json!({ "summary": format!("Wrote {file}") })),
            ),
        ]),
        Script::text("Reported."),
    ];
    route_task(&h.model, marker, rounds);
    let replies = h.dispatch_turn(&[("t1", marker)]).await;
    assert!(!replies[0].0, "{replies:?}");
    h.finished("t1").await
}

/// The main agent sets `t1` back to pending: `(is_error, text)`.
async fn agent_retry(h: &mut Harness) -> (bool, String) {
    let input = update_task("t1", json!({ "status": "pending" }));
    h.model.push(Script::tool("TaskUpdate", input));
    h.model.push(Script::text("Retried."));
    h.settled_turn("try t1 again").await;
    tool_reply(h.main_requests().last().unwrap())
}

fn status(h: &Harness, id: &str) -> TaskStatus {
    h.plan().task(&task(id)).unwrap().status.clone()
}

/// The failure is a reconciliation finding: the plan waits in Replanning
/// until the user approves it again.
async fn approve_replanned(h: &mut Harness) {
    let plan = h
        .plan_until(|plan| plan.header.status == PlanStatus::Replanning)
        .await;
    assert!(
        plan.findings.iter().any(|finding| matches!(finding,
            ReconcileFinding::DependencyFailed { task_id, .. } if *task_id == task("t2"))),
        "{:?}",
        plan.findings
    );
    h.edit_plan(WorkEdit::Approve);
    h.plan_until(|plan| plan.header.status == PlanStatus::Active)
        .await;
}

#[tokio::test]
async fn a_failed_dependency_blocks_dependents_until_a_retry_passes() {
    let mut h = planned().await;
    let failed = attempt(&mut h, "DEP-ONE", "src/one.txt").await;
    assert!(
        matches!(&failed.status, TaskStatus::Failed { reason } if reason.contains("Marker test")),
        "{:?}",
        failed.status
    );
    let blocked = h
        .task_until("t2", |t| matches!(t.status, TaskStatus::Blocked { .. }))
        .await;
    assert_eq!(
        blocked.status,
        TaskStatus::Blocked {
            reason: BlockReason::Dependency {
                task_id: task("t1")
            }
        }
    );

    let (error, text) = agent_retry(&mut h).await;
    assert!(!error, "{text}");
    assert_eq!(status(&h, "t1"), TaskStatus::Pending);
    approve_replanned(&mut h).await;
    let failed = attempt(&mut h, "DEP-TWO", "src/two.txt").await;
    assert!(matches!(failed.status, TaskStatus::Failed { .. }));
    assert_eq!(failed.run.dispatch.as_ref().unwrap().attempt, 2);

    let (error, text) = agent_retry(&mut h).await;
    assert!(error && text.contains("the limit is 2"), "{text}");
    assert!(matches!(status(&h, "t1"), TaskStatus::Failed { .. }));

    h.edit_plan(WorkEdit::RetryTask {
        task_id: task("t1"),
    });
    let retried = h
        .task_until("t1", |t| t.status == TaskStatus::Pending)
        .await;
    assert_eq!(retried.run.forgiven_attempts, 2);
    // The same finding still stands, so the second failure is not new.
    assert_eq!(h.plan().header.status, PlanStatus::Active);
    let done = attempt(&mut h, "DEP-THREE", "src/ok.txt").await;
    assert_eq!(done.status, TaskStatus::Done, "{done:?}");
    assert_eq!(done.run.dispatch.as_ref().unwrap().attempt, 3);
    let freed = h
        .task_until("t2", |t| t.status == TaskStatus::Pending)
        .await;
    assert_eq!(freed.status, TaskStatus::Pending);
}
