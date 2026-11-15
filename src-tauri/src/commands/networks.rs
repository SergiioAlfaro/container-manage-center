// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Fase 5: redes. Listado, detalle, creación (con vista previa), eliminación,
//! limpieza y conexión o desconexión de contenedores.

use cmc_engine::networks::{
    ConnectOptions, CreatedNetwork, NetworkDetails, NetworkSpec, NetworkSummary,
};
use cmc_engine::spec::SpecIssue;
use cmc_engine::EngineError;
use serde::Serialize;
use tauri::State;
use ts_rs::TS;

use crate::error::CmdResult;
use crate::state::AppState;

#[tauri::command]
pub async fn networks_list(
    state: State<'_, AppState>,
    server: String,
) -> CmdResult<Vec<NetworkSummary>> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move { e.list_networks(None).await })
        .await?)
}

#[tauri::command]
pub async fn network_details(
    state: State<'_, AppState>,
    server: String,
    id: String,
    name: Option<String>,
) -> CmdResult<NetworkDetails> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move { e.network_details(&id, name.as_deref()).await })
        .await?)
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NetworkPreview {
    pub issues: Vec<SpecIssue>,
    /// `docker network create …` equivalente.
    pub command: String,
    /// `POST /networks/create`.
    pub request: String,
    /// Cuerpo JSON exacto que se enviará (con `CheckDuplicate`, D-048).
    pub body: String,
}

/// Valida la red y muestra lo que se enviará. No habla con Docker.
#[tauri::command]
pub fn network_preview(spec: NetworkSpec) -> NetworkPreview {
    NetworkPreview {
        issues: spec.validate(),
        command: spec.command(),
        request: "POST /networks/create".into(),
        body: serde_json::to_string_pretty(&spec.wire_body()).unwrap_or_default(),
    }
}

#[tauri::command]
pub async fn network_create(
    state: State<'_, AppState>,
    server: String,
    spec: NetworkSpec,
) -> CmdResult<CreatedNetwork> {
    let mgr = state.server(&server)?;
    // La validación no necesita conexión: los errores se devuelven antes.
    let issues = spec.validate();
    if NetworkSpec::has_errors(&issues) {
        return Err(EngineError::InvalidSpec { issues }.into());
    }
    Ok(mgr
        .with_engine(|e| async move { e.create_network(&spec).await })
        .await?)
}

/// Eliminación: el backend exige `confirmed = true` además del diálogo.
#[tauri::command]
pub async fn network_remove(
    state: State<'_, AppState>,
    server: String,
    id: String,
    name: Option<String>,
    confirmed: bool,
) -> CmdResult<()> {
    let mgr = state.server(&server)?;
    if !confirmed {
        return Err(EngineError::NotConfirmed.into());
    }
    Ok(mgr
        .with_engine(|e| async move { e.remove_network(&id, name.as_deref(), true).await })
        .await?)
}

#[tauri::command]
pub async fn networks_prune(
    state: State<'_, AppState>,
    server: String,
    confirmed: bool,
) -> CmdResult<Vec<String>> {
    let mgr = state.server(&server)?;
    if !confirmed {
        return Err(EngineError::NotConfirmed.into());
    }
    Ok(mgr
        .with_engine(|e| async move { e.prune_networks(None, true).await })
        .await?)
}

/// `subnets`: subredes de la red de destino, para validar una IP fija antes
/// de enviarla (Docker vuelve a validarla).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn network_connect(
    state: State<'_, AppState>,
    server: String,
    network: String,
    network_name: Option<String>,
    container: String,
    container_name: Option<String>,
    options: ConnectOptions,
    subnets: Vec<String>,
) -> CmdResult<()> {
    let mgr = state.server(&server)?;
    options.validate(&subnets)?;
    Ok(mgr
        .with_engine(|e| async move {
            e.connect_network(
                &network,
                network_name.as_deref(),
                &container,
                container_name.as_deref(),
                &options,
                &subnets,
            )
            .await
        })
        .await?)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn network_disconnect(
    state: State<'_, AppState>,
    server: String,
    network: String,
    network_name: Option<String>,
    container: String,
    container_name: Option<String>,
    force: bool,
    confirmed: bool,
) -> CmdResult<()> {
    let mgr = state.server(&server)?;
    if !confirmed {
        return Err(EngineError::NotConfirmed.into());
    }
    Ok(mgr
        .with_engine(|e| async move {
            e.disconnect_network(
                &network,
                network_name.as_deref(),
                &container,
                container_name.as_deref(),
                force,
                true,
            )
            .await
        })
        .await?)
}
