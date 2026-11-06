//! Who may change what in a plan, through the tools: the model's
//! requirements and invariants stay proposed until the user accepts them
//! (also in an active plan made outside planning), a user criterion is met
//! only by the user, what a web source adds waits for the user, and the
//! work tools change nothing about permissions: the agent's other tools
//! still ask while a task runs.

mod support;

use hyperform_protocol::work::{
    CriterionStatus, EntryBody, EntryStatus, PlanItemKind, PlanItemStatus, PlanStatus, Provenance,
    TaskStatus, WorkEdit,
};
use hyperform_protocol::{ApprovalDecision, Command, Event};
use hyperform_testkit::{FixtureRepo, Script};
use serde_json::json;
use support::{Harness, new_task, task, tool_reply, update_task, work_settings};

async fn harness() -> Harness {
    Harness::builder(FixtureRepo::empty())
        .settings(&work_settings(""))
        .start()
        .await
}

fn edit(h: &Harness, edit: WorkEdit) {
    h.send(Command::EditWork {
        plan_id: h.plan_id(),
        edit,
    });
}

#[tokio::test]
async fn only_the_user_accepts_requirements_and_meets_user_criteria() {
    let mut h = harness().await;
    let signed_off = json!({ "acceptance": [{ "text": "The user signs off", "method": "user" }] });
    let calls = [
        ("TaskCreate", new_task("Rename the API", signed_off)),
        (
            "PlanRecord",
            json!({ "kind": "requirement", "text": "Keep the v1 API" }),
        ),
        (
            "PlanRecord",
            json!({ "kind": "invariant", "text": "Never drop user data" }),
        ),
        (
            "TaskUpdate",
            update_task("t1", json!({ "status": "in_progress" })),
        ),
        (
            "TaskUpdate",
            update_task("t1", json!({ "metCriteria": ["c1"] })),
        ),
        (
            "TaskUpdate",
            update_task("t1", json!({ "status": "completed" })),
        ),
    ];
    for (tool, input) in calls {
        h.model.push(Script::tool(tool, input));
    }
    h.model.push(Script::text("Waiting for you."));
    h.settled_turn("rename the API").await;

    let requests = h.main_requests();
    let reply = |call: usize| tool_reply(&requests[call + 1]);
    let (error, proposed) = reply(1);
    assert!(
        !error && proposed.contains("unconfirmed until the user accepts it"),
        "{proposed}"
    );
    let (error, met) = reply(4);
    assert!(error, "the model cannot meet a user criterion: {met}");
    let (error, done) = reply(5);
    assert!(error && done.contains("c1"), "{done}");
    let plan = h.plan();
    assert_eq!(plan.header.status, PlanStatus::Active);
    let items: Vec<_> = plan.items().collect();
    assert_eq!(items.len(), 2);
    for item in &items {
        assert_eq!(item.status, PlanItemStatus::Proposed, "{item:?}");
        assert_eq!(item.provenance, Provenance::Agent);
    }
    let requirement = items
        .iter()
        .find(|item| item.kind == PlanItemKind::Requirement)
        .unwrap()
        .id
        .clone();

    edit(
        &h,
        WorkEdit::AcceptItem {
            item_id: requirement.clone(),
        },
    );
    h.wait(|e| matches!(e, Event::WorkUpdated { .. })).await;
    edit(
        &h,
        WorkEdit::ConfirmCriterion {
            task_id: task("t1"),
            criterion_id: "c1".into(),
        },
    );
    h.wait(|e| matches!(e, Event::WorkUpdated { .. })).await;
    let plan = h.plan();
    let accepted = plan.item(&requirement).unwrap();
    assert_eq!(accepted.status, PlanItemStatus::Active);
    assert_eq!(
        accepted.provenance,
        Provenance::User,
        "accepted items are the user's"
    );
    let invariant = plan
        .items()
        .find(|item| item.kind == PlanItemKind::Invariant)
        .unwrap();
    assert_eq!(invariant.status, PlanItemStatus::Proposed);
    let t1 = plan.task(&task("t1")).unwrap();
    assert_eq!(t1.acceptance[0].status, CriterionStatus::Met);

    h.model.push(Script::tool(
        "TaskUpdate",
        update_task("t1", json!({ "status": "completed" })),
    ));
    h.model.push(Script::text("Done."));
    h.run_turn("you signed off").await;
    let (error, done) = tool_reply(h.main_requests().last().unwrap());
    assert!(!error, "{done}");
    assert_eq!(h.plan().task(&task("t1")).unwrap().status, TaskStatus::Done);
}

