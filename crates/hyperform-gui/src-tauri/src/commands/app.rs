//! About the running app.

use serde::Serialize;
use tauri::State;

use crate::state::AppState;
use crate::window::Material;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppInfo {
    version: String,
    config_dir: String,
    data_dir: String,
    /// The window shows a native material, so the webview may be transparent.
    native_glass: bool,
}

#[tauri::command]
pub(crate) fn app_info(state: State<'_, AppState>, material: State<'_, Material>) -> AppInfo {
    let paths = state.engine.paths();
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        config_dir: paths.config_dir.to_string_lossy().into_owned(),
        data_dir: paths.data_dir.to_string_lossy().into_owned(),
        native_glass: material.translucent(),
    }
}
