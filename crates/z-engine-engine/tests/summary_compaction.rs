//! Above `compact_at_percent`, older history is summarized by the fast
//! model: `CompactionStarted` then `Compacted` are emitted, the marker is
//! persisted, the working set becomes summary + recent tail (still valid),
//! and the display keeps everything.

mod support;

use serde_json::json;
use support::{Harness, all_text, assert_valid_transcript};
use z_engine_protocol::{CompactionTrigger, Event};
use z_engine_testkit::{FixtureRepo, Script};

const SETTINGS: &str = "schema = 2\n\n[model]\nmain = \"test-model\"\ncontext_window = 10000\n\n\
                        [context]\nkeep_recent_tool_results = 8\ncompact_at_percent = 50\n";

#[tokio::test]
async fn older_history_is_summarized() {
    let content: String = (0..500)
        .map(|line| format!("line {line:04} with some filler text for size\n"))
        .collect();
    let repo = FixtureRepo::git(&[("big.txt", &content)]);
    let mut h = Harness::builder(repo).settings(SETTINGS).start().await;
    h.model.push(Script::text("hi"));
    h.run_turn("hello").await;
    h.model.push(Script::tool(
        "Read",
        json!({ "file_path": h.path("big.txt") }),
    ));
    h.model.push(Script::text("read it"));
    h.run_turn("read the big file").await;

    let marker = match h.expect(|e| matches!(e, Event::Compacted { .. })).await {
        Event::Compacted { marker } => marker,
        other => panic!("{other:?}"),
    };
    assert_eq!(marker.summary, "Summary of the earlier work.");
    assert!(marker.keep_from.is_some());
    let seen = h.events.seen();
    let started = seen.iter().position(|e| {
        matches!(
            e,
            Event::CompactionStarted {
                trigger: CompactionTrigger::Auto
            }
        )
    });
    let compacted = seen
        .iter()
        .position(|e| matches!(e, Event::Compacted { .. }));
    assert!(
        started.is_some() && started < compacted,
        "CompactionStarted precedes Compacted: {seen:#?}"
    );

    let requests = h.main_requests();
    let last = requests.last().unwrap();
    assert_valid_transcript(&last.messages);
    assert!(all_text(&last.messages[0]).contains("Summary of the earlier work."));
    assert!(
        last.messages.len() < h.transcript().len(),
        "working set shrank"
    );
    assert_eq!(h.transcript().len(), 6, "display keeps every message");

    h.reopen().await;
    match h.wait(|e| matches!(e, Event::Snapshot { .. })).await {
        Event::Snapshot { snapshot } => {
            assert_eq!(snapshot.compactions.len(), 1);
            assert_eq!(snapshot.messages.len(), 6);
        }
        other => panic!("{other:?}"),
    }
}
