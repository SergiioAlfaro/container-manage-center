// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Fase 8: eventos de Docker e historial de operaciones.

use cmc_engine::events::{EventQuery, EventQueryResult};
use tauri::State;

use crate::error::{CmdResult, CommandError};
use crate::journal::{page_events, EventFilter, EventPage, HistoryFilter, HistoryPage};
use crate::state::AppState;

/// Eventos recibidos en esta sesión (en memoria, los últimos 2000).
#[tauri::command]
pub fn events_recent(state: State<'_, AppState>, filter: EventFilter) -> EventPage {
    let all: Vec<_> = state.journals.recent.lock().iter().cloned().collect();
    let n = all.len();
    page_events(all.into_iter(), &filter, n, false)
}

/// Eventos guardados por la aplicación (también de sesiones anteriores).
#[tauri::command]
pub fn events_journal(state: State<'_, AppState>, filter: EventFilter) -> EventPage {
    let persistent = state.config().activity.keep_history && state.journals.events.is_some();
    match &state.journals.events {
        Some(j) => {
            let all = j.entries();
            let n = all.len();
            page_events(all.into_iter(), &filter, n, persistent)
        }
        None => page_events(std::iter::empty(), &filter, 0, false),
    }
}

/// Consulta a Docker de los eventos que conserva (los últimos 256).
#[tauri::command]
pub async fn events_query(
    state: State<'_, AppState>,
    server: String,
    query: EventQuery,
) -> CmdResult<EventQueryResult> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move { e.events_query(&query).await })
        .await?)
}

#[tauri::command]
pub fn events_journal_clear(state: State<'_, AppState>) -> CmdResult<()> {
    state.journals.recent.lock().clear();
    if let Some(j) = &state.journals.events {
        j.clear().map_err(|e| CommandError::new("history.io", e))?;
    }
    Ok(())
}

/// Historial de operaciones guardado (también de sesiones anteriores).
#[tauri::command]
pub fn history_list(state: State<'_, AppState>, filter: HistoryFilter) -> HistoryPage {
    let j = &state.journals;
    let all = j.history.as_ref().map(|h| h.entries()).unwrap_or_default();
    let limit = if filter.limit == 0 {
        1000
    } else {
        filter.limit.min(5000)
    } as usize;
    let mut total = 0u32;
    let mut entries = Vec::new();
    for e in all.iter().rev() {
        if filter.matches(e) {
            total += 1;
            if entries.len() < limit {
                entries.push(e.clone());
            }
        }
    }
    HistoryPage {
        entries,
        total,
        stored: all.len() as u32,
        persistent: state.config().activity.keep_history && j.history.is_some(),
        session_start: j.session_start,
        location: j.dir.as_ref().map(|d| d.display().to_string()),
    }
}

/// Borra el historial guardado (no afecta al panel de actividad de la sesión).
#[tauri::command]
pub fn history_clear(state: State<'_, AppState>) -> CmdResult<()> {
    if let Some(j) = &state.journals.history {
        j.clear().map_err(|e| CommandError::new("history.io", e))?;
    }
    Ok(())
}
