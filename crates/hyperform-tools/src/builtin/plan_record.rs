//! `PlanRecord` (`work_plans`): a design decision, discovery, experiment,
//! evidence note, or a proposed requirement or invariant.

use async_trait::async_trait;
use hyperform_policy::Action;
use hyperform_prompts::tools as prompts;
use serde_json::{Value, json};

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::{str_field, whole};
use crate::names;
use crate::output::ToolOutput;
use crate::ports::PlanNote;
use crate::schema;
use crate::tool::Tool;

const KINDS: [&str; 6] = [
    "decision",
    "discovery",
    "experiment",
    "evidence",
    "requirement",
    "invariant",
];

#[derive(Debug, Default)]
pub struct PlanRecordTool;

#[async_trait]
impl Tool for PlanRecordTool {
    fn name(&self) -> &str {
        names::PLAN_RECORD
    }

    fn description(&self) -> String {
        prompts::PLAN_RECORD.to_string()
    }

    fn input_schema(&self) -> Value {
        let text = |description: &str| json!({"type": "string", "description": description});
        let ids = |description: &str| json!({"type": "array", "items": {"type": "string"}, "description": description});
        schema::object(
            json!({
                "kind": {"type": "string", "enum": KINDS},
                "text": text("The choice, fact, hypothesis, note, requirement or invariant."),
                "title": text("decision: the question it answers."),
                "alternatives": ids("decision: the options you rejected."),
                "reason": text("decision: why this choice."),
                "source": text("discovery: the path, command or URL it comes from."),
                "method": text("experiment: how you test the hypothesis."),
                "experimentId": text("Concludes this experiment."),
                "outcome": {"type": "string", "enum": ["confirmed", "refuted", "inconclusive"]},
                "result": text("experiment: what happened."),
                "conclusion": text("experiment: what it means for the plan."),
                "taskIds": ids("Tasks it is about."),
                "files": ids("Paths it concerns."),
                "criterionId": text("evidence: the criterion of the one task in taskIds.")
            }),
            &["kind", "text"],
        )
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        false
    }

    fn action(&self, _input: &Value, _ctx: &ToolCtx) -> Action {
        Action::Other { read_only: true }
    }

    fn title(&self, input: &Value, _ctx: &ToolCtx) -> String {
        match str_field(input, "kind").map(str::trim) {
            Some(kind) if KINDS.contains(&kind) => format!("Record {kind}"),
            _ => "Record in plan".to_string(),
        }
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let note: PlanNote = whole(&input)?;
        if !KINDS.contains(&note.kind.trim()) {
            return Err(ToolError::invalid(format!(
                "`kind` must be one of {}",
                KINDS.join(", ")
            )));
        }
        if note.text.trim().is_empty() && note.experiment_id.is_none() {
            return Err(ToolError::invalid("`text` must not be empty"));
        }
        let work = ctx.ports.work()?;
        let reply = work.record(ctx, note).await.map_err(ToolError::failed)?;
        Ok(ToolOutput::text(reply.text, reply.summary))
    }
}
