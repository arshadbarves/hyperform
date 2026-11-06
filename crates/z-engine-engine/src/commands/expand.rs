//! Expanding a prompt command into the text of a turn: placeholders,
//! inline commands and file inclusions for templates, the rendered prompt
//! for MCP prompts, plus the turn-scoped grants and model override.

use tokio_util::sync::CancellationToken;
use z_engine_config::{CommandDef, ExtensionScope};

use super::args::substitute;
use super::catalog::Target;
use super::include::included_files;
use super::inline::run_inline_commands;
use super::mcp_prompt::render_mcp_prompt;
use crate::session::SessionCore;

/// A prompt command the user invoked.
#[derive(Debug, Clone)]
pub(crate) struct CommandCall {
    pub name: String,
    pub args: String,
    /// `Target::Template` or `Target::Mcp`.
    pub target: Target,
}

impl CommandCall {
    /// What the user typed, shown as the first block of the message.
    pub(crate) fn invocation(&self) -> String {
        let args = self.args.trim();
        if args.is_empty() {
            format!("/{}", self.name)
        } else {
            format!("/{} {args}", self.name)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Expansion {
    /// The expanded body wrapped in `<command name="...">`.
    pub body: String,
    /// `allowed-tools` rules granted for this turn only.
    pub grants: Vec<String>,
    /// The model for this turn only.
    pub model: Option<String>,
}

/// The expansion, or the user-facing reason the command cannot run.
pub(crate) async fn expand(
    core: &SessionCore,
    call: &CommandCall,
    cancel: &CancellationToken,
) -> Result<Expansion, String> {
    let (body, grants, model) = match &call.target {
        Target::Template(def) => {
            let grants = trusted_grants(core, def);
            let body = substitute(&def.body, &call.args);
            let files = included_files(core, &body).await;
            let mut body = run_inline_commands(core, &body, &grants, cancel).await;
            if !files.is_empty() {
                body = format!("{}\n\n{files}", body.trim_end());
            }
            (body, grants, def.model.clone())
        }
        Target::Mcp { server, prompt } => {
            let body = render_mcp_prompt(core, &call.name, server, prompt, &call.args).await?;
            (body, Vec::new(), None)
        }
        Target::Engine | Target::Ui => {
            return Err(format!("/{} does not prompt the model", call.name));
        }
    };
    Ok(Expansion {
        body: wrap(&call.name, &body),
        grants,
        model: model
            .map(|model| model.trim().to_string())
            .filter(|model| !model.is_empty() && model != "inherit"),
    })
}

fn wrap(name: &str, body: &str) -> String {
    format!("<command name=\"{name}\">\n{}\n</command>", body.trim())
}

/// A command shipped by an untrusted project grants nothing: a repository
/// must not approve its own inline commands or tool calls.
fn trusted_grants(core: &SessionCore, def: &CommandDef) -> Vec<String> {
    let from_project = matches!(
        def.source.scope,
        ExtensionScope::Project | ExtensionScope::ClaudeProject
    );
    if from_project && !core.settings().trusted {
        Vec::new()
    } else {
        def.allowed_tools.clone()
    }
}
