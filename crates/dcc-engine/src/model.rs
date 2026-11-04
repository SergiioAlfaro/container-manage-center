//! Tipos públicos del motor, compartidos con la interfaz mediante ts-rs.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Cómo elige la aplicación el socket de Docker.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "mode",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ConnectionMode {
    /// Detección automática (DOCKER_HOST, contexto de Docker, rootless, sistema).
    #[default]
    Auto,
    /// Un socket Unix concreto indicado por el usuario.
    Custom { socket_path: String },
}

/// De dónde salió un socket candidato.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum EndpointSource {
    /// Configurado manualmente en la aplicación.
    Custom,
    /// Variable de entorno `DOCKER_HOST`.
    DockerHost,
    /// Contexto de Docker (`docker context use …`).
    Context { name: String },
    /// Socket de Docker en modo rootless del usuario.
    Rootless,
    /// Socket del daemon del sistema.
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Endpoint {
    /// Ruta absoluta del socket Unix.
    pub socket_path: String,
    pub source: EndpointSource,
}

impl Endpoint {
    /// URI en el formato que entiende el CLI (`-H unix:///…`).
    pub fn host_uri(&self) -> String {
        format!("unix://{}", self.socket_path)
    }
}

/// Resultado de comprobar un socket candidato.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "result",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ProbeResult {
    /// Acepta conexiones (todavía no se sabe si es Docker).
    Reachable,
    Missing,
    NotASocket,
    PermissionDenied,
    /// El archivo existe pero nadie escucha (daemon detenido).
    Refused,
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CandidateReport {
    pub endpoint: Endpoint,
    pub result: ProbeResult,
}

/// Propietario y permisos de un socket, para explicar un acceso denegado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SocketPermissions {
    pub socket_path: String,
    pub owner: String,
    pub group: String,
    /// Permisos en octal, p. ej. `660`.
    pub mode: String,
    /// El usuario figura como miembro del grupo en /etc/group.
    pub user_in_group: bool,
    /// El proceso actual tiene ese grupo activo (si no, hay que volver a iniciar sesión).
    pub group_active_in_session: bool,
}

/// Qué partes de Docker hay instaladas en el sistema.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Installation {
    pub cli_path: Option<String>,
    pub daemon_path: Option<String>,
    pub rootless_tools: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UserContext {
    pub name: String,
    pub uid: u32,
}

/// Motivo por el que no hay conexión.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Problem {
    /// Ningún candidato existe: Docker no está instalado o no se ha iniciado nunca.
    NoSocket,
    /// El socket existe pero el daemon no responde.
    DaemonNotRunning {
        socket_path: String,
    },
    PermissionDenied {
        permissions: SocketPermissions,
    },
    /// `DOCKER_HOST` o el contexto apuntan a un transporte no soportado (tcp, ssh…).
    UnsupportedHost {
        host: String,
    },
    /// La ruta configurada no es válida.
    InvalidPath {
        path: String,
        reason: String,
    },
    NotASocket {
        socket_path: String,
    },
    /// El socket responde pero no es Docker Engine.
    NotDocker {
        socket_path: String,
        message: String,
    },
    Timeout {
        socket_path: String,
    },
    /// Versión de API demasiado antigua.
    ApiTooOld {
        socket_path: String,
        api_version: String,
        minimum: String,
    },
    /// Error devuelto por Docker o del sistema durante la conexión.
    Other {
        socket_path: Option<String>,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Diagnosis {
    pub problem: Problem,
    pub candidates: Vec<CandidateReport>,
    pub installation: Installation,
    pub user: UserContext,
    pub mode: ConnectionMode,
}

/// Lo mínimo para mostrar la conexión activa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EngineSummary {
    pub version: String,
    pub api_version: String,
    pub os: String,
    pub arch: String,
    pub rootless: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ConnectionState {
    Connecting,
    Connected {
        endpoint: Endpoint,
        engine: EngineSummary,
        since: DateTime<Utc>,
    },
    Disconnected {
        diagnosis: Box<Diagnosis>,
        since: DateTime<Utc>,
    },
}

/// `GET /version`, campos relevantes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EngineVersion {
    pub version: Option<String>,
    pub api_version: Option<String>,
    pub min_api_version: Option<String>,
    pub go_version: Option<String>,
    pub git_commit: Option<String>,
    pub build_time: Option<String>,
    pub os: Option<String>,
    pub arch: Option<String>,
    pub kernel_version: Option<String>,
    pub components: Vec<ComponentVersion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ComponentVersion {
    pub name: String,
    pub version: String,
}

/// `GET /info`, campos relevantes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EngineInfo {
    pub name: Option<String>,
    pub server_version: Option<String>,
    pub operating_system: Option<String>,
    pub os_type: Option<String>,
    pub kernel_version: Option<String>,
    pub architecture: Option<String>,
    #[ts(type = "number | null")]
    pub ncpu: Option<i64>,
    #[ts(type = "number | null")]
    pub mem_total: Option<i64>,
    pub docker_root_dir: Option<String>,
    pub storage_driver: Option<String>,
    pub logging_driver: Option<String>,
    pub cgroup_driver: Option<String>,
    pub cgroup_version: Option<String>,
    #[ts(type = "number | null")]
    pub containers: Option<i64>,
    #[ts(type = "number | null")]
    pub containers_running: Option<i64>,
    #[ts(type = "number | null")]
    pub containers_paused: Option<i64>,
    #[ts(type = "number | null")]
    pub containers_stopped: Option<i64>,
    #[ts(type = "number | null")]
    pub images: Option<i64>,
    pub security_options: Vec<String>,
    pub warnings: Vec<String>,
    pub swarm_active: bool,
    pub rootless: bool,
}

/// Docker Compose (plugin del CLI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ComposeStatus {
    Available {
        version: String,
        cli_path: String,
    },
    /// No se encontró el CLI `docker`.
    CliMissing,
    /// El CLI existe pero el plugin compose no.
    PluginMissing {
        cli_path: String,
        message: String,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemOverview {
    pub endpoint: Endpoint,
    pub version: EngineVersion,
    pub info: EngineInfo,
}
