//! A command's `allowed-tools` hold for its own turn only: the same call
//! in the next turn asks again.

mod support;

use serde_json::json;
use support::Harness;
use z_engine_protocol::{ApprovalDecision, Command, Event};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn allowed_tools_are_turn_scoped() {
    let repo = FixtureRepo::empty();
    repo.write(
        ".z-engine/commands/make.md",
        "---\nallowed-tools: Bash(touch:*)\n---\nCreate the files.",
    );
    let mut h = Harness::builder(repo).trusted().start().await;

    h.model
        .push(Script::tool("Bash", json!({ "command": "touch one.txt" })));
    h.model.push(Script::text("made one"));
    h.command("make", "");
    h.turn_finished().await;
    assert!(h.repo.exists("one.txt"));
    assert_eq!(
        h.events
            .count(|e| matches!(e, Event::ApprovalRequested { .. })),
        0
    );

    h.model
        .push(Script::tool("Bash", json!({ "command": "touch two.txt" })));
    h.model.push(Script::text("asked"));
    h.submit("make another");
    let request = h.approval().await;
    assert_eq!(request.tool, "Bash");
    h.send(Command::ResolveApproval {
        request_id: request.request_id,
        decision: ApprovalDecision::Deny { feedback: None },
    });
    h.turn_finished().await;
    assert!(!h.repo.exists("two.txt"));
}
