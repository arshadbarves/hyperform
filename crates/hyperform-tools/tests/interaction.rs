//! Interaction-port tools: `TodoWrite`, `AskUserQuestion`, `ExitPlanMode`,
//! including plan-mode checks and subagent unavailability.

mod support;

use std::sync::Arc;

use hyperform_protocol::{PermissionMode, PlanDecision, QuestionAnswer, TodoStatus};
use hyperform_tools::builtin::{AskUserQuestionTool, ExitPlanModeTool, TodoWriteTool};
use hyperform_tools::{Ports, Tool, ToolCtx, ToolError};
use serde_json::{Value, json};
use support::fakes::FakeInteraction;
use support::{ctx, ctx_with, err_text, ok_text, project};

fn with(dir: &tempfile::TempDir, port: Arc<FakeInteraction>) -> ToolCtx {
    ctx_with(
        dir.path(),
        Ports {
            interaction: Some(port),
            ..Ports::default()
        },
    )
}

fn question() -> Value {
    json!({"questions": [{
        "question": "Which database should the service use?",
        "header": "Database",
        "options": [
            {"label": "PostgreSQL", "description": "Relational, already deployed"},
            {"label": "SQLite", "description": "Embedded, simplest"}
        ],
        "multiSelect": false
    }]})
}

#[tokio::test]
async fn todo_write_updates_the_list_and_nudges_one_in_progress() {
    let dir = project(&[]);
    let port = Arc::new(FakeInteraction::new(true));
    let ctx = with(&dir, Arc::clone(&port));
    let text = ok_text(
        &TodoWriteTool,
        &ctx,
        json!({"todos": [
            {"content": "Add retry", "activeForm": "Adding retry", "status": "completed"},
            {"content": "Run tests", "activeForm": "Running tests", "status": "in_progress"},
            {"content": "Update docs", "status": "pending"}
        ]}),
    )
    .await;
    assert!(
        text.starts_with("Todo list updated: 1 completed, 1 in progress, 1 pending."),
        "{text}"
    );
    assert!(text.contains("Keep exactly one item in_progress"), "{text}");
    let lists = port.todos.lock().unwrap();
    assert_eq!(lists.len(), 1);
    assert_eq!(lists[0][1].status, TodoStatus::InProgress);
    assert_eq!(lists[0][2].active_form, "Update docs");
}

#[tokio::test]
async fn todo_write_validates_items() {
    let dir = project(&[]);
    let ctx = with(&dir, Arc::new(FakeInteraction::new(true)));
    let err = err_text(
        &TodoWriteTool,
        &ctx,
        json!({"todos": [{"content": " ", "status": "pending"}]}),
    )
    .await;
    assert!(err.contains("todo 1: `content` must not be empty"), "{err}");
    let err = err_text(
        &TodoWriteTool,
        &ctx,
        json!({"todos": [{"content": "x", "status": "done"}]}),
    )
    .await;
    assert!(err.contains("`todos`: unknown variant `done`"), "{err}");
    let text = ok_text(
        &TodoWriteTool,
        &ctx,
        json!({"todos": [
            {"content": "a", "activeForm": "A", "status": "in_progress"},
            {"content": "b", "activeForm": "B", "status": "in_progress"}
        ]}),
    )
    .await;
    assert!(
        text.contains("2 items are in_progress; keep exactly one"),
        "{text}"
    );
    assert!(!TodoWriteTool.is_concurrency_safe(&json!({})));
}

#[tokio::test]
async fn ask_user_question_formats_answers_and_dismissal() {
    let dir = project(&[]);
    let mut port = FakeInteraction::new(true);
    port.answers = Some(vec![QuestionAnswer {
        question: "Which database should the service use?".into(),
        answers: vec!["PostgreSQL".into()],
    }]);
    let port = Arc::new(port);
    let ctx = with(&dir, Arc::clone(&port));
    let text = ok_text(&AskUserQuestionTool, &ctx, question()).await;
    assert_eq!(
        text,
        "The user answered your questions:\n- \"Which database should the service use?\" -> PostgreSQL\nContinue with these answers in mind."
    );
    let asked = port.asked.lock().unwrap().clone();
    assert_eq!(asked[0][0].header, "Database");
    assert_eq!(
        asked[0][0].options[1].description.as_deref(),
        Some("Embedded, simplest")
    );

    let ctx = with(&dir, Arc::new(FakeInteraction::new(true)));
    let text = ok_text(&AskUserQuestionTool, &ctx, question()).await;
    assert!(
        text.starts_with("The user dismissed the questions"),
        "{text}"
    );
    assert_eq!(
        AskUserQuestionTool.title(&question(), &ctx),
        "Ask: Database"
    );
}

