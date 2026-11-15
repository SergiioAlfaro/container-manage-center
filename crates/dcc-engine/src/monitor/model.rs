// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Modelo de la monitorización (E5, 1.5.0, D-102 a D-107).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Límites del intervalo de muestreo, en segundos (plan §5.3).
pub const MIN_INTERVAL_SECS: u32 = 2;
pub const MAX_INTERVAL_SECS: u32 = 60;
pub const DEFAULT_INTERVAL_SECS: u32 = 5;

/// Una muestra de un contenedor. `None` cuando el dato no se puede calcular
/// (primera muestra, contador que vuelve a empezar, Docker no lo da).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "MonitorPoint")]
pub struct ContainerPoint {
    /// Momento de la muestra (ms desde 1970).
    #[ts(type = "number")]
    pub t: i64,
    /// CPU en % de una CPU, como `docker stats` (puede superar 100).
    pub cpu: Option<f64>,
    #[ts(type = "number")]
    pub mem: u64,
    #[ts(type = "number")]
    pub mem_limit: u64,
    pub mem_percent: Option<f64>,
    /// Bytes por segundo recibidos y enviados.
    pub net_rx: Option<f64>,
    pub net_tx: Option<f64>,
    /// Bytes por segundo leídos y escritos en disco (solo donde Docker los da).
    pub blk_read: Option<f64>,
    pub blk_write: Option<f64>,
    #[ts(type = "number | null")]
    pub pids: Option<u64>,
}

/// Datos de un contenedor que no cambian en cada muestra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "MonitorContainer")]
pub struct ContainerMeta {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub compose_project: Option<String>,
    /// Inicio de la ejecución actual (RFC 3339) y número de reinicios, de `inspect`.
    pub started_at: Option<String>,
    #[ts(type = "number | null")]
    pub restart_count: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "MonitorContainerSample")]
pub struct ContainerSample {
    pub meta: ContainerMeta,
    /// `None` si el contenedor no está en marcha o falló su lectura.
    pub point: Option<ContainerPoint>,
}

/// Recuentos del servidor en una muestra.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "MonitorCounts")]
pub struct Counts {
    pub running: u32,
    pub paused: u32,
    pub stopped: u32,
    pub total: u32,
}

/// Una muestra de todo un servidor. `connected = false` es un hueco: no hay
/// datos y no se inventan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "MonitorSample")]
pub struct ServerSample {
    #[ts(type = "number")]
    pub t: i64,
    pub connected: bool,
    pub counts: Counts,
    /// Suma de la CPU de los contenedores (en % de una CPU).
    pub cpu_total: Option<f64>,
    #[ts(type = "number")]
    pub mem_total: u64,
    pub containers: Vec<ContainerSample>,
    /// Lecturas que fallaron en esta muestra.
    pub errors: u32,
    /// Duración del muestreo (ms).
    #[ts(type = "number")]
    pub took_ms: u64,
}

impl ServerSample {
    pub fn gap(t: i64) -> Self {
        Self {
            t,
            connected: false,
            counts: Counts::default(),
            cpu_total: None,
            mem_total: 0,
            containers: vec![],
            errors: 0,
            took_ms: 0,
        }
    }
}

/// Datos del equipo que da Docker (`info`): no es una monitorización del
/// sistema operativo (plan §5.3).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "MonitorHost")]
pub struct HostInfo {
    #[ts(type = "number | null")]
    pub ncpu: Option<i64>,
    #[ts(type = "number | null")]
    pub mem_total: Option<i64>,
    pub server_version: Option<String>,
    pub operating_system: Option<String>,
    pub kernel_version: Option<String>,
}

/// Resumen de un minuto de un contenedor (lo que se guarda en disco).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "MonitorMinuteContainer")]
pub struct MinuteContainer {
    pub id: String,
    pub name: String,
    pub samples: u32,
    pub cpu_avg: Option<f64>,
    pub cpu_max: Option<f64>,
    #[ts(type = "number")]
    pub mem_avg: u64,
    #[ts(type = "number")]
    pub mem_max: u64,
    #[ts(type = "number")]
    pub mem_limit: u64,
    /// Bytes en el minuto (a partir de las velocidades medidas).
    #[ts(type = "number")]
    pub net_rx: u64,
    #[ts(type = "number")]
    pub net_tx: u64,
    #[ts(type = "number")]
    pub blk_read: u64,
    #[ts(type = "number")]
    pub blk_write: u64,
    #[ts(type = "number | null")]
    pub pids_max: Option<u64>,
}

