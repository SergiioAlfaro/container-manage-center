//! Monitorización (E5, 1.5.0, D-102 a D-107): un muestreador por servidor
//! activado, sin agentes, mientras la aplicación está abierta.
//!
//! - Configuración en `monitor.json` junto a `config.toml` (0600): intervalo,
//!   retención, servidores activados, reglas y notificaciones.
//! - Anillo en memoria (30 min) por servidor; un registro por minuto en
//!   `<estado>/monitor/<servidor>/metricas.jsonl` (0600), que se lee del disco
//!   solo al pedirlo, limitado por tiempo y por tamaño.
//! - Alertas en `<estado>/monitor/<servidor>/alertas.jsonl` (0600).
//! - Nunca bloquea: los comandos leen lo último que hay en memoria; las
//!   desconexiones son huecos.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use dcc_activity::{Category, Failure, OperationSpec, Outcome, Transport};
use dcc_engine::events::DockerEvent;
use dcc_engine::monitor::alerts::AlertEngine;
use dcc_engine::monitor::history::{MinuteAggregator, Ring, ServerPoint};
use dcc_engine::monitor::sampler::{now_ms, SamplerState};
use dcc_engine::monitor::*;
use dcc_engine::ConnectionState;
use dcc_store::journal::Journal;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::error::{CmdResult, CommandError};
use crate::state::AppState;

pub const SETTINGS_FILE: &str = "monitor.json";
/// Una muestra nueva de un servidor (`{ server, t }`).
pub const SAMPLE_EVENT: &str = "monitor://sample";
/// Una alerta (`Alert`).
pub const ALERT_EVENT: &str = "monitor://alert";
/// Anillo en memoria: 30 min o, como mucho, 900 muestras (30 min a 2 s).
pub const RING_MINUTES: i64 = 30;
const RING_MAX: usize = 900;
const ALERTS_MAX: usize = 1000;
/// Tamaño máximo del archivo de minutos de un servidor.
pub const MINUTES_MAX_BYTES: u64 = 64 * 1024 * 1024;
/// Cada cuánto se vuelve a leer `info` (equipo).
const HOST_EVERY: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MonitorSettings {
    /// Entre 2 y 60 s.
    pub interval_secs: u32,
    /// Horas de historial por minuto en el disco (1–168).
    pub retention_hours: u32,
    /// Notificaciones de escritorio de las alertas.
    pub notifications: bool,
    /// Servidores monitorizados (`local`, `srv-…`).
    pub servers: Vec<String>,
    pub rules: AlertRules,
}

impl Default for MonitorSettings {
    fn default() -> Self {
        Self {
            interval_secs: DEFAULT_INTERVAL_SECS,
            retention_hours: 24,
            notifications: true,
            servers: vec![],
            rules: AlertRules::default(),
        }
    }
}

impl MonitorSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !(MIN_INTERVAL_SECS..=MAX_INTERVAL_SECS).contains(&self.interval_secs) {
            return Err(format!(
                "el intervalo debe estar entre {MIN_INTERVAL_SECS} y {MAX_INTERVAL_SECS} s"
            ));
        }
        if !(1..=168).contains(&self.retention_hours) {
            return Err("la retención debe estar entre 1 y 168 horas".into());
        }
        if self.servers.len() > 100 || self.servers.iter().any(|s| !safe_id(s)) {
            return Err("servidores no válidos".into());
        }
        self.rules.validate()
    }
}

fn safe_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn settings_path(config: &Path) -> PathBuf {
    config.with_file_name(SETTINGS_FILE)
}

pub fn load_settings(config: &Path) -> MonitorSettings {
    std::fs::read_to_string(settings_path(config))
        .ok()
        .and_then(|t| serde_json::from_str::<MonitorSettings>(&t).ok())
        .filter(|s| s.validate().is_ok())
        .unwrap_or_default()
}

pub fn save_settings(config: &Path, s: &MonitorSettings) -> CmdResult<()> {
    s.validate()
        .map_err(|m| CommandError::new("monitor.invalid", m))?;
    let text = serde_json::to_string_pretty(s).map_err(|e| CommandError::new("monitor.io", e))?;
    dcc_store::write_atomic(&settings_path(config), &text)
        .map_err(|e| CommandError::new("monitor.io", e))
}

// ------------------------------------------------------------------ minutos

