//! Fase 8: historial de operaciones y diario de eventos persistentes (D-058),
//! y observador de eventos de Docker en tiempo real (D-057).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use dcc_activity::{ActivityEvent, ActivityLog, Category, HistoryEntry, OpStatus};
use dcc_engine::events::DockerEvent;
use dcc_engine::{ConnectionState, EngineManager};
use dcc_store::journal::Journal;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::sync::broadcast::error::RecvError;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::state::AppState;

pub const HISTORY_FILE: &str = "historial.jsonl";
pub const EVENTS_FILE: &str = "eventos.jsonl";
/// Operaciones conservadas en el historial persistente.
pub const HISTORY_MAX: usize = 10_000;
/// Eventos conservados en el diario persistente.
pub const EVENTS_MAX: usize = 20_000;
/// Eventos recientes en memoria para la vista en directo.
pub const RECENT_MAX: usize = 2_000;
/// Evento Tauri con cada evento de Docker.
pub const DOCKER_EVENT: &str = "docker://event";

/// Carpeta de datos de la aplicación: `DCC_STATE_DIR`, o
/// `$XDG_STATE_HOME/docker-control-center` con la configuración por defecto, o
/// junto al archivo de configuración si se indicó otro (`DCC_CONFIG`, pruebas).
pub fn state_dir(config_path: &Path) -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("DCC_STATE_DIR") {
        return Some(PathBuf::from(d));
    }
    let default_config =
        dirs::config_dir().map(|d| d.join("docker-control-center").join("config.toml"));
    if default_config.as_deref() == Some(config_path) {
        return dirs::state_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join(".local/state")))
            .map(|d| d.join("docker-control-center"));
    }
    config_path.parent().map(|p| p.join("estado"))
}

/// Diarios persistentes y eventos recientes.
pub struct Journals {
    pub history: Option<Journal<HistoryEntry>>,
    pub events: Option<Journal<DockerEvent>>,
    pub recent: Mutex<VecDeque<DockerEvent>>,
    pub session_start: DateTime<Utc>,
    pub dir: Option<PathBuf>,
}

impl Journals {
    /// Abre los diarios. Devuelve también un aviso si alguno no se pudo abrir
    /// o tenía líneas dañadas.
    pub fn open(dir: Option<PathBuf>) -> (Self, Option<String>) {
        let mut warnings = Vec::new();
        let open = |name: &str, max: usize| -> Option<(PathBuf, usize)> {
            let d = dir.as_ref()?;
            Some((d.join(name), max))
        };
        let history =
            open(HISTORY_FILE, HISTORY_MAX).and_then(|(p, max)| match Journal::open(&p, max) {
                Ok((j, st)) => {
                    if st.skipped > 0 {
                        warnings.push(format!(
                            "Se han descartado {} líneas dañadas de {}.",
                            st.skipped,
                            p.display()
                        ));
                    }
                    Some(j)
                }
                Err(e) => {
                    warnings.push(format!(
                        "No se pudo abrir el historial ({}): {e}. No se guardará en esta sesión.",
                        p.display()
                    ));
                    None
                }
            });
        let events = open(EVENTS_FILE, EVENTS_MAX).and_then(|(p, max)| match Journal::open(&p, max) {
            Ok((j, st)) => {
                if st.skipped > 0 {
                    warnings.push(format!("Se han descartado {} líneas dañadas de {}.", st.skipped, p.display()));
                }
                Some(j)
            }
            Err(e) => {
                warnings.push(format!("No se pudo abrir el diario de eventos ({}): {e}. No se guardará en esta sesión.", p.display()));
                None
            }
        });
        if dir.is_none() {
            warnings.push("No se ha encontrado una carpeta para guardar el historial: no se guardará en esta sesión.".into());
        }
        (
            Self {
                history,
                events,
                recent: Mutex::new(VecDeque::new()),
                session_start: Utc::now(),
                dir,
            },
            (!warnings.is_empty()).then(|| warnings.join(" ")),
        )
    }

    pub fn record_event(&self, e: &DockerEvent, keep: bool) {
        {
            let mut r = self.recent.lock();
            r.push_back(e.clone());
            while r.len() > RECENT_MAX {
                r.pop_front();
            }
        }
        if keep {
            if let Some(j) = &self.events {
                if let Err(err) = j.append(e.clone()) {
                    tracing::warn!(error = %err, "no se pudo guardar un evento");
                }
            }
        }
    }
}

