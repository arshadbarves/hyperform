//! Pausing, holding and stopping from the Plan tab. Pausing the plan stops
//! its helpers at their next round: the task waits with a resume pointer
//! and the kept worktree, nothing new starts while it is paused, the kept
//! change cannot be applied by hand (discarding it drops the worktree from
//! the pointer), and resuming lets work start again. Holding a task stops
//! only its helper and refuses it until it is released. Stopping the plan
//! cancels every helper at once and leaves no resume pointers.

mod support;

use std::path::Path;
use std::time::Duration;

use hyperform_protocol::work::{PlanStatus, TaskStatus, WorkEdit};
use hyperform_protocol::{AgentInfo, AgentStatus, Command, Event, SessionStatus};
use hyperform_testkit::{FixtureRepo, Script};
use serde_json::json;
use support::{
    Harness, dispatch, new_task, parallel_settings, report, route_task, slow, task, tool_reply,
};

const AUTO: &str = "[permissions]\nmode = \"bypass\"\n\n[work]\nmerge = \"auto\"\n";

/// A plan of `t1` (`src/a`) and `t2` (`src/b`), both directories in the
/// project, whose parallel work the user approved.
async fn planned() -> Harness {
    let repo = FixtureRepo::git(&[("src/a/mod.rs", "// a\n"), ("src/b/mod.rs", "// b\n")]);
    let mut h = Harness::builder(repo)
        .settings(&parallel_settings(AUTO))
        .start()
        .await;
    for name in ["a", "b"] {
        let scope = json!({ "scope": [format!("src/{name}")] });
        let input = new_task(&format!("Build {name}"), scope);
        h.model.push(Script::tool("TaskCreate", input));
    }
    h.model.push(Script::text("Planned."));
    h.settled_turn("split the work").await;
    h.approve_parallel().await;
    h
}

/// The helper whose instructions carry `marker` writes `src/<name>/lib.rs`
/// after `delay`, then reports in a round of its own.
fn helper(h: &Harness, marker: &'static str, task_id: &str, name: &str, delay: u64) {
    let file = format!("src/{name}/lib.rs");
    let write = Script::tool(
        "Write",
        json!({ "file_path": file, "content": "// work\n" }),
    );
    let summary = json!({ "summary": format!("Wrote {file}") });
    let rounds = vec![
        slow(write, Duration::from_millis(delay)),
        Script::tool("TaskUpdate", report(task_id, summary)),
        Script::text("Reported."),
    ];
    route_task(&h.model, marker, rounds);
}

/// Waits for the running turn to finish and the session to be idle again.
async fn settle(h: &mut Harness) {
    h.turn_finished().await;
    h.wait(|e| {
        matches!(
            e,
            Event::StatusChanged {
                status: SessionStatus::Idle
            }
        )
    })
    .await;
}

fn helper_of(started: &[AgentInfo], id: &str) -> AgentInfo {
    let id = task(id);
    let found = started
        .iter()
        .find(|info| info.task_id.as_ref() == Some(&id));
    found.expect("a helper on the task").clone()
}

#[tokio::test]
async fn pausing_keeps_the_helpers_work_for_its_task_until_the_plan_resumes() {
    let mut h = planned().await;
    helper(&h, "PAUSE-ONE", "t1", "a", 1500);
    h.model
        .push(Script::tools(&[("Agent", dispatch("t1", "PAUSE-ONE"))]));
    h.model
        .push(Script::tools(&[("Agent", dispatch("t2", "PAUSE-TWO"))]));
    h.model.push(Script::text("Paused."));
    h.submit("run the tasks");
    let stopped = h.agent_started().await;
    h.edit_plan(WorkEdit::Pause);
    settle(&mut h).await;

    let info = h.agent_info(&stopped.agent_id);
    assert_eq!(info.status, AgentStatus::Cancelled);
    let plan = h.plan();
    assert_eq!(plan.header.status, PlanStatus::Paused);
    let t1 = plan.task(&task("t1")).unwrap();
    assert_eq!(t1.status, TaskStatus::Pending);
    let pointer = t1.resume.clone().expect("a resume pointer");
    assert_eq!(pointer.note, "the plan was paused");
    let tree = pointer.worktree.expect("the kept worktree");
    assert!(Path::new(&tree).join("src/a/lib.rs").is_file(), "{tree}");
    let (error, refused) = tool_reply(h.main_requests().last().unwrap());
    assert!(error && refused.contains("the plan is paused"), "{refused}");
    assert!(plan.task(&task("t2")).unwrap().run.dispatch.is_none());

    h.send(Command::ApplyAgentChanges {
        agent_id: stopped.agent_id.clone(),
    });
    let notice = h.notice("belongs to task").await;
    assert_eq!(
        notice,
        "This helper's work belongs to task t1. Resume the task or discard its work."
    );
    assert!(!h.repo.path().join("src/a/lib.rs").exists());
    h.send(Command::DiscardAgentChanges {
        agent_id: stopped.agent_id.clone(),
    });
    let dropped = h
        .task_until("t1", |t| {
            t.resume.as_ref().is_some_and(|r| r.worktree.is_none())
        })
        .await;
    assert_eq!(dropped.resume.unwrap().note, "the plan was paused");
    assert!(!Path::new(&tree).exists(), "the worktree is removed");

    h.edit_plan(WorkEdit::Resume);
    h.plan_until(|plan| plan.header.status == PlanStatus::Active)
        .await;
    helper(&h, "PAUSE-TWO", "t2", "b", 0);
    let replies = h.dispatch_turn(&[("t2", "PAUSE-TWO")]).await;
    assert!(!replies[0].0, "{replies:?}");
    assert_eq!(h.finished("t2").await.status, TaskStatus::Done);
    assert_eq!(h.repo.read("src/b/lib.rs"), "// work\n");
}

