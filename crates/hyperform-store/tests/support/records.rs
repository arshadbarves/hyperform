//! Builders for protocol values and log records.

use hyperform_protocol::{
    AgentId, AgentInfo, AgentStatus, CheckKind, CheckRecord, CheckpointId, CheckpointInfo,
    CompactionMarker, Effort, Isolation, Message, MessageId, ModeName, PermissionMode, Question,
    QuestionAnswer, QuestionOption, RequestId, SessionId, TodoItem, TodoStatus, TurnId,
    TurnOutcome, TurnRecord, Usage, VerificationOutcome,
};
use hyperform_store::LogRecord;

pub fn usage(input: u64, output: u64) -> Usage {
    Usage {
        input_tokens: input,
        output_tokens: output,
        ..Usage::default()
    }
}

pub fn turn(
    turn_id: &TurnId,
    message_id: &MessageId,
    outcome: TurnOutcome,
    usage: Usage,
    cost_usd: f64,
) -> TurnRecord {
    TurnRecord {
        turn_id: turn_id.clone(),
        message_id: message_id.clone(),
        outcome,
        verification: VerificationOutcome::NotApplicable,
        usage,
        cost_usd,
        started_at: 1_000,
        finished_at: 2_000,
    }
}

pub fn agent(id: &str, status: AgentStatus) -> AgentInfo {
    AgentInfo {
        agent_id: AgentId::from(id),
        parent_id: Some(AgentId::main()),
        call_id: None,
        agent_type: "explore".into(),
        description: "find callers".into(),
        model: "claude-haiku".into(),
        background: false,
        isolation: Isolation::Shared,
        worktree: None,
        status,
        depth: 1,
        started_at: 1_000,
        finished_at: None,
        usage: Usage::default(),
        cost_usd: 0.0,
        tool_calls: 0,
        result_preview: None,
        read_web: false,
        task_id: None,
        error: None,
    }
}

pub fn check(record_id: &str, passed: bool) -> CheckRecord {
    CheckRecord {
        record_id: record_id.into(),
        check_id: "cargo-test".into(),
        label: "cargo test".into(),
        kind: CheckKind::Test,
        command: "cargo test".into(),
        cwd: "/work/app".into(),
        agent_id: AgentId::main(),
        exit_code: Some(if passed { 0 } else { 101 }),
        passed,
        timed_out: false,
        started_at: 1_500,
        duration_ms: 900,
        tests: None,
        artifact: None,
        fingerprint_before: "fp-a".into(),
        fingerprint_after: "fp-a".into(),
        output_tail: "test result: ok".into(),
    }
}

pub fn question() -> Question {
    Question {
        question: "Which database?".into(),
        header: "Database".into(),
        options: vec![QuestionOption {
            label: "Postgres".into(),
            description: None,
        }],
        multi_select: false,
    }
}

pub fn todo(content: &str) -> TodoItem {
    TodoItem {
        content: content.into(),
        active_form: String::new(),
        status: TodoStatus::Pending,
    }
}

/// `SessionStarted` for session `S1` in `/work/app`, `model-a`, written in
/// plan mode before planning was its own switch.
pub fn session_started() -> LogRecord {
    LogRecord::SessionStarted {
        schema: 2,
        session_id: SessionId::from("S1"),
        project_root: "/work/app".into(),
        model: "model-a".into(),
        mode: ModeName::Plan,
        planning: false,
        created_at: 7,
    }
}

pub fn message_record(message: &Message) -> LogRecord {
    LogRecord::Message {
        message: message.clone(),
        turn_id: None,
    }
}

pub fn turn_started(turn_id: &TurnId, message: &Message) -> LogRecord {
    LogRecord::TurnStarted {
        turn_id: turn_id.clone(),
        message_id: message.id.clone(),
        started_at: 10,
    }
}

