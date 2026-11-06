//! `auto` verification: at the stop boundary of a turn that changed files
//! the harness runs the selected checks (trusted projects only), feeds a
//! failure back once within `max_continuations`, then finishes `Failed`.

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness, last_user_text};
use z_engine_protocol::{AgentId, Event, VerificationOutcome};
use z_engine_testkit::{FixtureRepo, Script};

fn settings() -> String {
    format!(
        "{BASE_SETTINGS}\n[permissions]\nmode = \"bypass\"\n\n[verification]\nmode = \"auto\"\nmax_continuations = 1\nauto_checks = [\"test\"]\n\n[[verification.checks]]\nid = \"unit\"\ncommand = \"test -f fixed.txt\"\nkind = \"test\"\n"
    )
}

fn repo() -> FixtureRepo {
    FixtureRepo::git(&[("README.md", "demo\n")])
}

#[tokio::test]
async fn auto_feeds_a_failure_back_once_then_finishes() {
    let mut h = Harness::builder(repo())
        .settings(&settings())
        .trusted()
        .start()
        .await;
    h.model.push(Script::tool(
        "Write",
        json!({"file_path": h.path("a.txt"), "content": "x\n"}),
    ));
    h.model.push(Script::text("Done."));
    h.model.push(Script::text("I could not fix it."));
    let turn = h.run_turn("change it").await;
    assert!(
        matches!(turn.verification, VerificationOutcome::Failed { .. }),
        "{:?}",
        turn.verification
    );
    let requests = h.main_requests();
    assert_eq!(requests.len(), 3);
    let fed_back = last_user_text(&requests[2]);
    assert!(
        fed_back.contains("The harness ran the project's checks")
            && fed_back.contains("exit code 1"),
        "{fed_back}"
    );
    let checks: Vec<Event> = h
        .events
        .seen()
        .iter()
        .filter(|e| matches!(e, Event::CheckRecorded { .. }))
        .cloned()
        .collect();
    assert_eq!(checks.len(), 1, "no re-run without new changes");
    let Event::CheckRecorded { record } = &checks[0] else {
        unreachable!()
    };
    assert_eq!(record.agent_id, AgentId::main());
}

#[tokio::test]
async fn auto_skips_checks_in_untrusted_projects() {
    let mut h = Harness::builder(repo()).settings(&settings()).start().await;
    h.model.push(Script::tool(
        "Write",
        json!({"file_path": h.path("a.txt"), "content": "x\n"}),
    ));
    h.model.push(Script::text("Done."));
    let turn = h.run_turn("change it").await;
    assert_eq!(
        turn.verification,
        VerificationOutcome::Unverified {
            reason: "automatic checks are disabled for untrusted projects".into()
        }
    );
    assert_eq!(h.main_requests().len(), 2);
    assert_eq!(
        h.events.count(|e| matches!(e, Event::CheckRecorded { .. })),
        0
    );
}
