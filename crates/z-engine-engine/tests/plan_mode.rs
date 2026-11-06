//! Plan mode: writes are refused, `ExitPlanMode` opens a plan review, and
//! approving it (Build) turns planning off, keeping the permission level
//! unless the decision overrides it, so the edit then runs. Plan mode
//! combines with any level: with bypass it researches without asking.

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness, last_user_text, results};
use z_engine_protocol::{Command, Event, PermissionMode, PlanDecision};
use z_engine_testkit::{FixtureRepo, Script};

/// Plans, has a write refused, proposes, gets `decision`, then writes.
async fn plan_then_build(permissions: &str, decision: PlanDecision) -> Harness {
    let settings = format!("{BASE_SETTINGS}\n[permissions]\n{permissions}\n");
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    let write = json!({ "file_path": "plan.txt", "content": "implemented" });
    h.model.push(Script::tool("Write", write.clone()));
    h.model.push(Script::tool(
        "ExitPlanMode",
        json!({ "plan": "1. Write plan.txt" }),
    ));
    h.model.push(Script::tool("Write", write));
    h.model.push(Script::text("Done."));
    h.submit("plan and then write the file");

    let request_id = match h.wait(|e| matches!(e, Event::PlanProposed { .. })).await {
        Event::PlanProposed {
            request_id, plan, ..
        } => {
            assert_eq!(plan, "1. Write plan.txt");
            request_id
        }
        other => panic!("{other:?}"),
    };
    h.send(Command::ResolvePlan {
        request_id,
        decision,
    });
    h.wait(|e| matches!(e, Event::PlanningChanged { planning: false }))
        .await;
    h.turn_finished().await;

    let requests = h.main_requests();
    assert!(last_user_text(&requests[0]).contains("Plan mode is active"));
    let refused = results(requests[1].messages.last().unwrap());
    assert!(
        refused[0].1 && refused[0].2.contains("plan mode"),
        "{:?}",
        refused[0]
    );
    let approved = results(requests[2].messages.last().unwrap());
    assert!(approved[0].2.contains("approved"), "{:?}", approved[0]);
    assert_eq!(h.repo.read("plan.txt"), "implemented");
    assert_eq!(
        h.events
            .count(|e| matches!(e, Event::PlanResolved { approved: true, .. })),
        1
    );
    h
}

async fn reopened(h: &mut Harness) -> (PermissionMode, bool) {
    h.reopen().await;
    match h.wait(|e| matches!(e, Event::Snapshot { .. })).await {
        Event::Snapshot { snapshot } => {
            assert!(snapshot.pending_plans.is_empty());
            (snapshot.info.mode, snapshot.info.planning)
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn build_turns_planning_off_and_keeps_the_level() {
    let build = PlanDecision::Approve {
        mode: None,
        edited_plan: None,
        handoff: false,
    };
    let mut h = plan_then_build("mode = \"acceptEdits\"\nplanning = true", build).await;
    assert_eq!(
        h.events.count(|e| matches!(e, Event::ModeChanged { .. })),
        0
    );
    assert_eq!(reopened(&mut h).await, (PermissionMode::AcceptEdits, false));
}

#[tokio::test]
async fn an_approval_may_override_the_level_and_old_plan_settings_still_plan() {
    let approve = PlanDecision::Approve {
        mode: Some(PermissionMode::AcceptEdits),
        edited_plan: None,
        handoff: false,
    };
    let mut h = plan_then_build("mode = \"plan\"", approve).await;
    assert_eq!(
        h.events.count(|e| matches!(
            e,
            Event::ModeChanged {
                mode: PermissionMode::AcceptEdits
            }
        )),
        1
    );
    assert_eq!(reopened(&mut h).await, (PermissionMode::AcceptEdits, false));
}

#[tokio::test]
async fn plan_with_bypass_researches_without_asking_and_never_writes() {
    let settings = format!("{BASE_SETTINGS}\n[permissions]\nmode = \"bypass\"\nplanning = true\n");
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    let outside = tempfile::tempdir().unwrap();
    let notes = outside.path().join("notes.txt");
    std::fs::write(&notes, "outside notes").unwrap();
    h.model.push(Script::tool(
        "Read",
        json!({ "file_path": notes.to_string_lossy() }),
    ));
    h.model.push(Script::tool(
        "Write",
        json!({ "file_path": "x.txt", "content": "no" }),
    ));
    h.model
        .push(Script::tool("Bash", json!({ "command": "cargo test" })));
    h.model.push(Script::text("Here is what I found."));
    h.run_turn("look around first").await;

    let requests = h.main_requests();
    let read = results(requests[1].messages.last().unwrap());
    assert!(
        !read[0].1 && read[0].2.contains("outside notes"),
        "{read:?}"
    );
    for request in &requests[2..4] {
        let refused = results(request.messages.last().unwrap());
        assert!(
            refused[0].1 && refused[0].2.contains("plan mode"),
            "{refused:?}"
        );
    }
    assert!(!h.repo.path().join("x.txt").exists());
    assert_eq!(
        h.events
            .count(|e| matches!(e, Event::ApprovalRequested { .. })),
        0
    );
}

#[tokio::test]
async fn set_planning_toggles_plan_mode_and_survives_a_reopen() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    h.send(Command::SetPlanning { planning: true });
    h.wait(|e| matches!(e, Event::PlanningChanged { planning: true }))
        .await;
    h.send(Command::SetMode {
        mode: PermissionMode::AcceptEdits,
    });
    h.wait(|e| matches!(e, Event::ModeChanged { .. })).await;
    assert_eq!(reopened(&mut h).await, (PermissionMode::AcceptEdits, true));
}
