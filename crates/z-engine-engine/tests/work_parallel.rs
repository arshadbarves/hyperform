//! Parallel helpers (`work_parallel`): disjoint ready tasks of a plan whose
//! parallel work the user approved run at the same time, each in its own
//! worktree; each reports, and with `merge = "auto"` both changes come back
//! into the project tree and the tasks finish there. Without the approval
//! a second task-bound helper is refused, and `work.max_parallel` caps how
//! many run at once.

mod support;

use std::time::Duration;

use serde_json::json;
use support::{Harness, new_task, parallel_settings, report, route_task, slow, task};
use z_engine_protocol::work::{Integration, ReportSource, TaskStatus};
use z_engine_protocol::{AgentStatus, Event, Isolation};
use z_engine_testkit::{FixtureRepo, Script};

const AUTO: &str = "[permissions]\nmode = \"bypass\"\n\n[work]\nmerge = \"auto\"\n";

/// A plan of three tasks (`t1`..`t3`), one directory each (`src/a`, ...).
async fn planned(extra: &str) -> Harness {
    let repo = FixtureRepo::git(&[("README.md", "fixture\n")]);
    let mut h = Harness::builder(repo)
        .settings(&parallel_settings(extra))
        .start()
        .await;
    for name in ["a", "b", "c"] {
        let scope = json!({ "scope": [format!("src/{name}")] });
        let input = new_task(&format!("Build {name}"), scope);
        h.model.push(Script::tool("TaskCreate", input));
    }
    h.model.push(Script::text("Planned."));
    h.settled_turn("split the work").await;
    h
}

/// The helper whose instructions carry `marker` writes `src/<name>/lib.rs`
/// after a pause and reports. Admissions take turns, each making its
/// helper's worktree, so the pause outlasts the next helper's start.
fn helper(h: &Harness, marker: &'static str, task_id: &str, name: &str) {
    let file = format!("src/{name}/lib.rs");
    let content = format!("// {name}\n");
    let summary = format!("Wrote {file}");
    let work = Script::tools(&[
        ("Write", json!({ "file_path": file, "content": content })),
        ("TaskUpdate", report(task_id, json!({ "summary": summary }))),
    ]);
    let rounds = vec![
        slow(work, Duration::from_millis(1500)),
        Script::text("Reported."),
    ];
    route_task(&h.model, marker, rounds);
}

fn refusals(replies: &[(bool, String)]) -> Vec<&String> {
    replies.iter().filter(|r| r.0).map(|r| &r.1).collect()
}

fn dispatched(h: &Harness, ids: &[&str]) -> Vec<String> {
    let plan = h.plan();
    ids.iter()
        .filter(|id| plan.task(&task(id)).unwrap().run.dispatch.is_some())
        .map(|id| id.to_string())
        .collect()
}

#[tokio::test]
async fn approved_disjoint_tasks_run_together_in_worktrees_and_merge() {
    let mut h = planned(AUTO).await;
    h.approve_parallel().await;
    helper(&h, "PAR-ONE", "t1", "a");
    helper(&h, "PAR-TWO", "t2", "b");
    let replies = h
        .dispatch_turn(&[("t1", "PAR-ONE"), ("t2", "PAR-TWO")])
        .await;
    assert!(refusals(&replies).is_empty(), "{replies:?}");

    let mut completed = Vec::new();
    let order: Vec<&str> = h
        .events
        .seen()
        .iter()
        .filter_map(|event| match event {
            Event::AgentStarted { info } if info.task_id.is_some() => Some("started"),
            Event::AgentUpdated { info }
                if info.task_id.is_some()
                    && info.status == AgentStatus::Completed
                    && !completed.contains(&info.agent_id) =>
            {
                completed.push(info.agent_id.clone());
                Some("completed")
            }
            _ => None,
        })
        .collect();
    assert_eq!(order, ["started", "started", "completed", "completed"]);

    let started = h.agents_started(2).await;
    let trees: Vec<String> = started
        .iter()
        .map(|info| {
            assert_eq!(info.isolation, Isolation::Worktree, "{info:?}");
            info.worktree.as_ref().expect("a worktree").path.clone()
        })
        .collect();
    assert_ne!(trees[0], trees[1]);

    for (id, name) in [("t1", "a"), ("t2", "b")] {
        let task = h.finished(id).await;
        assert_eq!(task.status, TaskStatus::Done, "{task:?}");
        let file = format!("src/{name}/lib.rs");
        assert_eq!(h.repo.read(&file), format!("// {name}\n"));
        let run = &task.run;
        assert!(run.dispatch.as_ref().unwrap().worktree.is_some());
        let applied = Integration::Applied {
            files: vec![file.clone()],
        };
        assert_eq!(run.integration, Some(applied));
        let report = run.report.as_ref().unwrap();
        assert_eq!(report.source, ReportSource::Worker);
        assert_eq!(report.files_changed, [file]);
        assert!(report.diff_known);
    }
    let idle = h.plan().task(&task("t3")).unwrap().status.clone();
    assert_eq!(idle, TaskStatus::Pending);
}

#[tokio::test]
async fn without_approval_a_second_task_helper_is_refused() {
    let mut h = planned(AUTO).await;
    helper(&h, "SOLO-ONE", "t1", "a");
    helper(&h, "SOLO-TWO", "t2", "b");
    let replies = h
        .dispatch_turn(&[("t1", "SOLO-ONE"), ("t2", "SOLO-TWO")])
        .await;
    // Admissions take turns in call order: the second sees the first's
    // helper and is refused before its helper starts.
    assert!(!replies[0].0, "{replies:?}");
    let refused = &replies[1].1;
    assert!(replies[1].0, "{replies:?}");
    assert!(
        refused.contains("a helper already works on t1") && refused.contains("approve"),
        "{refused}"
    );

    assert_eq!(dispatched(&h, &["t1", "t2"]), ["t1"]);
    let task_done = h.finished("t1").await;
    assert_eq!(task_done.status, TaskStatus::Done, "{task_done:?}");
    let idle = h.plan().task(&task("t2")).unwrap().clone();
    assert_eq!(idle.status, TaskStatus::Pending);
    assert!(idle.run.is_empty(), "{:?}", idle.run);
}

#[tokio::test]
async fn max_parallel_caps_the_helpers_running_at_once() {
    let settings = format!("{AUTO}max_parallel = 2\n");
    let mut h = planned(&settings).await;
    h.approve_parallel().await;
    helper(&h, "CAP-ONE", "t1", "a");
    helper(&h, "CAP-TWO", "t2", "b");
    helper(&h, "CAP-THREE", "t3", "c");
    let pairs = [("t1", "CAP-ONE"), ("t2", "CAP-TWO"), ("t3", "CAP-THREE")];
    let replies = h.dispatch_turn(&pairs).await;
    let refused = refusals(&replies);
    assert_eq!(refused.len(), 1, "{replies:?}");
    assert!(replies[2].0, "the third call is refused: {replies:?}");
    assert!(refused[0].contains("max_parallel 2"), "{}", refused[0]);

    let ran = dispatched(&h, &["t1", "t2", "t3"]);
    assert_eq!(ran, ["t1", "t2"]);
    for id in &ran {
        assert_eq!(h.finished(id).await.status, TaskStatus::Done);
    }
}
