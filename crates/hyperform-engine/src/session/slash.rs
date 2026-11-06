//! Slash command dispatch. Engine built-ins are answered here without the
//! model (informational ones as `CommandOutput`); prompt commands (custom,
//! built-in prompt and MCP prompt) go back to the actor to run as a turn;
//! an unknown name gets a notice with close matches.

use std::sync::Arc;

use hyperform_protocol::{Effort, Event, ModeName, NoticeLevel};

use super::control;
use super::grants::add_dir;
use super::remember::remember;
use super::reports;
use crate::commands::{
    CommandCall, CommandEntry, Target, command_catalog, doctor, mcp_report, suggest,
};
use crate::session::SessionCore;
use crate::work::{self, Replan};

#[derive(Debug)]
pub(crate) enum Slash {
    /// The actor runs a manual compaction with these instructions.
    Compact(Option<String>),
    /// The actor runs this prompt command as a turn.
    Prompt(CommandCall),
    Handled,
}

/// Every command this session can run, in precedence order.
pub(crate) fn session_commands(core: &SessionCore) -> Vec<CommandEntry> {
    command_catalog(
        &core.settings().extensions.commands,
        &core.mcp.prompts(),
        work::enabled(core),
    )
}

pub(crate) async fn run_command(core: &Arc<SessionCore>, name: &str, args: &str) -> Slash {
    let name = name.trim().trim_start_matches('/');
    let catalog = session_commands(core);
    let Some(entry) = catalog.iter().find(|entry| entry.name == name) else {
        unknown(core, name, &catalog);
        return Slash::Handled;
    };
    match &entry.target {
        Target::Engine => engine_command(core, name, args).await,
        Target::Template(_) | Target::Mcp { .. } => Slash::Prompt(CommandCall {
            name: name.to_string(),
            args: args.trim().to_string(),
            target: entry.target.clone(),
        }),
        Target::Ui => {
            core.events
                .notice(NoticeLevel::Info, format!("/{name} is handled by the app."));
            Slash::Handled
        }
    }
}

async fn engine_command(core: &Arc<SessionCore>, name: &str, args: &str) -> Slash {
    let args = args.trim();
    match name {
        "compact" => {
            let instructions = Some(args.to_string()).filter(|text| !text.is_empty());
            return Slash::Compact(instructions);
        }
        "context" => output(core, "context", reports::context(core)),
        "cost" => output(core, "cost", reports::cost(core)),
        "status" => output(core, "status", reports::status(core)),
        "todos" => output(core, "todos", reports::todos(core)),
        "mcp" => output(core, "mcp", mcp_report(core)),
        "doctor" => output(core, "doctor", doctor(core).await),
        "remember" => match remember(core, args).await {
            Ok((path, bullet)) => output(
                core,
                "remember",
                format!("Saved to `{}`:\n\n{bullet}", path.display()),
            ),
            Err(error) => core.events.notice(NoticeLevel::Warn, error.to_string()),
        },
        "add-dir" => match add_dir(core, args).await {
            Ok(markdown) => output(core, "add-dir", markdown),
            Err(error) => core.events.notice(NoticeLevel::Warn, error.to_string()),
        },
        "model" => model(core, args),
        "replan" => match work::replan(core, args).await {
            Replan::Turn(def) => {
                return Slash::Prompt(CommandCall {
                    name: "replan".to_string(),
                    args: args.to_string(),
                    target: Target::Template(def),
                });
            }
            Replan::Nothing(markdown) => output(core, "replan", markdown),
        },
        "mode" => match ModeName::parse(args) {
            Some(ModeName::Plan) => {
                control::set_planning(core, !core.planning());
                output(core, "mode", mode_report(core));
            }
            Some(ModeName::Level(mode)) => {
                control::set_mode(core, mode);
                output(core, "mode", mode_report(core));
            }
            None => usage(core, "/mode <default|acceptEdits|bypass|plan>"),
        },
        "effort" => {
            let effort = match args {
                "default" => Some(None),
                other => Effort::parse(other).map(Some),
            };
            match effort {
                Some(effort) => {
                    control::set_effort(core, effort);
                    let label = effort.map_or("default", Effort::label);
                    output(core, "effort", format!("Reasoning effort: **{label}**"));
                }
                None => usage(core, "/effort <low|medium|high|max|default>"),
            }
        }
        other => core
            .events
            .notice(NoticeLevel::Warn, format!("/{other} has no handler.")),
    }
    Slash::Handled
}

fn model(core: &SessionCore, args: &str) {
    if args.is_empty() {
        let current = core.main_model();
        output(
            core,
            "model",
            format!("Current model: `{current}`. Switch with `/model <id>`."),
        );
        return;
    }
    control::set_model(core, args.to_string());
    output(core, "model", format!("Model: `{}`", core.main_model()));
}

fn mode_report(core: &SessionCore) -> String {
    let planning = if core.planning() { "on" } else { "off" };
    format!(
        "Permission mode: **{}** · plan mode: **{planning}**",
        core.mode().label()
    )
}

fn usage(core: &SessionCore, usage: &str) {
    core.events
        .notice(NoticeLevel::Warn, format!("Usage: {usage}"));
}

fn unknown(core: &SessionCore, name: &str, catalog: &[CommandEntry]) {
    let matches = suggest(name, catalog.iter().map(|entry| entry.name.as_str()));
    let hint = match matches.as_slice() {
        [] => String::new(),
        names => {
            let names: Vec<String> = names.iter().map(|name| format!("/{name}")).collect();
            format!(" Did you mean {}?", names.join(", "))
        }
    };
    core.events.notice(
        NoticeLevel::Info,
        format!("/{name} is not a known command.{hint}"),
    );
}

fn output(core: &SessionCore, name: &str, markdown: String) {
    core.events.emit(Event::CommandOutput {
        name: name.to_string(),
        markdown,
    });
}
