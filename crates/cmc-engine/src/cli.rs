// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Ejecución de programas del CLI de Docker **sin shell**.
//!
//! Cada argumento se pasa por separado a `execve`; ningún texto del usuario se
//! interpreta como sintaxis de shell. Toda ejecución queda en el registro de
//! actividad como transporte CLI con los argumentos exactos.

use std::time::Duration;

use cmc_activity::{ActivityLog, Category, Failure, OperationSpec, Outcome, Transport};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

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
    /// Directorio de trabajo del proceso (`None`: el de la aplicación).
    pub cwd: Option<std::path::PathBuf>,
    pub timeout: Duration,
    /// Ejecutar en un servidor remoto por su sesión SSH (0.10.0, D-080):
    /// `program` y `args` se ejecutan allí, con `env` como variables del
    /// proceso remoto (por `env`, sin shell propio); `cwd` no se usa.
    pub remote: Option<RemoteExec>,
}

/// Ejecución remota de una orden del CLI (0.10.0).
#[derive(Clone)]
pub struct RemoteExec {
    pub session: std::sync::Arc<crate::ssh::SshSession>,
    /// Con terminal remota: al cancelar (cerrar el canal) el servidor envía
    /// SIGHUP al proceso, que termina; sin ella seguiría hasta escribir. Solo
    /// para órdenes sin entrada estándar.
    pub tty: bool,
}

impl std::fmt::Debug for RemoteExec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteExec")
            .field("server", &self.session.profile().id)
            .field("tty", &self.tty)
            .finish()
    }
}

impl CliRequest {
    /// Argumentos que se ejecutan en el servidor: `[env K=V…] programa args…`.
    pub fn remote_argv(&self) -> Vec<String> {
        let mut v = Vec::new();
        if !self.env.is_empty() {
            v.push("env".to_owned());
            v.extend(self.env.iter().map(|(k, val)| format!("{k}={val}")));
        }
        v.push(self.program.clone());
        v.extend(self.args.iter().cloned());
        v
    }

    /// Lo que se registra en actividad: la orden local, o en remoto la orden
    /// `ssh` equivalente (sin las opciones de seguridad internas).
    fn transport(&self) -> Transport {
        match &self.remote {
            None => Transport::Cli {
                program: self.program.clone(),
                args: self.args.clone(),
            },
            Some(r) => {
                let p = r.session.profile();
                let mut args = Vec::new();
                if p.port != 22 {
                    args.push("-p".to_owned());
                    args.push(p.port.to_string());
                }
                if !p.options.proxy_jump.is_empty() {
                    args.push("-J".to_owned());
                    args.push(p.options.proxy_jump.clone());
                }
                args.push(format!("{}@{}", p.user, p.host));
                args.push("--".to_owned());
                args.extend(self.remote_argv());
                Transport::Cli {
                    program: "ssh".into(),
                    args,
                }
            }
        }
    }

    fn command(&self) -> Command {
        match &self.remote {
            None => {
                let mut cmd = Command::new(&self.program);
                cmd.args(&self.args)
                    .envs(self.env.iter().map(|(k, v)| (k.as_str(), v.as_str())));
                if let Some(d) = &self.cwd {
                    cmd.current_dir(d);
                }
                cmd
            }
            Some(r) => r.session.channel(&self.remote_argv(), r.tty),
        }
    }
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
        transport: req.transport(),
        request_body: None,
        equivalent_command: None,
    });

    let mut cmd = req.command();
    cmd.stdin(std::process::Stdio::null()).kill_on_drop(true);

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

// ------------------------------------------------------------------ salida en vivo

/// Flujo de salida de un proceso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OutputLine {
    pub stream: OutputStream,
    pub text: String,
}

/// Opciones de una ejecución con salida en vivo.
pub struct StreamOptions {
    /// Datos para la entrada estándar (p. ej. un archivo Compose sin guardar).
    pub stdin: Option<Vec<u8>>,
    /// Guardar la salida estándar en el registro de actividad. `false` cuando
    /// puede contener secretos (configuración resuelta) o es muy grande (logs).
    pub record_stdout: bool,
    /// Devolver la salida estándar completa al llamador (p. ej. JSON).
    pub collect_stdout: bool,
    /// Que el cierre por el usuario sea un final normal (logs) y no una cancelación.
    pub close_is_success: bool,
    /// Resumen para el registro si termina con código 0.
    pub summary: Option<String>,
    pub cancel: CancellationToken,
}

