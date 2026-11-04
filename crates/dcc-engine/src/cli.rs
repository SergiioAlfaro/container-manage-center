//! Ejecución de programas del CLI de Docker **sin shell**.
//!
//! Cada argumento se pasa por separado a `execve`; ningún texto del usuario se
//! interpreta como sintaxis de shell. Toda ejecución queda en el registro de
//! actividad como transporte CLI con los argumentos exactos.

use std::time::Duration;

use dcc_activity::{ActivityLog, Category, Failure, OperationSpec, Outcome, Transport};
use tokio::process::Command;

use crate::error::EngineError;

#[derive(Debug, Clone)]
pub struct CliRequest {
    pub category: Category,
    pub kind: String,
    pub target: Option<String>,
    /// Ruta absoluta del ejecutable.
    pub program: String,
    pub args: Vec<String>,
    /// Variables añadidas al entorno heredado (p. ej. `DOCKER_HOST`).
    pub env: Vec<(String, String)>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl CliOutput {
    pub fn success(&self) -> bool {
        self.exit_code == Some(0)
    }
}

/// Ejecuta el programa y registra la operación. Un código de salida distinto de 0
/// se registra como fallo, pero se devuelve igualmente la salida para que el
/// llamador pueda interpretarla.
pub async fn run(log: &ActivityLog, req: CliRequest) -> Result<CliOutput, EngineError> {
    let handle = log.begin(OperationSpec {
        category: req.category,
        kind: req.kind.clone(),
        target: req.target.clone(),
        transport: Transport::Cli {
            program: req.program.clone(),
            args: req.args.clone(),
        },
        request_body: None,
        equivalent_command: None,
    });

    let mut cmd = Command::new(&req.program);
    cmd.args(&req.args)
        .envs(req.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);

    let result = tokio::time::timeout(req.timeout, cmd.output()).await;
    match result {
        Err(_) => {
            handle.fail(Failure {
                error: format!(
                    "El proceso superó el tiempo máximo de {} s",
                    req.timeout.as_secs()
                ),
                ..Default::default()
            });
            Err(EngineError::Timeout)
        }
        Ok(Err(e)) => {
            let msg = format!("No se pudo ejecutar {}: {e}", req.program);
            handle.fail(Failure {
                error: msg.clone(),
                ..Default::default()
            });
            Err(EngineError::Process { message: msg })
        }
        Ok(Ok(out)) => {
            let output = CliOutput {
                exit_code: out.status.code(),
                stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            };
            let nonempty = |s: &str| (!s.is_empty()).then(|| s.to_owned());
            if output.success() {
                handle.succeed(Outcome {
                    stdout: nonempty(&output.stdout),
                    stderr: nonempty(&output.stderr),
                    exit_code: output.exit_code,
                    ..Default::default()
                });
            } else {
                let error = match output.exit_code {
                    Some(c) => format!("El proceso terminó con código {c}"),
                    None => "El proceso terminó por una señal".to_owned(),
                };
                handle.fail(Failure {
                    error,
                    stdout: nonempty(&output.stdout),
                    stderr: nonempty(&output.stderr),
                    exit_code: output.exit_code,
                    ..Default::default()
                });
            }
            Ok(output)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcc_activity::OpStatus;

    fn req(program: &str, args: &[&str]) -> CliRequest {
        CliRequest {
            category: Category::System,
            kind: "test".into(),
            target: None,
            program: program.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            env: vec![],
            timeout: Duration::from_secs(5),
        }
    }

    #[tokio::test]
    async fn arguments_are_not_interpreted_by_a_shell() {
        let log = ActivityLog::new(10);
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("pwned");
        let evil = format!("$(touch {})", marker.display());
        let out = run(&log, req("/bin/echo", &[&evil, ";", "rm", "-rf", "/"]))
            .await
            .unwrap();
        assert!(
            !marker.exists(),
            "la sustitución de comandos no debe ejecutarse"
        );
        assert_eq!(out.stdout.trim(), format!("{evil} ; rm -rf /"));
        let r = &log.snapshot()[0];
        assert_eq!(r.status, OpStatus::Succeeded);
        assert_eq!(r.exit_code, Some(0));
        match &r.transport {
            Transport::Cli { program, args } => {
                assert_eq!(program, "/bin/echo");
                assert_eq!(args[0], evil);
            }
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn non_zero_exit_is_recorded_as_failure_with_stderr() {
        let log = ActivityLog::new(10);
        let out = run(&log, req("/bin/sh", &["-c", "echo fallo >&2; exit 3"]))
            .await
            .unwrap();
        assert_eq!(out.exit_code, Some(3));
        let r = &log.snapshot()[0];
        assert_eq!(r.status, OpStatus::Failed);
        assert_eq!(r.exit_code, Some(3));
        assert_eq!(r.stderr.as_ref().unwrap().text.trim(), "fallo");
    }

    #[tokio::test]
    async fn missing_program_is_a_recorded_error() {
        let log = ActivityLog::new(10);
        let err = run(&log, req("/no/existe", &[])).await.unwrap_err();
        assert!(matches!(err, EngineError::Process { .. }));
        assert_eq!(log.snapshot()[0].status, OpStatus::Failed);
    }

    #[tokio::test]
    async fn timeout_kills_and_records() {
        let log = ActivityLog::new(10);
        let mut r = req("/bin/sleep", &["10"]);
        r.timeout = Duration::from_millis(200);
        assert_eq!(run(&log, r).await.unwrap_err(), EngineError::Timeout);
        assert_eq!(log.snapshot()[0].status, OpStatus::Failed);
    }
}
