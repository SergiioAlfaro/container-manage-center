//! Error serializable que reciben los comandos IPC en el frontend.

use dcc_engine::EngineError;
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
#[error("{message}")]
pub struct CommandError {
    /// Código estable para que la interfaz decida qué mostrar (p. ej. `config.io`,
    /// `engine.docker`, `engine.notConnected`).
    pub code: String,
    /// Mensaje técnico (el de Docker cuando el error viene de Docker).
    pub message: String,
    /// Código HTTP devuelto por Docker, si lo hubo.
    pub http_status: Option<u16>,
}

impl CommandError {
    pub fn new(code: impl Into<String>, message: impl ToString) -> Self {
        Self {
            code: code.into(),
            message: message.to_string(),
            http_status: None,
        }
    }
}

impl From<dcc_store::StoreError> for CommandError {
    fn from(e: dcc_store::StoreError) -> Self {
        Self::new("config.io", e)
    }
}

impl From<EngineError> for CommandError {
    fn from(e: EngineError) -> Self {
        let code = match &e {
            EngineError::NotConnected => "engine.notConnected",
            EngineError::Docker { .. } => "engine.docker",
            EngineError::Timeout => "engine.timeout",
            EngineError::Io { .. } => "engine.io",
            EngineError::InvalidResponse { .. } => "engine.invalidResponse",
            EngineError::Process { .. } => "engine.process",
            EngineError::InvalidInput { .. } => "engine.invalidInput",
            EngineError::Busy { .. } => "engine.busy",
            EngineError::NotConfirmed => "engine.notConfirmed",
            EngineError::Other { .. } => "engine.other",
        };
        let message = match &e {
            EngineError::Docker { message, .. } => message.clone(),
            other => other.to_string(),
        };
        Self {
            code: code.into(),
            message,
            http_status: e.http_status(),
        }
    }
}

pub type CmdResult<T> = Result<T, CommandError>;
