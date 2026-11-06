//! Fase 6: volúmenes y directorios vinculados.

use dcc_engine::binds::BindSummary;
use dcc_engine::spec::SpecIssue;
use dcc_engine::volumes::{
    VolumeDetails, VolumePruneResult, VolumeSize, VolumeSpec, VolumeSummary,
};
use dcc_engine::EngineError;
use serde::Serialize;
use tauri::State;
use ts_rs::TS;

use crate::error::CmdResult;
use crate::state::AppState;

#[tauri::command]
pub async fn volumes_list(state: State<'_, AppState>) -> CmdResult<Vec<VolumeSummary>> {
    Ok(state
        .engine
        .with_engine(|e| async move { e.list_volumes(None).await })
        .await?)
}

#[tauri::command]
pub async fn volume_details(state: State<'_, AppState>, name: String) -> CmdResult<VolumeDetails> {
    Ok(state
        .engine
        .with_engine(|e| async move { e.volume_details(&name).await })
        .await?)
}

/// Tamaños (`docker system df --verbose`): bajo demanda, puede tardar.
#[tauri::command]
pub async fn volume_sizes(state: State<'_, AppState>) -> CmdResult<Vec<VolumeSize>> {
    Ok(state
        .engine
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
    spec: VolumeSpec,
) -> CmdResult<VolumeSummary> {
    let issues = spec.validate();
    if VolumeSpec::has_errors(&issues) {
        return Err(EngineError::InvalidSpec { issues }.into());
    }
    Ok(state
        .engine
        .with_engine(|e| async move { e.create_volume(&spec).await })
        .await?)
}

#[tauri::command]
pub async fn volume_remove(
    state: State<'_, AppState>,
    name: String,
    confirmed: bool,
) -> CmdResult<()> {
    if !confirmed {
        return Err(EngineError::NotConfirmed.into());
    }
    Ok(state
        .engine
        .with_engine(|e| async move { e.remove_volume(&name, true).await })
        .await?)
}

#[tauri::command]
pub async fn volumes_prune(
    state: State<'_, AppState>,
    all: bool,
    confirmed: bool,
) -> CmdResult<VolumePruneResult> {
    if !confirmed {
        return Err(EngineError::NotConfirmed.into());
    }
    Ok(state
        .engine
        .with_engine(|e| async move { e.prune_volumes(all, None, true).await })
        .await?)
}

#[tauri::command]
pub async fn binds_list(state: State<'_, AppState>) -> CmdResult<Vec<BindSummary>> {
    Ok(state
        .engine
        .with_engine(|e| async move { e.list_binds(None).await })
        .await?)
}
