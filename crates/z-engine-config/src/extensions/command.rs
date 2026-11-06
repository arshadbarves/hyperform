//! Custom slash commands (`commands/**/*.md`), in Claude Code's format.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::ExtensionSource;
use super::frontmatter::{ListStyle, parse_document};

/// Length cap of a description taken from the body.
pub const COMMAND_DESCRIPTION_LIMIT: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "config/")]
pub struct CommandDef {
    /// Path under `commands/` without `.md`, with `/` written as `:`,
    /// e.g. `frontend:component`.
    pub name: String,
    pub description: String,
    pub argument_hint: Option<String>,
    /// Tool rules granted while the command runs.
    pub allowed_tools: Vec<String>,
    pub model: Option<String>,
    /// Only the user may invoke the command, not the model.
    pub disable_model_invocation: bool,
    /// The prompt template (`$ARGUMENTS`, `$1`, ...).
    pub body: String,
    pub source: ExtensionSource,
}

/// Parses a command file. Without a `description`, the first non-empty
/// body line (heading markers removed) is used, capped at 100 characters.
pub fn parse_command(
    name: &str,
    markdown: &str,
    source: ExtensionSource,
) -> Result<CommandDef, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("missing command name".to_string());
    }
    let doc = parse_document(markdown)?;
    let meta = &doc.meta;
    let description = match meta.text(&["description"])? {
        Some(description) => description,
        None => first_line(doc.body),
    };
    Ok(CommandDef {
        name: name.to_string(),
        description,
        argument_hint: meta.hint(&["argument-hint", "argument_hint", "argumentHint"])?,
        allowed_tools: meta
            .list(
                &["allowed-tools", "allowed_tools", "allowedTools"],
                ListStyle::Tools,
            )?
            .unwrap_or_default(),
        model: meta.text(&["model"])?,
        disable_model_invocation: meta
            .flag(&[
                "disable-model-invocation",
                "disable_model_invocation",
                "disableModelInvocation",
            ])?
            .unwrap_or(false),
        body: doc.body.to_string(),
        source,
    })
}

fn first_line(body: &str) -> String {
    let line = body
        .lines()
        .map(|line| line.trim().trim_start_matches('#').trim())
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    if line.chars().count() <= COMMAND_DESCRIPTION_LIMIT {
        return line.to_string();
    }
    let mut short: String = line.chars().take(COMMAND_DESCRIPTION_LIMIT - 1).collect();
    short.push('…');
    short
}
