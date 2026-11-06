//! Replay of conversation history: compaction shapes the working set, and
//! a conversation rewind drops the message and every later record.

mod support;

use hyperform_protocol::{
    AgentId, AgentStatus, CompactionMarker, Message, MessageId, PermissionMode, RequestId, TurnId,
};
use hyperform_store::{LogRecord, replay};
use support::records::{
    agent, check, message_record as msg, session_started as started, todo, turn_finished,
    turn_started, usage,
};

fn compacted(keep_from: Option<&MessageId>, summary: &Message) -> LogRecord {
    LogRecord::Compacted {
        marker: CompactionMarker {
            keep_from: keep_from.cloned(),
            summary: summary.text(),
            tokens_before: 10_000,
            tokens_after: 800,
            created_at: 50,
        },
        summary: summary.clone(),
    }
}

fn rewound(message: &Message, conversation: bool) -> LogRecord {
    LogRecord::Rewound {
        message_id: message.id.clone(),
        conversation,
        code: true,
    }
}

fn ids(messages: &[Message]) -> Vec<MessageId> {
    messages.iter().map(|message| message.id.clone()).collect()
}

#[test]
fn compaction_replaces_older_history_in_the_working_set_only() {
    let m: Vec<Message> = (1..=4)
        .map(|i| Message::user_text(format!("m{i}")))
        .collect();
    let summary = Message::user_text("summary of m1 and m2");
    let later = Message::assistant_text("m5");
    let mut records = vec![started()];
    records.extend(m.iter().map(msg));
    records.push(compacted(Some(&m[2].id), &summary));
    records.push(msg(&later));

    let state = replay(&records);
    let mut everything = ids(&m);
    everything.push(later.id.clone());
    assert_eq!(ids(&state.transcript), everything);
    assert_eq!(
        ids(&state.working),
        [
            summary.id.clone(),
            m[2].id.clone(),
            m[3].id.clone(),
            later.id.clone()
        ]
    );
    assert_eq!(state.compactions.len(), 1);
    assert_eq!(state.compactions[0].keep_from, Some(m[2].id.clone()));
}

#[test]
fn compaction_without_a_kept_message_leaves_only_the_summary() {
    let m1 = Message::user_text("m1");
    let summary = Message::user_text("summary");
    for keep_from in [None, Some(MessageId::from("msg_gone"))] {
        let records = vec![started(), msg(&m1), compacted(keep_from.as_ref(), &summary)];
        let state = replay(&records);
        assert_eq!(state.working, std::slice::from_ref(&summary));
        assert_eq!(state.transcript, std::slice::from_ref(&m1));
    }
}

#[test]
fn successive_compactions_apply_to_the_current_working_set() {
    let (m1, m2, m3) = (
        Message::user_text("m1"),
        Message::assistant_text("m2"),
        Message::user_text("m3"),
    );
    let (s1, s2) = (Message::user_text("s1"), Message::user_text("s2"));
    let records = vec![
        started(),
        msg(&m1),
        msg(&m2),
        compacted(Some(&m2.id), &s1),
        msg(&m3),
        compacted(Some(&m3.id), &s2),
    ];
    let state = replay(&records);
    assert_eq!(ids(&state.working), [s2.id.clone(), m3.id.clone()]);
    assert_eq!(state.transcript.len(), 3);
    assert_eq!(state.compactions.len(), 2);
}

