//! The approval flow: "allow for this session" grants a rule so the next
//! matching call runs without asking; a denial with feedback reaches the
//! model as an error result.

mod support;

use serde_json::json;
use support::{Harness, results};
use z_engine_protocol::{ApprovalDecision, Command, Event, SessionStatus, ToolStatus};
use z_engine_testkit::{FixtureRepo, Script};

#[tokio::test]
async fn allow_session_grants_a_rule_for_later_calls() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    h.model.push(Script::tool(
        "Write",
        json!({ "file_path": "one.txt", "content": "1" }),
    ));
    h.model.push(Script::tool(
        "Write",
        json!({ "file_path": "two.txt", "content": "2" }),
    ));
    h.model.push(Script::text("written"));
    h.submit("write two files");

    let request = h.approval().await;
    assert_eq!(request.tool, "Write");
    assert!(request.can_persist);
    let rule = request.suggested_rule.clone().expect("a suggested rule");
    h.expect(|e| {
        matches!(
            e,
            Event::StatusChanged {
                status: SessionStatus::Waiting
            }
        )
    })
    .await;
    h.send(Command::ResolveApproval {
        request_id: request.request_id.clone(),
        decision: ApprovalDecision::AllowSession { rule },
    });
    h.turn_finished().await;

    assert_eq!(
        h.events
            .count(|e| matches!(e, Event::ApprovalRequested { .. })),
        1
    );
    assert_eq!(
        h.events
            .count(|e| matches!(e, Event::ApprovalResolved { allowed: true, .. })),
        1
    );
    assert_eq!(h.repo.read("one.txt"), "1");
    assert_eq!(h.repo.read("two.txt"), "2");
}

#[tokio::test]
async fn denial_feedback_reaches_the_model() {
    let mut h = Harness::start(FixtureRepo::empty()).await;
    h.model.push(Script::tool(
        "Write",
        json!({ "file_path": "denied.txt", "content": "x" }),
    ));
    h.model.push(Script::text("understood"));
    h.submit("write a file");

    let request = h.approval().await;
    h.send(Command::ResolveApproval {
        request_id: request.request_id,
        decision: ApprovalDecision::Deny {
            feedback: Some("use notes.md instead".into()),
        },
    });
    h.turn_finished().await;

    assert!(!h.repo.exists("denied.txt"));
    let requests = h.main_requests();
    let answered = results(requests[1].messages.last().unwrap());
    assert_eq!(answered.len(), 1);
    assert!(answered[0].1, "a denial is an error result");
    assert!(answered[0].2.contains("The user denied this action"));
    assert!(answered[0].2.contains("use notes.md instead"));
    assert_eq!(
        h.events.count(|e| matches!(
            e,
            Event::ToolFinished {
                status: ToolStatus::Denied,
                ..
            }
        )),
        1
    );
}
