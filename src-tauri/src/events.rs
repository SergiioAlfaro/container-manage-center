//! Puentes entre el backend y la ventana: reenvían cambios como eventos Tauri.

use dcc_activity::ActivityLog;
use dcc_engine::EngineManager;
use tauri::{AppHandle, Emitter, Runtime};
use tokio::sync::broadcast::error::RecvError;

pub const ACTIVITY_EVENT: &str = "activity://event";
/// Se emite si el puente ha perdido eventos; la interfaz debe recargar la lista.
pub const ACTIVITY_RESYNC: &str = "activity://resync";
/// Nuevo estado de la conexión con Docker (`ConnectionState`).
pub const ENGINE_STATE: &str = "engine://state";

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
    tauri::async_runtime::spawn(async move {
        loop {
            let state = rx.borrow_and_update().clone();
            if let Err(e) = app.emit(ENGINE_STATE, &state) {
                tracing::warn!(error = %e, "no se pudo emitir el estado de Docker");
            }
            if rx.changed().await.is_err() {
                break;
            }
        }
    });
    let e = engine.clone();
    tauri::async_runtime::spawn(async move {
        e.connect().await;
        e.run_monitor().await;
    });
}
