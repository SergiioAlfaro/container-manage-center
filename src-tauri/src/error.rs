//! Error serializable que reciben los comandos IPC en el frontend.

use dcc_engine::EngineError;
use serde::Serialize;
use ts_rs::TS;

/// Error que recibe la interfaz. El mensaje se envía siempre sin secretos
/// (E1, D-085): la ocultación se aplica al serializar, así cubre todas las
/// formas de construirlo.
#[derive(Debug, Clone, TS, thiserror::Error)]
#[ts(export, rename_all = "camelCase")]
#[error("{message}")]
pub struct CommandError {
    /// Código estable para que la interfaz decida qué mostrar (p. ej. `config.io`,
    /// `engine.docker`, `engine.notConnected`).
    pub code: String,
    /// Mensaje técnico (el de Docker cuando el error viene de Docker).
    pub message: String,
    /// Código HTTP devuelto por Docker, si lo hubo.
    pub http_status: Option<u16>,
    /// Problemas de validación de una especificación (`engine.invalidSpec`).
    pub issues: Option<Vec<dcc_engine::spec::SpecIssue>>,
}

impl Serialize for CommandError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Shown<'a> {
            code: &'a str,
            message: std::borrow::Cow<'a, str>,
            http_status: Option<u16>,
            issues: &'a Option<Vec<dcc_engine::spec::SpecIssue>>,
        }
        Shown {
            code: &self.code,
            message: dcc_activity::redact::redact_text(&self.message),
            http_status: self.http_status,
            issues: &self.issues,
        }
        .serialize(s)
    }
}

impl CommandError {
    pub fn new(code: impl Into<String>, message: impl ToString) -> Self {
        Self {
            code: code.into(),
            message: message.to_string(),
            http_status: None,
            issues: None,
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
            EngineError::FileChanged { .. } => "engine.fileChanged",
            EngineError::AlreadyExists { .. } => "engine.alreadyExists",
            EngineError::PathNotAllowed { .. } => "engine.pathNotAllowed",
            EngineError::ComposeUnavailable => "engine.composeUnavailable",
            EngineError::InvalidSpec { .. } => "engine.invalidSpec",
            EngineError::RisksNotAccepted { .. } => "engine.risksNotAccepted",
            EngineError::Other { .. } => "engine.other",
        };
        let message = match &e {
            EngineError::Docker { message, .. } => message.clone(),
            other => other.to_string(),
        };
        let issues = match &e {
            EngineError::InvalidSpec { issues } => Some(issues.clone()),
            // Los riesgos sin aceptar viajan como avisos para mostrarlos.
            EngineError::RisksNotAccepted { risks } => Some(
                risks
                    .iter()
                    .map(|r| dcc_engine::spec::SpecIssue {
                        severity: dcc_engine::spec::Severity::Warning,
                        field: r.field.clone(),
                        code: r.code.clone(),
                        params: r.params.clone(),
                    })
                    .collect(),
            ),
            _ => None,
        };
        Self {
            code: code.into(),
            message,
            http_status: e.http_status(),
            issues,
        }
    }
}

pub type CmdResult<T> = Result<T, CommandError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_reach_the_interface_without_secrets() {
        const V: &str = "valor-ficticio-err";
        let e = CommandError::from(EngineError::Docker {
            status: 500,
            message: format!("pull https://u:{V}@reg.example/v2/ failed: MYSQL_PWD={V}"),
        });
        let json = serde_json::to_string(&e).unwrap();
        assert!(!json.contains(V), "{json}");
        assert!(json.contains("\"httpStatus\":500"));
        assert!(json.contains("reg.example/v2/"));
        let e = CommandError::new("x", format!("--password {V}"));
        assert!(!serde_json::to_string(&e).unwrap().contains(V));
    }
}