/// Un minuto de un servidor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "MonitorMinute")]
pub struct MinuteRecord {
    /// Inicio del minuto (ms desde 1970).
    #[ts(type = "number")]
    pub t: i64,
    /// Muestras con conexión y huecos en ese minuto.
    pub samples: u32,
    pub gaps: u32,
    pub cpu_total_avg: Option<f64>,
    #[ts(type = "number")]
    pub mem_total_avg: u64,
    pub containers: Vec<MinuteContainer>,
}

// ------------------------------------------------------------------ alertas

/// Regla de umbral con histéresis: salta cuando el valor supera `above`
/// durante `samples` muestras seguidas y se resuelve cuando baja de `clear_below`
/// durante otras tantas.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThresholdRule {
    pub enabled: bool,
    pub above: f64,
    pub clear_below: f64,
    pub samples: u32,
}

/// Reglas de alerta (globales; se aplican a todos los contenedores
/// monitorizados).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AlertRules {
    /// CPU en % de una CPU, como `docker stats`.
    pub cpu: ThresholdRule,
    /// Memoria en % de su límite (o de la memoria del equipo si no tiene).
    pub memory: ThresholdRule,
    /// Un contenedor que se cae y Docker vuelve a arrancar (política de reinicio).
    pub restart: bool,
    /// Docker lo mata por falta de memoria (OOM).
    pub oom: bool,
    /// Termina con un código distinto de 0 sin que nadie lo pare.
    pub exit_error: bool,
    /// Se pierde la conexión con el servidor.
    pub disconnected: bool,
    /// Tras avisar de una alerta, no se vuelve a avisar de la misma durante
    /// este tiempo aunque se resuelva y vuelva a saltar.
    pub silence_minutes: u32,
}

impl Default for AlertRules {
    fn default() -> Self {
        Self {
            cpu: ThresholdRule {
                enabled: true,
                above: 90.0,
                clear_below: 70.0,
                samples: 3,
            },
            memory: ThresholdRule {
                enabled: true,
                above: 90.0,
                clear_below: 80.0,
                samples: 3,
            },
            restart: true,
            oom: true,
            exit_error: true,
            disconnected: true,
            silence_minutes: 15,
        }
    }
}

impl AlertRules {
    pub fn validate(&self) -> Result<(), String> {
        for (name, r) in [("cpu", &self.cpu), ("memoria", &self.memory)] {
            if !(r.above.is_finite() && r.clear_below.is_finite()) {
                return Err(format!("{name}: valores no válidos"));
            }
            if r.above <= 0.0 || r.clear_below < 0.0 || r.clear_below >= r.above {
                return Err(format!(
                    "{name}: el valor de recuperación debe ser menor que el umbral"
                ));
            }
            if !(1..=60).contains(&r.samples) {
                return Err(format!("{name}: entre 1 y 60 muestras"));
            }
        }
        if self.memory.above > 100.0 {
            return Err("memoria: el umbral es un porcentaje (≤ 100)".into());
        }
        if self.cpu.above > 10_000.0 {
            return Err("cpu: umbral demasiado alto".into());
        }
        if self.silence_minutes > 24 * 60 {
            return Err("el silencio no puede pasar de 24 h".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AlertKind {
    Cpu,
    Memory,
    Restart,
    Oom,
    ExitError,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AlertState {
    Fired,
    Resolved,
}

/// Una alerta (al saltar o al resolverse).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Alert {
    pub server: String,
    pub kind: AlertKind,
    pub state: AlertState,
    pub container_id: Option<String>,
    pub container: Option<String>,
    /// Valor que la hizo saltar (CPU o memoria en %, código de salida…).
    pub value: Option<f64>,
    /// Momento (ms desde 1970).
    #[ts(type = "number")]
    pub at: i64,
    /// Si se avisó con una notificación (no, dentro del periodo de silencio o
    /// al resolverse).
    pub notified: bool,
}

impl Alert {
    /// Clave que identifica «la misma» alerta.
    pub fn key(&self) -> String {
        format!(
            "{}|{:?}|{}",
            self.server,
            self.kind,
            self.container_id.as_deref().unwrap_or("")
        )
    }
}
