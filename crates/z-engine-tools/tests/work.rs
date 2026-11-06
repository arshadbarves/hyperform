//! Work-plan tools: inputs reach the `WorkPort` decoded, the port's
//! rejections come back as errors, and `ExitPlanMode` drafts the plan only
//! in its structured variant.

mod support;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::json;
use support::fakes::FakeInteraction;
use support::{ctx_with, err_text, ok_text, project};
use z_engine_protocol::PlanDecision;
use z_engine_protocol::work::TaskKind;
use z_engine_tools::builtin::{
    ExitPlanModeTool, PlanRecordTool, TaskCreateTool, TaskGetTool, TaskListTool, TaskUpdateTool,
};
use z_engine_tools::{
    NewTask, PlanNote, PlanOutline, Ports, TaskChange, Tool, ToolCtx, WorkPort, WorkReply,
};

#[derive(Debug, Default)]
struct FakeWork {
    created: Mutex<Vec<NewTask>>,
    changes: Mutex<Vec<TaskChange>>,
    notes: Mutex<Vec<PlanNote>>,
    outlines: Mutex<Vec<PlanOutline>>,
}

#[async_trait]
impl WorkPort for FakeWork {
    async fn create_task(&self, _ctx: &ToolCtx, task: NewTask) -> Result<WorkReply, String> {
        self.created.lock().unwrap().push(task);
        Ok(WorkReply::new("Created t1.", "Created t1"))
    }

    async fn update_task(&self, _ctx: &ToolCtx, change: TaskChange) -> Result<WorkReply, String> {
        let done = change.status.as_deref() == Some("completed");
        self.changes.lock().unwrap().push(change);
        if done {
            return Err("task t1: criterion c1 needs a passing test check".into());
        }
        Ok(WorkReply::new("Updated t1.", "Updated t1"))
    }

    async fn get_task(&self, _ctx: &ToolCtx, task_id: &str) -> Result<WorkReply, String> {
        Ok(WorkReply::new(format!("Goal: x\nTask {task_id}"), task_id))
    }

    async fn list_tasks(&self, _ctx: &ToolCtx) -> Result<WorkReply, String> {
        Ok(WorkReply::new("Plan: x", "2 tasks"))
    }

    async fn record(&self, _ctx: &ToolCtx, note: PlanNote) -> Result<WorkReply, String> {
        self.notes.lock().unwrap().push(note);
        Ok(WorkReply::new("Recorded e1.", "Recorded e1"))
    }

    async fn outline_plan(&self, _ctx: &ToolCtx, outline: PlanOutline) -> Result<(), String> {
        self.outlines.lock().unwrap().push(outline);
        Ok(())
    }
}

fn with(dir: &tempfile::TempDir, work: &Arc<FakeWork>, interaction: FakeInteraction) -> ToolCtx {
    ctx_with(
        dir.path(),
        Ports {
            work: Some(Arc::clone(work) as Arc<dyn WorkPort>),
            interaction: Some(Arc::new(interaction)),
            ..Ports::default()
        },
    )
}

#[tokio::test]
async fn task_create_decodes_core_fields_and_extras() {
    let dir = project(&[]);
    let work = Arc::new(FakeWork::default());
    let ctx = with(&dir, &work, FakeInteraction::new(true));
    let input = json!({
        "subject": "Add retry",
        "description": "Retries 429 and 503",
        "activeForm": "Adding retry",
        "blockedBy": ["t0"],
        "kind": "implement",
        "acceptance": [{"text": "Retry tests pass", "method": "test"}],
        "parentId": null
    });
    assert_eq!(ok_text(&TaskCreateTool, &ctx, input).await, "Created t1.");
    let created = work.created.lock().unwrap()[0].clone();
    assert_eq!(created.active_form, "Adding retry");
    assert_eq!(created.blocked_by, vec!["t0".to_string()]);
    assert_eq!(created.kind, Some(TaskKind::Implement));
    assert_eq!(created.acceptance[0].method, "test");
    let err = err_text(
        &TaskCreateTool,
        &ctx,
        json!({"subject": " ", "description": ""}),
    )
    .await;
    assert!(err.contains("`subject` must not be empty"), "{err}");
}

