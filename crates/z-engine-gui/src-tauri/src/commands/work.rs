//! Work plan queries for the Plan tab: a project's plans, the packet the
//! model gets for a task of a session's attached plan, why that task
//! changed, and what may run in parallel. Plan edits travel as session
//! commands (`editWork`, `attachPlan`, `detachPlan`).

use std::path::Path;

use tauri::State;
use z_engine_protocol::SessionId;
use z_engine_protocol::work::{ParallelSet, PlanSummary, TaskPacket, WhyStep};

use crate::ipc::{IpcResult, fail};
use crate::state::AppState;

/// The project's plans, most recently updated first.
#[tauri::command]
pub(crate) fn work_plans(
    project_root: String,
    state: State<'_, AppState>,
) -> IpcResult<Vec<PlanSummary>> {
    state
        .engine
        .work_plans(Path::new(&project_root))
        .map_err(fail)
}

#[tauri::command]
pub(crate) async fn work_packet(
    session_id: String,
    task_id: String,
    state: State<'_, AppState>,
) -> IpcResult<TaskPacket> {
    state
        .engine
        .work_packet(&SessionId::from(session_id), &task_id)
        .await
        .map_err(fail)
}

#[tauri::command]
pub(crate) fn work_why(
    session_id: String,
    task_id: String,
    state: State<'_, AppState>,
) -> IpcResult<Vec<WhyStep>> {
    state
        .engine
        .work_why(&SessionId::from(session_id), &task_id)
        .map_err(fail)
}

/// The tasks of the session's attached plan that may run in parallel now
/// (a preview while the plan is a draft).
#[tauri::command]
pub(crate) fn work_parallel(
    session_id: String,
    state: State<'_, AppState>,
) -> IpcResult<ParallelSet> {
    state
        .engine
        .work_parallel(&SessionId::from(session_id))
        .map_err(fail)
}
