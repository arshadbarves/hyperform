//! Workspace commands (`lib/commands/workspace.ts`): the registry, the
//! sidebar's repository summary, the git-scope review panel, worktrees,
//! and opening or revealing project files. Git commands act on the given
//! project, else the active one (the last one opened or added).

use std::path::{Path, PathBuf};

use tauri::State;
use tauri_plugin_opener::OpenerExt;
use z_engine_engine::{GitChangedFile, RepoSummary};

use crate::guard::project_path;
use crate::ipc::{IpcResult, fail};
use crate::state::AppState;

#[tauri::command]
pub(crate) fn list_workspaces(state: State<'_, AppState>) -> Vec<String> {
    state
        .workspaces
        .load()
        .iter()
        .map(|root| display(root))
        .collect()
}

/// Registers an existing directory (duplicates are ignored), makes it the
/// active project, and returns the canonical path stored.
#[tauri::command]
pub(crate) fn add_workspace(path: String, state: State<'_, AppState>) -> IpcResult<String> {
    let canonical = std::fs::canonicalize(&path).map_err(|error| format!("{path}: {error}"))?;
    if !canonical.is_dir() {
        return Err(format!("{} is not a directory", canonical.display()));
    }
    state.workspaces.add(canonical.clone())?;
    state.set_active(canonical.clone());
    Ok(display(&canonical))
}

/// Unregisters a workspace and deletes its chats.
#[tauri::command]
pub(crate) async fn remove_workspace(path: String, state: State<'_, AppState>) -> IpcResult<()> {
    let target = PathBuf::from(&path);
    let canonical = std::fs::canonicalize(&target).unwrap_or_else(|_| target.clone());
    let targets = [target, canonical];
    let sessions = state.engine.list_sessions().map_err(fail)?;
    for session in sessions {
        let root = PathBuf::from(&session.project_root);
        if targets.contains(&root) {
            state
                .engine
                .delete_session(&session.session_id)
                .await
                .map_err(fail)?;
        }
    }
    state.workspaces.remove(&targets)
}

/// Branch and changed-file count of a known project; null outside git.
#[tauri::command]
pub(crate) async fn git_summary(
    root: String,
    state: State<'_, AppState>,
) -> IpcResult<Option<RepoSummary>> {
    let root = project_root(Some(root), &state)?;
    state.engine.git_summary(&root).await.map_err(fail)
}

#[tauri::command]
pub(crate) async fn list_changed_files(
    root: Option<String>,
    state: State<'_, AppState>,
) -> IpcResult<Vec<GitChangedFile>> {
    let root = project_root(root, &state)?;
    state.engine.git_changed_files(&root).await.map_err(fail)
}

#[tauri::command]
pub(crate) async fn diff_for_file(
    path: String,
    root: Option<String>,
    state: State<'_, AppState>,
) -> IpcResult<String> {
    let root = project_root(root, &state)?;
    state.engine.git_file_diff(&root, &path).await.map_err(fail)
}

/// Creates `.z-engine/worktrees/<name>` in the given (else the active)
/// project on its own branch and registers it as a workspace.
#[tauri::command]
pub(crate) async fn create_worktree(
    name: String,
    root: Option<String>,
    state: State<'_, AppState>,
) -> IpcResult<String> {
    let root = project_root(root, &state)?;
    let path = state
        .engine
        .create_worktree(&root, &name)
        .await
        .map_err(|error| format!("git worktree add failed: {error}"))?;
    state.workspaces.add(path.clone())?;
    Ok(display(&path))
}

/// Opens a file or folder inside a project with the system's default app.
#[tauri::command]
pub(crate) fn open_path(
    app: tauri::AppHandle,
    path: String,
    state: State<'_, AppState>,
) -> IpcResult<()> {
    let path = project_path(&path, &state.known_roots())?;
    app.opener()
        .open_path(display(&path), None::<&str>)
        .map_err(fail)
}

/// Shows a file or folder inside a project in Finder or Explorer.
#[tauri::command]
pub(crate) fn reveal_path(
    app: tauri::AppHandle,
    path: String,
    state: State<'_, AppState>,
) -> IpcResult<()> {
    let path = project_path(&path, &state.known_roots())?;
    app.opener().reveal_item_in_dir(&path).map_err(fail)
}

/// `root` when it is a known project, else the active project.
fn project_root(root: Option<String>, state: &AppState) -> IpcResult<PathBuf> {
    let Some(root) = root.filter(|root| !root.trim().is_empty()) else {
        return Ok(state.active_root());
    };
    let wanted = std::fs::canonicalize(&root).unwrap_or_else(|_| PathBuf::from(&root));
    state
        .known_roots()
        .into_iter()
        .find(|known| std::fs::canonicalize(known).unwrap_or_else(|_| known.clone()) == wanted)
        .ok_or_else(|| format!("{root} is not a registered project"))
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
