//! Replay of task views: the working set is rebuilt from the full history
//! by id, restored by "Include full history", and dropped by compaction
//! and by a rewind to before the view.

mod support;

use support::records::{message_record as msg, session_started as started};
use z_engine_protocol::decisions::TaskViewInfo;
use z_engine_protocol::{CompactionMarker, Message, MessageId};
use z_engine_store::{LogRecord, replay};

fn ids(messages: &[Message]) -> Vec<MessageId> {
    messages.iter().map(|message| message.id.clone()).collect()
}

fn info(boundary: &Message, restored: bool) -> TaskViewInfo {
    TaskViewInfo {
        boundary: boundary.id.clone(),
        set_aside: 1,
        tokens: 900,
        restored,
        created_at: 7,
    }
}

fn view(boundary: &Message, working: &[&Message], index: &Message) -> LogRecord {
    LogRecord::TaskView {
        view: info(boundary, false),
        working: working.iter().map(|message| message.id.clone()).collect(),
        index: Some(index.clone()),
    }
}

fn restored(boundary: &Message) -> LogRecord {
    LogRecord::TaskView {
        view: info(boundary, true),
        working: Vec::new(),
        index: None,
    }
}

struct Chat {
    old: [Message; 2],
    kept: [Message; 2],
    boundary: Message,
    index: Message,
    later: Message,
}

fn chat() -> Chat {
    Chat {
        old: [
            Message::user_text("old"),
            Message::assistant_text("old done"),
        ],
        kept: [
            Message::user_text("kept"),
            Message::assistant_text("kept done"),
        ],
        boundary: Message::user_text("new task"),
        index: Message::user_text("index"),
        later: Message::assistant_text("working on it"),
    }
}

fn applied(c: &Chat) -> Vec<LogRecord> {
    let mut records = vec![started()];
    records.extend(c.old.iter().chain(&c.kept).chain([&c.boundary]).map(msg));
    records.push(view(
        &c.boundary,
        &[&c.index, &c.kept[0], &c.kept[1], &c.boundary],
        &c.index,
    ));
    records.push(msg(&c.later));
    records
}

#[test]
fn a_view_rebuilds_the_same_working_set_and_keeps_the_full_history() {
    let c = chat();
    let state = replay(&applied(&c));
    assert_eq!(
        ids(&state.working),
        ids(&[
            c.index.clone(),
            c.kept[0].clone(),
            c.kept[1].clone(),
            c.boundary.clone(),
            c.later.clone()
        ])
    );
    assert_eq!(
        state.transcript.len(),
        6,
        "the index is not in the transcript"
    );
    assert_eq!(state.full_working.as_ref().map(Vec::len), Some(6));
    assert_eq!(state.task_views, [info(&c.boundary, false)]);
}

#[test]
fn include_full_history_restores_every_message() {
    let c = chat();
    let mut records = applied(&c);
    records.push(restored(&c.boundary));
    let state = replay(&records);
    assert_eq!(ids(&state.working), ids(&state.transcript));
    assert_eq!(state.full_working, None);
    assert_eq!(state.task_views, [info(&c.boundary, true)]);
}

#[test]
fn compaction_ends_the_view() {
    let c = chat();
    let summary = Message::user_text("summary");
    let mut records = applied(&c);
    records.push(LogRecord::Compacted {
        marker: CompactionMarker {
            keep_from: Some(c.boundary.id.clone()),
            summary: "summary".into(),
            tokens_before: 1_000,
            tokens_after: 100,
            created_at: 9,
        },
        summary: summary.clone(),
    });
    let state = replay(&records);
    assert_eq!(
        ids(&state.working),
        ids(&[summary, c.boundary.clone(), c.later.clone()])
    );
    assert_eq!(state.full_working, None);
}

#[test]
fn rewinding_to_the_boundary_drops_the_view() {
    let c = chat();
    let mut records = applied(&c);
    records.push(LogRecord::Rewound {
        message_id: c.boundary.id.clone(),
        conversation: true,
        code: false,
    });
    let state = replay(&records);
    assert_eq!(ids(&state.working), ids(&state.transcript));
    assert_eq!(state.transcript.len(), 4);
    assert!(state.task_views.is_empty() && state.full_working.is_none());
}
