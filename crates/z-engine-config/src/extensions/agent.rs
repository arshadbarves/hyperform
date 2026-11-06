//! Custom agent definitions (`agents/*.md`), in Claude Code's format.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use z_engine_protocol::{Isolation, ModeName, PermissionMode};

use super::ExtensionSource;
use super::frontmatter::{ListStyle, parse_document};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "config/")]
pub struct AgentDef {
    pub name: String,
    /// When to use the agent; shown to the model choosing a subagent.
    pub description: String,
    /// Allowed tools; `None` allows every tool.
    pub tools: Option<Vec<String>>,
    pub disallowed_tools: Vec<String>,
    /// `inherit`, an alias (`main`, `fast`, `review`), or a model id;
    /// `None` inherits.
    pub model: Option<String>,
    /// The permission level; `None` takes the caller's.
    pub permission_mode: Option<PermissionMode>,
    /// `permissionMode: plan`: the agent always runs in plan mode. A child
    /// of a planning caller plans whatever its definition says.
    pub planning: bool,
    pub isolation: Isolation,
    pub max_turns: Option<u32>,
    pub color: Option<String>,
    /// The system prompt (markdown body).
    pub prompt: String,
    pub source: ExtensionSource,
}

/// Parses an agent file. The name defaults to the file stem; `tools` may
/// be a comma-separated string or a list, and `*` means every tool. Also
/// used by the engine for built-in agents.
pub fn parse_agent(markdown: &str, source: ExtensionSource) -> Result<AgentDef, String> {
    let doc = parse_document(markdown)?;
    let meta = &doc.meta;
    let name = match meta.text(&["name"])? {
        Some(name) => name,
        None => source.stem().ok_or("missing `name`")?,
    };
    let description = meta
        .text(&["description"])?
        .ok_or("missing `description`")?;
    let tools = meta
        .list(&["tools"], ListStyle::Tools)?
        .filter(|tools| !tools.iter().any(|tool| tool == "*"));
    let disallowed_tools = meta
        .list(&["disallowedTools", "disallowed_tools"], ListStyle::Tools)?
        .unwrap_or_default();
    let mode = match meta.text(&["permissionMode", "permission_mode"])? {
        Some(mode) => {
            let parsed = ModeName::parse(&mode);
            Some(parsed.ok_or_else(|| format!("unknown permissionMode `{mode}`"))?)
        }
        None => None,
    };
    let (permission_mode, planning) = match mode {
        Some(ModeName::Level(level)) => (Some(level), false),
        Some(ModeName::Plan) => (None, true),
        None => (None, false),
    };
    let isolation = match meta.text(&["isolation"])?.as_deref() {
        None | Some("shared") => Isolation::Shared,
        Some("worktree") => Isolation::Worktree,
        Some(other) => {
            return Err(format!(
                "unknown isolation `{other}`; use shared or worktree"
            ));
        }
    };
    Ok(AgentDef {
        name,
        description,
        tools,
        disallowed_tools,
        model: meta.text(&["model"])?,
        permission_mode,
        planning,
        isolation,
        max_turns: meta.count(&["maxTurns", "max_turns"])?,
        color: meta.text(&["color"])?,
        prompt: doc.body.to_string(),
        source,
    })
}