#[tokio::test]
async fn ask_user_question_returns_an_earlier_answer_without_asking() {
    let dir = project(&[]);
    let mut port = FakeInteraction::new(true);
    port.answered_earlier = Some("See message 2: \"use PostgreSQL\".".into());
    let port = Arc::new(port);
    let ctx = with(&dir, Arc::clone(&port));
    let text = ok_text(&AskUserQuestionTool, &ctx, question()).await;
    assert_eq!(text, "See message 2: \"use PostgreSQL\".");
    assert!(port.asked.lock().unwrap().is_empty());
}

#[tokio::test]
async fn ask_user_question_validates_and_is_unavailable_to_subagents() {
    let dir = project(&[]);
    let ctx = with(&dir, Arc::new(FakeInteraction::new(true)));
    let mut one_option = question();
    one_option["questions"][0]["options"] = json!([{"label": "Only", "description": "x"}]);
    let err = err_text(&AskUserQuestionTool, &ctx, one_option).await;
    assert!(err.contains("give 2-4 options, not 1"), "{err}");
    let five: Vec<Value> = (0..5)
        .map(|i| {
            let mut q = question()["questions"][0].clone();
            q["question"] = json!(format!("Question {i}?"));
            q
        })
        .collect();
    let err = err_text(&AskUserQuestionTool, &ctx, json!({"questions": five})).await;
    assert!(err.contains("ask 1-4 questions per call, not 5"), "{err}");

    let subagent = with(&dir, Arc::new(FakeInteraction::new(false)));
    let err = AskUserQuestionTool
        .call(question(), &subagent)
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::Unavailable(ref m) if m.contains("subagents")),
        "{err:?}"
    );
    let no_port = AskUserQuestionTool
        .call(question(), &ctx_nothing(&dir))
        .await
        .unwrap_err();
    assert!(matches!(no_port, ToolError::Unavailable(_)));
}

fn ctx_nothing(dir: &tempfile::TempDir) -> ToolCtx {
    ctx(dir.path())
}

#[tokio::test]
async fn exit_plan_mode_requires_plan_mode() {
    let dir = project(&[]);
    let ctx = with(&dir, Arc::new(FakeInteraction::new(true)));
    let err = ExitPlanModeTool::default()
        .call(json!({"plan": "1. Do it"}), &ctx)
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::Unavailable(ref m) if m.contains("only available in plan mode") && m.contains("plan mode is off")),
        "{err:?}"
    );
}

async fn approve(dir: &tempfile::TempDir, mode: Option<PermissionMode>) -> (String, String) {
    let mut port = FakeInteraction::new(true);
    port.decision = PlanDecision::Approve {
        mode,
        edited_plan: Some("1. Do it carefully".into()),
        handoff: false,
    };
    let port = Arc::new(port);
    let mut ctx = with(dir, Arc::clone(&port));
    ctx.mode = PermissionMode::Bypass;
    ctx.planning = true;
    let output = ExitPlanModeTool::default()
        .call(json!({"plan": "1. Do it"}), &ctx)
        .await
        .unwrap();
    assert_eq!(*port.plans.lock().unwrap(), vec!["1. Do it".to_string()]);
    (output.text_content(), output.summary)
}

#[tokio::test]
async fn exit_plan_mode_reports_approval_with_the_edited_plan() {
    let dir = project(&[]);
    let (text, summary) = approve(&dir, Some(PermissionMode::AcceptEdits)).await;
    assert!(
        text.contains("approved the plan. Plan mode is off and the permission mode is acceptEdits"),
        "{text}"
    );
    assert!(
        text.ends_with("implement this version:\n\n1. Do it carefully"),
        "{text}"
    );
    assert_eq!(summary, "Plan approved (acceptEdits)");
}

#[tokio::test]
async fn building_a_plan_keeps_the_permission_level() {
    let dir = project(&[]);
    let (text, summary) = approve(&dir, None).await;
    assert!(text.contains("the permission mode is bypass"), "{text}");
    assert_eq!(summary, "Plan approved (bypass)");
}

#[tokio::test]
async fn exit_plan_mode_reports_revision_feedback() {
    let dir = project(&[]);
    let mut ctx = with(&dir, Arc::new(FakeInteraction::new(true)));
    ctx.planning = true;
    let text = ok_text(
        &ExitPlanModeTool::default(),
        &ctx,
        json!({"plan": "1. Do it"}),
    )
    .await;
    assert!(text.contains("Plan mode is still active"), "{text}");
    assert!(text.ends_with("Feedback:\nmore detail"), "{text}");
    let mut subagent = with(&dir, Arc::new(FakeInteraction::new(false)));
    subagent.planning = true;
    let err = ExitPlanModeTool::default()
        .call(json!({"plan": "x"}), &subagent)
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::Unavailable(_)), "{err:?}");
}
