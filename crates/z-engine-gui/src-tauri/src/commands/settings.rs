//! Settings commands (`lib/commands/settings.ts`): the effective settings,
//! one layer's table, writes to the user, project or local file (every
//! write reloads the sessions it affects), the feature registry, and the
//! MCP and decision-model connection tests, and the native decision model's
//! download.

use std::path::{Path, PathBuf};

use serde_json::Value;
use tauri::State;
use z_engine_config::{
    ConfigError, FEATURES, FeatureSpec, HookConfig, LoadedSettings, McpServerConfig, RuleKind,
    writer,
};
use z_engine_engine::{DecisionModelStatus, DecisionModelTest, McpTestReport};

use crate::ipc::{IpcResult, fail};
use crate::layers::{LayerFile, Scope, layer_file, read_layer, toml_value};
use crate::state::AppState;

/// Effective settings with every layer; no project loads the user level.
#[tauri::command]
pub(crate) fn get_settings(
    project_root: Option<String>,
    state: State<'_, AppState>,
) -> LoadedSettings {
    let root = project_root.map(PathBuf::from);
    state.engine.settings(root.as_deref())
}

#[tauri::command]
pub(crate) fn get_layer(
    scope: Scope,
    project_root: Option<String>,
    state: State<'_, AppState>,
) -> IpcResult<LayerFile> {
    let root = project_root.map(PathBuf::from);
    read_layer(state.engine.paths(), scope, root.as_deref())
}

#[tauri::command]
pub(crate) fn set_setting(
    scope: Scope,
    project_root: Option<String>,
    key_path: Vec<String>,
    value: Value,
    state: State<'_, AppState>,
) -> IpcResult<()> {
    let value = toml_value(value)?;
    write(&state, scope, project_root, |file| {
        writer::set_value(file, &keys(&key_path), value)
    })
}

#[tauri::command]
pub(crate) fn remove_setting(
    scope: Scope,
    project_root: Option<String>,
    key_path: Vec<String>,
    state: State<'_, AppState>,
) -> IpcResult<()> {
    write(&state, scope, project_root, |file| {
        writer::remove_value(file, &keys(&key_path))
    })
}

#[tauri::command]
pub(crate) fn add_permission_rule(
    scope: Scope,
    project_root: Option<String>,
    kind: RuleKind,
    rule: String,
    state: State<'_, AppState>,
) -> IpcResult<()> {
    write(&state, scope, project_root, |file| {
        writer::add_permission_rule(file, kind, &rule)
    })
}

#[tauri::command]
pub(crate) fn remove_permission_rule(
    scope: Scope,
    project_root: Option<String>,
    kind: RuleKind,
    rule: String,
    state: State<'_, AppState>,
) -> IpcResult<()> {
    write(&state, scope, project_root, |file| {
        writer::remove_permission_rule(file, kind, &rule)
    })
}

#[tauri::command]
pub(crate) fn set_mcp_server(
    scope: Scope,
    project_root: Option<String>,
    name: String,
    server: McpServerConfig,
    state: State<'_, AppState>,
) -> IpcResult<()> {
    write(&state, scope, project_root, |file| {
        writer::set_mcp_server(file, &name, &server)
    })
}

#[tauri::command]
pub(crate) fn remove_mcp_server(
    scope: Scope,
    project_root: Option<String>,
    name: String,
    state: State<'_, AppState>,
) -> IpcResult<()> {
    write(&state, scope, project_root, |file| {
        writer::remove_mcp_server(file, &name)
    })
}

/// Replaces the layer's hooks for `event`; an empty list removes them.
#[tauri::command]
pub(crate) fn set_hooks(
    scope: Scope,
    project_root: Option<String>,
    event: String,
    hooks: Vec<HookConfig>,
    state: State<'_, AppState>,
) -> IpcResult<()> {
    write(&state, scope, project_root, |file| {
        writer::set_hooks(file, &event, &hooks)
    })
}

/// Connects, lists and disconnects within the server's timeout.
#[tauri::command]
pub(crate) async fn test_mcp_server(
    server: McpServerConfig,
    project_root: Option<String>,
    state: State<'_, AppState>,
) -> IpcResult<McpTestReport> {
    let root = project_root.map(PathBuf::from);
    Ok(state
        .engine
        .test_mcp_server("test", &server, root.as_deref())
        .await)
}

/// Every registered feature, for the Experimental tab.
#[tauri::command]
pub(crate) fn feature_catalog() -> Vec<FeatureSpec> {
    FEATURES.to_vec()
}

/// Asks the configured decision model one known question.
#[tauri::command]
pub(crate) async fn test_decision_model(
    project_root: Option<String>,
    state: State<'_, AppState>,
) -> IpcResult<DecisionModelTest> {
    let root = project_root.map(PathBuf::from);
    Ok(state.engine.test_decision_model(root.as_deref()).await)
}

/// The native decision model for the effective checkpoint: on disk, downloading or missing.
#[tauri::command]
pub(crate) async fn decision_model_status(
    project_root: Option<String>,
    state: State<'_, AppState>,
) -> IpcResult<DecisionModelStatus> {
    let root = project_root.map(PathBuf::from);
    Ok(state.engine.decision_model_status(root.as_deref()).await)
}

/// Starts or resumes the model download; the UI polls the status for progress.
#[tauri::command]
pub(crate) async fn download_decision_model(
    project_root: Option<String>,
    state: State<'_, AppState>,
) -> IpcResult<DecisionModelStatus> {
    let root = project_root.map(PathBuf::from);
    let engine = &state.engine;
    engine
        .download_decision_model(root.as_deref())
        .await
        .map_err(fail)
}

#[tauri::command]
pub(crate) fn cancel_decision_model_download(state: State<'_, AppState>) {
    state.engine.cancel_decision_model_download();
}

#[tauri::command]
pub(crate) async fn remove_decision_model(
    project_root: Option<String>,
    state: State<'_, AppState>,
) -> IpcResult<DecisionModelStatus> {
    let root = project_root.map(PathBuf::from);
    let engine = &state.engine;
    engine
        .remove_decision_model(root.as_deref())
        .await
        .map_err(fail)
}

fn keys(key_path: &[String]) -> Vec<&str> {
    key_path.iter().map(String::as_str).collect()
}

/// Applies `change` to the scope's file, then reloads: every session for
/// the user file, that project's sessions otherwise.
fn write(
    state: &AppState,
    scope: Scope,
    project_root: Option<String>,
    change: impl FnOnce(&Path) -> Result<(), ConfigError>,
) -> IpcResult<()> {
    let root = project_root.map(PathBuf::from);
    let file = layer_file(state.engine.paths(), scope, root.as_deref())?;
    change(&file).map_err(fail)?;
    state
        .engine
        .reload_settings(reload_root(scope, root.as_deref()));
    Ok(())
}

fn reload_root(scope: Scope, root: Option<&Path>) -> Option<&Path> {
    match scope {
        Scope::User => None,
        Scope::Project | Scope::Local => root,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_writes_reload_every_session_and_project_writes_one_project() {
        let root = Path::new("/p");
        assert_eq!(reload_root(Scope::User, Some(root)), None);
        assert_eq!(reload_root(Scope::Project, Some(root)), Some(root));
        assert_eq!(reload_root(Scope::Local, Some(root)), Some(root));
        assert_eq!(keys(&["model".into(), "main".into()]), ["model", "main"]);
    }
}
