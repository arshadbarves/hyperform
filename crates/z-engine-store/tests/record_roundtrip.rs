//! Every record kind survives append -> read with its camelCase wire shape.

mod support;

use std::collections::BTreeSet;

use support::records::one_of_each;
use support::{SESSION, new_session, temp_store};
use z_engine_protocol::{AgentId, Message, ModeName, PermissionMode, SessionId, TurnId, Usage};
use z_engine_store::{LogRecord, SESSION_SCHEMA, read_records};

fn kind_of(record: &LogRecord) -> String {
    let json = serde_json::to_value(record).unwrap();
    json["kind"].as_str().unwrap().to_string()
}

#[test]
fn every_record_kind_round_trips_through_a_session_log() {
    let (_dir, store) = temp_store();
    let (mut log, _) = store.create(new_session(SESSION)).unwrap();
    let records = one_of_each();
    for record in &records {
        log.append(record).unwrap();
    }
    log.sync().unwrap();

    let read = read_records(log.path()).unwrap();
    assert!(!read.torn_tail);
    assert_eq!(read.corrupt_lines, 0);
    assert_eq!(read.records.len(), records.len() + 1);
    assert_eq!(&read.records[1..], records.as_slice());
    let LogRecord::SessionStarted {
        schema,
        session_id,
        project_root,
        model,
        mode,
        planning,
        ..
    } = &read.records[0]
    else {
        panic!("the first record must be SessionStarted");
    };
    assert_eq!(*schema, SESSION_SCHEMA);
    assert_eq!(session_id, &SessionId::from(SESSION));
    assert_eq!(project_root, "/work/app");
    assert_eq!(model, "claude-sonnet-4");
    assert_eq!(*mode, ModeName::Level(PermissionMode::Default));
    assert!(!planning);
}

#[test]
fn reopened_logs_append_after_existing_records() {
    let (_dir, store) = temp_store();
    let id = SessionId::from(SESSION);
    let (first, _) = store.create(new_session(SESSION)).unwrap();
    let path = first.path().to_path_buf();
    drop(first);
    let records = one_of_each();
    for record in &records {
        let mut log = store.open_append(&id).unwrap();
        log.append(record).unwrap();
    }
    let read = read_records(&path).unwrap();
    assert_eq!(&read.records[1..], records.as_slice());
}

#[test]
fn records_are_tagged_by_camel_case_kind() {
    let mut kinds: BTreeSet<String> = one_of_each().iter().map(kind_of).collect();
    kinds.insert(kind_of(&LogRecord::SessionStarted {
        schema: SESSION_SCHEMA,
        session_id: SessionId::from(SESSION),
        project_root: String::new(),
        model: String::new(),
        mode: ModeName::Plan,
        planning: false,
        created_at: 0,
    }));
    let expected: BTreeSet<String> = [
        "sessionStarted",
        "message",
        "turnStarted",
        "turnFinished",
        "todos",
        "planProposed",
        "planResolved",
        "questionAsked",
        "questionAnswered",
        "approval",
        "agentUpdated",
        "check",
        "checkpoint",
        "compacted",
        "rewound",
        "modeChanged",
        "planningChanged",
        "modelChanged",
        "effortChanged",
        "title",
        "usage",
        "note",
        "planAttached",
        "planDetached",
        "taskView",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(kinds, expected);
}

#[test]
fn record_fields_are_camel_case() {
    let usage = serde_json::to_value(LogRecord::Usage {
        agent_id: AgentId::main(),
        usage: Usage {
            input_tokens: 3,
            ..Usage::default()
        },
        cost_usd: 0.5,
    })
    .unwrap();
    assert_eq!(usage["kind"], "usage");
    assert_eq!(usage["agentId"], "main");
    assert_eq!(usage["costUsd"], 0.5);
    assert_eq!(usage["usage"]["inputTokens"], 3);

    let message = serde_json::to_value(LogRecord::Message {
        message: Message::user_text("hi"),
        turn_id: Some(TurnId::from("trn_1")),
    })
    .unwrap();
    assert_eq!(message["turnId"], "trn_1");
    assert_eq!(message["message"]["role"], "user");

    let started = serde_json::to_value(LogRecord::SessionStarted {
        schema: SESSION_SCHEMA,
        session_id: SessionId::from(SESSION),
        project_root: "/p".into(),
        model: "m".into(),
        mode: PermissionMode::AcceptEdits.into(),
        planning: true,
        created_at: 5,
    })
    .unwrap();
    assert_eq!(started["sessionId"], SESSION);
    assert_eq!(started["projectRoot"], "/p");
    assert_eq!(started["mode"], "acceptEdits");
    assert_eq!(started["planning"], true);
    assert_eq!(started["createdAt"], 5);
}
