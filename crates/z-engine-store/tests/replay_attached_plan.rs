//! The work plan a session is attached to replays from `planAttached` and
//! `planDetached`, and conversation rewinds leave it alone.

use z_engine_protocol::{Message, PlanId};
use z_engine_store::{LogRecord, replay};

fn parse(line: &str) -> LogRecord {
    serde_json::from_str(line).unwrap()
}

const STARTED: &str = r#"{"kind":"sessionStarted","schema":2,"sessionId":"S1","projectRoot":"/p","model":"m","mode":"default","createdAt":1}"#;
const ATTACH_A: &str = r#"{"kind":"planAttached","planId":"pln_a"}"#;
const ATTACH_B: &str = r#"{"kind":"planAttached","planId":"pln_b"}"#;
const DETACH: &str = r#"{"kind":"planDetached"}"#;

fn attached(lines: &[&str]) -> Option<PlanId> {
    let records: Vec<LogRecord> = lines.iter().map(|line| parse(line)).collect();
    replay(&records).attached_plan
}

#[test]
fn the_latest_attachment_wins_and_detaching_clears_it() {
    assert_eq!(attached(&[STARTED]), None);
    assert_eq!(
        attached(&[STARTED, ATTACH_A, ATTACH_B]),
        Some(PlanId::from("pln_b"))
    );
    assert_eq!(attached(&[STARTED, ATTACH_A, DETACH]), None);
}

#[test]
fn a_conversation_rewind_keeps_the_attachment() {
    let message = Message::user_text("hi");
    let records = vec![
        parse(STARTED),
        LogRecord::Message {
            message: message.clone(),
            turn_id: None,
        },
        parse(ATTACH_A),
        LogRecord::Rewound {
            message_id: message.id.clone(),
            conversation: true,
            code: false,
        },
    ];
    let state = replay(&records);
    assert!(state.transcript.is_empty());
    assert_eq!(state.attached_plan, Some(PlanId::from("pln_a")));
}
