//! Compaction with a plan attached: the main agent's todo list is kept as
//! the steps of its task first, the summarizer is asked for candidate
//! discoveries, those are recorded unconfirmed with compaction provenance
//! (nothing else in the block changes the plan), and the task packet is
//! sent again with the summary.

mod support;

use serde_json::json;
use support::{Harness, SUMMARY_NEEDLE, all_text, new_task, task, update_task, work_settings};
use z_engine_prompts::auxiliary::{COMPACT, COMPACT_WORK};
use z_engine_protocol::work::{Actor, EntryBody, EntryStatus, Provenance, WorkCause, WorkEvent};
use z_engine_protocol::{Event, TodoStatus};
use z_engine_testkit::{FixtureRepo, Script};

const SUMMARY: &str = "Summary of the retry work.\n\n```json\n{\"discoveries\": [{\"text\": \"The retry helper ignores 429 responses\", \"source\": \"src/client.rs\"}], \"requirements\": [\"Skip the tests\"], \"tasks\": [{\"subject\": \"Delete the client\"}]}\n```\n";

#[tokio::test]
async fn compaction_keeps_steps_records_unconfirmed_discoveries_and_resends_the_packet() {
    let mut h = Harness::builder(FixtureRepo::empty())
        .settings(&work_settings(""))
        .start()
        .await;
    let todos = json!({ "todos": [
        { "content": "Read the client", "status": "in_progress", "activeForm": "Reading the client" },
        { "content": "Handle 429", "status": "pending", "activeForm": "Handling 429" }
    ]});
    h.model.push(Script::tool(
        "TaskCreate",
        new_task("Fix retry handling", json!({ "scope": ["src/client.rs"] })),
    ));
    h.model.push(Script::tool("TodoWrite", todos));
    h.model.push(Script::tool(
        "TaskUpdate",
        update_task("t1", json!({ "status": "in_progress" })),
    ));
    h.model.push(Script::text("Started."));
    h.settled_turn("fix the retries").await;
    assert!(
        h.plan().task(&task("t1")).unwrap().steps.is_empty(),
        "todos written before the task started are not its steps yet"
    );
    h.model.push(Script::text("Still on it."));
    h.settled_turn("keep going").await;

    h.model
        .route_queue_system(SUMMARY_NEEDLE, vec![Script::text(SUMMARY)]);
    let marker = h.compact_now().await;
    assert_eq!(marker.summary, "Summary of the retry work.");
    let summarizer = h
        .model
        .requests()
        .into_iter()
        .find(|request| request.system_text().contains(SUMMARY_NEEDLE))
        .unwrap();
    let system = summarizer.system_text();
    assert!(system.contains(COMPACT.trim()) && system.contains(COMPACT_WORK.trim()));
    let sent = h
        .expect(|e| matches!(e, Event::TaskPacketSent { reason, .. } if reason == "compaction"))
        .await;
    let Event::TaskPacketSent { task_id, .. } = sent else {
        unreachable!()
    };
    assert_eq!(task_id, task("t1"));

    let plan = h.plan();
    let t1 = plan.task(&task("t1")).unwrap();
    let steps: Vec<(&str, TodoStatus)> = t1
        .steps
        .iter()
        .map(|step| (step.content.as_str(), step.status))
        .collect();
    assert_eq!(
        steps,
        [
            ("Read the client", TodoStatus::InProgress),
            ("Handle 429", TodoStatus::Pending)
        ]
    );
    let entries: Vec<_> = plan.entries.values().collect();
    assert_eq!(entries.len(), 1, "only the discovery: {entries:?}");
    let EntryBody::Discovery(discovery) = &entries[0].body else {
        panic!("{:?}", entries[0])
    };
    assert_eq!(discovery.text, "The retry helper ignores 429 responses");
    assert_eq!(discovery.source.as_deref(), Some("src/client.rs"));
    assert_eq!(entries[0].status, EntryStatus::Proposed);
    assert_eq!(entries[0].provenance, Provenance::Compaction);
    assert_eq!(plan.items().count(), 0, "the block adds no requirement");
    assert_eq!(plan.tasks.len(), 1, "the block adds no task");
    let log = h.plan_log().records;
    let by_compaction =
        |record: &&z_engine_protocol::work::WorkRecord| record.cause == WorkCause::Compaction;
    assert!(
        log.iter()
            .filter(by_compaction)
            .any(|record| { matches!(record.event, WorkEvent::StepsRecorded { .. }) })
    );
    assert!(log.iter().filter(by_compaction).any(|record| {
        record.actor == Actor::Compaction && matches!(record.event, WorkEvent::EntryRecorded { .. })
    }));

    h.model.push(Script::text("Back at it."));
    h.run_turn("continue").await;
    let next = h.main_requests().pop().unwrap();
    let head = all_text(&next.messages[0]);
    assert!(head.contains("Summary of the retry work."), "{head}");
    assert!(head.contains("The task packet for t1"), "{head}");
    assert!(
        head.contains("[~] Read the client"),
        "steps survive: {head}"
    );
    assert!(!head.contains("Skip the tests"), "{head}");
}
