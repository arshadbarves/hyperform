//! Every slash command source in precedence order: engine built-ins,
//! custom commands (already layered project > `.claude` project > user >
//! `.claude` user by discovery; they may shadow a prompt built-in but never
//! an engine built-in), prompt built-ins, and MCP prompts exposed as
//! `mcp__<server>__<prompt>`. GUI commands are listed, never resolved.

use std::path::Path;

use z_engine_config::{CommandDef, ExtensionScope, ExtensionSource, parse_command};
use z_engine_integrations::McpPromptInfo;
use z_engine_prompts::commands::BUILTIN;
use z_engine_tools::mcp_tool_name;

/// `(name, description, argument hint)` of the commands `session/slash.rs`
/// answers without the model.
pub(crate) const ENGINE_COMMANDS: &[(&str, &str, Option<&str>)] = &[
    (
        "compact",
        "Summarize older history to free context",
        Some("[instructions]"),
    ),
    ("context", "Show context usage by prompt layer", None),
    ("cost", "Show token usage and cost of this chat", None),
    ("status", "Show model, mode, and session status", None),
    (
        "remember",
        "Save a note to AGENTS.md",
        Some("<project|local|user> <text>"),
    ),
    ("model", "Switch the model of this chat", Some("<model id>")),
    (
        "mode",
        "Set the permission mode, or turn plan mode on or off",
        Some("<default|acceptEdits|bypass|plan>"),
    ),
    (
        "effort",
        "Set the reasoning effort",
        Some("<low|medium|high|max|default>"),
    ),
    ("mcp", "Show MCP server status", None),
    ("todos", "Show the current todo list", None),
    (
        "doctor",
        "Diagnose the provider, tools, and project setup",
        None,
    ),
    (
        "add-dir",
        "Allow another directory for this chat",
        Some("<path> [--save]"),
    ),
];

/// Engine commands listed only while `work_plans` is On.
pub(crate) const WORK_COMMANDS: &[(&str, &str, Option<&str>)] = &[(
    "replan",
    "Re-check the work plan against the workspace and replan what changed",
    Some("[what to reconsider]"),
)];

/// Commands the GUI handles itself.
pub(crate) const UI_COMMANDS: &[(&str, &str, Option<&str>)] = &[
    ("help", "Show commands and keyboard shortcuts", None),
    ("agents", "Open the agents panel", None),
    ("jobs", "Open background jobs", None),
    ("permissions", "Manage permission rules", None),
    ("hooks", "Show recent hook runs", None),
    ("memory", "Remember something in AGENTS.md", None),
    ("config", "Open settings", None),
    ("resume", "Switch to another chat", None),
    (
        "export",
        "Copy the transcript to the clipboard",
        Some("[markdown|json]"),
    ),
    ("clear", "Start a new chat", None),
    ("context", "Show context usage by prompt layer", None),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    Builtin,
    User,
    Project,
    Mcp,
}

impl Origin {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::User => "user",
            Self::Project => "project",
            Self::Mcp => "mcp",
        }
    }

    fn of(scope: ExtensionScope) -> Self {
        match scope {
            ExtensionScope::ClaudeUser | ExtensionScope::User => Self::User,
            ExtensionScope::ClaudeProject | ExtensionScope::Project => Self::Project,
        }
    }
}

/// What running a command means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    /// Answered by the engine without the model.
    Engine,
    /// A markdown template expanded into a prompt.
    Template(Box<CommandDef>),
    /// An MCP prompt rendered by its server.
    Mcp {
        server: String,
        prompt: McpPromptInfo,
    },
    /// Handled by the GUI.
    Ui,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommandEntry {
    pub name: String,
    pub description: String,
    pub argument_hint: Option<String>,
    pub origin: Origin,
    pub target: Target,
}

impl CommandEntry {
    fn table(entry: &(&str, &str, Option<&str>), target: Target) -> Self {
        let (name, description, hint) = *entry;
        Self {
            name: name.to_string(),
            description: description.to_string(),
            argument_hint: hint.map(str::to_string),
            origin: Origin::Builtin,
            target,
        }
    }

    fn template(def: CommandDef, origin: Origin) -> Self {
        Self {
            name: def.name.clone(),
            description: def.description.clone(),
            argument_hint: def.argument_hint.clone(),
            origin,
            target: Target::Template(Box::new(def)),
        }
    }
}

