//! One chat drives a plan. Attaching it in a second chat moves it there: a
//! live first chat lets it go at once (it logs `planDetached`, says "This
//! plan moved to another chat." and its helper stops at its next round,
//! recording its own resume pointer, which the new chat's resume leaves
//! alone); a closed one logs that when it opens again. The plan's summary
//! names the chat it is attached to now (`attachedTo`, cleared by a
//! detach) and the chat that had it last. `needsYou` counts a merge
//! conflict but not a medium-severity audit finding.

mod support;

use std::time::Duration;

use serde_json::json;
use support::{
    Harness, dispatch, new_task, parallel_settings, report, route_task, slow, work_settings,
};
use z_engine_protocol::work::{
    AuditSeverity, AuditStatus, BlockReason, Integration, PlanSummary, TaskStatus,
};
use z_engine_protocol::{AgentStatus, Command, SessionId};
use z_engine_testkit::{FixtureRepo, Script};

const BYPASS: &str = "[permissions]\nmode = \"bypass\"\n";
const ASK: &str = "[permissions]\nmode = \"bypass\"\n\n[work]\nmerge = \"ask\"\n";

/// A plan of one task over `src/a` in `settings`.
async fn planned(settings: &str) -> Harness {
    let repo = FixtureRepo::git(&[("src/a/lib.rs", "base\n")]);
    let mut h = Harness::builder(repo).settings(settings).start().await;
    let input = new_task("Change a", json!({ "scope": ["src/a"] }));
    h.model.push(Script::tool("TaskCreate", input));
    h.model.push(Script::text("Planned."));
    h.settled_turn("plan the change").await;
    h
}

fn summary(h: &Harness) -> PlanSummary {
    h.engine.work_plans(h.repo.path()).unwrap().remove(0)
}

async fn summary_until(h: &Harness, done: impl Fn(&PlanSummary) -> bool) -> PlanSummary {
    for _ in 0..200 {
        let found = summary(h);
        if done(&found) {
            return found;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the plan summary never matched: {:?}", summary(h));
}

/// `session`'s log recorded that the chat let its plan go.
fn logged_detach(h: &Harness, session: &SessionId) -> bool {
    let log = h
        .paths
        .sessions_dir
        .join(session.as_str())
        .join("log.jsonl");
    std::fs::read_to_string(log).is_ok_and(|text| text.contains("\"kind\":\"planDetached\""))
}

/// The helper with `marker` reads and rewrites `src/a/lib.rs` (after
/// `delay`), then reports.
fn helper(h: &Harness, marker: &'static str, delay: u64) {
    let file = "src/a/lib.rs";
    let work = Script::tools(&[
        ("Read", json!({ "file_path": file })),
        ("Write", json!({ "file_path": file, "content": "helper\n" })),
    ]);
    let rounds = vec![
        slow(work, Duration::from_millis(delay)),
        Script::tool("TaskUpdate", report("t1", json!({ "summary": "Wrote a" }))),
        Script::text("Reported."),
    ];
    route_task(&h.model, marker, rounds);
}

#[tokio::test]
async fn attaching_in_a_second_chat_moves_the_plan_from_the_live_first_one() {
    let mut h = planned(&work_settings(BYPASS)).await;
    let (plan_id, first) = (h.plan_id(), h.session.clone());
    assert_eq!(summary(&h).attached_to, Some(first.clone()));
    helper(&h, "MOVE-ONE", 1500);
    h.model
        .push(Script::tools(&[("Agent", dispatch("t1", "MOVE-ONE"))]));
    h.model.push(Script::text("It moved."));
    h.submit("run t1");
    let stopped = h.agent_started().await;

    let second = h.engine.open_session(h.repo.path(), None).await.unwrap();
    h.engine
        .send(&second, Command::AttachPlan { plan_id })
        .unwrap();
    h.notice("This plan moved to another chat.").await;
    let ended = h.agent_finished(&stopped.agent_id).await;
    assert_eq!(ended.status, AgentStatus::Cancelled);
    assert_eq!(
        h.repo.read("src/a/lib.rs"),
        "helper\n",
        "its round finished"
    );
    h.turn_finished().await;

    assert!(logged_detach(&h, &first));
    let moved = summary_until(&h, |s| s.attached_to.as_ref() == Some(&second)).await;
    assert_eq!(moved.last_session_id, Some(second.clone()));
    let t1 = h
        .task_until("t1", |t| t.status == TaskStatus::Pending)
        .await;
    let pointer = t1.resume.expect("the task waits for its helper");
    assert_eq!(
        pointer.note, "the plan moved to another chat",
        "{pointer:?}"
    );
    assert_eq!(
        (pointer.session_id, pointer.agent_id),
        (first, stopped.agent_id)
    );
    assert_eq!(summary(&h).running, 0);
}

#[tokio::test]
async fn a_closed_chat_lets_the_plan_go_when_it_opens_and_a_detach_clears_it() {
    let h = planned(&work_settings(BYPASS)).await;
    let (plan_id, first) = (h.plan_id(), h.session.clone());
    h.engine.close_session(&first).await.unwrap();
    assert_eq!(
        summary(&h).attached_to,
        Some(first.clone()),
        "closing keeps it"
    );

    let second = h.engine.open_session(h.repo.path(), None).await.unwrap();
    h.engine
        .send(&second, Command::AttachPlan { plan_id })
        .unwrap();
    let moved = summary_until(&h, |s| s.attached_to.as_ref() == Some(&second)).await;
    assert_eq!(moved.last_session_id, Some(second.clone()));
    assert!(
        !logged_detach(&h, &first),
        "the closed chat has not seen it yet"
    );
    let reopened = h.engine.open_session(h.repo.path(), Some(first.clone()));
    assert_eq!(reopened.await.unwrap(), first);
    assert!(logged_detach(&h, &first));
    assert_eq!(summary(&h).attached_to, Some(second.clone()));

    h.engine.send(&second, Command::DetachPlan).unwrap();
    let detached = summary_until(&h, |s| s.attached_to.is_none()).await;
    assert_eq!(detached.last_session_id, Some(second), "it had it last");
}

#[tokio::test]
async fn needs_you_counts_a_conflict_but_not_a_medium_finding() {
    let mut h = planned(&parallel_settings(ASK)).await;
    h.approve_parallel().await;
    let medium = |h: &Harness| {
        h.plan().audit.iter().any(|finding| {
            finding.status == AuditStatus::Open && finding.severity == AuditSeverity::Medium
        })
    };
    assert!(medium(&h), "{:?}", h.plan().audit);
    assert_eq!(summary(&h).needs_you, 0, "a medium finding only shows");

    helper(&h, "NEEDS-ONE", 0);
    let replies = h.dispatch_turn(&[("t1", "NEEDS-ONE")]).await;
    assert!(!replies[0].0, "{replies:?}");
    let waiting = h
        .task_until("t1", |t| {
            matches!(t.run.integration, Some(Integration::PendingApply { .. }))
        })
        .await;
    h.repo.write("src/a/lib.rs", "user\n");
    h.send(Command::ApplyAgentChanges {
        agent_id: waiting.run.dispatch.unwrap().agent_id,
    });
    h.task_until("t1", |t| {
        matches!(
            t.status,
            TaskStatus::Blocked {
                reason: BlockReason::MergeConflict { .. }
            }
        )
    })
    .await;
    assert!(medium(&h), "the medium finding is still open");
    assert_eq!(summary(&h).needs_you, 1, "the conflict alone");
}
