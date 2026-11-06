//! `/doctor`: diagnostics of the session's setup as markdown — provider,
//! model and key, catalog, git and ripgrep, language and MCP servers,
//! hooks, workspace trust, and verification.

use z_engine_config::{Credentials, resolve_api_key};
use z_engine_host::{git_available, rg_available};
use z_engine_integrations::{LspServerState, LspServerStatus};

use super::mcp_report::state_label;
use crate::session::SessionCore;

pub(crate) async fn doctor(core: &SessionCore) -> String {
    let settings = core.settings();
    let config = &settings.settings;
    let base_url = &config.provider.base_url;
    let key = Credentials::load(&core.shared.paths.auth_file)
        .ok()
        .and_then(|credentials| resolve_api_key(&credentials, base_url))
        .is_some();
    let model = core.main_model();
    let limit = core.with_state(|state| state.context_limit);
    let catalog = match core.catalog() {
        Some(catalog) => format!("loaded ({} models)", catalog.models.len()),
        None => "not loaded; context windows use defaults".to_string(),
    };
    let hooks: usize = config.hooks.values().map(Vec::len).sum();
    let hook_events = config
        .hooks
        .values()
        .filter(|list| !list.is_empty())
        .count();
    let trust = if settings.trusted {
        "trusted".to_string()
    } else if settings.withheld.is_empty() {
        "not trusted (the project defines nothing that needs trust)".to_string()
    } else {
        format!(
            "**not trusted**; disabled until trusted: {}",
            settings.withheld.join(", ")
        )
    };
    let profile = core.checks.profile();
    let checks = if profile.checks.is_empty() {
        "none discovered".to_string()
    } else {
        let ids: Vec<String> = profile
            .checks
            .iter()
            .map(|check| format!("`{}`", check.id))
            .collect();
        ids.join(", ")
    };
    let mode = serde_json::to_value(config.verification.mode)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default();
    [
        format!(
            "- **Provider:** {} at `{base_url}`; API key {}",
            core.client().provider(),
            if key { "present" } else { "**missing**" }
        ),
        format!("- **Model:** `{model}` (context window {limit} tokens)"),
        format!("- **Model catalog:** {catalog}"),
        format!("- **git:** {}", available(git_available())),
        format!(
            "- **ripgrep:** {}",
            if rg_available() {
                "available"
            } else {
                "not found; search uses the built-in engine"
            }
        ),
        format!("- **Language servers:** {}", lsp(core).await),
        format!("- **MCP servers:** {}", mcp(core)),
        format!("- **Hooks:** {hooks} configured across {hook_events} event(s)"),
        format!("- **Workspace:** {trust}"),
        format!("- **Verification:** mode `{mode}`; checks: {checks}"),
    ]
    .join("\n")
}

fn available(found: bool) -> &'static str {
    if found { "available" } else { "**not found**" }
}

async fn lsp(core: &SessionCore) -> String {
    let names = core.lsp.server_names();
    let Some(worker) = core.lsp.worker() else {
        return "off".to_string();
    };
    let running: Vec<LspServerStatus> = worker
        .run(|manager| async move { manager.status() })
        .await
        .unwrap_or_default();
    let started: Vec<String> = running
        .iter()
        .map(|status| {
            let state = match &status.state {
                LspServerState::Starting => "starting".to_string(),
                LspServerState::Running => "running".to_string(),
                LspServerState::Failed(reason) => format!("failed: {reason}"),
            };
            format!("{} ({state})", status.name)
        })
        .collect();
    let started = if started.is_empty() {
        "none started yet (they start on the first file asked about)".to_string()
    } else {
        started.join(", ")
    };
    format!(
        "{} configured ({}); {started}",
        names.len(),
        names.join(", ")
    )
}

fn mcp(core: &SessionCore) -> String {
    let states: Vec<String> = core
        .mcp
        .managers()
        .iter()
        .flat_map(|manager| manager.status())
        .map(|status| format!("{} ({})", status.name, state_label(&status.state)))
        .collect();
    if states.is_empty() {
        "none".to_string()
    } else {
        states.join(", ")
    }
}
