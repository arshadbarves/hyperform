//! Session commands (`lib/commands/engine.ts`): open, drive, list, delete,
//! transcripts, export, the prompt inspector, the context card's
//! breakdown, the Context tab's decision trace, and the session-scope diff.

use serde_json::Value;
use tauri::State;
use z_engine_engine::ExportFormat;
use z_engine_protocol::{AgentId, Command, ContextBreakdown, Message, SessionId, SessionSummary};

use crate::ipc::{IpcResult, fail, json};
use crate::state::AppState;

/// Opens (or creates, when `session_id` is null) a session; the engine
/// then emits its snapshot. Its project becomes the active one.
#[tauri::command]
pub(crate) async fn open_session(
    project_root: String,
    session_id: Option<String>,
    state: State<'_, AppState>,
) -> IpcResult<String> {
    let root = AppState::root_arg(&project_root)?;
    let id = session_id
        .filter(|id| !id.trim().is_empty())
        .map(SessionId::from);
    let opened = state.engine.open_session(&root, id).await.map_err(fail)?;
    state.set_active(root);
    Ok(opened.to_string())
}

#[tauri::command]
pub(crate) fn send_command(
    session_id: String,
    command: Command,
    state: State<'_, AppState>,
) -> IpcResult<()> {
    state
        .engine
        .send(&SessionId::from(session_id), command)
        .map_err(fail)
}

#[tauri::command]
pub(crate) fn list_sessions(state: State<'_, AppState>) -> IpcResult<Vec<SessionSummary>> {
    state.engine.list_sessions().map_err(fail)
}

#[tauri::command]
pub(crate) async fn delete_session(
    session_id: String,
    state: State<'_, AppState>,
) -> IpcResult<()> {
    state
        .engine
        .delete_session(&SessionId::from(session_id))
        .await
        .map_err(fail)
}

#[tauri::command]
pub(crate) fn agent_transcript(
    session_id: String,
    agent_id: String,
    state: State<'_, AppState>,
) -> IpcResult<Vec<Message>> {
    state
        .engine
        .agent_transcript(&SessionId::from(session_id), &AgentId::from(agent_id))
        .map_err(fail)
}

#[tauri::command]
pub(crate) fn export_session(
    session_id: String,
    format: String,
    state: State<'_, AppState>,
) -> IpcResult<String> {
    state
        .engine
        .export_session(&SessionId::from(session_id), export_format(&format)?)
        .map_err(fail)
}

/// The last model request of a live session, or null.
#[tauri::command]
pub(crate) fn inspect_request(session_id: String, state: State<'_, AppState>) -> Option<Value> {
    state.engine.last_request(&SessionId::from(session_id))
}

/// Token estimate per prompt layer of a live session's next request, or
/// null; the view also receives it as a `contextReport` event.
#[tauri::command]
pub(crate) fn context_breakdown(
    session_id: String,
    state: State<'_, AppState>,
) -> Option<ContextBreakdown> {
    state.engine.context_breakdown(&SessionId::from(session_id))
}

/// Running decision features and the newest `limit` decisions of a live
/// session, or null.
#[tauri::command]
pub(crate) fn session_decisions(
    session_id: String,
    limit: usize,
    state: State<'_, AppState>,
) -> IpcResult<Value> {
    json(
        state
            .engine
            .session_decisions(&SessionId::from(session_id), limit),
    )
}

#[tauri::command]
pub(crate) async fn session_changed_files(
    session_id: String,
    state: State<'_, AppState>,
) -> IpcResult<Value> {
    let changes = state
        .engine
        .session_changes(&SessionId::from(session_id))
        .await
        .map_err(fail)?;
    json(changes)
}

#[tauri::command]
pub(crate) async fn session_diff_for_file(
    session_id: String,
    path: String,
    state: State<'_, AppState>,
) -> IpcResult<String> {
    state
        .engine
        .session_file_diff(&SessionId::from(session_id), &path)
        .await
        .map_err(fail)
}

fn export_format(format: &str) -> IpcResult<ExportFormat> {
    match format {
        "markdown" | "md" => Ok(ExportFormat::Markdown),
        "json" => Ok(ExportFormat::Json),
        other => Err(format!("unknown export format `{other}`")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_formats_map_from_the_webview_names() {
        assert_eq!(export_format("markdown").unwrap(), ExportFormat::Markdown);
        assert_eq!(export_format("json").unwrap(), ExportFormat::Json);
        assert!(export_format("pdf").is_err());
    }
}
