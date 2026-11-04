use dcc_activity::OperationRecord;
use tauri::State;

use crate::state::AppState;

/// Registros actuales, del más reciente al más antiguo.
#[tauri::command]
pub fn activity_list(state: State<'_, AppState>) -> Vec<OperationRecord> {
    state.activity.snapshot()
}

/// Vacía el historial visual (las operaciones en curso se conservan).
#[tauri::command]
pub fn activity_clear(state: State<'_, AppState>) {
    state.activity.clear();
}
