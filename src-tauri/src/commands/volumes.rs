// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Fase 6: volúmenes y directorios vinculados.

use cmc_engine::binds::BindSummary;
use cmc_engine::spec::SpecIssue;
use cmc_engine::volumes::{
    VolumeDetails, VolumePruneResult, VolumeSize, VolumeSpec, VolumeSummary,
};
use cmc_engine::EngineError;
use serde::Serialize;
use tauri::State;
use ts_rs::TS;

use crate::error::CmdResult;
use crate::state::AppState;

#[tauri::command]
pub async fn volumes_list(
    state: State<'_, AppState>,
    server: String,
) -> CmdResult<Vec<VolumeSummary>> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move { e.list_volumes(None).await })
        .await?)
}

#[tauri::command]
pub async fn volume_details(
    state: State<'_, AppState>,
    server: String,
    name: String,
) -> CmdResult<VolumeDetails> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move { e.volume_details(&name).await })
        .await?)
}

/// Tamaños (`docker system df --verbose`): bajo demanda, puede tardar.
#[tauri::command]
pub async fn volume_sizes(
    state: State<'_, AppState>,
    server: String,
) -> CmdResult<Vec<VolumeSize>> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move { e.volume_sizes().await })
        .await?)
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VolumePreview {
    pub issues: Vec<SpecIssue>,
    pub command: String,
    pub request: String,
    pub body: String,
}

/// Valida el volumen y muestra lo que se enviará. No habla con Docker.
#[tauri::command]
pub fn volume_preview(spec: VolumeSpec) -> VolumePreview {
    VolumePreview {
        issues: spec.validate(),
        command: spec.command(),
        request: "POST /volumes/create".into(),
        body: serde_json::to_string_pretty(&spec.to_create_body()).unwrap_or_default(),
    }
}

#[tauri::command]
pub async fn volume_create(
    state: State<'_, AppState>,
    server: String,
    spec: VolumeSpec,
) -> CmdResult<VolumeSummary> {
    let mgr = state.server(&server)?;
    let issues = spec.validate();
    if VolumeSpec::has_errors(&issues) {
        return Err(EngineError::InvalidSpec { issues }.into());
    }
    Ok(mgr
        .with_engine(|e| async move { e.create_volume(&spec).await })
        .await?)
}

#[tauri::command]
pub async fn volume_remove(
    state: State<'_, AppState>,
    server: String,
    name: String,
    confirmed: bool,
) -> CmdResult<()> {
    let mgr = state.server(&server)?;
    if !confirmed {
        return Err(EngineError::NotConfirmed.into());
    }
    Ok(mgr
        .with_engine(|e| async move { e.remove_volume(&name, true).await })
        .await?)
}

#[tauri::command]
pub async fn volumes_prune(
    state: State<'_, AppState>,
    server: String,
    all: bool,
    confirmed: bool,
) -> CmdResult<VolumePruneResult> {
    let mgr = state.server(&server)?;
    if !confirmed {
        return Err(EngineError::NotConfirmed.into());
    }
    Ok(mgr
        .with_engine(|e| async move { e.prune_volumes(all, None, true).await })
        .await?)
}

#[tauri::command]
pub async fn binds_list(state: State<'_, AppState>, server: String) -> CmdResult<Vec<BindSummary>> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move { e.list_binds(None).await })
        .await?)
}
