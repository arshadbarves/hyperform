//! What the composer offers: agents for `@` mentions and slash commands
//! (engine built-ins, prompt commands from every source, GUI commands).

use std::path::Path;

use hyperform_config::{
    AgentDef, ExtensionScope, ExtensionSource, FeatureId, discover_extensions, parse_agent,
};
use hyperform_integrations::McpPromptInfo;
use hyperform_prompts::agents::BUILTIN;
use serde::Serialize;

use crate::commands::{CommandEntry, Target, listed_commands};
use crate::engine::Engine;
use crate::orchestration::offered;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCard {
    pub name: String,
    pub description: String,
    /// `builtin`, `user` or `project` (`.claude/` folders count as their level).
    pub source: String,
    pub model: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SlashKind {
    /// Answered by the engine without the model; sent as `runCommand`.
    Engine,
    /// Expanded into a prompt for the model; sent as `runCommand`.
    Prompt,
    /// Handled by the GUI.
    Ui,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlashCommandInfo {
    pub name: String,
    pub description: String,
    pub argument_hint: Option<String>,
    /// `builtin`, `user`, `project` or `mcp`.
    pub source: String,
    pub kind: SlashKind,
}

impl Engine {
    /// Built-in agents (`audit` with `work_plans`), then custom agents of
    /// `project_root` (a custom agent replaces a built-in of the same
    /// name), sorted by name.
    pub fn agent_cards(&self, project_root: &Path) -> Vec<AgentCard> {
        let settings = self.settings(Some(project_root)).settings;
        let custom = discover_extensions(self.paths(), project_root, settings.compat.claude).agents;
        let work_plans = settings.feature(FeatureId::WorkPlans).acts();
        let offered = BUILTIN.iter().filter(|(name, _)| offered(name, work_plans));
        let builtin = offered.filter_map(|(name, markdown)| {
            let source = ExtensionSource::new(ExtensionScope::User, Path::new(name));
            parse_agent(markdown, source)
                .inspect_err(|error| tracing::warn!(%name, %error, "built-in agent skipped"))
                .ok()
                .map(|def| card(def, "builtin"))
        });
        let mut cards: Vec<AgentCard> = builtin
            .filter(|card| custom.iter().all(|def| def.name != card.name))
            .collect();
        cards.extend(custom.into_iter().map(|def| {
            let source = match def.source.scope {
                ExtensionScope::ClaudeUser | ExtensionScope::User => "user",
                ExtensionScope::ClaudeProject | ExtensionScope::Project => "project",
            };
            card(def, source)
        }));
        cards.sort_by(|a, b| a.name.cmp(&b.name));
        cards
    }

    /// Slash commands for `project_root`: engine built-ins, custom and
    /// built-in prompt commands, the MCP prompts of a live session of the
    /// project, `/replan` while `work_plans` is On, then GUI commands.
    pub fn slash_commands(&self, project_root: &Path) -> Vec<SlashCommandInfo> {
        let settings = self.settings(Some(project_root)).settings;
        let custom =
            discover_extensions(self.paths(), project_root, settings.compat.claude).commands;
        let prompts = self.live_mcp_prompts(project_root);
        let work = settings.feature(FeatureId::WorkPlans).acts();
        listed_commands(&custom, &prompts, work)
            .into_iter()
            .map(info)
            .collect()
    }

    fn live_mcp_prompts(&self, project_root: &Path) -> Vec<(String, McpPromptInfo)> {
        let root = std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.into());
        self.handles()
            .iter()
            .find(|handle| handle.core.root == root)
            .map(|handle| handle.core.mcp.prompts().to_vec())
            .unwrap_or_default()
    }
}

fn info(entry: CommandEntry) -> SlashCommandInfo {
    let kind = match entry.target {
        Target::Engine => SlashKind::Engine,
        Target::Template(_) | Target::Mcp { .. } => SlashKind::Prompt,
        Target::Ui => SlashKind::Ui,
    };
    SlashCommandInfo {
        name: entry.name,
        description: entry.description,
        argument_hint: entry.argument_hint,
        source: entry.origin.label().to_string(),
        kind,
    }
}

fn card(def: AgentDef, source: &str) -> AgentCard {
    AgentCard {
        name: def.name,
        description: def.description,
        source: source.to_string(),
        model: def.model,
        color: def.color,
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::engine;
    use super::*;

    #[test]
    fn builtin_agents_are_listed_and_custom_ones_override_them() {
        let dir = tempfile::tempdir().unwrap();
        let engine = engine(dir.path());
        let root = dir.path().join("project");
        let agents = root.join(".hyperform").join("agents");
        std::fs::create_dir_all(&agents).unwrap();
        std::fs::write(
            agents.join("explore.md"),
            "---\ndescription: Local explorer\ncolor: teal\nmodel: fast\n---\nLook around.\n",
        )
        .unwrap();
        let cards = engine.agent_cards(&root);
        let names: Vec<&str> = cards.iter().map(|card| card.name.as_str()).collect();
        assert_eq!(names, ["explore", "general", "plan", "review", "verify"]);
        let explore = &cards[0];
        assert_eq!(explore.source, "project");
        assert_eq!(explore.description, "Local explorer");
        assert_eq!(explore.color.as_deref(), Some("teal"));
        assert!(cards[1..].iter().all(|card| card.source == "builtin"));
        let json = serde_json::to_value(explore).unwrap();
        assert!(json.get("description").is_some() && json.get("model").is_some());
    }

    #[test]
    fn slash_commands_list_every_source() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        let commands_dir = root.join(".hyperform").join("commands");
        std::fs::create_dir_all(&commands_dir).unwrap();
        std::fs::write(
            commands_dir.join("ship.md"),
            "---\ndescription: Ship it\nargument-hint: <version>\n---\nShip $1.\n",
        )
        .unwrap();
        let commands = engine(dir.path()).slash_commands(&root);
        let names = |kind: SlashKind| -> Vec<&str> {
            commands
                .iter()
                .filter(|command| command.kind == kind)
                .map(|command| command.name.as_str())
                .collect()
        };
        assert_eq!(
            names(SlashKind::Engine),
            [
                "compact", "cost", "status", "remember", "model", "mode", "effort", "mcp", "todos",
                "doctor", "add-dir"
            ]
        );
        assert_eq!(
            names(SlashKind::Prompt),
            ["ship", "init", "review", "security-review", "commit"]
        );
        assert!(names(SlashKind::Ui).contains(&"context"));
        let ship = commands.iter().find(|c| c.name == "ship").unwrap();
        assert_eq!(ship.source, "project");
        let json = serde_json::to_value(ship).unwrap();
        assert_eq!(json["argumentHint"], "<version>");
        assert_eq!(json["kind"], "prompt");
        let compact = serde_json::to_value(&commands[0]).unwrap();
        assert_eq!(compact["kind"], "engine");
        assert_eq!(compact["source"], "builtin");
    }
}
