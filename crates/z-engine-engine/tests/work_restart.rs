//! Quitting and restarting the app with a work plan: a helper running when
//! the engine shuts down leaves its task Pending with a resume pointer and
//! its kept worktree; the restarted chat records one resume plan change
//! ("Resumed: 1 task was interrupted") and its first turn hears it in the
//! resume reminder; a quiet second reopen records nothing; and only the
//! helper the pointer names continues the task, in its worktree. A file
//! edited while the app was closed becomes a reconciliation finding whose
//! resume plan change carries the diff.

mod support;

use std::path::Path;
use std::time::Duration;

use serde_json::json;
use support::{
    Harness, dispatch, last_user_text, new_task, parallel_settings, report, route_task, task,
    task_requests, tool_reply, update_task, work_settings,
};
use z_engine_protocol::work::{
    ChangeCause, EntryBody, EvidenceRef, PlanChange, PlanStatus, ReconcileFinding, TaskStatus,
};
use z_engine_testkit::{FixtureRepo, Script};

const AUTO: &str = "[permissions]\nmode = \"bypass\"\n\n[work]\nmerge = \"auto\"\n";
const MARKER: &str = "RESTART-ONE";

/// The plan's resume plan changes, oldest first.
fn resumes(h: &Harness) -> Vec<PlanChange> {
    let plan = h.plan();
    let mut entries: Vec<_> = plan.entries.values().collect();
    entries.sort_by_key(|entry| entry.created_revision);
    entries
        .into_iter()
        .filter_map(|entry| match &entry.body {
            EntryBody::PlanChange(change) if change.cause == ChangeCause::Resume => {
                Some(change.clone())
            }
            _ => None,
        })
        .collect()
}