// ------------------------------------------------------------------ filtros

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct EventFilter {
    /// Desde / hasta (segundos desde 1970), inclusive.
    #[ts(type = "number | null")]
    pub since: Option<i64>,
    #[ts(type = "number | null")]
    pub until: Option<i64>,
    pub types: Vec<String>,
    pub actions: Vec<String>,
    /// Texto en tipo, acción, ID, nombre o imagen.
    pub text: String,
    /// Máximo de resultados (los más recientes); 0 = 1000.
    pub limit: u32,
    /// Servidores (0.10.0); vacío = todos. Sin servidor = equipo local.
    pub servers: Vec<String>,
}

/// Identificador del servidor de un registro; los anteriores a 0.10.0 no lo
/// tienen y son del equipo local.
fn server_id(tag: Option<&dcc_activity::ServerTag>) -> &str {
    tag.map(|t| t.id.as_str())
        .unwrap_or(dcc_activity::ServerTag::LOCAL_ID)
}

impl EventFilter {
    pub fn matches(&self, e: &DockerEvent) -> bool {
        let secs = e.time_nano.div_euclid(1_000_000_000);
        self.since.map_or(true, |s| secs >= s)
            && self.until.map_or(true, |u| secs <= u)
            && (self.types.is_empty() || self.types.contains(&e.kind))
            && (self.actions.is_empty()
                || self
                    .actions
                    .iter()
                    .any(|a| e.action == *a || e.action.starts_with(&format!("{a}:"))))
            && e.matches_text(&self.text)
            && (self.servers.is_empty()
                || self
                    .servers
                    .iter()
                    .any(|s| s == server_id(e.server.as_ref())))
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EventPage {
    /// Los más recientes primero.
    pub events: Vec<DockerEvent>,
    /// Cuántos cumplen el filtro (puede ser más que los devueltos).
    pub total: u32,
    /// Eventos guardados en total.
    pub stored: u32,
    /// El diario está activo (se guardan eventos).
    pub persistent: bool,
}

pub fn page_events(
    all: impl DoubleEndedIterator<Item = DockerEvent>,
    f: &EventFilter,
    stored: usize,
    persistent: bool,
) -> EventPage {
    let limit = if f.limit == 0 {
        1000
    } else {
        f.limit.min(5000)
    } as usize;
    let mut total = 0u32;
    let mut events = Vec::new();
    for e in all.rev() {
        if f.matches(&e) {
            total += 1;
            if events.len() < limit {
                events.push(e);
            }
        }
    }
    EventPage {
        events,
        total,
        stored: stored as u32,
        persistent,
    }
}

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryFilter {
    #[ts(type = "number | null")]
    pub since: Option<i64>,
    #[ts(type = "number | null")]
    pub until: Option<i64>,
    pub categories: Vec<Category>,
    pub statuses: Vec<OpStatus>,
    /// Texto en el tipo de operación, recurso, resumen o error.
    pub text: String,
    pub limit: u32,
    /// Servidores (0.10.0); vacío = todos. Sin servidor = equipo local.
    pub servers: Vec<String>,
}

impl HistoryFilter {
    pub fn matches(&self, h: &HistoryEntry) -> bool {
        let secs = h.started_at.timestamp();
        let q = self.text.trim().to_lowercase();
        self.since.map_or(true, |s| secs >= s)
            && self.until.map_or(true, |u| secs <= u)
            && (self.servers.is_empty()
                || self
                    .servers
                    .iter()
                    .any(|s| s == server_id(h.server.as_ref())))
            && (self.categories.is_empty() || self.categories.contains(&h.category))
            && (self.statuses.is_empty() || self.statuses.contains(&h.status))
            && (q.is_empty()
                || [
                    Some(&h.kind),
                    h.target.as_ref(),
                    h.summary.as_ref(),
                    h.error.as_ref(),
                ]
                .into_iter()
                .flatten()
                .any(|s| s.to_lowercase().contains(&q)))
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryPage {
    /// Las más recientes primero.
    pub entries: Vec<HistoryEntry>,
    pub total: u32,
    pub stored: u32,
    /// Se está guardando (opción activada y archivo disponible).
    pub persistent: bool,
    /// Inicio de esta sesión: lo anterior es de sesiones previas.
    pub session_start: DateTime<Utc>,
    /// Carpeta donde se guarda.
    pub location: Option<String>,
}

// ------------------------------------------------------------------ puentes

/// Guarda en el historial cada operación terminada (si la opción está activada).
pub fn spawn_history_bridge<R: Runtime>(app: AppHandle<R>, log: ActivityLog) {
    let mut rx = log.subscribe();
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(ActivityEvent::Upsert { record }) => {
                    let state = app.state::<AppState>();
                    if !state.config().activity.keep_history {
                        continue;
                    }
                    if let (Some(j), Some(e)) =
                        (&state.journals.history, HistoryEntry::from_record(&record))
                    {
                        if let Err(err) = j.append(e) {
                            tracing::warn!(error = %err, "no se pudo guardar el historial");
                        }
                    }
                }
                Ok(ActivityEvent::Cleared) => {}
                Err(RecvError::Lagged(n)) => {
                    tracing::warn!(lost = n, "el historial ha perdido operaciones")
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
}

async fn until_disconnected(rx: &mut tokio::sync::watch::Receiver<ConnectionState>) {
    loop {
        if !matches!(*rx.borrow_and_update(), ConnectionState::Connected { .. }) {
            return;
        }
        if rx.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

/// Mantiene una suscripción a los eventos de Docker mientras haya conexión:
/// cada evento se guarda y se envía a la ventana (que actualiza lo afectado).
pub fn spawn_events_watcher<R: Runtime>(
    app: AppHandle<R>,
    engine: EngineManager,
    stop: CancellationToken,
) {
    tauri::async_runtime::spawn(async move {
        let mut rx = engine.subscribe();
        loop {
            if stop.is_cancelled() {
                return;
            }
            // Espera a estar conectado.
            while !matches!(*rx.borrow_and_update(), ConnectionState::Connected { .. }) {
                tokio::select! {
                    r = rx.changed() => if r.is_err() { return },
                    _ = stop.cancelled() => return,
                }
            }
            let Ok(e) = engine.engine().await else {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            };
            let cancel = CancellationToken::new();
            let a2 = app.clone();
            let watch = e.events_watch(cancel.clone(), move |ev| {
                let state = a2.state::<AppState>();
                state
                    .journals
                    .record_event(&ev, state.config().activity.keep_history);
                let _ = a2.emit(DOCKER_EVENT, &ev);
                true
            });
            let mut rx2 = rx.clone();
            tokio::select! {
                r = watch => {
                    if let Err(err) = r {
                        if err.is_connection_error() {
                            engine.report_connection_error();
                        }
                        // Reintento sin saturar: si la conexión sigue, se vuelve a suscribir.
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    }
                }
                _ = until_disconnected(&mut rx2) => cancel.cancel(),
                _ = stop.cancelled() => {
                    cancel.cancel();
                    return;
                }
            }
        }
    });
}

#[cfg(test)]
mod server_filter_tests {
    use super::*;
    use serde_json::json;

    fn tag(id: &str) -> serde_json::Value {
        json!({ "id": id, "name": "Prod", "address": "u@h", "remote": true })
    }

    fn entry(server: serde_json::Value) -> HistoryEntry {
        serde_json::from_value(json!({
            "id": "00000000-0000-4000-8000-000000000001",
            "category": "container", "kind": "container.start", "target": "web",
            "transport": { "via": "app", "action": "x" }, "status": "succeeded",
            "startedAt": "2026-11-11T10:00:00Z", "finishedAt": null, "durationMs": 1,
            "summary": null, "error": null, "httpStatus": null, "exitCode": null,
            "server": server
        }))
        .unwrap()
    }

    fn event(server: serde_json::Value) -> DockerEvent {
        serde_json::from_value(json!({
            "timeNano": 1_794_000_000_000_000_000i64, "kind": "container", "action": "start",
            "actorId": "abc", "name": "web", "attributes": {}, "scope": "local", "server": server
        }))
        .unwrap()
    }

    #[test]
    fn history_and_events_filter_by_server() {
        let remote = entry(tag("srv-0123abcd"));
        let old = entry(serde_json::Value::Null);
        let mut f: HistoryFilter = serde_json::from_value(json!({})).unwrap();
        assert!(f.matches(&remote) && f.matches(&old), "sin filtro: todos");
        f.servers = vec!["srv-0123abcd".into()];
        assert!(f.matches(&remote) && !f.matches(&old));
        f.servers = vec!["local".into()];
        assert!(
            !f.matches(&remote) && f.matches(&old),
            "los anteriores a 0.10.0 son del equipo local"
        );

        let mut e: EventFilter = serde_json::from_value(json!({})).unwrap();
        assert!(e.matches(&event(tag("srv-0123abcd"))));
        e.servers = vec!["local".into()];
        assert!(!e.matches(&event(tag("srv-0123abcd"))));
        assert!(e.matches(&event(serde_json::Value::Null)));
    }
}
