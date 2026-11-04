//! Puente entre el registro de actividad y la ventana: reenvía cada cambio
//! como evento Tauri para que el panel se actualice en tiempo real.

use dcc_activity::ActivityLog;
use tauri::{AppHandle, Emitter, Runtime};
use tokio::sync::broadcast::error::RecvError;

pub const ACTIVITY_EVENT: &str = "activity://event";
/// Se emite si el puente ha perdido eventos; la interfaz debe recargar la lista.
pub const ACTIVITY_RESYNC: &str = "activity://resync";

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
