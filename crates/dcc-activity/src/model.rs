//! Modelo de datos de una operación registrada.
//!
//! Regla de oro del proyecto: un `OperationRecord` describe algo que **ocurrió
//! de verdad**. `transport` es la operación realmente ejecutada (petición a la
//! Docker Engine API o proceso CLI lanzado). `equivalent_command` es solo una
//! traducción orientativa a la sintaxis del CLI `docker` y se presenta siempre
//! como "equivalente" cuando el transporte es la API.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Área funcional de la operación. Se usa para filtrar en el panel de actividad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Category {
    System,
    Container,
    Image,
    Network,
    Volume,
    Bind,
    Compose,
}

/// Cómo se ejecutó realmente la operación.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "via", rename_all = "camelCase")]
#[ts(export)]
pub enum Transport {
    /// Petición HTTP a la Docker Engine API a través del socket Unix.
    Api {
        method: String,
        /// Ruta de la petición, incluida la query string (p. ej. `/v1.47/containers/json?all=true`).
        path: String,
        /// Socket usado (p. ej. `/var/run/docker.sock`).
        endpoint: String,
    },
    /// Proceso lanzado sin shell; `args` son los argumentos exactos, uno por elemento.
    Cli { program: String, args: Vec<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum OpStatus {
    Running,
    Succeeded,
    Failed,
    /// La operación se abandonó antes de terminar (p. ej. tarea cancelada).
    Cancelled,
}

/// Texto capturado con indicación de si se ha recortado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CapturedText {
    pub text: String,
    pub truncated: bool,
    /// Tamaño original en bytes antes de recortar.
    #[ts(type = "number")]
    pub original_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OperationRecord {
    pub id: Uuid,
    pub category: Category,
    /// Identificador estable del tipo de operación, p. ej. `container.start`.
    pub kind: String,
    /// Recurso afectado (nombre o ID), si aplica.
    pub target: Option<String>,
    pub transport: Transport,
    /// Cuerpo de la petición API (JSON) o `None`.
    pub request_body: Option<CapturedText>,
    /// Comando `docker` equivalente, ya entrecomillado para copiar y pegar.
    /// `None` si la operación no tiene un equivalente CLI fiel.
    pub equivalent_command: Option<String>,
    pub status: OpStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    #[ts(type = "number | null")]
    pub duration_ms: Option<u64>,
    /// Resumen legible del resultado (p. ej. "Contenedor iniciado").
    pub summary: Option<String>,
    pub stdout: Option<CapturedText>,
    pub stderr: Option<CapturedText>,
    /// Mensaje de error devuelto por Docker o por el sistema.
    pub error: Option<String>,
    pub http_status: Option<u16>,
    pub exit_code: Option<i32>,
}

/// Evento emitido a los suscriptores (la interfaz) cuando cambia el registro.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum ActivityEvent {
    /// Un registro nuevo o actualizado (inicio o finalización).
    Upsert { record: Box<OperationRecord> },
    /// El historial visual se ha vaciado.
    Cleared,
}
