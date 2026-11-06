//! Steering sent while the session waits on an approval is not lost: it
//! rides along with the tool results of that round.

mod support;

use serde_json::json;
use support::{Harness, all_text};
use z_engine_protocol::{ApprovalDecision, Command, Event};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn steering_during_an_approval_reaches_the_next_request() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    h.model.push(Script::tool(
        "Write",
        json!({ "file_path": "draft.txt", "content": "draft" }),
    ));
    h.model.push(Script::text("adjusted"));
    h.submit("write a draft");

    let request = h.approval().await;
    h.send(Command::Steer {
        text: "also mention the deadline".into(),
    });
    h.wait(|e| matches!(e, Event::QueueChanged { queued } if queued.len() == 1))
        .await;
    h.send(Command::ResolveApproval {
        request_id: request.request_id,
        decision: ApprovalDecision::AllowOnce,
    });
    h.turn_finished().await;

    let requests = h.main_requests();
    assert_eq!(requests.len(), 2);
    let last = requests[1].messages.last().unwrap();
    let text = all_text(last);
    assert!(text.contains("also mention the deadline"), "{text}");
    assert!(
        h.events
            .count(|e| matches!(e, Event::UserMessage { steering: true, .. }))
            == 1
    );
    assert!(
        h.events
            .count(|e| matches!(e, Event::QueueChanged { queued } if queued.is_empty()))
            >= 1
    );
}
