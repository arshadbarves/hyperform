//! `/add-dir` lets the session read another directory without asking;
//! with `--save` it is also written to the project's local settings.

mod support;

use serde_json::json;
use support::{Harness, results};
use z_engine_protocol::Event;
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn added_directories_are_allowed_and_saved() {
    let shared = tempfile::tempdir().unwrap();
    let data = std::fs::canonicalize(shared.path()).unwrap();
    std::fs::write(data.join("notes.txt"), "shared notes\n").unwrap();
    let mut h = Harness::start(FixtureRepo::empty()).await;

    h.command("add-dir", &format!("{} --save", data.display()));
    let answer = h.command_output("add-dir").await;
    assert!(answer.contains(&data.display().to_string()), "{answer}");
    assert!(answer.contains("settings.local.toml"), "{answer}");
    let local = h.repo.read(".z-engine/settings.local.toml");
    assert!(local.contains("additional_directories"), "{local}");
    assert!(local.contains(&data.display().to_string()), "{local}");

    let file = data.join("notes.txt").display().to_string();
    h.model
        .push(Script::tool("Read", json!({ "file_path": file })));
    h.model.push(Script::text("read it"));
    h.run_turn("read the shared notes").await;
    assert_eq!(
        h.events
            .count(|e| matches!(e, Event::ApprovalRequested { .. })),
        0
    );
    let requests = h.main_requests();
    let read = results(requests[1].messages.last().unwrap());
    assert!(!read[0].1 && read[0].2.contains("shared notes"), "{read:?}");

    h.command("add-dir", "");
    h.notice("Usage: /add-dir").await;
}