#[tokio::test]
async fn task_update_reports_the_kernel_reason() {
    let dir = project(&[]);
    let work = Arc::new(FakeWork::default());
    let ctx = with(&dir, &work, FakeInteraction::new(true));
    let input = json!({"taskId": "t1", "status": "in_progress", "metCriteria": ["c2"]});
    assert_eq!(ok_text(&TaskUpdateTool, &ctx, input).await, "Updated t1.");
    assert_eq!(
        work.changes.lock().unwrap()[0].met_criteria,
        vec!["c2".to_string()]
    );
    let err = err_text(
        &TaskUpdateTool,
        &ctx,
        json!({"taskId": "t1", "status": "completed"}),
    )
    .await;
    assert!(err.contains("needs a passing test check"), "{err}");
    assert_eq!(
        TaskUpdateTool.title(&json!({"taskId": "t1", "status": "completed"}), &ctx),
        "Update task t1: completed"
    );
}

#[tokio::test]
async fn reads_and_records_go_through_the_port() {
    let dir = project(&[]);
    let work = Arc::new(FakeWork::default());
    let ctx = with(&dir, &work, FakeInteraction::new(true));
    let packet = ok_text(&TaskGetTool, &ctx, json!({"taskId": "t2"})).await;
    assert_eq!(packet, "Goal: x\nTask t2");
    assert_eq!(ok_text(&TaskListTool, &ctx, json!({})).await, "Plan: x");
    let note = json!({"kind": "decision", "text": "Use backoff", "alternatives": ["fixed delay"]});
    assert_eq!(ok_text(&PlanRecordTool, &ctx, note).await, "Recorded e1.");
    assert_eq!(work.notes.lock().unwrap()[0].alternatives.len(), 1);
    let err = err_text(&PlanRecordTool, &ctx, json!({"kind": "rule", "text": "x"})).await;
    assert!(err.contains("`kind` must be one of"), "{err}");
}

#[tokio::test]
async fn work_tools_without_the_port_are_unavailable() {
    let dir = project(&[]);
    let ctx = support::ctx(dir.path());
    let err = err_text(&TaskListTool, &ctx, json!({})).await;
    assert!(err.contains("work plans are not available"), "{err}");
}

#[tokio::test]
async fn structured_exit_plan_mode_drafts_the_plan_and_hands_off() {
    let dir = project(&[]);
    let work = Arc::new(FakeWork::default());
    let mut interaction = FakeInteraction::new(true);
    interaction.decision = PlanDecision::Approve {
        mode: None,
        edited_plan: None,
        handoff: true,
    };
    let mut ctx = with(&dir, &work, interaction);
    ctx.planning = true;
    let tool = ExitPlanModeTool::structured();
    assert!(tool.input_schema()["properties"]["goal"].is_object());
    assert!(ExitPlanModeTool::default().input_schema()["properties"]["goal"].is_null());
    let input = json!({
        "plan": "1. Do it",
        "goal": "Ship retries",
        "requirements": ["Retries 429", " "],
        "invariants": ["API unchanged"]
    });
    let text = ok_text(&tool, &ctx, input).await;
    assert!(text.contains("will build it in a new chat"), "{text}");
    assert!(text.contains("Do not implement anything here"), "{text}");
    let outline = work.outlines.lock().unwrap()[0].clone();
    assert_eq!(outline.goal, "Ship retries");
    assert_eq!(outline.requirements, vec!["Retries 429".to_string()]);
    assert_eq!(outline.document, "1. Do it");
    let plain = ok_text(&tool, &ctx, json!({"plan": "1. Do it"})).await;
    assert!(plain.contains("new chat"), "{plain}");
    assert_eq!(work.outlines.lock().unwrap().len(), 1, "no goal, no draft");
}