/// Every command the engine runs, in precedence order (first name wins);
/// `work` adds the work-plan commands.
pub(crate) fn command_catalog(
    custom: &[CommandDef],
    mcp_prompts: &[(String, McpPromptInfo)],
    work: bool,
) -> Vec<CommandEntry> {
    let work_commands = if work { WORK_COMMANDS } else { &[] };
    let mut entries: Vec<CommandEntry> = ENGINE_COMMANDS
        .iter()
        .chain(work_commands)
        .map(|entry| CommandEntry::table(entry, Target::Engine))
        .collect();
    let taken = |entries: &[CommandEntry], name: &str| entries.iter().any(|e| e.name == name);
    for def in custom {
        if taken(&entries, &def.name) {
            tracing::debug!(name = %def.name, "custom command shadowed by an engine built-in");
            continue;
        }
        let origin = Origin::of(def.source.scope);
        entries.push(CommandEntry::template(def.clone(), origin));
    }
    for def in prompt_builtins() {
        if !taken(&entries, &def.name) {
            entries.push(CommandEntry::template(def, Origin::Builtin));
        }
    }
    for (server, prompt) in mcp_prompts {
        let name = mcp_tool_name(server, &prompt.name);
        if taken(&entries, &name) {
            continue;
        }
        entries.push(CommandEntry {
            name,
            description: prompt
                .description
                .clone()
                .unwrap_or_else(|| format!("MCP prompt from {server}")),
            argument_hint: argument_hint(prompt),
            origin: Origin::Mcp,
            target: Target::Mcp {
                server: server.clone(),
                prompt: prompt.clone(),
            },
        });
    }
    entries
}

/// The catalog as the composer lists it: an engine built-in the GUI also
/// handles (`context`) is shown once, as the GUI command.
pub(crate) fn listed_commands(
    custom: &[CommandDef],
    mcp_prompts: &[(String, McpPromptInfo)],
    work: bool,
) -> Vec<CommandEntry> {
    let ui_names = |name: &str| UI_COMMANDS.iter().any(|(ui, _, _)| *ui == name);
    let mut entries: Vec<CommandEntry> = command_catalog(custom, mcp_prompts, work)
        .into_iter()
        .filter(|entry| entry.target != Target::Engine || !ui_names(&entry.name))
        .collect();
    entries.extend(
        UI_COMMANDS
            .iter()
            .map(|entry| CommandEntry::table(entry, Target::Ui)),
    );
    entries
}

fn prompt_builtins() -> impl Iterator<Item = CommandDef> {
    BUILTIN.iter().filter_map(|(name, markdown)| {
        let source = ExtensionSource::new(ExtensionScope::User, Path::new(name));
        parse_command(name, markdown, source)
            .inspect_err(|error| tracing::warn!(%name, %error, "built-in command skipped"))
            .ok()
    })
}

/// `<required> [optional]` from the prompt's declared arguments.
fn argument_hint(prompt: &McpPromptInfo) -> Option<String> {
    let words: Vec<String> = prompt
        .arguments
        .iter()
        .map(|arg| {
            if arg.required {
                format!("<{}>", arg.name)
            } else {
                format!("[{}]", arg.name)
            }
        })
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

#[cfg(test)]
mod tests {
    use z_engine_integrations::PromptArgument;

    use super::*;

    fn custom(name: &str, scope: ExtensionScope) -> CommandDef {
        let source = ExtensionSource::new(scope, Path::new("x.md"));
        parse_command(name, "---\ndescription: mine\n---\nbody", source).unwrap()
    }

    #[test]
    fn precedence_is_engine_custom_builtin_mcp() {
        let customs = [
            custom("review", ExtensionScope::Project),
            custom("status", ExtensionScope::User),
        ];
        let prompt = McpPromptInfo {
            name: "review".into(),
            description: None,
            arguments: vec![PromptArgument {
                name: "file".into(),
                description: None,
                required: true,
            }],
        };
        let entries = command_catalog(&customs, &[("fake".into(), prompt)], false);
        let find = |name: &str| entries.iter().find(|e| e.name == name).unwrap();
        assert_eq!(find("status").target, Target::Engine);
        assert_eq!(find("review").origin, Origin::Project);
        assert_eq!(find("init").origin, Origin::Builtin);
        let mcp = find("mcp__fake__review");
        assert_eq!(mcp.argument_hint.as_deref(), Some("<file>"));
        assert_eq!(mcp.origin, Origin::Mcp);
        assert_eq!(entries.iter().filter(|e| e.name == "review").count(), 1);
    }

    #[test]
    fn listing_shows_context_once_as_a_gui_command() {
        let listed = listed_commands(&[], &[], false);
        assert!(listed.iter().all(|e| e.name != "replan"));
        assert!(
            listed_commands(&[], &[], true)
                .iter()
                .any(|e| e.name == "replan")
        );
        let context: Vec<&CommandEntry> = listed.iter().filter(|e| e.name == "context").collect();
        assert_eq!(context.len(), 1);
        assert_eq!(context[0].target, Target::Ui);
        assert!(
            listed
                .iter()
                .any(|e| e.name == "mcp" && e.target == Target::Engine)
        );
    }
}
