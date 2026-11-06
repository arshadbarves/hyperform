//! One `LSP` tool query mapped to the manager and rendered with paths
//! relative to the agent's root.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use z_engine_integrations::lsp::format::{
    format_calls, format_diagnostics, format_hover, format_locations, format_rename_plan,
    format_symbols, format_workspace_symbols,
};
use z_engine_integrations::{CallDirection, IntegrationError, LspManager};
use z_engine_tools::LspRequest;

pub(crate) async fn answer(
    manager: Arc<LspManager>,
    root: PathBuf,
    req: LspRequest,
) -> Result<String, String> {
    let root = root.as_path();
    let fail = |error: IntegrationError| error.to_string();
    let file = || {
        req.file_path
            .as_deref()
            .map(Path::new)
            .ok_or_else(|| format!("`file_path` is required for {}", req.operation))
    };
    let at = || match (req.line, req.character) {
        (Some(line), Some(character)) => Ok((line, character)),
        _ => Err(format!(
            "`line` and `character` are required for {}",
            req.operation
        )),
    };
    let text = match req.operation.as_str() {
        "definition" => {
            let (line, column) = at()?;
            let found = manager.definition(file()?, line, column).await;
            format_locations(root, &found.map_err(fail)?)
        }
        "references" => {
            let (line, column) = at()?;
            let found = manager.references(file()?, line, column).await;
            format_locations(root, &found.map_err(fail)?)
        }
        "implementations" => {
            let (line, column) = at()?;
            let found = manager.implementations(file()?, line, column).await;
            format_locations(root, &found.map_err(fail)?)
        }
        "hover" => {
            let (line, column) = at()?;
            let hover = manager.hover(file()?, line, column).await.map_err(fail)?;
            format_hover(hover.as_ref())
        }
        "documentSymbols" => {
            format_symbols(&manager.document_symbols(file()?).await.map_err(fail)?)
        }
        "workspaceSymbols" => {
            let query = req.query.as_deref().unwrap_or_default();
            let found = manager.workspace_symbols(query).await.map_err(fail)?;
            format_workspace_symbols(root, &found)
        }
        "incomingCalls" | "outgoingCalls" => {
            let (line, column) = at()?;
            let (edges, direction) = if req.operation == "incomingCalls" {
                let edges = manager.incoming_calls(file()?, line, column).await;
                (edges, CallDirection::Incoming)
            } else {
                let edges = manager.outgoing_calls(file()?, line, column).await;
                (edges, CallDirection::Outgoing)
            };
            format_calls(root, &edges.map_err(fail)?, direction)
        }
        "diagnostics" => {
            format_diagnostics(root, &manager.diagnostics(file()?).await.map_err(fail)?)
        }
        "renamePreview" => {
            let (line, column) = at()?;
            let new_name = req
                .new_name
                .as_deref()
                .ok_or("`new_name` is required for renamePreview")?;
            let plan = manager
                .rename_preview(file()?, line, column, new_name)
                .await;
            format_rename_plan(root, &plan.map_err(fail)?)
        }
        other => return Err(format!("unknown LSP operation `{other}`")),
    };
    Ok(text)
}