#[tokio::test]
async fn holding_a_task_stops_only_its_helper_until_it_is_released() {
    let mut h = planned().await;
    helper(&h, "HOLD-ONE", "t1", "a", 1500);
    helper(&h, "HOLD-TWO", "t2", "b", 1500);
    let calls = [
        ("Agent", dispatch("t1", "HOLD-ONE")),
        ("Agent", dispatch("t2", "HOLD-TWO")),
    ];
    h.model.push(Script::tools(&calls));
    h.model.push(Script::text("Dispatched."));
    h.submit("run both");
    let started = h.agents_started(2).await;
    let held = helper_of(&started, "t1");
    let other = helper_of(&started, "t2");
    h.edit_plan(WorkEdit::HoldTask {
        task_id: task("t1"),
        held: true,
    });
    settle(&mut h).await;

    assert_eq!(h.agent_info(&held.agent_id).status, AgentStatus::Cancelled);
    assert_eq!(h.agent_info(&other.agent_id).status, AgentStatus::Completed);
    let t1 = h.plan().task(&task("t1")).unwrap().clone();
    assert_eq!((t1.status, t1.held), (TaskStatus::Pending, true));
    assert_eq!(t1.resume.unwrap().note, "the user held the task");
    assert_eq!(h.finished("t2").await.status, TaskStatus::Done);

    let replies = h.dispatch_turn(&[("t1", "HOLD-AGAIN")]).await;
    assert!(
        replies[0].0 && replies[0].1.contains("the user holds it"),
        "{replies:?}"
    );
    h.edit_plan(WorkEdit::ReleaseTask {
        task_id: task("t1"),
    });
    h.task_until("t1", |t| !t.held).await;
    helper(&h, "HOLD-THREE", "t1", "a", 0);
    let replies = h.dispatch_turn(&[("t1", "HOLD-THREE")]).await;
    assert!(!replies[0].0, "{replies:?}");
    assert_eq!(h.finished("t1").await.status, TaskStatus::Done);
}

#[tokio::test]
async fn stopping_the_plan_cancels_every_helper_without_resume_pointers() {
    let mut h = planned().await;
    for marker in ["STOP-ONE", "STOP-TWO"] {
        route_task(&h.model, marker, vec![Script::Stall(Vec::new())]);
    }
    let calls = [
        ("Agent", dispatch("t1", "STOP-ONE")),
        ("Agent", dispatch("t2", "STOP-TWO")),
    ];
    h.model.push(Script::tools(&calls));
    h.model.push(Script::text("Stopped."));
    h.submit("run both");
    let started = h.agents_started(2).await;
    h.plan_until(|plan| {
        ["t1", "t2"]
            .iter()
            .all(|id| plan.task(&task(id)).unwrap().status == TaskStatus::Active)
    })
    .await;
    h.edit_plan(WorkEdit::Stop);
    settle(&mut h).await;

    for info in &started {
        assert_eq!(h.agent_info(&info.agent_id).status, AgentStatus::Cancelled);
    }
    let plan = h.plan();
    assert_eq!(plan.header.status, PlanStatus::Paused);
    for id in ["t1", "t2"] {
        let stopped = plan.task(&task(id)).unwrap();
        assert_eq!(stopped.status, TaskStatus::Pending, "{id}");
        assert!(stopped.resume.is_none() && stopped.owner.is_none(), "{id}");
    }
}
