//! `Agent`: starts a subagent (foreground, background, or resumed) through
//! the agent port. The description lists the agent types captured when
//! the tool was built, so it stays stable for prompt caching.

use async_trait::async_trait;
use hyperform_policy::Action;
use hyperform_prompts::tools as prompts;
use hyperform_protocol::{AgentId, Isolation};
use serde_json::{Value, json};

use crate::context::ToolCtx;
use crate::error::ToolError;
use crate::input::{Fields, str_field};
use crate::names;
use crate::output::ToolOutput;
use crate::ports::{AgentCard, SpawnRequest};
use crate::schema;
use crate::text::truncate_output;
use crate::tool::Tool;

#[derive(Debug, Clone, Default)]
pub struct AgentTool {
    catalog: Vec<AgentCard>,
    /// Takes `task_id` (`work_plans`).
    tasks: bool,
}

impl AgentTool {
    pub fn new(catalog: Vec<AgentCard>) -> Self {
        Self {
            catalog,
            tasks: false,
        }
    }

    /// The variant the engine registers while work plans are on.
    pub fn with_tasks(catalog: Vec<AgentCard>) -> Self {
        Self {
            catalog,
            tasks: true,
        }
    }
}

/// One `- name: description (Tools: ...)` line per agent type.
fn catalog_lines(catalog: &[AgentCard]) -> String {
    if catalog.is_empty() {
        return "- (no agent types are available)".to_string();
    }
    let lines: Vec<String> = catalog
        .iter()
        .map(|card| {
            let description = card
                .description
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            match card.tools.trim() {
                "" => format!("- {}: {description}", card.name),
                tools => format!("- {}: {description} (Tools: {tools})", card.name),
            }
        })
        .collect();
    lines.join("\n")
}

fn request(fields: &Fields<'_>, tasks: bool) -> Result<SpawnRequest, ToolError> {
    let task_id = if tasks {
        fields
            .optional_text("task_id")?
            .map(|id| id.trim().to_string())
    } else {
        None
    };
    let isolation = match fields.optional_text("isolation")? {
        None => None,
        Some("shared") => Some(Isolation::Shared),
        Some("worktree") => Some(Isolation::Worktree),
        Some(other) => {
            return Err(ToolError::invalid(format!(
                "`isolation` must be shared or worktree, not {other:?}"
            )));
        }
    };
    Ok(SpawnRequest {
        agent_type: fields.non_empty("subagent_type")?.trim().to_string(),
        description: fields.non_empty("description")?.trim().to_string(),
        prompt: fields.non_empty("prompt")?.to_string(),
        background: fields.bool("run_in_background")?.unwrap_or(false),
        resume: fields
            .optional_text("resume")?
            .map(|id| AgentId::from(id.trim())),
        isolation,
        task_id: task_id.filter(|id| !id.is_empty()),
    })
}

#[async_trait]
impl Tool for AgentTool {
    fn name(&self) -> &str {
        names::AGENT
    }

    fn description(&self) -> String {
        prompts::AGENT.replace("{{agents}}", &catalog_lines(&self.catalog))
    }

    fn input_schema(&self) -> Value {
        let mut properties = json!({
            "description": {"type": "string", "description": "A short (3-5 word) label for the agent."},
            "prompt": {"type": "string", "description": "The complete, self-contained task for the agent (or the follow-up message when resuming)."},
            "subagent_type": {"type": "string", "description": "The agent type to use, from the list in the description."},
            "run_in_background": {"type": "boolean", "description": "Return a job id at once instead of waiting for the report."},
            "resume": {"type": "string", "description": "The agent_id of an earlier agent to continue with its context."},
            "isolation": {"type": "string", "enum": ["shared", "worktree"], "description": "shared (default) works in the project tree; worktree works in a separate git worktree."}
        });
        if self.tasks {
            properties["task_id"] = json!({"type": "string", "description": "A ready task of the work plan (t1, t2, ...) to dispatch this helper for. The helper gets the task's packet, owns the task, and must report its result. It works in its own worktree beside a running task, and whenever the plan's parallel work is approved. An interrupted task (its resume pointer names a helper of this chat) continues with `resume` set to that helper's agent_id; any other helper starts it afresh."});
        }
        schema::object(properties, &["description", "prompt", "subagent_type"])
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        false
    }

