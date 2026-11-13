use serde::Serialize;
use ts_rs::TS;

/// Error de una operación contra Docker, ya clasificado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS, thiserror::Error)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum EngineError {
    /// No hay conexión activa con Docker Engine.
    #[error("no hay conexión con Docker Engine")]
    NotConnected,
    /// Docker respondió con un código de error.
    #[error("Docker respondió {status}: {message}")]
    Docker { status: u16, message: String },
    #[error("tiempo de espera agotado")]
    Timeout,
    /// Fallo de comunicación con el socket.
    #[error("error de comunicación con Docker: {message}")]
    Io { message: String },
    /// Respuesta que no se pudo interpretar.
    #[error("respuesta no válida de Docker: {message}")]
    InvalidResponse { message: String },
    /// Error al ejecutar un programa del CLI.
    #[error("{message}")]
    Process { message: String },
    /// Un parámetro no superó la validación antes de enviarse a Docker.
    #[error("{message}")]
    InvalidInput { message: String },
    /// La especificación del asistente tiene errores (no se envía nada a Docker).
    #[error("la configuración tiene {} error(es)", issues.iter().filter(|i| i.severity == crate::spec::Severity::Error).count())]
    InvalidSpec { issues: Vec<crate::spec::SpecIssue> },
    /// La configuración tiene riesgos altos o críticos para el equipo que no se
    /// han aceptado explícitamente (E1, D-086). No se envía nada a Docker.
    #[error("la configuración tiene {} riesgo(s) para el equipo sin aceptar", risks.len())]
    RisksNotAccepted { risks: Vec<crate::risk::Risk> },
    /// Ya hay otra operación en curso sobre el mismo recurso.
    #[error("ya hay una operación en curso sobre {resource}")]
    Busy { resource: String },
    /// Falta la confirmación explícita de una operación destructiva.
    #[error("la operación requiere confirmación explícita")]
    NotConfirmed,
    /// El archivo cambió en el disco desde que se leyó (no se sobrescribe).
    #[error("{path} ha cambiado desde que se abrió")]
    FileChanged { path: String },
    /// Ya existe un archivo que no se debe sobrescribir.
    #[error("ya existe {path}")]
    AlreadyExists { path: String },
    /// La ruta no se puede usar como archivo Compose (E2, D-089): no es YAML,
    /// está en una carpeta oculta o del sistema, o es un enlace a una de ellas.
    #[error("{path}: no se permite como archivo Compose ({reason})")]
    PathNotAllowed { path: String, reason: String },
    /// Docker Compose v2+ no está disponible.
    #[error("Docker Compose no está disponible")]
    ComposeUnavailable,
    #[error("{message}")]
    Other { message: String },
}

impl EngineError {
    /// Indica que el problema es de conexión (y conviene volver a comprobarla).
    pub fn is_connection_error(&self) -> bool {
        matches!(self, Self::Io { .. } | Self::Timeout | Self::NotConnected)
    }

    pub fn http_status(&self) -> Option<u16> {
        match self {
            Self::Docker { status, .. } => Some(*status),
            _ => None,
        }
    }
}

impl From<bollard::errors::Error> for EngineError {
    fn from(e: bollard::errors::Error) -> Self {
        use bollard::errors::Error as B;
        match e {
            B::DockerResponseServerError {
                status_code,
                message,
            } => Self::Docker {
                status: status_code,
                message,
            },
            B::RequestTimeoutError => Self::Timeout,
            B::IOError { err } => Self::Io {
                message: err.to_string(),
            },
            B::HyperResponseError { err } => Self::Io {
                message: err.to_string(),
            },
            B::JsonDataError { message, .. } => Self::InvalidResponse { message },
            B::JsonSerdeError { err } => Self::InvalidResponse {
                message: err.to_string(),
            },
            other => Self::Other {
                message: other.to_string(),
            },
        }
    }
}
