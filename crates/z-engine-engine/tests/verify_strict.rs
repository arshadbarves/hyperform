//! `strict` verification keeps a changing turn going until the badge is
//! `Verified` or `max_continuations` is spent.

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness, last_user_text};
use z_engine_protocol::{Event, NoticeLevel, VerificationOutcome};
use z_engine_testkit::{FixtureRepo, Script};

fn settings(command: &str, budget: u32) -> String {
    format!(
        "{BASE_SETTINGS}\n[permissions]\nmode = \"bypass\"\n\n[verification]\nmode = \"strict\"\nmax_continuations = {budget}\nauto_checks = [\"test\"]\n\n[[verification.checks]]\nid = \"unit\"\ncommand = \"{command}\"\nkind = \"test\"\n"
    )
}

async fn harness(command: &str, budget: u32) -> Harness {
    Harness::builder(FixtureRepo::git(&[("README.md", "demo\n")]))
        .settings(&settings(command, budget))
        .trusted()
        .start()
        .await
}

#[tokio::test]
async fn strict_continues_until_the_checks_pass() {
    let mut h = harness("test -f ok.txt", 3).await;
    let write = |h: &Harness, name: &str| {
        Script::tool(
            "Write",
            json!({"file_path": h.path(name), "content": "x\n"}),
        )
    };
    h.model.push(write(&h, "a.txt"));
    h.model.push(Script::text("Done."));
    h.model.push(write(&h, "ok.txt"));
    h.model.push(Script::text("Fixed."));
    let turn = h.run_turn("change it").await;
    assert!(
        matches!(turn.verification, VerificationOutcome::Verified { .. }),
        "{:?}",
        turn.verification
    );
    assert_eq!(h.main_requests().len(), 4);
    let passed: Vec<bool> = h
        .events
        .seen()
        .iter()
        .filter_map(|e| match e {
            Event::CheckRecorded { record } => Some(record.passed),
            _ => None,
        })
        .collect();
    assert_eq!(passed, [false, true]);
}

#[tokio::test]
async fn strict_stops_when_the_budget_is_spent() {
    let mut h = harness("false", 2).await;
    h.model.push(Script::tool(
        "Write",
        json!({"file_path": h.path("a.txt"), "content": "x\n"}),
    ));
    for text in ["Done.", "Still failing.", "Giving up."] {
        h.model.push(Script::text(text));
    }
    let turn = h.run_turn("change it").await;
    assert!(
        matches!(turn.verification, VerificationOutcome::Failed { .. }),
        "{:?}",
        turn.verification
    );
    let requests = h.main_requests();
    assert_eq!(requests.len(), 4);
    assert!(last_user_text(&requests[2]).contains("The harness ran the project's checks"));
    assert!(
        last_user_text(&requests[3]).contains("the changes are not verified"),
        "{}",
        last_user_text(&requests[3])
    );
    assert_eq!(
        h.events.count(|e| matches!(e, Event::CheckRecorded { .. })),
        1
    );
    h.expect(|e| {
        matches!(e, Event::Notice { level: NoticeLevel::Warn, text } if text.contains("Verification ended after 2"))
    })
    .await;
}