pub fn turn_finished(turn_id: &TurnId, message: &Message, input: u64, cost_usd: f64) -> LogRecord {
    LogRecord::TurnFinished {
        turn: turn(
            turn_id,
            &message.id,
            TurnOutcome::Completed,
            usage(input, 1),
            cost_usd,
        ),
    }
}

/// One record of every kind except `SessionStarted`, which `create` writes.
pub fn one_of_each() -> Vec<LogRecord> {
    let prompt = Message::user_text("fix the login test");
    let turn_id = TurnId::new();
    let plan = RequestId::new();
    let asked = RequestId::new();
    vec![
        LogRecord::Message {
            message: prompt.clone(),
            turn_id: Some(turn_id.clone()),
        },
        turn_started(&turn_id, &prompt),
        LogRecord::TurnFinished {
            turn: turn(
                &turn_id,
                &prompt.id,
                TurnOutcome::Completed,
                usage(10, 5),
                0.01,
            ),
        },
        LogRecord::Todos {
            agent_id: AgentId::main(),
            todos: vec![TodoItem {
                content: "Run tests".into(),
                active_form: "Running tests".into(),
                status: TodoStatus::InProgress,
            }],
        },
        LogRecord::PlanProposed {
            request_id: plan.clone(),
            agent_id: AgentId::main(),
            plan: "1. read\n2. fix".into(),
        },
        LogRecord::PlanResolved {
            request_id: plan,
            approved: true,
            feedback: None,
            final_plan: Some("1. read\n2. fix".into()),
        },
        LogRecord::QuestionAsked {
            request_id: asked.clone(),
            agent_id: AgentId::main(),
            questions: vec![question()],
        },
        LogRecord::QuestionAnswered {
            request_id: asked,
            answers: Some(vec![QuestionAnswer {
                question: "Which database?".into(),
                answers: vec!["Postgres".into()],
            }]),
        },
        LogRecord::Approval {
            request_id: RequestId::new(),
            tool: "Bash".into(),
            title: "Run cargo test".into(),
            allowed: true,
            rule: Some("Bash(cargo test:*)".into()),
        },
        LogRecord::AgentUpdated {
            info: agent("agt_1", AgentStatus::Running),
        },
        LogRecord::Check {
            record: check("chk_1", true),
        },
        LogRecord::Checkpoint {
            info: CheckpointInfo {
                checkpoint_id: CheckpointId::new(),
                message_id: prompt.id.clone(),
                created_at: 999,
            },
            snapshot: "0123abcd".into(),
        },
        LogRecord::Compacted {
            marker: CompactionMarker {
                keep_from: Some(prompt.id.clone()),
                summary: "summary".into(),
                tokens_before: 9_000,
                tokens_after: 1_000,
                created_at: 3_000,
            },
            summary: Message::user_text("Summary of earlier work"),
        },
        LogRecord::Rewound {
            message_id: MessageId::new(),
            conversation: false,
            code: true,
        },
        LogRecord::ModeChanged {
            mode: PermissionMode::AcceptEdits.into(),
            planning: Some(true),
        },
        LogRecord::PlanningChanged { planning: false },
        LogRecord::ModelChanged {
            model: "claude-opus-4".into(),
        },
        LogRecord::EffortChanged {
            effort: Some(Effort::High),
        },
        LogRecord::Title {
            title: "Fix login test".into(),
        },
        LogRecord::Usage {
            agent_id: AgentId::main(),
            usage: usage(3, 1),
            cost_usd: 0.002,
        },
        LogRecord::Note {
            text: "imported note".into(),
        },
        LogRecord::PlanAttached {
            plan_id: hyperform_protocol::PlanId::from("pln_1"),
        },
        LogRecord::PlanDetached,
        LogRecord::TaskView {
            view: hyperform_protocol::decisions::TaskViewInfo {
                boundary: MessageId::new(),
                set_aside: 3,
                tokens: 12_000,
                restored: false,
                created_at: 1_700_000_000_000,
            },
            working: vec![MessageId::new()],
            index: Some(Message::user_text("index")),
        },
    ]
}