    /// Agents run in parallel; their own calls are gated individually.
    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        true
    }

    fn action(&self, input: &Value, _ctx: &ToolCtx) -> Action {
        Action::Agent {
            agent_type: str_field(input, "subagent_type")
                .unwrap_or_default()
                .trim()
                .to_string(),
        }
    }

    fn title(&self, input: &Value, _ctx: &ToolCtx) -> String {
        let kind = str_field(input, "subagent_type").unwrap_or("agent").trim();
        match str_field(input, "description").map(str::trim) {
            Some(description) if !description.is_empty() => format!("{kind}: {description}"),
            _ => format!("{kind} agent"),
        }
    }

    async fn call(&self, input: Value, ctx: &ToolCtx) -> Result<ToolOutput, ToolError> {
        ctx.check_cancelled()?;
        let req = request(&Fields::new(&input)?, self.tasks)?;
        let agents = ctx.ports.agents()?;
        let known = agents.catalog();
        if !known.is_empty() && !known.iter().any(|card| card.name == req.agent_type) {
            let names: Vec<&str> = known.iter().map(|card| card.name.as_str()).collect();
            return Err(ToolError::invalid(format!(
                "unknown subagent_type {:?}; available types: {}",
                req.agent_type,
                names.join(", ")
            )));
        }
        let kind = req.agent_type.clone();
        let outcome = ctx
            .until_cancelled(agents.spawn(ctx, req))
            .await?
            .map_err(ToolError::failed)?;
        let report = truncate_output(ctx, "agent", outcome.text.trim_end());
        let footer = outcome.footer.trim();
        let (text, summary) = match &outcome.job_id {
            Some(job) => (
                format!("{report}\n\nagent_id: {}\njob_id: {job}", outcome.agent_id),
                format!("{kind} agent running in the background"),
            ),
            None => (
                format!(
                    "{report}\n\n{footer}{}agent_id: {} (pass it as `resume` to continue this agent)",
                    if footer.is_empty() { "" } else { "\n" },
                    outcome.agent_id
                ),
                format!("{kind} agent finished"),
            ),
        };
        Ok(ToolOutput::text(text, summary))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_lines_list_every_type() {
        let lines = catalog_lines(&[
            AgentCard {
                name: "explore".into(),
                description: "Fast search.\nRead-only.".into(),
                tools: "Read, Grep".into(),
            },
            AgentCard {
                name: "general".into(),
                description: "Everything.".into(),
                tools: String::new(),
            },
        ]);
        assert_eq!(
            lines,
            "- explore: Fast search. Read-only. (Tools: Read, Grep)\n- general: Everything."
        );
    }

    #[test]
    fn only_the_work_plan_variant_takes_a_task_id() {
        let plain = AgentTool::new(Vec::new());
        let tasks = AgentTool::with_tasks(Vec::new());
        assert!(plain.input_schema()["properties"].get("task_id").is_none());
        let task_id = &tasks.input_schema()["properties"]["task_id"]["description"];
        let worktree = task_id.as_str().unwrap();
        assert!(worktree.contains("parallel work is approved"), "{worktree}");
        assert!(worktree.contains("`resume`"), "{worktree}");
        assert_eq!(plain.description(), tasks.description());
        let input = json!({"description": "d", "prompt": "p", "subagent_type": "general", "task_id": " t2 "});
        let fields = Fields::new(&input).unwrap();
        assert_eq!(request(&fields, false).unwrap().task_id, None);
        assert_eq!(
            request(&fields, true).unwrap().task_id.as_deref(),
            Some("t2")
        );
    }
}
