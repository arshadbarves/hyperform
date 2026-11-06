//! Work plans (`work_plans`): the attached plan's tasks and ledger. The
//! engine turns these inputs into kernel proposals; the kernel accepts or
//! rejects them with precise reasons, which come back as `Err` text.

use async_trait::async_trait;
use hyperform_protocol::work::{ReportDraft, TaskKind, TaskPriority};
use serde::Deserialize;

use crate::context::ToolCtx;

/// An acceptance criterion as the model writes it; `method` is a check
/// kind (`test`, `build`, `lint`, `typecheck`, `format`, `custom`),
/// `inspection` or `user`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CriterionInput {
    pub text: String,
    pub method: String,
}

/// `TaskCreate` input: Claude Code's core fields plus optional extras.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NewTask {
    pub subject: String,
    pub description: String,
    pub active_form: String,
    pub parent_id: Option<String>,
    pub blocked_by: Vec<String>,
    pub scope: Vec<String>,
    pub acceptance: Vec<CriterionInput>,
    pub requirements: Vec<String>,
    pub kind: Option<TaskKind>,
    pub priority: Option<TaskPriority>,
}

/// `TaskUpdate` input; only the fields that are set change.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TaskChange {
    pub task_id: String,
    /// `pending`, `in_progress`, `completed`, `blocked`, `failed` or
    /// `deleted`.
    pub status: Option<String>,
    /// Why the task is blocked or failed.
    pub reason: Option<String>,
    pub subject: Option<String>,
    pub description: Option<String>,
    pub active_form: Option<String>,
    pub result: Option<String>,
    pub scope: Option<Vec<String>>,
    pub acceptance: Option<Vec<CriterionInput>>,
    pub kind: Option<TaskKind>,
    pub priority: Option<TaskPriority>,
    pub add_blocked_by: Vec<String>,
    pub remove_blocked_by: Vec<String>,
    pub add_blocks: Vec<String>,
    /// Inspection criteria the agent confirms.
    pub met_criteria: Vec<String>,
    /// A dispatched helper's result: recorded when its run ends, with the
    /// changed files and checks the engine observed.
    pub report: Option<ReportDraft>,
}

/// `PlanRecord` input.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PlanNote {
    /// `decision`, `discovery`, `experiment`, `evidence`, `requirement` or
    /// `invariant`.
    pub kind: String,
    pub text: String,
    pub title: Option<String>,
    pub alternatives: Vec<String>,
    pub reason: Option<String>,
    pub source: Option<String>,
    pub method: Option<String>,
    pub experiment_id: Option<String>,
    pub outcome: Option<String>,
    pub result: Option<String>,
    pub conclusion: Option<String>,
    pub task_ids: Vec<String>,
    pub files: Vec<String>,
    pub criterion_id: Option<String>,
}

/// The structured part of an `ExitPlanMode` plan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanOutline {
    pub goal: String,
    /// The plan markdown.
    pub document: String,
    pub requirements: Vec<String>,
    pub invariants: Vec<String>,
}

/// What a work tool returns: text for the model and a short title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkReply {
    pub text: String,
    pub summary: String,
}

impl WorkReply {
    pub fn new(text: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            summary: summary.into(),
        }
    }
}

#[async_trait]
pub trait WorkPort: Send + Sync {
    /// Creates a task, starting a plan when none is attached.
    async fn create_task(&self, ctx: &ToolCtx, task: NewTask) -> Result<WorkReply, String>;

    async fn update_task(&self, ctx: &ToolCtx, change: TaskChange) -> Result<WorkReply, String>;

    /// The task's packet, rendered.
    async fn get_task(&self, ctx: &ToolCtx, task_id: &str) -> Result<WorkReply, String>;

    async fn list_tasks(&self, ctx: &ToolCtx) -> Result<WorkReply, String>;

    async fn record(&self, ctx: &ToolCtx, note: PlanNote) -> Result<WorkReply, String>;

    /// Drafts (or revises) the plan behind an `ExitPlanMode` call before
    /// the user reviews it.
    async fn outline_plan(&self, ctx: &ToolCtx, outline: PlanOutline) -> Result<(), String>;
}
