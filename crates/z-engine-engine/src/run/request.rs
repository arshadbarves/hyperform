//! Request assembly: cache-stable system sections (base prompt,
//! environment, instructions, skills, output style, repository map), the
//! offered tools, the working transcript with cache breakpoints on the
//! last two user messages, thinking from the session effort (else the
//! main agent's routed one, `decisions_routing`), and the output ceiling.

use serde_json::{Value, json};
use z_engine_context::{
    Environment, PromptSection, SystemInputs, build_system, estimate_text, estimate_tools,
    message_breakpoints, system_breakpoint, today, well_formed,
};
use z_engine_llm::{ModelRequest, SystemBlock, ThinkingConfig, ToolChoice, ToolSpec};
use z_engine_protocol::Message;

use super::spec::RunContext;
use crate::decisions::routed_effort;
use crate::settings::models;

/// The parts of a request that do not depend on the working set.
#[derive(Debug, Clone)]
pub(crate) struct Prepared {
    pub sections: Vec<PromptSection>,
    pub tools: Vec<ToolSpec>,
    /// Estimated tokens of the system prompt and tool definitions.
    pub overhead: u64,
}

pub(crate) fn prepare(ctx: &RunContext, model: &str, tools: Vec<ToolSpec>) -> Prepared {
    let settings = ctx.core.settings();
    let shell = settings
        .shell
        .program
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let worktree = ctx.spec.worktree.as_ref();
    let environment = Environment {
        cwd: ctx.resources.cwd().to_string_lossy().into_owned(),
        project_root: ctx.spec.root.to_string_lossy().into_owned(),
        platform: std::env::consts::OS.to_string(),
        shell,
        date: today(),
        model: model.to_string(),
        git: match worktree {
            Some(scope) => scope.git.clone(),
            None => ctx.core.git_info(),
        },
        additional_dirs: ctx
            .core
            .additional_dirs()
            .iter()
            .chain(worktree.map(|scope| &scope.project))
            .map(|dir| dir.to_string_lossy().into_owned())
            .collect(),
    };
    let skills: Vec<(String, String)> = settings
        .extensions
        .skills
        .iter()
        .map(|skill| (skill.name.clone(), skill.description.clone()))
        .collect();
    let output_style = settings
        .settings
        .ui
        .output_style
        .as_deref()
        .and_then(|name| {
            let styles = &settings.extensions.output_styles;
            styles.iter().find(|style| style.name == name)
        });
    let mut sections = build_system(&SystemInputs {
        base_prompt: &ctx.spec.base_prompt,
        environment: &environment,
        instructions: &settings.instructions,
        skills: &skills,
        output_style: output_style.map(|style| style.body.as_str()),
        extra: worktree.map(|scope| scope.note.as_str()),
    });
    if let Some(map) = worktree
        .is_none()
        .then(|| ctx.core.repo_map.current())
        .flatten()
    {
        let after_cached = system_breakpoint(&sections).map_or(0, |index| index + 1);
        sections.insert(after_cached, PromptSection::cached(map.as_ref()));
    }
    let described: Vec<(String, String, Value)> = tools
        .iter()
        .map(|tool| {
            (
                tool.name.clone(),
                tool.description.clone(),
                tool.input_schema.clone(),
            )
        })
        .collect();
    let overhead = sections
        .iter()
        .map(|section| estimate_text(&section.text))
        .sum::<u64>()
        + estimate_tools(&described);
    Prepared {
        sections,
        tools,
        overhead,
    }
}

pub(crate) fn assemble(
    ctx: &RunContext,
    prepared: &Prepared,
    model: &str,
    messages: Vec<Message>,
) -> ModelRequest {
    let settings = ctx.core.settings();
    let catalog = ctx.core.catalog();
    let cached = system_breakpoint(&prepared.sections);
    let system = prepared
        .sections
        .iter()
        .enumerate()
        .map(|(index, section)| SystemBlock {
            text: section.text.clone(),
            cache: Some(index) == cached,
        })
        .collect();
    let effort = ctx.core.with_state(|state| state.effort);
    let effort = effort.or_else(|| ctx.spec.is_main().then(|| routed_effort(&ctx.core))?);
    let mut request = ModelRequest::new(model, well_formed(messages))
        .with_system(system)
        .with_tools(prepared.tools.clone())
        .with_max_tokens(models::max_output_tokens(
            &settings.settings,
            catalog.as_deref(),
            model,
        ));
    request.cache_breakpoints = message_breakpoints(&request.messages);
    request.cache_tools = !request.tools.is_empty();
    request.thinking = effort.map(|effort| ThinkingConfig {
        effort,
        budget_tokens: None,
    });
    request.session_key = Some(ctx.core.id.to_string());
    request
}

/// A readable JSON rendering of a request for the prompt inspector.
pub(crate) fn inspect(request: &ModelRequest) -> Value {
    let tool_choice = match &request.tool_choice {
        ToolChoice::Auto => json!("auto"),
        ToolChoice::None => json!("none"),
        ToolChoice::Any => json!("any"),
        ToolChoice::Tool(name) => json!({ "tool": name }),
    };
    json!({
        "model": request.model,
        "maxTokens": request.max_tokens,
        "temperature": request.temperature,
        "thinking": request.thinking.map(|thinking| json!({
            "effort": thinking.effort.label(),
            "budgetTokens": thinking.budget_tokens,
        })),
        "toolChoice": tool_choice,
        "cacheTools": request.cache_tools,
        "cacheBreakpoints": request.cache_breakpoints,
        "stopSequences": request.stop_sequences,
        "system": request.system.iter().map(|block| json!({
            "text": block.text,
            "cache": block.cache,
        })).collect::<Vec<_>>(),
        "tools": request.tools.iter().map(|tool| json!({
            "name": tool.name,
            "description": tool.description,
            "inputSchema": tool.input_schema,
        })).collect::<Vec<_>>(),
        "messages": request.messages,
    })
}
