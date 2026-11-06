//! Catalogs the composer and pickers read: slash commands, agents, `@file`
//! search, and the model catalog.

use serde_json::Value;
use tauri::State;
use z_engine_engine::{AgentCard, SlashCommandInfo};

use crate::ipc::{IpcResult, fail, json};
use crate::state::AppState;

/// Engine built-ins, prompt commands (built-in, custom, MCP) and GUI commands.
#[tauri::command]
pub(crate) fn list_commands(
    project_root: String,
    state: State<'_, AppState>,
) -> IpcResult<Vec<SlashCommandInfo>> {
    let root = AppState::root_arg(&project_root)?;
    Ok(state.engine.slash_commands(&root))
}

#[tauri::command]
pub(crate) fn list_agents(
    project_root: String,
    state: State<'_, AppState>,
) -> IpcResult<Vec<AgentCard>> {
    let root = AppState::root_arg(&project_root)?;
    Ok(state.engine.agent_cards(&root))
}

#[tauri::command]
pub(crate) async fn list_files(
    project_root: String,
    query: String,
    limit: usize,
    state: State<'_, AppState>,
) -> IpcResult<Vec<String>> {
    let root = AppState::root_arg(&project_root)?;
    state
        .engine
        .list_files(&root, &query, limit)
        .await
        .map_err(fail)
}

/// `ModelInfo[]`: the cached catalog, downloaded when there is none yet. A
/// day-old download is refreshed in the background; the next call sees it.
#[tauri::command]
pub(crate) async fn fetch_model_catalog(state: State<'_, AppState>) -> IpcResult<Value> {
    let catalog = match state.engine.catalog() {
        Some(catalog) => {
            if state.engine.catalog_is_stale() {
                let engine = state.engine.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = engine.refresh_catalog().await {
                        tracing::warn!(%error, "model catalog refresh failed; keeping the cache");
                    }
                });
            }
            catalog
        }
        None => state.engine.refresh_catalog().await.map_err(fail)?,
    };
    json(&catalog.models)
}
