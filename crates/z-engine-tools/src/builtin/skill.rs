//! `Skill`: loads a skill's instructions and names its base directory.

use async_trait::async_trait;
use serde_json::{Value, json};
use z_engine_policy::Action;
use z_engine_prompts::tools as prompts;

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::{Fields, str_field};
use crate::names;
use crate::output::ToolOutput;
use crate::schema;
use crate::tool::Tool;

#[derive(Debug, Default)]
pub struct SkillTool;

/// Accepts `name` and `/name` (the slash-command spelling).
fn skill_name(raw: &str) -> &str {
    raw.trim().trim_start_matches('/')
}

#[async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str {
        names::SKILL
    }

    fn description(&self) -> String {
        prompts::SKILL.to_string()
    }

    fn input_schema(&self) -> Value {
        schema::object(
            json!({
                "skill": {"type": "string", "description": "The name of the skill to load, as listed."}
            }),
            &["skill"],
        )
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    fn action(&self, input: &Value, _ctx: &ToolCtx) -> Action {
        Action::Skill {
            name: skill_name(str_field(input, "skill").unwrap_or_default()).to_string(),
        }
    }

    fn title(&self, input: &Value, _ctx: &ToolCtx) -> String {
        format!(
            "Load skill {}",
            skill_name(str_field(input, "skill").unwrap_or_default())
        )
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let name = skill_name(Fields::new(&input)?.non_empty("skill")?);
        let skills = ctx.ports.skills()?;
        let content = skills.load(name).map_err(|reason| {
            let available: Vec<String> = skills.list().into_iter().map(|(name, _)| name).collect();
            let listing = if available.is_empty() {
                "No skills are available.".to_string()
            } else {
                format!("Available skills: {}.", available.join(", "))
            };
            ToolError::failed(format!(
                "Skill {name:?} could not be loaded: {reason}. {listing}"
            ))
        })?;
        Ok(ToolOutput::text(
            format!(
                "Skill: {name}\nBase directory for this skill: {}\n\n{}",
                content.dir, content.body
            ),
            format!("Loaded skill {name}"),
        ))
    }
}
