//! Overlap and merge conflicts between parallel helpers: tasks whose scopes
//! overlap are never eligible together, so they run one after the other
//! (and a task's own merged change never reads as its stale evidence; the
//! plan is Done once both are); a
//! helper's change that conflicts with the project tree blocks its task,
//! leaves the tree byte-identical and keeps the worktree, and a serial
//! retry runs it again on the new HEAD (reopening the chat keeps that
//! worktree until then). With `merge = "ask"` the user's
//! Apply verifies the task in the project tree (Verifying, then Done after
//! its check), and Discard blocks it as `changeDiscarded`.

mod support;

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use support::{Harness, new_task, parallel_settings, report, route_task, task};
use z_engine_protocol::work::{
    BlockReason, CriterionStatus, Integration, ParallelBlock, PlanStatus, TaskStatus, WorkEdit,
};
use z_engine_protocol::{AgentId, Command, Event};
use z_engine_testkit::{FixtureRepo, Script};

const AUTO: &str = "[permissions]\nmode = \"bypass\"\n\n[work]\nmerge = \"auto\"\n";
const ASK: &str = "[permissions]\nmode = \"bypass\"\n\n[work]\nmerge = \"ask\"\n\n\
    [[verification.checks]]\nid = \"unit\"\nlabel = \"Unit tests\"\ncommand = \"echo all good\"\nkind = \"test\"\n";

/// An approved plan of `tasks` (`(subject, fields)`) over `src/core/lib.rs`.
async fn planned(extra: &str, tasks: &[(&str, Value)]) -> Harness {
    let repo = FixtureRepo::git(&[("src/core/lib.rs", "base\n")]);
    let mut h = Harness::builder(repo)
        .settings(&parallel_settings(extra))
        .start()
        .await;
    for (subject, fields) in tasks {
        let input = new_task(subject, fields.clone());
        h.model.push(Script::tool("TaskCreate", input));
    }
    h.model.push(Script::text("Planned."));
    h.settled_turn("plan the change").await;
    h.approve_parallel().await;
    h
}

/// The helper with `marker` writes `content` to `file` (reading it first
/// when it exists) and reports on `task_id`.
fn helper(h: &Harness, marker: &'static str, task_id: &str, file: &str, content: &str) {
    let mut rounds = Vec::new();
    if h.repo.exists(file) {
        rounds.push(Script::tool("Read", json!({ "file_path": file })));
    }
    rounds.push(Script::tools(&[
        ("Write", json!({ "file_path": file, "content": content })),
        (
            "TaskUpdate",
            report(task_id, json!({ "summary": format!("Wrote {file}") })),
        ),
    ]));
    rounds.push(Script::text("Reported."));
    route_task(&h.model, marker, rounds);
}

/// Every file of the project tree (outside `.git` and `.hyperform`).
fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if name == ".git" || name == ".hyperform" {
                continue;
            }
            if path.is_dir() {
                dirs.push(path);
            } else {
                files.insert(name, std::fs::read(&path).unwrap());
            }
        }
    }
    files
}