/// Registros por minuto de un servidor, en disco: se añaden al final y se leen
/// solo al pedirlos, filtrando por tiempo y contenedores (no se guardan en
/// memoria).
pub struct MinuteStore {
    path: PathBuf,
}

impl MinuteStore {
    pub fn open(dir: &Path) -> std::io::Result<Self> {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
        Ok(Self {
            path: dir.join("metricas.jsonl"),
        })
    }

    pub fn append(&self, r: &MinuteRecord) -> std::io::Result<()> {
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .mode(0o600)
            .open(&self.path)?;
        let mut line = serde_json::to_vec(r).map_err(std::io::Error::other)?;
        line.push(b'\n');
        f.write_all(&line)
    }

    /// Registros desde `since` (ms), solo con los contenedores pedidos (todos
    /// si `ids` está vacío). Las líneas dañadas se saltan.
    pub fn read(&self, since: i64, ids: &HashSet<String>) -> Vec<MinuteRecord> {
        let Ok(f) = std::fs::File::open(&self.path) else {
            return vec![];
        };
        let mut out = Vec::new();
        for line in BufReader::new(f).split(b'\n').map_while(Result::ok) {
            // Filtro barato por tiempo antes de interpretar toda la línea.
            let Ok(mut r) = serde_json::from_slice::<MinuteRecord>(&line) else {
                continue;
            };
            if r.t < since {
                continue;
            }
            if !ids.is_empty() {
                r.containers.retain(|c| ids.contains(&c.id));
            }
            out.push(r);
        }
        out
    }

    /// Quita lo anterior a `cutoff` y, si el archivo pasa de `max_bytes`, lo
    /// más antiguo hasta quedar en la mitad. Se reescribe en un temporal.
    pub fn compact(&self, cutoff: i64, max_bytes: u64) -> std::io::Result<()> {
        let Ok(meta) = std::fs::metadata(&self.path) else {
            return Ok(());
        };
        let f = std::fs::File::open(&self.path)?;
        let mut lines: Vec<(i64, Vec<u8>)> = Vec::new();
        let mut dropped = false;
        for line in BufReader::new(f).split(b'\n').map_while(Result::ok) {
            #[derive(Deserialize)]
            struct T {
                t: i64,
            }
            match serde_json::from_slice::<T>(&line) {
                Ok(T { t }) if t >= cutoff => lines.push((t, line)),
                _ => dropped = true,
            }
        }
        if meta.len() > max_bytes {
            let mut total: u64 = lines.iter().map(|(_, l)| l.len() as u64 + 1).sum();
            let mut skip = 0;
            while total > max_bytes / 2 && skip < lines.len() {
                total -= lines[skip].1.len() as u64 + 1;
                skip += 1;
            }
            lines.drain(..skip);
            dropped = dropped || skip > 0;
        }
        if !dropped {
            return Ok(());
        }
        let tmp = self.path.with_extension("jsonl.tmp");
        {
            let mut w = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp)?;
            for (_, l) in &lines {
                w.write_all(l)?;
                w.write_all(b"\n")?;
            }
            w.sync_all()?;
        }
        std::fs::rename(tmp, &self.path)
    }
}

// ------------------------------------------------------------------ servicio

struct ServerMon {
    ring: Ring,
    alerts: AlertEngine,
    host: Option<HostInfo>,
    last_error: Option<String>,
    cancel: CancellationToken,
    dirty: Vec<String>,
    minutes: Option<MinuteStore>,
    alert_log: Option<Journal<Alert>>,
}

#[derive(Default)]
struct Inner {
    settings: MonitorSettings,
    paused: bool,
    servers: HashMap<String, ServerMon>,
}

/// Estado de la monitorización de un servidor, para la interfaz.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MonitorStatus {
    pub server: String,
    pub enabled: bool,
    pub paused: bool,
    /// Hay un muestreador en marcha para este servidor.
    pub running: bool,
    pub interval_secs: u32,
    pub host: Option<HostInfo>,
    /// Última muestra con datos.
    pub last: Option<ServerSample>,
    /// La muestra más reciente es un hueco (sin conexión).
    pub gap: bool,
    pub last_error: Option<String>,
    /// Alertas de umbral activas: (contenedor, tipo).
    pub firing: Vec<FiringAlert>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FiringAlert {
    pub container_id: String,
    pub kind: AlertKind,
}

