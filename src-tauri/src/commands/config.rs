use dcc_store::AppConfig;
use tauri::{AppHandle, Manager, Runtime, State};

use crate::error::{CmdResult, CommandError};
use crate::state::AppState;

#[tauri::command]
pub fn config_get(state: State<'_, AppState>) -> AppConfig {
    state.config()
}

/// Guarda la configuración completa. La E/S se hace fuera del hilo principal.
#[tauri::command]
pub async fn config_set<R: Runtime>(app: AppHandle<R>, config: AppConfig) -> CmdResult<AppConfig> {
    tauri::async_runtime::spawn_blocking(move || app.state::<AppState>().replace_config(config))
        .await
        .map_err(|e| CommandError::new("internal", e))?
}
