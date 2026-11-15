// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Conexión con Docker Engine e información del sistema.

use cmc_engine::{
    ComposeStatus, ConnectionState, Diagnosis, Endpoint, EngineSummary, SystemOverview,
};
use cmc_store::DockerConfig;
use serde::Serialize;
use tauri::State;
use ts_rs::TS;

use crate::error::CmdResult;
use crate::state::{connection_mode, AppState};

#[tauri::command]
pub fn engine_state(state: State<'_, AppState>, server: String) -> CmdResult<ConnectionState> {
    Ok(state.server(&server)?.state())
}

/// Vuelve a detectar y conectar (acción del usuario: queda registrada).
#[tauri::command]
pub async fn engine_reconnect(
    state: State<'_, AppState>,
    server: String,
) -> CmdResult<ConnectionState> {
    Ok(state.server(&server)?.connect().await)
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(
    tag = "result",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ConnectionTest {
    Ok {
        endpoint: Endpoint,
        engine: EngineSummary,
    },
    Failed {
        diagnosis: Box<Diagnosis>,
    },
}

/// Prueba una configuración de conexión local sin guardarla ni sustituir la actual.
#[tauri::command]
pub async fn engine_test(
    state: State<'_, AppState>,
    docker: DockerConfig,
) -> CmdResult<ConnectionTest> {
    let docker = docker_normalized(docker);
    Ok(
        match state.servers.local.test(connection_mode(&docker)).await {
            Ok((endpoint, engine)) => ConnectionTest::Ok { endpoint, engine },
            Err(diagnosis) => ConnectionTest::Failed { diagnosis },
        },
    )
}

fn docker_normalized(d: DockerConfig) -> DockerConfig {
    cmc_store::AppConfig {
        docker: d,
        ..Default::default()
    }
    .normalized()
    .docker
}

#[tauri::command]
pub async fn engine_overview(
    state: State<'_, AppState>,
    server: String,
) -> CmdResult<SystemOverview> {
    Ok(state.server(&server)?.overview().await?)
}

#[tauri::command]
pub async fn compose_status(
    state: State<'_, AppState>,
    server: String,
) -> CmdResult<ComposeStatus> {
    Ok(state.server(&server)?.compose_status().await)
}
