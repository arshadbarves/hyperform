//! Replay of session state: open turns, pending plans and questions, usage
//! totals, and latest-wins agents, todos, settings, and title.

mod support;

use hyperform_protocol::{
    AgentId, AgentStatus, CheckpointId, CheckpointInfo, Effort, Message, MessageId, PermissionMode,
    RequestId, SessionId, TurnId,
};
use hyperform_store::{LogRecord, replay};
use support::records::{
    agent, message_record as msg, question, session_started as started, todo, turn_finished,
    turn_started, usage,
};

#[test]
fn a_started_turn_stays_open_until_its_finish() {
    let (u1, a1) = (Message::user_text("u1"), Message::assistant_text("a1"));
    let (t1, other) = (TurnId::new(), TurnId::new());
    let mut records = vec![started(), msg(&u1), turn_started(&t1, &u1), msg(&a1)];
    assert_eq!(
        replay(&records).open_turn,
        Some((t1.clone(), u1.id.clone()))
    );

    records.push(turn_finished(&other, &u1, 1, 0.0));
    assert_eq!(
        replay(&records).open_turn,
        Some((t1.clone(), u1.id.clone())),
        "finishing another turn keeps this one open"
    );

    records.push(turn_finished(&t1, &u1, 1, 0.0));
    let state = replay(&records);
    assert_eq!(state.open_turn, None);
    assert_eq!(state.turns.len(), 2);
}

#[test]
fn pending_plans_and_questions_are_those_without_a_resolution() {
    let (p1, p2) = (RequestId::new(), RequestId::new());
    let (q1, q2) = (RequestId::new(), RequestId::new());
    let worker = AgentId::from("agt_worker");
    let records = vec![
        started(),
        LogRecord::PlanProposed {
            request_id: p1.clone(),
            agent_id: AgentId::main(),
            plan: "plan a".into(),
        },
        LogRecord::PlanProposed {
            request_id: p2.clone(),
            agent_id: worker.clone(),
            plan: "plan b".into(),
        },
        LogRecord::PlanResolved {
            request_id: p1,
            approved: false,
            feedback: Some("smaller steps".into()),
            final_plan: None,
        },
        LogRecord::QuestionAsked {
            request_id: q1.clone(),
            agent_id: AgentId::main(),
            questions: vec![question()],
        },
        LogRecord::QuestionAsked {
            request_id: q2.clone(),
            agent_id: worker.clone(),
            questions: vec![question()],
        },
        LogRecord::QuestionAnswered {
            request_id: q1,
            answers: None,
        },
    ];
    let state = replay(&records);
    assert_eq!(state.pending_plans, [(p2, worker.clone(), "plan b".into())]);
    assert_eq!(state.pending_questions, [(q2, worker, vec![question()])]);
}

#[test]
fn usage_and_cost_sum_finished_turns_and_side_requests() {
    let u1 = Message::user_text("u1");
    let (t1, t2) = (TurnId::new(), TurnId::new());
    let records = vec![
        started(),
        turn_finished(&t1, &u1, 10, 0.10),
        LogRecord::Usage {
            agent_id: AgentId::main(),
            usage: usage(3, 4),
            cost_usd: 0.02,
        },
        turn_finished(&t2, &u1, 7, 0.05),
    ];
    let state = replay(&records);
    assert_eq!(state.usage.input_tokens, 20);
    assert_eq!(state.usage.output_tokens, 6);
    assert!((state.cost_usd - 0.17).abs() < 1e-9);
}

#[test]
fn latest_state_wins_for_agents_todos_settings_and_title() {
    let worker = AgentId::from("agt_2");
    let checkpoint = CheckpointInfo {
        checkpoint_id: CheckpointId::new(),
        message_id: MessageId::new(),
        created_at: 9,
    };
    let records = vec![
        started(),
        LogRecord::AgentUpdated {
            info: agent("agt_1", AgentStatus::Running),
        },
        LogRecord::AgentUpdated {
            info: agent("agt_2", AgentStatus::Running),
        },
        LogRecord::AgentUpdated {
            info: agent("agt_1", AgentStatus::Completed),
        },
        LogRecord::Todos {
            agent_id: AgentId::main(),
            todos: vec![todo("first")],
        },
        LogRecord::Todos {
            agent_id: worker.clone(),
            todos: vec![todo("worker")],
        },
        LogRecord::Todos {
            agent_id: AgentId::main(),
            todos: vec![],
        },
        LogRecord::ModeChanged {
            mode: PermissionMode::AcceptEdits.into(),
            planning: None,
        },
        LogRecord::ModelChanged {
            model: "model-b".into(),
        },
        LogRecord::EffortChanged {
            effort: Some(Effort::Low),
        },
        LogRecord::EffortChanged { effort: None },
        LogRecord::Title {
            title: "First".into(),
        },
        LogRecord::Title {
            title: "  Second  ".into(),
        },
        LogRecord::Checkpoint {
            info: checkpoint.clone(),
            snapshot: "sha1".into(),
        },
    ];
    let state = replay(&records);
    assert_eq!(
        state.info,
        Some((SessionId::from("S1"), "/work/app".into(), 7))
    );
    let agents: Vec<(&str, AgentStatus)> = state
        .agents
        .iter()
        .map(|info| (info.agent_id.as_str(), info.status))
        .collect();
    assert_eq!(
        agents,
        [
            ("agt_1", AgentStatus::Completed),
            ("agt_2", AgentStatus::Running)
        ]
    );
    assert!(state.todos[&AgentId::main()].is_empty());
    assert_eq!(state.todos[&worker], [todo("worker")]);
    assert_eq!(state.mode, PermissionMode::AcceptEdits);
    assert!(!state.planning, "an old level change left plan mode");
    assert_eq!(state.model.as_deref(), Some("model-b"));
    assert_eq!(state.effort, None);
    assert_eq!(state.title.as_deref(), Some("Second"));
    assert_eq!(state.checkpoints, [(checkpoint, "sha1".to_string())]);
}