/// Serie en vivo (memoria) de un servidor y de los contenedores pedidos.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MonitorLive {
    pub server: Vec<ServerPoint>,
    pub containers: Vec<ContainerSeries>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContainerSeries {
    pub id: String,
    /// Un punto por muestra del servidor; `null` donde no hay dato.
    #[ts(as = "Vec<(f64, Option<ContainerPoint>)>")]
    pub points: Vec<(i64, Option<ContainerPoint>)>,
}

#[derive(Default)]
pub struct Monitor {
    inner: Mutex<Inner>,
    dir: Option<PathBuf>,
}

impl Monitor {
    pub fn new(config: &Path, state_dir: Option<PathBuf>) -> Self {
        Self {
            inner: Mutex::new(Inner {
                settings: load_settings(config),
                ..Default::default()
            }),
            dir: state_dir.map(|d| d.join("monitor")),
        }
    }

    pub fn settings(&self) -> MonitorSettings {
        self.inner.lock().settings.clone()
    }

    pub fn paused(&self) -> bool {
        self.inner.lock().paused
    }

    pub fn set_paused(&self, p: bool) {
        self.inner.lock().paused = p;
    }

    fn server_dir(&self, server: &str) -> Option<PathBuf> {
        safe_id(server).then(|| self.dir.as_ref().map(|d| d.join(server)))?
    }

    /// Cambia la configuración en memoria y devuelve los servidores que hay
    /// que arrancar y los que hay que parar.
    pub fn apply(&self, s: MonitorSettings) -> (Vec<String>, Vec<String>) {
        let mut g = self.inner.lock();
        let want: HashSet<String> = s.servers.iter().cloned().collect();
        let have: HashSet<String> = g.servers.keys().cloned().collect();
        for m in g.servers.values_mut() {
            m.alerts.set_rules(s.rules.clone());
        }
        g.settings = s;
        let start = want.difference(&have).cloned().collect();
        let stop: Vec<String> = have.difference(&want).cloned().collect();
        for id in &stop {
            if let Some(m) = g.servers.remove(id) {
                m.cancel.cancel();
            }
        }
        (start, stop)
    }

    fn register(&self, server: &str) -> Option<CancellationToken> {
        let mut g = self.inner.lock();
        if g.servers.contains_key(server) {
            return None;
        }
        let dir = self.server_dir(server);
        let minutes = dir.as_ref().and_then(|d| MinuteStore::open(d).ok());
        let alert_log = dir
            .as_ref()
            .and_then(|d| Journal::open(d.join("alertas.jsonl"), ALERTS_MAX).ok())
            .map(|(j, _)| j);
        let cancel = CancellationToken::new();
        let mut alerts = AlertEngine::new(g.settings.rules.clone());
        if let Some(j) = &alert_log {
            alerts.remember_notified(&j.entries());
        }
        g.servers.insert(
            server.to_owned(),
            ServerMon {
                ring: Ring::new(RING_MAX, RING_MINUTES * 60_000),
                alerts,
                host: None,
                last_error: None,
                cancel: cancel.clone(),
                dirty: vec![],
                minutes,
                alert_log,
            },
        );
        Some(cancel)
    }

    pub fn stop(&self, server: &str) {
        if let Some(m) = self.inner.lock().servers.remove(server) {
            m.cancel.cancel();
        }
    }

    pub fn stop_all(&self) {
        for (_, m) in self.inner.lock().servers.drain() {
            m.cancel.cancel();
        }
    }