impl StreamOptions {
    pub fn new(cancel: CancellationToken) -> Self {
        Self {
            stdin: None,
            record_stdout: true,
            collect_stdout: false,
            close_is_success: false,
            summary: None,
            cancel,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "end", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum RunEnd {
    /// El proceso terminó por sí mismo.
    Exited { code: Option<i32> },
    /// Lo detuvo el usuario (o la ventana dejó de escuchar).
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamedRun {
    pub end: RunEnd,
    /// Salida estándar completa si se pidió `collect_stdout`.
    pub stdout: String,
    /// Final de la salida de errores (para mensajes).
    pub stderr_tail: String,
    pub lines: u64,
}

impl StreamedRun {
    pub fn success(&self) -> bool {
        self.end == RunEnd::Exited { code: Some(0) }
    }
}

const FLUSH_EVERY: Duration = Duration::from_millis(80);
const FLUSH_LINES: usize = 200;
/// Lo que se conserva de cada flujo para el registro de actividad.
const KEEP_BYTES: usize = 256 * 1024;
/// Tiempo de gracia tras SIGTERM antes de SIGKILL.
const GRACE: Duration = Duration::from_secs(8);

/// Conserva solo el final de un texto que crece.
#[derive(Default)]
struct Tail {
    text: String,
    dropped: bool,
}

impl Tail {
    fn push(&mut self, line: &str) {
        self.text.push_str(line);
        self.text.push('\n');
        if self.text.len() > KEEP_BYTES * 2 {
            let mut cut = self.text.len() - KEEP_BYTES;
            while !self.text.is_char_boundary(cut) {
                cut += 1;
            }
            self.text.drain(..cut);
            self.dropped = true;
        }
    }
    fn finish(self) -> Option<String> {
        match (self.text.is_empty(), self.dropped) {
            (true, _) => None,
            (false, false) => Some(self.text),
            (false, true) => Some(format!("[…]\n{}", self.text)),
        }
    }
}

fn signal_group(pid: Option<u32>, sig: i32) {
    if let Some(pid) = pid {
        // El proceso es líder de su propio grupo: se avisa también a sus hijos
        // (`docker compose` lanza el plugin como proceso hijo).
        unsafe {
            libc::kill(-(pid as i32), sig);
        }
    }
}

/// Ejecuta un programa sin shell enviando su salida, línea a línea, a `on_lines`
/// (en lotes). Queda en el registro de actividad como **una** operación que se
/// cierra al terminar. Si `on_lines` devuelve `false` o se cancela `cancel`, se
/// envía SIGTERM al grupo del proceso y, si no termina, SIGKILL.
pub async fn run_streaming<F>(
    log: &ActivityLog,
    req: CliRequest,
    opts: StreamOptions,
    mut on_lines: F,
) -> Result<StreamedRun, EngineError>
where
    F: FnMut(Vec<OutputLine>) -> bool,
{
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let handle = log.begin(OperationSpec {
        category: req.category,
        kind: req.kind.clone(),
        target: req.target.clone(),
        transport: req.transport(),
        request_body: None,
        equivalent_command: None,
    });

    let mut cmd = req.command();
    cmd.stdin(if opts.stdin.is_some() {
        std::process::Stdio::piped()
    } else {
        std::process::Stdio::null()
    })
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .process_group(0)
    .kill_on_drop(true);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("No se pudo ejecutar {}: {e}", req.program);
            handle.fail(Failure {
                error: msg.clone(),
                ..Default::default()
            });
            return Err(EngineError::Process { message: msg });
        }
    };
    let pid = child.id();
    if let (Some(data), Some(mut stdin)) = (opts.stdin, child.stdin.take()) {
        tokio::spawn(async move {
            let _ = stdin.write_all(&data).await;
            let _ = stdin.shutdown().await;
        });
    }
    let mut out = BufReader::new(child.stdout.take().expect("stdout")).split(b'\n');
    let mut err = BufReader::new(child.stderr.take().expect("stderr")).split(b'\n');

    let deadline = tokio::time::sleep(req.timeout);
    tokio::pin!(deadline);
    let mut tick = tokio::time::interval(FLUSH_EVERY);
    let mut batch: Vec<OutputLine> = Vec::new();
    let mut collected = String::new();
    let (mut out_tail, mut err_tail) = (Tail::default(), Tail::default());
    let (mut out_open, mut err_open) = (true, true);
    let mut stopped = false;
    let mut timed_out = false;
    let mut listener_gone = false;
    let mut lines: u64 = 0;
    let mut last_flush = std::time::Instant::now();

    let decode = |raw: Vec<u8>| -> String {
        let mut s = String::from_utf8_lossy(&raw).into_owned();
        if s.ends_with('\r') {
            s.pop();
        }
        s
    };

    while out_open || err_open {
        tokio::select! {
            r = out.next_segment(), if out_open => match r {
                Ok(Some(raw)) => {
                    let text = decode(raw);
                    if opts.collect_stdout {
                        collected.push_str(&text);
                        collected.push('\n');
                    }
                    if opts.record_stdout {
                        out_tail.push(&text);
                    }
                    lines += 1;
                    batch.push(OutputLine { stream: OutputStream::Stdout, text });
                }
                _ => out_open = false,
            },
            r = err.next_segment(), if err_open => match r {
                Ok(Some(raw)) => {
                    let text = decode(raw);
                    err_tail.push(&text);
                    lines += 1;
                    batch.push(OutputLine { stream: OutputStream::Stderr, text });
                }
                _ => err_open = false,
            },
            _ = tick.tick() => {}
            _ = opts.cancel.cancelled(), if !stopped => {
                stopped = true;
                signal_group(pid, libc::SIGTERM);
            }
            _ = &mut deadline, if !stopped => {
                stopped = true;
                timed_out = true;
                signal_group(pid, libc::SIGTERM);
            }
        }
        if !batch.is_empty()
            && (batch.len() >= FLUSH_LINES
                || last_flush.elapsed() >= FLUSH_EVERY
                || !out_open
                || !err_open
                || stopped)
        {
            last_flush = std::time::Instant::now();
            if !listener_gone && !on_lines(std::mem::take(&mut batch)) {
                listener_gone = true;
                if !stopped {
                    stopped = true;
                    signal_group(pid, libc::SIGTERM);
                }
            }
            batch.clear();
        }
    }
    if !batch.is_empty() && !listener_gone {
        on_lines(batch);
    }

    let status = match tokio::time::timeout(GRACE, child.wait()).await {
        Ok(s) => s.ok(),
        Err(_) => {
            signal_group(pid, libc::SIGKILL);
            child.wait().await.ok()
        }
    };
    let code = status.and_then(|s| s.code());
    let stdout_rec = out_tail.finish();
    let stderr_tail = err_tail.text.clone();
    let stderr_rec = err_tail.finish();

    if timed_out {
        handle.fail(Failure {
            error: format!(
                "El proceso superó el tiempo máximo de {} s",
                req.timeout.as_secs()
            ),
            stdout: stdout_rec,
            stderr: stderr_rec,
            exit_code: code,
            ..Default::default()
        });
        return Err(EngineError::Timeout);
    }
    let end = if stopped {
        RunEnd::Stopped
    } else {
        RunEnd::Exited { code }
    };
    match end {
        RunEnd::Stopped if opts.close_is_success => handle.succeed(Outcome {
            summary: Some(format!("Cerrado por el usuario ({lines} líneas)")),
            stdout: stdout_rec,
            stderr: stderr_rec,
            exit_code: code,
            ..Default::default()
        }),
        RunEnd::Stopped => handle.cancel(Failure {
            error: "Detenido por el usuario antes de terminar".into(),
            stdout: stdout_rec,
            stderr: stderr_rec,
            exit_code: code,
            ..Default::default()
        }),
        RunEnd::Exited { code: Some(0) } => handle.succeed(Outcome {
            summary: opts.summary,
            stdout: stdout_rec,
            stderr: stderr_rec,
            exit_code: code,
            ..Default::default()
        }),
        RunEnd::Exited { code } => handle.fail(Failure {
            error: match code {
                Some(c) => format!("El proceso terminó con código {c}"),
                None => "El proceso terminó por una señal".to_owned(),
            },
            stdout: stdout_rec,
            stderr: stderr_rec,
            exit_code: code,
            ..Default::default()
        }),
    }
    Ok(StreamedRun {
        end,
        stdout: collected,
        stderr_tail,
        lines,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cmc_activity::OpStatus;

    fn req(program: &str, args: &[&str]) -> CliRequest {
        CliRequest {
            category: Category::System,
            kind: "test".into(),
            target: None,
            program: program.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            env: vec![],
            cwd: None,
            timeout: Duration::from_secs(5),
            remote: None,
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

    fn sreq(args: &[&str]) -> CliRequest {
        let mut r = req("/bin/sh", args);
        r.timeout = Duration::from_secs(20);
        r
    }

    #[tokio::test]
    async fn streaming_delivers_both_streams_in_order_and_records_once() {
        let log = ActivityLog::new(10);
        let mut got = Vec::new();
        let mut opts = StreamOptions::new(CancellationToken::new());
        opts.collect_stdout = true;
        opts.summary = Some("hecho".into());
        let r = run_streaming(
            &log,
            sreq(&[
                "-c",
                "echo uno; echo dos >&2; printf 'tres\\r\\n'; echo cuatro",
            ]),
            opts,
            |l| {
                got.extend(l);
                true
            },
        )
        .await
        .unwrap();
        assert!(r.success());
        assert_eq!(r.stdout, "uno\ntres\ncuatro\n", "el \\r final se elimina");
        assert_eq!(r.lines, 4);
        let out: Vec<_> = got
            .iter()
            .filter(|l| l.stream == OutputStream::Stdout)
            .map(|l| l.text.as_str())
            .collect();
        assert_eq!(out, ["uno", "tres", "cuatro"]);
        assert!(got
            .iter()
            .any(|l| l.stream == OutputStream::Stderr && l.text == "dos"));
        let recs = log.snapshot();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].status, OpStatus::Succeeded);
        assert_eq!(recs[0].summary.as_deref(), Some("hecho"));
        assert_eq!(recs[0].stderr.as_ref().unwrap().text, "dos\n");
    }

    #[tokio::test]
    async fn streaming_feeds_stdin_and_can_keep_stdout_out_of_the_log() {
        let log = ActivityLog::new(10);
        let mut opts = StreamOptions::new(CancellationToken::new());
        opts.stdin = Some(b"secreto=1\n".to_vec());
        opts.collect_stdout = true;
        opts.record_stdout = false;
        let r = run_streaming(&log, req("/bin/cat", &[]), opts, |_| true)
            .await
            .unwrap();
        assert_eq!(r.stdout, "secreto=1\n");
        assert!(
            log.snapshot()[0].stdout.is_none(),
            "la salida no se guarda en el registro"
        );
    }

    #[tokio::test]
    async fn non_zero_exit_is_a_recorded_failure() {
        let log = ActivityLog::new(10);
        let r = run_streaming(
            &log,
            sreq(&["-c", "echo malo >&2; exit 15"]),
            StreamOptions::new(CancellationToken::new()),
            |_| true,
        )
        .await
        .unwrap();
        assert_eq!(r.end, RunEnd::Exited { code: Some(15) });
        assert_eq!(r.stderr_tail, "malo\n");
        let rec = &log.snapshot()[0];
        assert_eq!(rec.status, OpStatus::Failed);
        assert_eq!(rec.exit_code, Some(15));
    }

    #[tokio::test]
    async fn cancelling_stops_the_whole_process_group() {
        let log = ActivityLog::new(10);
        let dir = tempfile::tempdir().unwrap();
        let pidfile = dir.path().join("hijo");
        let cancel = CancellationToken::new();
        let c2 = cancel.clone();
        let script = format!(
            "sleep 300 & echo $! > {}; echo listo; wait",
            pidfile.display()
        );
        let started = std::time::Instant::now();
        let r = run_streaming(
            &log,
            sreq(&["-c", &script]),
            StreamOptions::new(cancel),
            move |l| {
                if l.iter().any(|x| x.text == "listo") {
                    c2.cancel();
                }
                true
            },
        )
        .await
        .unwrap();
        assert_eq!(r.end, RunEnd::Stopped);
        assert!(started.elapsed() < Duration::from_secs(15));
        let child: i32 = std::fs::read_to_string(&pidfile)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        // El hijo (sleep) también ha recibido la señal.
        let mut alive = true;
        for _ in 0..50 {
            alive = unsafe { libc::kill(child, 0) } == 0
                && !std::fs::read_to_string(format!("/proc/{child}/stat"))
                    .unwrap_or_default()
                    .contains(") Z ");
            if !alive {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(!alive, "el proceso hijo sigue vivo");
        let rec = &log.snapshot()[0];
        assert_eq!(rec.status, OpStatus::Cancelled);
        assert!(
            rec.stdout.as_ref().unwrap().text.contains("listo"),
            "se conserva la salida"
        );
    }

    #[tokio::test]
    async fn listener_gone_closes_and_close_can_be_a_normal_end() {
        let log = ActivityLog::new(10);
        let mut opts = StreamOptions::new(CancellationToken::new());
        opts.close_is_success = true;
        let r = run_streaming(
            &log,
            sreq(&["-c", "while true; do echo x; sleep 0.05; done"]),
            opts,
            |_| false,
        )
        .await
        .unwrap();
        assert_eq!(r.end, RunEnd::Stopped);
        let rec = &log.snapshot()[0];
        assert_eq!(rec.status, OpStatus::Succeeded);
        assert!(rec
            .summary
            .as_ref()
            .unwrap()
            .starts_with("Cerrado por el usuario"));
    }

    #[tokio::test]
    async fn streaming_timeout_is_an_error() {
        let log = ActivityLog::new(10);
        let mut r = req("/bin/sleep", &["30"]);
        r.timeout = Duration::from_millis(300);
        let e = run_streaming(
            &log,
            r,
            StreamOptions::new(CancellationToken::new()),
            |_| true,
        )
        .await
        .unwrap_err();
        assert_eq!(e, EngineError::Timeout);
        assert_eq!(log.snapshot()[0].status, OpStatus::Failed);
    }
}
