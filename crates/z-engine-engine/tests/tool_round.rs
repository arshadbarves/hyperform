//! A Read then an Edit in acceptEdits mode: the file changes, every call
//! gets its result, and the changed turn is unverified.

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness, assert_valid_transcript, results};
use z_engine_protocol::{Event, ToolStatus, TurnOutcome, VerificationOutcome};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn read_then_edit_changes_the_file() {
    let repo = FixtureRepo::git(&[("src/lib.rs", "pub fn old() {}\n")]);
    let settings = format!("{BASE_SETTINGS}\n[permissions]\nmode = \"acceptEdits\"\n");
    let mut h = Harness::builder(repo).settings(&settings).start().await;
    let file = h.path("src/lib.rs");
    h.model
        .push(Script::tool("Read", json!({ "file_path": file })));
    h.model.push(Script::tool(
        "Edit",
        json!({ "file_path": file, "old_string": "old", "new_string": "new" }),
    ));
    h.model.push(Script::text("Renamed."));

    let turn = h.run_turn("rename old to new").await;
    assert_eq!(turn.outcome, TurnOutcome::Completed);
    assert!(
        matches!(turn.verification, VerificationOutcome::Unverified { .. }),
        "{:?}",
        turn.verification
    );
    assert_eq!(h.repo.read("src/lib.rs"), "pub fn new() {}\n");

    let transcript = h.transcript();
    assert_valid_transcript(&transcript);
    let requests = h.main_requests();
    assert_eq!(requests.len(), 3);
    for request in &requests[1..] {
        let last = request.messages.last().unwrap();
        let answered = results(last);
        assert_eq!(answered.len(), 1);
        assert!(!answered[0].1, "unexpected error result: {}", answered[0].2);
    }
    let finished = h.events.count(|e| {
        matches!(
            e,
            Event::ToolFinished {
                status: ToolStatus::Ok,
                ..
            }
        )
    });
    assert_eq!(finished, 2);
}