    pub fn status(&self, server: &str) -> MonitorStatus {
        let g = self.inner.lock();
        let enabled = g.settings.servers.iter().any(|s| s == server);
        let m = g.servers.get(server);
        MonitorStatus {
            server: server.to_owned(),
            enabled,
            paused: g.paused,
            running: m.is_some(),
            interval_secs: g.settings.interval_secs,
            host: m.and_then(|m| m.host.clone()),
            last: m.and_then(|m| m.ring.last().cloned()),
            gap: m.is_some_and(|m| m.ring.last_is_gap()),
            last_error: m.and_then(|m| m.last_error.clone()),
            firing: m
                .map(|m| {
                    m.alerts
                        .firing()
                        .into_iter()
                        .map(|(container_id, kind)| FiringAlert { container_id, kind })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    pub fn live(&self, server: &str, ids: &[String], minutes: u32) -> MonitorLive {
        let g = self.inner.lock();
        let Some(m) = g.servers.get(server) else {
            return MonitorLive {
                server: vec![],
                containers: vec![],
            };
        };
        let since = now_ms() - i64::from(minutes.clamp(1, RING_MINUTES as u32)) * 60_000;
        MonitorLive {
            server: m.ring.server_series(since),
            containers: ids
                .iter()
                .take(8)
                .map(|id| ContainerSeries {
                    id: id.clone(),
                    points: m.ring.series(id, since),
                })
                .collect(),
        }
    }

    pub fn minutes(&self, server: &str, hours: u32, ids: &[String]) -> Vec<MinuteRecord> {
        let Some(dir) = self.server_dir(server) else {
            return vec![];
        };
        let Ok(store) = MinuteStore::open(&dir) else {
            return vec![];
        };
        let since = now_ms() - i64::from(hours.clamp(1, 168)) * 3_600_000;
        store.read(since, &ids.iter().take(8).cloned().collect())
    }

    pub fn alerts(&self, server: &str) -> Vec<Alert> {
        let Some(dir) = self.server_dir(server) else {
            return vec![];
        };
        // Del disco: así se ven también las de sesiones anteriores.
        let mut v = Journal::<Alert>::open(dir.join("alertas.jsonl"), ALERTS_MAX)
            .map(|(j, _)| j.entries())
            .unwrap_or_default();
        v.reverse();
        v
    }

    /// Evento de Docker de un servidor monitorizado: se relee el contenedor y
    /// se evalúan las alertas de eventos.
    pub fn on_event(&self, server: &str, ev: &DockerEvent) -> Vec<Alert> {
        let mut g = self.inner.lock();
        let Some(m) = g.servers.get_mut(server) else {
            return vec![];
        };
        if ev.kind == "container" {
            m.dirty.push(ev.actor_id.clone());
        }
        let out = m.alerts.on_event(server, ev);
        record_alerts(m, &out);
        out
    }
}

fn record_alerts(m: &ServerMon, alerts: &[Alert]) {
    if let Some(j) = &m.alert_log {
        for a in alerts {
            let _ = j.append(a.clone());
        }
    }
}

fn emit_alerts<R: Runtime>(app: &AppHandle<R>, alerts: &[Alert]) {
    for a in alerts {
        let _ = app.emit(ALERT_EVENT, a);
    }
}

/// Arranca el muestreador de un servidor (si no lo está ya).
pub fn start<R: Runtime>(app: &AppHandle<R>, server: &str) {
    let state = app.state::<AppState>();
    let Some(cancel) = state.monitor.register(server) else {
        return;
    };
    let h = state.activity.begin(OperationSpec {
        category: Category::System,
        kind: "monitor.start".into(),
        target: Some(server.to_owned()),
        transport: Transport::App {
            action: format!(
                "Monitorización de «{server}»: una lectura de la lista de contenedores y de `stats` de cada uno en marcha cada {} s (no se registra cada lectura)",
                state.monitor.settings().interval_secs
            ),
        },
        request_body: None,
        equivalent_command: None,
    });
    h.succeed(Outcome {
        summary: Some("Muestreo iniciado".into()),
        ..Default::default()
    });
    let app = app.clone();
    let server = server.to_owned();
    tauri::async_runtime::spawn(async move {
        run(app, server, cancel).await;
    });
}

async fn run<R: Runtime>(app: AppHandle<R>, server: String, cancel: CancellationToken) {
    let mut st = SamplerState::default();
    let mut agg = MinuteAggregator::default();
    let mut last_host: Option<std::time::Instant> = None;
    let mut last_compact = std::time::Instant::now() - Duration::from_secs(3600);
    let mut samples: u64 = 0;
    loop {
        let state = app.state::<AppState>();
        let settings = state.monitor.settings();
        let interval = Duration::from_secs(u64::from(settings.interval_secs));
        tokio::select! {
            () = tokio::time::sleep(interval) => {}
            () = cancel.cancelled() => break,
        }
        if state.monitor.paused() {
            continue;
        }
        // El servidor puede haberse eliminado.
        let Ok(mgr) = state.server(&server) else {
            state.monitor.stop(&server);
            break;
        };
        let connected = matches!(mgr.state(), ConnectionState::Connected { .. });
        let t = now_ms();
        let sample = if connected {
            match mgr.engine().await {
                Ok(engine) => {
                    // Contenedores tocados por eventos: se releen.
                    let dirty: Vec<String> = {
                        let mut g = state.monitor.inner.lock();
                        g.servers
                            .get_mut(&server)
                            .map(|m| std::mem::take(&mut m.dirty))
                            .unwrap_or_default()
                    };
                    for id in &dirty {
                        st.mark_dirty(id);
                    }
                    if last_host.is_none_or_elapsed(HOST_EVERY) {
                        if let Ok(h) = engine.monitor_host().await {
                            if let Some(m) = state.monitor.inner.lock().servers.get_mut(&server) {
                                m.host = Some(h);
                            }
                            last_host = Some(std::time::Instant::now());
                        }
                    }
                    match engine.monitor_sample(&mut st).await {
                        Ok(s) => Ok(s),
                        Err(e) => {
                            if e.is_connection_error() {
                                mgr.report_connection_error();
                            }
                            Err(e.to_string())
                        }
                    }
                }
                Err(e) => Err(e.to_string()),
            }
        } else {
            Err(String::new())
        };
        let sample = match sample {
            Ok(s) => s,
            Err(err) => {
                st.reset_counters();
                let mut g = state.monitor.inner.lock();
                if let Some(m) = g.servers.get_mut(&server) {
                    if !err.is_empty() {
                        m.last_error = Some(dcc_activity::redact::redact_text(&err).into_owned());
                    }
                }
                drop(g);
                ServerSample::gap(t)
            }
        };
        let mut alerts = Vec::new();
        let minute;
        {
            let mut g = state.monitor.inner.lock();
            let Some(m) = g.servers.get_mut(&server) else {
                break;
            };
            alerts.extend(m.alerts.on_connection(&server, sample.connected, sample.t));
            alerts.extend(m.alerts.on_sample(&server, &sample));
            if sample.connected {
                m.last_error = None;
            }
            minute = agg.push(&sample);
            record_alerts(m, &alerts);
            m.ring.push(sample);
            if let (Some(r), Some(store)) = (&minute, &m.minutes) {
                if let Err(e) = store.append(r) {
                    tracing::warn!(error = %e, "no se pudo guardar el minuto de monitorización");
                }
                if last_compact.elapsed() > Duration::from_secs(3600) {
                    let cutoff = now_ms() - i64::from(settings.retention_hours) * 3_600_000;
                    let _ = store.compact(cutoff, MINUTES_MAX_BYTES);
                    last_compact = std::time::Instant::now();
                }
            }
        }
        samples += 1;
        emit_alerts(&app, &alerts);
        let _ = app.emit(
            SAMPLE_EVENT,
            serde_json::json!({ "server": server, "t": t }),
        );
    }
    // Al parar se guarda el minuto en curso.
    if let Some(r) = agg.flush() {
        let state = app.state::<AppState>();
        if let Some(dir) = state.monitor.server_dir(&server) {
            if let Ok(store) = MinuteStore::open(&dir) {
                let _ = store.append(&r);
            }
        }
    }
    let state = app.state::<AppState>();
    let h = state.activity.begin(OperationSpec {
        category: Category::System,
        kind: "monitor.stop".into(),
        target: Some(server.clone()),
        transport: Transport::App {
            action: format!("Fin de la monitorización de «{server}»"),
        },
        request_body: None,
        equivalent_command: None,
    });
    if samples > 0 {
        h.succeed(Outcome {
            summary: Some(format!("{samples} muestras")),
            ..Default::default()
        });
    } else {
        h.fail(Failure {
            error: "sin muestras".into(),
            ..Default::default()
        });
    }
}

trait ElapsedExt {
    fn is_none_or_elapsed(&self, d: Duration) -> bool;
}

impl ElapsedExt for Option<std::time::Instant> {
    fn is_none_or_elapsed(&self, d: Duration) -> bool {
        self.map_or(true, |t| t.elapsed() >= d)
    }
}

/// Arranca los muestreadores de los servidores activados (al abrir la
/// aplicación): la monitorización sobrevive a un reinicio.
pub fn start_enabled<R: Runtime>(app: &AppHandle<R>) {
    let servers = app.state::<AppState>().monitor.settings().servers;
    for s in servers {
        if app.state::<AppState>().server(&s).is_ok() {
            start(app, &s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minute(t: i64, ids: &[&str]) -> MinuteRecord {
        MinuteRecord {
            t,
            samples: 1,
            gaps: 0,
            cpu_total_avg: Some(1.0),
            mem_total_avg: 1,
            containers: ids
                .iter()
                .map(|id| MinuteContainer {
                    id: (*id).into(),
                    name: (*id).into(),
                    samples: 1,
                    cpu_avg: Some(1.0),
                    cpu_max: Some(1.0),
                    mem_avg: 1,
                    mem_max: 1,
                    mem_limit: 1,
                    net_rx: 0,
                    net_tx: 0,
                    blk_read: 0,
                    blk_write: 0,
                    pids_max: None,
                })
                .collect(),
        }
    }

    #[test]
    fn minute_store_filters_compacts_by_age_and_size_and_is_private() {
        let d = tempfile::Builder::new()
            .prefix("dcc-test")
            .tempdir()
            .unwrap();
        let s = MinuteStore::open(&d.path().join("monitor/local")).unwrap();
        for i in 0..100 {
            s.append(&minute(i * 60_000, &["a", "b"])).unwrap();
        }
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&s.path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(d.path().join("monitor/local"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        let only_a = s.read(90 * 60_000, &HashSet::from(["a".to_owned()]));
        assert_eq!(only_a.len(), 10);
        assert!(only_a
            .iter()
            .all(|r| r.containers.len() == 1 && r.containers[0].id == "a"));
        // Una línea dañada se salta.
        std::fs::OpenOptions::new()
            .append(true)
            .open(&s.path)
            .unwrap()
            .write_all(b"{roto\n")
            .unwrap();
        assert_eq!(s.read(0, &HashSet::new()).len(), 100);
        // Por antigüedad.
        s.compact(50 * 60_000, MINUTES_MAX_BYTES).unwrap();
        let all = s.read(0, &HashSet::new());
        assert_eq!(all.len(), 50);
        assert_eq!(all[0].t, 50 * 60_000);
        // Por tamaño: queda la mitad del máximo como mucho.
        let size = std::fs::metadata(&s.path).unwrap().len();
        s.compact(0, size / 2).unwrap();
        let after = std::fs::metadata(&s.path).unwrap().len();
        assert!(after <= size / 4 + 1024, "{after} de {size}");
        assert_eq!(
            s.read(0, &HashSet::new()).last().unwrap().t,
            99 * 60_000,
            "se quedan los más recientes"
        );
    }

    #[test]
    fn settings_are_validated_and_survive_a_restart() {
        let d = tempfile::Builder::new()
            .prefix("dcc-test")
            .tempdir()
            .unwrap();
        let cfg = d.path().join("config.toml");
        assert_eq!(load_settings(&cfg), MonitorSettings::default());
        let mut s = MonitorSettings {
            servers: vec!["local".into()],
            interval_secs: 10,
            ..Default::default()
        };
        save_settings(&cfg, &s).unwrap();
        assert_eq!(load_settings(&cfg), s);
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(settings_path(&cfg))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        s.interval_secs = 1;
        assert!(save_settings(&cfg, &s).is_err());
        s.interval_secs = 61;
        assert!(s.validate().is_err());
        s.interval_secs = 5;
        s.retention_hours = 0;
        assert!(s.validate().is_err());
        s.retention_hours = 24;
        s.servers = vec!["../x".into()];
        assert!(s.validate().is_err());
        // Un archivo dañado no impide arrancar: valores por defecto.
        std::fs::write(settings_path(&cfg), "{roto").unwrap();
        assert_eq!(load_settings(&cfg), MonitorSettings::default());
    }

    #[test]
    fn service_starts_stops_and_answers_without_a_sampler() {
        let d = tempfile::Builder::new()
            .prefix("dcc-test")
            .tempdir()
            .unwrap();
        let m = Monitor::new(&d.path().join("config.toml"), Some(d.path().join("estado")));
        let st = m.status("local");
        assert!(!st.enabled && !st.running && st.last.is_none());
        let (start, stop) = m.apply(MonitorSettings {
            servers: vec!["local".into()],
            ..Default::default()
        });
        assert_eq!((start, stop), (vec!["local".to_owned()], vec![]));
        let cancel = m.register("local").unwrap();
        assert!(
            m.register("local").is_none(),
            "un solo muestreador por servidor"
        );
        assert!(m.status("local").running);
        let (start, stop) = m.apply(MonitorSettings::default());
        assert!(start.is_empty());
        assert_eq!(stop, vec!["local".to_owned()]);
        assert!(cancel.is_cancelled());
        assert!(m.live("local", &["x".into()], 5).server.is_empty());
        assert!(m.server_dir("../x").is_none());
    }
}