/// Waits until the helper with `marker` sent `n` requests.
async fn asked(h: &Harness, marker: &str, n: usize) {
    for _ in 0..400 {
        if task_requests(&h.model, marker).len() >= n {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the helper {marker} sent fewer than {n} requests");
}

#[tokio::test]
async fn a_helper_running_at_shutdown_waits_with_its_worktree_after_a_restart() {
    let repo = FixtureRepo::git(&[("src/core/lib.rs", "base\n")]);
    let mut h = Harness::builder(repo)
        .settings(&parallel_settings(AUTO))
        .start()
        .await;
    let scope = json!({ "scope": ["src/core"] });
    h.model
        .push(Script::tool("TaskCreate", new_task("Add one", scope)));
    h.model.push(Script::text("Planned."));
    h.settled_turn("plan one").await;
    h.approve_parallel().await;
    let write = json!({ "file_path": "src/core/one.rs", "content": "one\n" });
    route_task(
        &h.model,
        MARKER,
        vec![Script::tool("Write", write), Script::Stall(Vec::new())],
    );
    h.model
        .push(Script::tools(&[("Agent", dispatch("t1", MARKER))]));
    h.submit("run t1");
    let helper = h.agent_started().await;
    asked(&h, MARKER, 2).await;

    let mut h = h.shut_down().await.restart().await;
    let t1 = h.plan().task(&task("t1")).unwrap().clone();
    assert_eq!(t1.status, TaskStatus::Pending);
    let pointer = t1.resume.expect("a resume pointer");
    assert_eq!(pointer.agent_id, helper.agent_id);
    assert_eq!(pointer.note, "the chat closed");
    let tree = pointer.worktree.expect("the kept worktree");
    assert!(Path::new(&tree).join("src/core/one.rs").is_file(), "{tree}");
    let [change] = resumes(&h).try_into().unwrap();
    assert_eq!(change.reason, "Resumed: 1 task was interrupted");
    assert_eq!(change.affected, [task("t1")]);

    h.model.push(Script::text("Picking up."));
    h.settled_turn("where were we?").await;
    let reminded = last_user_text(h.main_requests().last().unwrap());
    let line = format!(
        "interrupted: the chat closed; resume agent {}",
        helper.agent_id
    );
    assert!(
        reminded.contains("picks up the work plan")
            && reminded.contains("Resumed: 1 task was interrupted")
            && reminded.contains(&line),
        "{reminded}"
    );

    let revision = h.plan().revision();
    h.reopen().await;
    assert_eq!(
        h.plan().revision(),
        revision,
        "a quiet reopen records nothing"
    );
    assert_eq!(resumes(&h).len(), 1);

    let mut other = dispatch("t1", "RESTART-OTHER");
    other["resume"] = json!("agt_other");
    h.model.push(Script::tool("Agent", other));
    h.model.push(Script::text("Refused."));
    h.settled_turn("resume t1 with another agent").await;
    let (error, refused) = tool_reply(h.main_requests().last().unwrap());
    let only = format!(
        "only agent {} (its resume pointer) continues it",
        helper.agent_id
    );
    assert!(
        error && refused.contains("Task t1 cannot be resumed") && refused.contains(&only),
        "{refused}"
    );

    let summary = report("t1", json!({ "summary": "Wrote src/core/one.rs" }));
    let rounds = vec![
        Script::tool("TaskUpdate", summary),
        Script::text("Reported."),
    ];
    route_task(&h.model, MARKER, rounds);
    let mut same = dispatch("t1", MARKER);
    same["resume"] = json!(helper.agent_id.as_str());
    h.model.push(Script::tool("Agent", same));
    h.model.push(Script::text("Resumed."));
    h.settled_turn("continue t1").await;
    let (error, text) = tool_reply(h.main_requests().last().unwrap());
    assert!(!error, "{text}");
    let done = h.finished("t1").await;
    assert_eq!(done.status, TaskStatus::Done, "{done:?}");
    let dispatch = done.run.dispatch.unwrap();
    assert_eq!(dispatch.agent_id, helper.agent_id, "the same helper");
    assert_eq!(
        dispatch.worktree.as_deref(),
        Some(tree.as_str()),
        "in its worktree"
    );
    assert_eq!(h.repo.read("src/core/one.rs"), "one\n");
}

#[tokio::test]
async fn a_file_edited_while_the_app_was_closed_is_a_finding_with_its_diff() {
    let repo = FixtureRepo::git(&[("src/a.rs", "fn a() {}\n")]);
    let settings = work_settings("[permissions]\nmode = \"bypass\"\n");
    let mut h = Harness::builder(repo).settings(&settings).start().await;
    let calls = [
        (
            "TaskCreate",
            new_task("Tidy a", json!({ "scope": ["src/a.rs"] })),
        ),
        ("TaskCreate", new_task("Document a", json!({}))),
        (
            "TaskUpdate",
            update_task("t1", json!({ "status": "in_progress" })),
        ),
        ("Read", json!({ "file_path": "src/a.rs" })),
        (
            "Write",
            json!({ "file_path": "src/a.rs", "content": "fn a() { tidy() }\n" }),
        ),
        (
            "TaskUpdate",
            update_task("t1", json!({ "status": "completed" })),
        ),
    ];
    for (tool, input) in calls {
        h.model.push(Script::tool(tool, input));
    }
    h.model.push(Script::text("Tidied."));
    h.settled_turn("tidy a").await;
    assert_eq!(h.plan().task(&task("t1")).unwrap().status, TaskStatus::Done);

    let remains = h.shut_down().await;
    remains.repo.write("src/a.rs", "fn a() { changed() }\n");
    let h = remains.restart().await;
    let plan = h.plan();
    assert_eq!(plan.header.status, PlanStatus::Replanning);
    assert!(
        matches!(plan.findings.as_slice(),
            [ReconcileFinding::StaleEvidence { task_id, files, .. }]
                if *task_id == task("t1") && files == &["src/a.rs"]),
        "{:?}",
        plan.findings
    );
    let [change] = resumes(&h).try_into().unwrap();
    assert_eq!(change.reason, "Resumed: 1 file changed outside the plan");
    assert_eq!(change.affected, [task("t1")]);
    let [
        EvidenceRef::Diff {
            files,
            checkpoint,
            summary,
        },
    ] = change.evidence.as_slice()
    else {
        panic!("{:?}", change.evidence);
    };
    assert_eq!(files, &["src/a.rs"]);
    assert!(
        checkpoint.is_some(),
        "the diff starts at the plan's checkpoint"
    );
    assert_eq!(summary, "changed outside the plan");
}
