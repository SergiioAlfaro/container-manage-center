//! Error serializable que reciben los comandos IPC en el frontend.

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
#[error("{message}")]
pub struct CommandError {
    /// Código estable para que la interfaz decida qué mostrar (p. ej. `config.io`).
    pub code: String,
    /// Mensaje técnico, en el idioma del sistema de origen.
    pub message: String,
}

impl CommandError {
    pub fn new(code: impl Into<String>, message: impl ToString) -> Self {
        Self {
            code: code.into(),
            message: message.to_string(),
        }
    }
}

impl From<dcc_store::StoreError> for CommandError {
    fn from(e: dcc_store::StoreError) -> Self {
        Self::new("config.io", e)
    }
}

pub type CmdResult<T> = Result<T, CommandError>;
