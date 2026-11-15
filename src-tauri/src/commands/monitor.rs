// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! E5 (1.5.0): monitorización (D-102–D-107).

use cmc_engine::monitor::{Alert, MinuteRecord};
use tauri::{AppHandle, Runtime, State};

use crate::error::{CmdResult, CommandError};
use crate::monitor::{self, MonitorLive, MonitorSettings, MonitorStatus};
use crate::state::AppState;

#[tauri::command]
pub fn monitor_settings(state: State<'_, AppState>) -> MonitorSettings {
    state.monitor.settings()
}

/// Guarda la configuración y arranca o para los muestreadores que cambien.
#[tauri::command]
pub fn monitor_settings_set<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    settings: MonitorSettings,
) -> CmdResult<MonitorSettings> {
    for s in &settings.servers {
        state.server(s)?;
    }
    let before = state.monitor.settings();
    monitor::save_settings(state.store.path(), &settings)?;
    let (start, _stop) = state.monitor.apply(settings.clone());
    for s in start {
        monitor::start(&app, &s);
    }
    let summary = |s: &MonitorSettings| {
        format!(
            "cada {} s, {} h, servidores: {}",
            s.interval_secs,
            s.retention_hours,
            if s.servers.is_empty() {
                "ninguno".into()
            } else {
                s.servers.join(", ")
            }
        )
    };
    crate::state::record_preference(
        &state.activity,
        "monitor.settings",
        "Configuración de la monitorización",
        &summary(&before),
        &summary(&settings),
    );
    Ok(settings)
}

/// Activa o desactiva la monitorización de un servidor.
#[tauri::command]
pub fn monitor_set_server<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    server: String,
    enabled: bool,
) -> CmdResult<MonitorStatus> {
    state.server(&server)?;
    let mut s = state.monitor.settings();
    s.servers.retain(|x| *x != server);
    if enabled {
        s.servers.push(server.clone());
    }
    monitor_settings_set(app, state.clone(), s)?;
    Ok(state.monitor.status(&server))
}

/// Pausa o reanuda todos los muestreadores (no se guarda: al abrir la
/// aplicación, la monitorización vuelve a estar en marcha).
#[tauri::command]
pub fn monitor_pause(state: State<'_, AppState>, paused: bool) {
    state.monitor.set_paused(paused);
}

#[tauri::command]
pub fn monitor_status(state: State<'_, AppState>, server: String) -> CmdResult<MonitorStatus> {
    state.server(&server)?;
    Ok(state.monitor.status(&server))
}

#[tauri::command]
pub fn monitor_live(
    state: State<'_, AppState>,
    server: String,
    containers: Vec<String>,
    minutes: u32,
) -> CmdResult<MonitorLive> {
    state.server(&server)?;
    Ok(state.monitor.live(&server, &containers, minutes))
}

#[tauri::command]
pub async fn monitor_minutes(
    state: State<'_, AppState>,
    server: String,
    hours: u32,
    containers: Vec<String>,
) -> CmdResult<Vec<MinuteRecord>> {
    state.server(&server)?;
    Ok(state.monitor.minutes(&server, hours, &containers))
}

#[tauri::command]
pub fn monitor_alerts(state: State<'_, AppState>, server: String) -> CmdResult<Vec<Alert>> {
    state.server(&server)?;
    Ok(state.monitor.alerts(&server))
}

/// Notificación de escritorio (texto ya traducido por la interfaz).
#[tauri::command]
pub async fn monitor_notify(summary: String, body: String) -> CmdResult<u32> {
    crate::notify::send(&summary, &body)
        .await
        .map_err(|e| CommandError::new("monitor.notifyFailed", e))
}