async fn gone(path: &str) {
    for _ in 0..100 {
        if !Path::new(path).exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("{path} still exists");
}

async fn pending_change(h: &mut Harness, id: &str) -> AgentId {
    let waiting = h
        .task_until(id, |t| {
            matches!(t.run.integration, Some(Integration::PendingApply { .. }))
        })
        .await;
    waiting.run.dispatch.unwrap().agent_id
}

#[tokio::test]
async fn overlapping_tasks_are_never_eligible_together_and_run_serially() {
    let scope = json!({ "scope": ["src/core"] });
    let mut h = planned(AUTO, &[("One", scope.clone()), ("Two", scope)]).await;
    let set = h.engine.work_parallel(&h.session).unwrap();
    assert_eq!(set.eligible, [task("t1")]);
    let excluded = set
        .excluded
        .iter()
        .find(|e| e.task_id == task("t2"))
        .unwrap();
    assert!(
        matches!(&excluded.block, ParallelBlock::Overlap { task_id, .. } if *task_id == task("t1")),
        "{excluded:?}"
    );

    helper(&h, "OVER-ONE", "t1", "src/core/one.rs", "one\n");
    helper(&h, "OVER-TWO", "t2", "src/core/two.rs", "two\n");
    let replies = h
        .dispatch_turn(&[("t1", "OVER-ONE"), ("t2", "OVER-TWO")])
        .await;
    assert!(!replies[0].0, "{replies:?}");
    assert!(
        replies[1].0 && replies[1].1.contains("overlaps task t1"),
        "{replies:?}"
    );
    assert_eq!(h.finished("t1").await.status, TaskStatus::Done);

    let replies = h.dispatch_turn(&[("t2", "OVER-TWO")]).await;
    assert!(!replies[0].0, "{replies:?}");
    assert_eq!(h.finished("t2").await.status, TaskStatus::Done);
    let plan = h.plan();
    assert!(plan.findings.is_empty(), "{:?}", plan.findings);
    assert_eq!(plan.header.status, PlanStatus::Done, "every task closed");
    assert_eq!(h.repo.read("src/core/one.rs"), "one\n");
    assert_eq!(h.repo.read("src/core/two.rs"), "two\n");
}

#[tokio::test]
async fn a_conflict_keeps_the_tree_and_a_serial_retry_verifies_after_apply() {
    let fields = json!({
        "scope": ["src/core"],
        "acceptance": [{ "text": "Unit tests pass", "method": "test" }]
    });
    let mut h = planned(ASK, &[("Change core", fields)]).await;
    helper(&h, "CONFLICT-ONE", "t1", "src/core/lib.rs", "helper\n");
    let replies = h.dispatch_turn(&[("t1", "CONFLICT-ONE")]).await;
    assert!(!replies[0].0, "{replies:?}");
    let first = pending_change(&mut h, "t1").await;

    h.repo.write("src/core/lib.rs", "user\n");
    let before = snapshot(h.repo.path());
    h.send(Command::ApplyAgentChanges {
        agent_id: first.clone(),
    });
    let blocked = h
        .task_until("t1", |t| {
            matches!(
                t.status,
                TaskStatus::Blocked {
                    reason: BlockReason::MergeConflict { .. }
                }
            )
        })
        .await;
    assert_eq!(
        snapshot(h.repo.path()),
        before,
        "the project tree is untouched"
    );
    let Some(Integration::Conflict { paths, worktree }) = blocked.run.integration.clone() else {
        panic!("{:?}", blocked.run);
    };
    assert_eq!(paths, ["src/core/lib.rs"]);
    assert!(Path::new(&worktree).exists(), "the worktree is kept");

    h.repo.run_git(&["commit", "-qam", "user change"]);
    h.edit_plan(WorkEdit::RetrySerially {
        task_id: task("t1"),
    });
    let retried = h
        .task_until("t1", |t| t.status == TaskStatus::Pending && t.run.serial)
        .await;
    assert!(retried.run.integration.is_none(), "{:?}", retried.run);
    gone(&worktree).await;

    helper(
        &h,
        "CONFLICT-TWO",
        "t1",
        "src/core/lib.rs",
        "user\nhelper\n",
    );
    let replies = h.dispatch_turn(&[("t1", "CONFLICT-TWO")]).await;
    assert!(!replies[0].0, "{replies:?}");
    let second = pending_change(&mut h, "t1").await;
    assert_ne!(second, first);
    h.send(Command::ApplyAgentChanges { agent_id: second });
    let done = h.finished("t1").await;
    assert_eq!(done.status, TaskStatus::Done, "{done:?}");
    assert_eq!(done.acceptance[0].status, CriterionStatus::Met);
    assert_eq!(h.repo.read("src/core/lib.rs"), "user\nhelper\n");
    let verified = h.events.seen().iter().any(|event| {
        matches!(event, Event::WorkUpdated { update }
            if update.tasks.iter().any(|t| t.status == TaskStatus::Verifying))
    });
    assert!(verified, "the task verified in the project tree");
}

#[tokio::test]
async fn discarding_a_task_change_blocks_the_task() {
    let mut h = planned(ASK, &[("Add core", json!({ "scope": ["src/core"] }))]).await;
    helper(&h, "DISCARD-ONE", "t1", "src/core/new.rs", "new\n");
    let replies = h.dispatch_turn(&[("t1", "DISCARD-ONE")]).await;
    assert!(!replies[0].0, "{replies:?}");
    let agent = pending_change(&mut h, "t1").await;

    h.send(Command::DiscardAgentChanges { agent_id: agent });
    let blocked = h
        .task_until("t1", |t| matches!(t.status, TaskStatus::Blocked { .. }))
        .await;
    assert_eq!(
        blocked.status,
        TaskStatus::Blocked {
            reason: BlockReason::ChangeDiscarded
        }
    );
    assert_eq!(blocked.run.integration, Some(Integration::Discarded));
    assert!(!h.repo.exists("src/core/new.rs"));
}

#[tokio::test]
async fn reopening_the_chat_keeps_a_conflicts_worktree() {
    let mut h = planned(ASK, &[("Change core", json!({ "scope": ["src/core"] }))]).await;
    helper(&h, "KEPT-ONE", "t1", "src/core/lib.rs", "helper\n");
    let replies = h.dispatch_turn(&[("t1", "KEPT-ONE")]).await;
    assert!(!replies[0].0, "{replies:?}");
    let agent = pending_change(&mut h, "t1").await;
    h.repo.write("src/core/lib.rs", "user\n");
    h.send(Command::ApplyAgentChanges { agent_id: agent });
    let blocked = h
        .task_until("t1", |t| {
            matches!(t.run.integration, Some(Integration::Conflict { .. }))
        })
        .await;
    let Some(Integration::Conflict { worktree, .. }) = blocked.run.integration else {
        panic!("{:?}", blocked.run);
    };

    h.events.drain();
    h.reopen().await;
    h.wait(|e| matches!(e, Event::Snapshot { .. })).await;
    assert!(
        Path::new(&worktree).exists(),
        "the worktree survives a reopen"
    );
    let kept = h.plan().task(&task("t1")).unwrap().clone();
    assert!(
        matches!(kept.run.integration, Some(Integration::Conflict { .. })),
        "{:?}",
        kept.run
    );

    h.repo.run_git(&["commit", "-qam", "user change"]);
    h.edit_plan(WorkEdit::RetrySerially {
        task_id: task("t1"),
    });
    h.task_until("t1", |t| t.status == TaskStatus::Pending && t.run.serial)
        .await;
    gone(&worktree).await;
}
