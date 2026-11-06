//! Informational commands rendered as markdown: `/context` (with a
//! `ContextReport`), `/cost`, `/todos`, and `/status`.

use std::sync::Arc;

use serde_json::Value;
use tokio_util::sync::CancellationToken;
use z_engine_context::{context_breakdown, estimate_text, estimate_tools, render_instructions};
use z_engine_protocol::{AgentId, ContextBreakdown, Event, JobStatus, TodoStatus, Usage};

use crate::batch::ToolSet;
use crate::run::{AgentSpec, RunContext, prepare_request};
use crate::session::SessionCore;
use crate::settings::models::fast_model;

pub(crate) fn context(core: &Arc<SessionCore>) -> String {
    render_breakdown(&context_report(core))
}

/// Token estimate per prompt layer of the next main-agent request, also
/// sent to the GUI as a `ContextReport`.
pub(crate) fn context_report(core: &Arc<SessionCore>) -> ContextBreakdown {
    let settings = core.settings();
    let ctx = RunContext::new(
        Arc::clone(core),
        AgentSpec::main(core.root.clone(), 1),
        core.main.clone(),
        CancellationToken::new(),
    );
    let tools = ToolSet::offered(&ctx);
    let specs = tools.specs();
    let described: Vec<(String, String, Value)> = specs
        .iter()
        .map(|tool| {
            (
                tool.name.clone(),
                tool.description.clone(),
                tool.input_schema.clone(),
            )
        })
        .collect();
    let prepared = prepare_request(&ctx, &core.main_model(), specs);
    let instructions =
        render_instructions(&settings.instructions).map_or(0, |text| estimate_text(&text));
    let (working, limit) = core.with_state(|state| (state.working.clone(), state.context_limit));
    let breakdown = context_breakdown(
        &prepared.sections,
        estimate_tools(&described),
        instructions,
        &working,
        limit,
    );
    core.events.emit(Event::ContextReport { breakdown });
    breakdown
}

fn render_breakdown(breakdown: &ContextBreakdown) -> String {
    let percent = |tokens: u64| {
        if breakdown.limit == 0 {
            0.0
        } else {
            tokens as f64 * 100.0 / breakdown.limit as f64
        }
    };
    let rows = [
        ("System prompt", breakdown.system),
        ("Tool definitions", breakdown.tools),
        ("Instructions", breakdown.instructions),
        ("Messages", breakdown.messages),
    ];
    let mut out = String::from("| Layer | Tokens | Share |\n|---|---:|---:|\n");
    for (label, tokens) in rows {
        out.push_str(&format!(
            "| {label} | {tokens} | {:.1}% |\n",
            percent(tokens)
        ));
    }
    out.push_str(&format!(
        "| **Total** | **{}** | **{:.1}%** of {} |\n",
        breakdown.total,
        percent(breakdown.total),
        breakdown.limit
    ));
    out.push_str("\nEstimates; the provider's counts are authoritative.");
    out
}

pub(crate) fn cost(core: &SessionCore) -> String {
    let (usage, cost, agents) = core.with_state(|state| {
        let agents: Vec<(String, Usage)> = state
            .agent_usage
            .iter()
            .map(|(agent, usage)| (agent.to_string(), *usage))
            .collect();
        (state.usage, state.cost_usd, agents)
    });
    let mut out = format!(
        "**Session cost: ${cost:.4}**\n\n| Tokens | Count |\n|---|---:|\n\
         | Input | {} |\n| Output | {} |\n| Cache read | {} |\n| Cache write | {} |\n",
        usage.input_tokens, usage.output_tokens, usage.cache_read_tokens, usage.cache_write_tokens
    );
    if agents.len() > 1 {
        out.push_str("\n| Agent | Tokens |\n|---|---:|\n");
        for (agent, usage) in agents {
            out.push_str(&format!("| {agent} | {} |\n", usage.total_tokens()));
        }
    }
    out
}

/// The main agent's todo list as a checklist.
pub(crate) fn todos(core: &SessionCore) -> String {
    let todos = core.with_state(|state| state.todos_of(&AgentId::main()).to_vec());
    if todos.is_empty() {
        return "No todos yet. The agent keeps a list with `TodoWrite` for multi-step work."
            .to_string();
    }
    todos
        .iter()
        .map(|todo| match todo.status {
            TodoStatus::Completed => format!("- [x] {}", todo.content),
            TodoStatus::InProgress => format!("- [ ] **{}** (in progress)", todo.content),
            TodoStatus::Pending => format!("- [ ] {}", todo.content),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn status(core: &SessionCore) -> String {
    let settings = core.settings();
    let client = core.client();
    let running = core
        .jobs
        .list()
        .iter()
        .filter(|job| job.status == JobStatus::Running)
        .count();
    let (model, mode, planning, effort, tokens, limit) = core.with_state(|state| {
        (
            state.model.clone(),
            state.mode,
            state.planning,
            state.effort,
            state.context_tokens,
            state.context_limit,
        )
    });
    let trust = if settings.trusted {
        "trusted"
    } else {
        "not trusted"
    };
    let checkpoints = if core.checkpoints.available() {
        "on"
    } else {
        "off"
    };
    [
        format!("- **Session:** `{}`", core.id),
        format!("- **Project:** `{}` ({trust})", core.root.display()),
        format!(
            "- **Model:** `{model}` (fast: `{}`)",
            fast_model(&settings.settings, &model)
        ),
        format!(
            "- **Provider:** {} at `{}`",
            client.provider(),
            settings.settings.provider.base_url
        ),
        format!("- **Mode:** {}", mode.label()),
        format!("- **Plan mode:** {}", if planning { "on" } else { "off" }),
        format!(
            "- **Effort:** {}",
            effort.map_or("default", |effort| effort.label())
        ),
        format!("- **Context:** {tokens} of {limit} tokens"),
        format!("- **Code checkpoints:** {checkpoints}"),
        format!("- **Background jobs running:** {running}"),
    ]
    .join("\n")
}
