//! `ExitPlanMode`: submits the plan for review in plan mode. With work
//! plans on, it also takes the plan's goal, requirements and invariants,
//! which draft the work plan the user reviews.

use async_trait::async_trait;
use hyperform_policy::Action;
use hyperform_prompts::tools as prompts;
use hyperform_protocol::PlanDecision;
use serde_json::{Value, json};

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::Fields;
use crate::names;
use crate::output::ToolOutput;
use crate::ports::PlanOutline;
use crate::schema;
use crate::tool::Tool;

#[derive(Debug, Default)]
pub struct ExitPlanModeTool {
    /// Takes `goal`, `requirements` and `invariants` (`work_plans`).
    structured: bool,
}

impl ExitPlanModeTool {
    /// The variant the engine registers while work plans are on.
    pub fn structured() -> Self {
        Self { structured: true }
    }

    /// The work-plan outline of `input`; `None` without a goal.
    fn outline(&self, fields: Fields<'_>, plan: &str) -> Result<Option<PlanOutline>, ToolError> {
        if !self.structured {
            return Ok(None);
        }
        let Some(goal) = fields.optional_text("goal")? else {
            return Ok(None);
        };
        let list = |key: &str| -> Result<Vec<String>, ToolError> {
            let items: Vec<String> = fields.parse(key)?.unwrap_or_default();
            Ok(items
                .into_iter()
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect())
        };
        Ok(Some(PlanOutline {
            goal: goal.trim().to_string(),
            document: plan.to_string(),
            requirements: list("requirements")?,
            invariants: list("invariants")?,
        }))
    }
}

#[async_trait]
impl Tool for ExitPlanModeTool {
    fn name(&self) -> &str {
        names::EXIT_PLAN_MODE
    }

    fn description(&self) -> String {
        prompts::EXIT_PLAN_MODE.to_string()
    }

    fn input_schema(&self) -> Value {
        let mut properties = json!({
            "plan": {"type": "string", "description": "The complete plan in markdown."}
        });
        if self.structured {
            let strings = |description: &str| json!({"type": "array", "items": {"type": "string"}, "description": description});
            properties["goal"] = json!({"type": "string", "description": "One sentence: what the work delivers. Starts the work plan the user reviews with this plan."});
            properties["requirements"] = strings(
                "What the work must deliver, one per item; the user accepts them by approving.",
            );
            properties["invariants"] =
                strings("What must stay true while the work is done, one per item.");
        }
        schema::object(properties, &["plan"])
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

    fn title(&self, _input: &Value, _ctx: &ToolCtx) -> String {
        "Propose plan".to_string()
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let fields = Fields::new(&input)?;
        let plan = fields.non_empty("plan")?;
        if !ctx.planning {
            return Err(ToolError::unavailable(
                "ExitPlanMode is only available in plan mode, and plan mode is off. Continue with the task directly.",
            ));
        }
        let interaction = ctx.ports.interaction()?;
        if !interaction.can_ask(ctx) {
            return Err(ToolError::unavailable(
                "ExitPlanMode is not available to subagents; put the plan in your final report instead.",
            ));
        }
        if let Some(outline) = self.outline(fields, plan)? {
            let work = ctx.ports.work()?;
            work.outline_plan(ctx, outline)
                .await
                .map_err(ToolError::failed)?;
        }
        let decision = ctx
            .until_cancelled(interaction.propose_plan(ctx, plan.to_string()))
            .await?
            .map_err(ToolError::failed)?;
        Ok(match decision {
            PlanDecision::Approve { handoff: true, .. } => ToolOutput::text(
                "The user approved the plan and will build it in a new chat. Plan mode is off. Do not implement anything here: end your turn now with one short line saying the plan is ready to build.",
                "Plan approved (new chat)",
            ),
            PlanDecision::Approve {
                mode, edited_plan, ..
            } => {
                let mode = mode.unwrap_or(ctx.mode);
                let mut text = format!(
                    "The user approved the plan. Plan mode is off and the permission mode is {}, so you can start implementing it.",
                    mode.label()
                );
                if let Some(edited) = edited_plan.filter(|edited| !edited.trim().is_empty()) {
                    text.push_str(&format!(
                        "\n\nThe user edited the plan; implement this version:\n\n{edited}"
                    ));
                }
                ToolOutput::text(text, format!("Plan approved ({})", mode.label()))
            }
            PlanDecision::Revise { feedback } => ToolOutput::text(
                format!(
                    "The user wants changes before approving the plan. Plan mode is still active: revise the plan and submit it again with ExitPlanMode.\n\nFeedback:\n{feedback}"
                ),
                "Changes requested",
            ),
        })
    }
}
