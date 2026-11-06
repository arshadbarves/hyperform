//! Concurrency-safe calls run together; an Edit between Reads is a
//! barrier; results always come back in call order.

mod support;

use serde_json::json;
use support::{BASE_SETTINGS, Harness, results};
use z_engine_protocol::{CallId, Event, Message};
use z_engine_testkit::{FixtureRepo, Script};

/// `(started, call)` for tool start and finish events, in arrival order.
fn tool_events(events: &[Event]) -> Vec<(bool, CallId)> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::ToolStarted { call_id, .. } => Some((true, call_id.clone())),
            Event::ToolFinished { call_id, .. } => Some((false, call_id.clone())),
            _ => None,
        })
        .collect()
}

fn call_ids(message: &Message) -> Vec<CallId> {
    message.tool_uses().map(|(id, _, _)| id.clone()).collect()
}

#[tokio::test]
async fn reads_run_together_and_edits_are_barriers() {
    let repo = FixtureRepo::git(&[
        ("a.txt", "alpha\n"),
        ("b.txt", "beta\n"),
        ("c.txt", "gamma\n"),
    ]);
    let settings = format!("{BASE_SETTINGS}\n[permissions]\nmode = \"acceptEdits\"\n");
    let mut h = Harness::builder(repo).settings(&settings).start().await;
    let (a, b, c) = (h.path("a.txt"), h.path("b.txt"), h.path("c.txt"));
    h.model.push(Script::tools(&[
        ("Read", json!({ "file_path": a })),
        ("Read", json!({ "file_path": b })),
    ]));
    h.model.push(Script::tools(&[
        ("Read", json!({ "file_path": a })),
        (
            "Edit",
            json!({ "file_path": a, "old_string": "alpha", "new_string": "ALPHA" }),
        ),
        ("Read", json!({ "file_path": c })),
    ]));
    h.model.push(Script::text("done"));
    h.run_turn("read and edit").await;

    let transcript = h.transcript();
    let assistant: Vec<&Message> = transcript.iter().filter(|m| m.has_tool_use()).collect();
    let first = call_ids(assistant[0]);
    let second = call_ids(assistant[1]);
    let events = tool_events(h.events.seen());

    let round_one: Vec<&(bool, CallId)> =
        events.iter().filter(|(_, id)| first.contains(id)).collect();
    let first_finish = round_one.iter().position(|(started, _)| !started).unwrap();
    assert_eq!(
        first_finish, 2,
        "both reads start before either finishes: {round_one:?}"
    );

    let round_two: Vec<(bool, CallId)> = events
        .iter()
        .filter(|(_, id)| second.contains(id))
        .cloned()
        .collect();
    let expected: Vec<(bool, CallId)> = second
        .iter()
        .flat_map(|id| [(true, id.clone()), (false, id.clone())])
        .collect();
    assert_eq!(round_two, expected, "each call runs alone around the Edit");

    for (calls, answer_at) in [(&first, 2), (&second, 4)] {
        let answered: Vec<CallId> = results(&transcript[answer_at])
            .into_iter()
            .map(|r| r.0)
            .collect();
        assert_eq!(&answered, calls, "results follow call order");
    }
    assert_eq!(h.repo.read("a.txt"), "ALPHA\n");
}