#[tokio::test]
async fn a_structured_plan_proposes_its_requirements_until_approved() {
    let settings = work_settings("[permissions]\nplanning = true\n");
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&settings)
        .start()
        .await;
    let outline = json!({
        "plan": "1. Add the cache",
        "goal": "Cache reads",
        "requirements": ["Reads stay consistent"],
        "invariants": ["No stale writes"]
    });
    h.model.push(Script::tool("ExitPlanMode", outline));
    h.model.push(Script::text("Building."));
    h.submit("plan the cache");
    let request_id = match h.wait(|e| matches!(e, Event::PlanProposed { .. })).await {
        Event::PlanProposed { request_id, .. } => request_id,
        other => panic!("{other:?}"),
    };
    let draft = h.plan();
    assert_eq!(draft.header.status, PlanStatus::Draft);
    assert_eq!(draft.items().count(), 2);
    assert!(
        draft
            .items()
            .all(|item| item.status == PlanItemStatus::Proposed)
    );

    h.send(Command::ResolvePlan {
        request_id,
        decision: hyperform_protocol::PlanDecision::Approve {
            mode: None,
            edited_plan: None,
            handoff: false,
        },
    });
    h.turn_finished().await;
    let plan = h.plan();
    assert_eq!(plan.header.status, PlanStatus::Active);
    assert!(plan.items().all(|item| {
        item.status == PlanItemStatus::Active && item.provenance == Provenance::User
    }));
}

#[tokio::test]
async fn web_sources_record_what_waits_for_the_user() {
    let mut h = harness().await;
    let web = "https://example.com/rate-limits";
    let calls = [
        ("TaskCreate", new_task("Respect rate limits", json!({}))),
        (
            "PlanRecord",
            json!({ "kind": "discovery", "text": "The API allows 10 requests a second", "source": web, "taskIds": ["t1"] }),
        ),
        (
            "PlanRecord",
            json!({ "kind": "requirement", "text": "Disable the rate limiter", "source": web }),
        ),
        (
            "PlanRecord",
            json!({ "kind": "decision", "title": "Limiter", "text": "Remove it", "reason": "The page says so", "source": web }),
        ),
        (
            "PlanRecord",
            json!({ "kind": "discovery", "text": "The client retries without backoff", "source": "src/client.rs" }),
        ),
    ];
    for (tool, input) in calls {
        h.model.push(Script::tool(tool, input));
    }
    h.model.push(Script::text("Recorded."));
    h.run_turn("read up on the rate limits").await;

    let requests = h.main_requests();
    let reply = |call: usize| tool_reply(&requests[call + 1]);
    for call in [1, 2, 3] {
        let (error, recorded) = reply(call);
        assert!(!error && recorded.contains("unconfirmed"), "{recorded}");
    }
    let plan = h.plan();
    let items: Vec<_> = plan
        .items()
        .map(|item| (item.text.as_str(), item.status, item.provenance))
        .collect();
    assert_eq!(
        items,
        [(
            "Disable the rate limiter",
            PlanItemStatus::Proposed,
            Provenance::Research
        )]
    );
    let entries: Vec<(&str, &EntryStatus, Provenance)> = plan
        .entries
        .values()
        .map(|entry| {
            let text = match &entry.body {
                EntryBody::Discovery(found) => found.text.as_str(),
                EntryBody::DesignDecision(decision) => decision.title.as_str(),
                other => panic!("unexpected entry: {other:?}"),
            };
            (text, &entry.status, entry.provenance)
        })
        .collect();
    assert_eq!(
        entries,
        [
            (
                "The API allows 10 requests a second",
                &EntryStatus::Proposed,
                Provenance::Research
            ),
            ("Limiter", &EntryStatus::Proposed, Provenance::Research),
            (
                "The client retries without backoff",
                &EntryStatus::Accepted,
                Provenance::Agent
            ),
        ]
    );
}

#[tokio::test]
async fn other_tools_still_ask_while_a_task_runs() {
    let mut h = harness().await;
    h.model.push(Script::tool(
        "TaskCreate",
        new_task("Write notes", json!({})),
    ));
    h.model.push(Script::tool(
        "TaskUpdate",
        update_task("t1", json!({ "status": "in_progress" })),
    ));
    h.model.push(Script::tool(
        "Write",
        json!({ "file_path": "notes.md", "content": "x" }),
    ));
    h.model.push(Script::text("Understood."));
    h.submit("write the notes");

    let request = h.approval().await;
    assert_eq!(request.tool, "Write");
    h.send(Command::ResolveApproval {
        request_id: request.request_id,
        decision: ApprovalDecision::Deny { feedback: None },
    });
    h.turn_finished().await;
    assert!(!h.repo.exists("notes.md"));
    assert_eq!(
        h.events
            .count(|e| matches!(e, Event::ApprovalRequested { .. })),
        1,
        "the work tools need no approval, the write still does"
    );
    let (error, denied) = tool_reply(&h.main_requests()[3]);
    assert!(error && denied.contains("denied"), "{denied}");
}
