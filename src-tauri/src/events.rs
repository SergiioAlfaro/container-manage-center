// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Puentes entre el backend y la ventana: reenvían cambios como eventos Tauri.

use cmc_activity::ActivityLog;
use cmc_engine::{ConnectionState, EngineManager};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::sync::broadcast::error::RecvError;
use tokio_util::sync::CancellationToken;

pub const ACTIVITY_EVENT: &str = "activity://event";
/// Se emite si el puente ha perdido eventos; la interfaz debe recargar la lista.
pub const ACTIVITY_RESYNC: &str = "activity://resync";
/// Nuevo estado de la conexión con Docker del equipo local (`ConnectionState`).
pub const ENGINE_STATE: &str = "engine://state";
/// Nuevo estado de la conexión de cualquier servidor (`ServerStateEvent`, 0.10.0).
pub const SERVER_STATE: &str = "server://state";

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ServerStateEvent {
    pub server: String,
    pub state: ConnectionState,
}

pub fn spawn_activity_bridge<R: Runtime>(app: AppHandle<R>, log: ActivityLog) {
    let mut rx = log.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if let Err(e) = app.emit(ACTIVITY_EVENT, &event) {
                        tracing::warn!(error = %e, "no se pudo emitir evento de actividad");
                    }
                }
                Err(RecvError::Lagged(n)) => {
                    tracing::debug!(lost = n, "puente de actividad retrasado; resincronizando");
                    let _ = app.emit(ACTIVITY_RESYNC, ());
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
}

/// Conecta con Docker al arrancar, lanza el monitor y reenvía cada cambio de estado.
pub fn spawn_engine<R: Runtime>(app: AppHandle<R>, engine: EngineManager) {
    let mut rx = engine.subscribe();
    let a = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let state = rx.borrow_and_update().clone();
            if let Err(e) = a.emit(ENGINE_STATE, &state) {
                tracing::warn!(error = %e, "no se pudo emitir el estado de Docker");
            }
            if rx.changed().await.is_err() {
                break;
            }
        }
    });
    let never = CancellationToken::new();
    spawn_state_bridge(
        app.clone(),
        crate::servers::LOCAL.into(),
        engine.clone(),
        never.clone(),
    );
    crate::journal::spawn_events_watcher(app, engine.clone(), never);
    let e = engine.clone();
    tauri::async_runtime::spawn(async move {
        e.connect().await;
        e.run_monitor().await;
    });
}

/// Reenvía a la ventana los cambios de estado de un servidor y anota su
/// última conexión correcta.
fn spawn_state_bridge<R: Runtime>(
    app: AppHandle<R>,
    server: String,
    m: EngineManager,
    cancel: CancellationToken,
) {
    let mut rx = m.subscribe();
    tauri::async_runtime::spawn(async move {
        let run = async {
            loop {
                let state = rx.borrow_and_update().clone();
                if m.is_remote() && matches!(state, ConnectionState::Connected { .. }) {
                    if let Some(s) = app.try_state::<crate::state::AppState>() {
                        s.servers.mark_seen(&server);
                    }
                }
                let ev = ServerStateEvent {
                    server: server.clone(),
                    state,
                };
                if let Err(e) = app.emit(SERVER_STATE, &ev) {
                    tracing::warn!(error = %e, "no se pudo emitir el estado de un servidor");
                }
                if rx.changed().await.is_err() {
                    break;
                }
            }
        };
        tokio::select! {
            _ = run => {}
            _ = cancel.cancelled() => {}
        }
    });
}

/// Tareas de un servidor remoto: estado, eventos de Docker y monitor de la
/// conexión. Terminan al cancelar `cancel` (servidor eliminado o sustituido).
/// No conecta: un servidor remoto solo se conecta cuando se pide.
pub fn spawn_server_tasks<R: Runtime>(
    app: AppHandle<R>,
    server: String,
    m: EngineManager,
    cancel: CancellationToken,
) {
    spawn_state_bridge(app.clone(), server, m.clone(), cancel.clone());
    crate::journal::spawn_events_watcher(app, m.clone(), cancel.clone());
    tauri::async_runtime::spawn(async move {
        tokio::select! {
            _ = m.run_monitor() => {}
            _ = cancel.cancelled() => {}
        }
    });
}