#[test]
fn conversation_rewind_drops_the_message_and_every_later_record() {
    let (u1, a1) = (Message::user_text("u1"), Message::assistant_text("a1"));
    let (u2, a2) = (Message::user_text("u2"), Message::assistant_text("a2"));
    let u3 = Message::user_text("u3");
    let (t1, t2, t3) = (TurnId::new(), TurnId::new(), TurnId::new());
    let summary = Message::user_text("summary");
    let records = vec![
        started(),
        msg(&u1),
        turn_started(&t1, &u1),
        msg(&a1),
        turn_finished(&t1, &u1, 10, 0.1),
        LogRecord::Todos {
            agent_id: AgentId::main(),
            todos: vec![todo("kept")],
        },
        msg(&u2),
        turn_started(&t2, &u2),
        msg(&a2),
        turn_finished(&t2, &u2, 99, 9.0),
        LogRecord::Check {
            record: check("chk_late", true),
        },
        LogRecord::Title {
            title: "late title".into(),
        },
        LogRecord::PlanProposed {
            request_id: RequestId::new(),
            agent_id: AgentId::main(),
            plan: "late plan".into(),
        },
        LogRecord::AgentUpdated {
            info: agent("agt_late", AgentStatus::Running),
        },
        LogRecord::ModeChanged {
            mode: PermissionMode::Bypass.into(),
            planning: Some(false),
        },
        compacted(Some(&u2.id), &summary),
        rewound(&u2, true),
        msg(&u3),
        turn_started(&t3, &u3),
    ];

    let state = replay(&records);
    let expected = [u1.id.clone(), a1.id.clone(), u3.id.clone()];
    assert_eq!(ids(&state.transcript), expected);
    assert_eq!(ids(&state.working), expected);
    assert_eq!(state.turns.len(), 1);
    assert_eq!(state.turns[0].turn_id, t1);
    assert_eq!(state.open_turn, Some((t3, u3.id.clone())));
    assert!(state.checks.is_empty());
    assert_eq!(state.title, None);
    assert!(state.pending_plans.is_empty());
    assert!(state.agents.is_empty());
    assert!(state.compactions.is_empty());
    assert_eq!(state.mode, PermissionMode::Default);
    assert!(state.planning, "the rewind dropped the level change");
    assert_eq!(state.todos[&AgentId::main()], [todo("kept")]);
    assert_eq!(
        state.usage,
        usage(10 + 99, 2),
        "spent usage survives the rewind"
    );
    assert!((state.cost_usd - 9.1).abs() < 1e-9);
}

#[test]
fn rewinding_a_costly_turn_keeps_its_cost_but_drops_the_turn() {
    let (u1, u2) = (Message::user_text("u1"), Message::user_text("u2"));
    let (t1, t2) = (TurnId::new(), TurnId::new());
    let records = vec![
        started(),
        msg(&u1),
        turn_started(&t1, &u1),
        turn_finished(&t1, &u1, 10, 0.25),
        msg(&u2),
        turn_started(&t2, &u2),
        turn_finished(&t2, &u2, 5_000, 3.0),
        LogRecord::Usage {
            agent_id: AgentId::main(),
            usage: usage(40, 8),
            cost_usd: 0.05,
        },
        rewound(&u2, true),
    ];
    let state = replay(&records);
    assert_eq!(state.transcript, std::slice::from_ref(&u1));
    let turns: Vec<&TurnId> = state.turns.iter().map(|turn| &turn.turn_id).collect();
    assert_eq!(turns, [&t1], "the rewound turn is gone");
    assert_eq!(state.open_turn, None);
    assert_eq!(state.usage.input_tokens, 10 + 5_000 + 40);
    assert_eq!(state.usage.output_tokens, 1 + 1 + 8);
    assert!((state.cost_usd - 3.30).abs() < 1e-9);
}

#[test]
fn code_only_and_unknown_rewinds_change_nothing() {
    let (u1, a1) = (Message::user_text("u1"), Message::assistant_text("a1"));
    let base = vec![started(), msg(&u1), msg(&a1)];
    let mut code_only = base.clone();
    code_only.push(rewound(&u1, false));
    let mut unknown = base.clone();
    unknown.push(rewound(&Message::user_text("never logged"), true));
    let expected = replay(&base);
    for records in [code_only, unknown] {
        let state = replay(&records);
        assert_eq!(state.transcript, expected.transcript);
        assert_eq!(state.working, expected.working);
    }
}
