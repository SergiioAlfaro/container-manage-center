// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Contenedores e imágenes.

use cmc_engine::{ContainerAction, ContainerState, ContainerSummary, EngineError, ImageSummary};
use tauri::State;

use crate::error::CmdResult;
use crate::state::AppState;

#[tauri::command]
pub async fn containers_list(
    state: State<'_, AppState>,
    server: String,
) -> CmdResult<Vec<ContainerSummary>> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move { e.list_containers().await })
        .await?)
}

/// Iniciar, detener, reiniciar, pausar o reanudar. `name` solo se usa para el registro.
/// Devuelve el estado confirmado por inspect tras la acción (D-031).
#[tauri::command]
pub async fn container_action(
    state: State<'_, AppState>,
    server: String,
    id: String,
    name: Option<String>,
    action: ContainerAction,
) -> CmdResult<ContainerState> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move { e.container_action(&id, name.as_deref(), action).await })
        .await?)
}

/// Eliminación: operación destructiva. Además del diálogo de la interfaz, el
/// backend exige `confirmed = true` (defensa en profundidad).
#[tauri::command]
pub async fn container_remove(
    state: State<'_, AppState>,
    server: String,
    id: String,
    name: Option<String>,
    force: bool,
    volumes: bool,
    confirmed: bool,
) -> CmdResult<()> {
    let mgr = state.server(&server)?;
    if !confirmed {
        return Err(EngineError::NotConfirmed.into());
    }
    Ok(mgr
        .with_engine(|e| async move {
            e.remove_container(&id, name.as_deref(), force, volumes)
                .await
        })
        .await?)
}

#[tauri::command]
pub async fn images_list(
    state: State<'_, AppState>,
    server: String,
) -> CmdResult<Vec<ImageSummary>> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move { e.list_images(None).await })
        .await?)
}
