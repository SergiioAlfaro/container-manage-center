use dcc_store::AppConfig;
use tauri::{AppHandle, Manager, Runtime, State};

use crate::error::{CmdResult, CommandError};
use crate::state::{connection_mode, AppState};

#[tauri::command]
pub fn config_get(state: State<'_, AppState>) -> AppConfig {
    state.config()
}

/// Guarda la configuración completa. La E/S se hace fuera del hilo principal.
/// Si cambia la conexión con Docker, se reconecta en segundo plano; el nuevo
/// estado llega a la interfaz por el evento `engine://state`.
#[tauri::command]
pub async fn config_set<R: Runtime>(app: AppHandle<R>, config: AppConfig) -> CmdResult<AppConfig> {
    let handle = app.clone();
    let (saved, docker_changed) = tauri::async_runtime::spawn_blocking(move || {
        handle.state::<AppState>().replace_config(config)
    })
    .await
    .map_err(|e| CommandError::new("internal", e))??;
    if docker_changed {
        let engine = app.state::<AppState>().servers.local.clone();
        let mode = connection_mode(&saved.docker);
        tauri::async_runtime::spawn(async move {
            engine.set_mode(mode).await;
        });
    }
    Ok(saved)
}
