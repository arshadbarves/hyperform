//! Importing v1 transcripts: a realistic session end to end, idempotency,
//! import on first open, and refusing files that are not v1 transcripts.

mod support;

use std::fs;

use serde_json::{Value, json};
use support::v1::{v1_session_events, write_v1, write_v1_lines};
use support::{assert_tool_rounds_complete, set_mtime, temp_store, ulid_at};
use z_engine_protocol::{
    CallId, ContentBlock, MediaSource, Message, PermissionMode, Role, SessionId, TurnOutcome,
    VerificationOutcome,
};
use z_engine_store::{LogRecord, StoreError, import_v1, read_records};

const CREATED: u64 = 1_700_000_000_000;
const MODIFIED: u64 = 1_700_000_600_000;

#[test]
fn import_converts_a_realistic_v1_session() {
    let (_dir, store) = temp_store();
    let id = ulid_at(CREATED);
    let mut lines: Vec<String> = v1_session_events().iter().map(Value::to_string).collect();
    lines.insert(3, "this line is not json".into());
    let v1 = write_v1_lines(store.dir(), &id, &lines);
    set_mtime(&v1, MODIFIED);

    let imported = import_v1(store.dir(), &v1).unwrap();
    assert_eq!(imported.as_str(), id);
    let loaded = store.load(&imported).unwrap();

    let meta = &loaded.meta;
    assert!(meta.legacy);
    assert_eq!(meta.title.as_deref(), Some("Fix failing login test"));
    assert_eq!(meta.project_root, "/work/app");
    assert_eq!(meta.model, "anthropic/claude-sonnet-4");
    assert_eq!((meta.created_at, meta.updated_at), (CREATED, MODIFIED));
    assert_eq!(meta.message_count, 8);
    assert!(matches!(
        meta.last_outcome,
        Some(TurnOutcome::Failed { .. })
    ));

    let state = &loaded.state;
    assert_eq!(
        state.info,
        Some((imported.clone(), "/work/app".into(), CREATED))
    );
    assert_eq!(state.mode, PermissionMode::Default);
    let t = &state.transcript;
    assert_eq!(t.len(), 8);
    assert_eq!(t[0].role, Role::User);
    assert_eq!(
        t[0].content,
        [
            ContentBlock::text("Fix the failing login test\nIt started yesterday"),
            ContentBlock::Image {
                source: MediaSource::Base64 {
                    media_type: "image/png".into(),
                    data: "iVBORw0KGgo=".into()
                }
            }
        ]
    );
    assert_eq!(t[1].role, Role::Assistant);
    assert_eq!(t[1].text(), "Let me look.");
    let calls: Vec<(&str, &str, &Value)> = t[1]
        .tool_uses()
        .map(|(id, name, input)| (id.as_str(), name, input))
        .collect();
    assert_eq!(
        calls,
        [
            ("call_a", "Read", &json!({"path": "src/login.rs"})),
            ("call_b", "Bash", &json!({"command": "cargo test login"}))
        ]
    );
    assert_eq!(
        t[2].content,
        [
            ContentBlock::tool_result(CallId::from("call_a"), "fn login() {}", false),
            ContentBlock::tool_result(CallId::from("call_b"), "test login ... FAILED", false)
        ],
        "results of one round merge into one message in call order"
    );
    assert_eq!(t[3].text(), "Fixed the assertion.");
    assert_eq!(t[4].text(), "Now search for other callers");
    let (call, name, input) = t[5].tool_uses().next().unwrap();
    assert_eq!((call.as_str(), name, input), ("call_c", "Grep", &json!({})));
    assert!(matches!(
        &t[6].content[..],
        [ContentBlock::ToolResult { tool_use_id, is_error: true, .. }] if tool_use_id.as_str() == "call_c"
    ));
    assert_eq!(t[7].text(), "continue");
    assert_eq!(state.working, state.transcript);
    assert_tool_rounds_complete(&state.working);

    let outcomes: Vec<&TurnOutcome> = state.turns.iter().map(|turn| &turn.outcome).collect();
    assert_eq!(outcomes.len(), 3);
    assert_eq!(outcomes[0], &TurnOutcome::Completed);
    assert_eq!(outcomes[1], &TurnOutcome::Cancelled);
    assert!(matches!(outcomes[2], TurnOutcome::Failed { .. }));
    let starts: Vec<_> = state
        .turns
        .iter()
        .map(|turn| turn.message_id.clone())
        .collect();
    assert_eq!(starts, [t[0].id.clone(), t[4].id.clone(), t[7].id.clone()]);
    assert!(
        state
            .turns
            .iter()
            .all(|turn| turn.verification == VerificationOutcome::NotApplicable)
    );
    assert_eq!(state.open_turn, None);

    let times: Vec<u64> = t.iter().map(|message| message.created_at).collect();
    assert!(times.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(times.iter().all(|time| (CREATED..=MODIFIED).contains(time)));

    let records = read_records(&store.dir().join(&id).join("log.jsonl")).unwrap();
    assert_eq!(records.corrupt_lines, 0);
    let notes: Vec<&str> = records
        .records
        .iter()
        .filter_map(|record| match record {
            LogRecord::Note { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        notes,
        [
            "v1 import skipped 2 unreadable line(s)",
            "FACTS: login uses bcrypt",
            "v1 task report (complete): Fix the failing login test\ncheck passed: 3 tests passed",
            "v1 task report (stopped): Fix the failing login test",
        ]
    );
}

#[test]
fn import_is_idempotent_and_leaves_the_v1_file_untouched() {
    let (_dir, store) = temp_store();
    let id = ulid_at(CREATED);
    let v1 = write_v1(store.dir(), &id, &v1_session_events());
    let original = fs::read(&v1).unwrap();

    let first = import_v1(store.dir(), &v1).unwrap();
    let log_path = store.dir().join(&id).join("log.jsonl");
    let meta_path = store.dir().join(&id).join("meta.json");
    let (log_bytes, meta_bytes) = (fs::read(&log_path).unwrap(), fs::read(&meta_path).unwrap());
    let second = import_v1(store.dir(), &v1).unwrap();

    assert_eq!(first, second);
    assert_eq!(fs::read(&log_path).unwrap(), log_bytes);
    assert_eq!(fs::read(&meta_path).unwrap(), meta_bytes);
    assert_eq!(fs::read(&v1).unwrap(), original);
    let mut names: Vec<String> = fs::read_dir(store.dir())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [id.clone(), format!("{id}.jsonl")],
        "no staging dirs left"
    );
}

#[test]
fn opening_a_v1_session_imports_it_and_appends_continue_it() {
    let (_dir, store) = temp_store();
    let id = ulid_at(CREATED);
    write_v1(store.dir(), &id, &v1_session_events());
    let session = SessionId::from(id.as_str());
    assert!(store.exists(&session));
    assert!(!store.dir().join(&id).exists());

    let mut log = store.open_append(&session).unwrap();
    assert!(store.dir().join(&id).join("meta.json").is_file());
    let before = store.load(&session).unwrap();
    assert!(before.meta.legacy);
    log.append(&LogRecord::Message {
        message: Message::user_text("one more thing"),
        turn_id: None,
    })
    .unwrap();
    let after = store.load(&session).unwrap();
    assert_eq!(
        after.state.transcript.len(),
        before.state.transcript.len() + 1
    );
}

#[test]
fn import_refuses_files_that_are_not_v1_transcripts() {
    let (_dir, store) = temp_store();
    fs::create_dir_all(store.dir()).unwrap();
    for name in ["notes.txt", "bad name.jsonl", "index.json"] {
        let path = store.dir().join(name);
        fs::write(&path, "{}\n").unwrap();
        assert!(
            matches!(import_v1(store.dir(), &path), Err(StoreError::Invalid(_))),
            "{name}"
        );
    }
    let missing = store.dir().join(format!("{}.jsonl", ulid_at(CREATED)));
    assert!(matches!(
        import_v1(store.dir(), &missing),
        Err(StoreError::NotFound(_))
    ));
}
