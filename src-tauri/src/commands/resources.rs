//! Contenedores e imágenes.

use dcc_engine::{ContainerAction, ContainerSummary, EngineError, ImageSummary};
use tauri::State;

use crate::error::CmdResult;
use crate::state::AppState;

#[tauri::command]
pub async fn containers_list(state: State<'_, AppState>) -> CmdResult<Vec<ContainerSummary>> {
    Ok(state
        .engine
        .with_engine(|e| async move { e.list_containers().await })
        .await?)
}

/// Iniciar, detener, reiniciar, pausar o reanudar. `name` solo se usa para el registro.
#[tauri::command]
pub async fn container_action(
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
    action: ContainerAction,
) -> CmdResult<()> {
    Ok(state
        .engine
        .with_engine(|e| async move { e.container_action(&id, name.as_deref(), action).await })
        .await?)
}

/// Eliminación: operación destructiva. Además del diálogo de la interfaz, el
/// backend exige `confirmed = true` (defensa en profundidad).
#[tauri::command]
pub async fn container_remove(
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
    force: bool,
    volumes: bool,
    confirmed: bool,
) -> CmdResult<()> {
    if !confirmed {
        return Err(EngineError::NotConfirmed.into());
    }
    Ok(state
        .engine
        .with_engine(|e| async move {
            e.remove_container(&id, name.as_deref(), force, volumes)
                .await
        })
        .await?)
}

#[tauri::command]
pub async fn images_list(state: State<'_, AppState>) -> CmdResult<Vec<ImageSummary>> {
    Ok(state
        .engine
        .with_engine(|e| async move { e.list_images(None).await })
        .await?)
}
