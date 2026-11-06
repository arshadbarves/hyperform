//! Pet commands (`lib/commands/pet.ts`): load and save its growth.

use serde_json::Value;
use tauri::State;

use crate::ipc::IpcResult;
use crate::state::AppState;

#[tauri::command]
pub(crate) fn pet_load(state: State<'_, AppState>) -> Value {
    state.pet.load()
}

#[tauri::command]
pub(crate) fn pet_save(growth: Value, state: State<'_, AppState>) -> IpcResult<()> {
    state.pet.save(&growth)
}
