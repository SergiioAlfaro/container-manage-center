//! Flujos de larga duración: logs, estadísticas y terminal (D-036).
//!
//! Cada flujo es **una** petición HTTP real que permanece abierta, y queda en el
//! panel de actividad como una sola operación "en curso" que se cierra con un
//! resumen al terminar: cerrada por el usuario, finalizada por Docker (el
//! contenedor se detuvo o el proceso terminó) o fallida.

use std::time::Duration;

use bollard::container::LogOutput;
use bollard::exec::StartExecResults;
use bollard::models::{ContainerStatsResponse, ExecConfig};
use bollard::query_parameters::{LogsOptions, ResizeExecOptions, StatsOptions};
use bytes::Bytes;
use dcc_activity::{quote::render_argv, Category, Failure, OperationSpec, Outcome, Transport};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::client::{ApiOp, Engine};
use crate::error::EngineError;
use crate::resources::{path_with_query, validate_ref};

/// Cómo terminó un flujo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum EndReason {
    /// Lo cerró el usuario (o la ventana dejó de escuchar).
    Closed,
    /// Docker terminó el flujo: el contenedor se detuvo o el proceso acabó.
    Finished,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StreamEnd {
    pub reason: EndReason,
    pub summary: String,
    #[ts(type = "number | null")]
    pub exit_code: Option<i64>,
    /// Terminal: el contenedor ya no estaba en ejecución al terminar la sesión
    /// (se detuvo o se eliminó). `None` en los demás casos.
    #[serde(default)]
    pub container: Option<ContainerAfter>,
}

/// Qué le pasó al contenedor cuando una sesión de terminal terminó sola.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ContainerAfter {
    Stopped,
    Removed,
}

/// Intervalo máximo entre envíos de líneas de log a la interfaz.
const LOG_FLUSH: Duration = Duration::from_millis(60);
const LOG_BATCH: usize = 500;

fn human_bytes(b: u64) -> String {
    match b {
        b if b >= 1 << 20 => format!("{:.1} MiB", b as f64 / (1u64 << 20) as f64),
        b if b >= 1 << 10 => format!("{:.1} KiB", b as f64 / 1024.0),
        b => format!("{b} B"),
    }
}

// ------------------------------------------------------------------ logs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LogStream {
    Stdout,
    Stderr,
    /// Contenedor con TTY: salida única sin separar.
    Console,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LogLine {
    pub stream: LogStream,
    /// Marca de tiempo de Docker (RFC 3339 con nanosegundos).
    pub ts: Option<String>,
    pub text: String,
}

/// Ensambla líneas completas a partir de los fragmentos de Docker, que pueden
/// cortar una línea (o un carácter UTF-8) en cualquier punto.
#[derive(Default)]
pub struct LineAssembler {
    out: Vec<u8>,
    err: Vec<u8>,
    console: Vec<u8>,
}

fn split_ts(line: &str) -> (Option<String>, String) {
    // "2026-11-05T17:00:00.123456789Z texto"
    if let Some((first, rest)) = line.split_once(' ') {
        let b = first.as_bytes();
        if b.len() >= 20 && b[4] == b'-' && b[10] == b'T' && first.ends_with('Z') {
            return (Some(first.to_owned()), rest.to_owned());
        }
    } else {
        let b = line.as_bytes();
        if b.len() >= 20 && b[4] == b'-' && b[10] == b'T' && line.ends_with('Z') {
            return (Some(line.to_owned()), String::new());
        }
    }
    (None, line.to_owned())
}

impl LineAssembler {
    pub fn push(&mut self, kind: LogStream, data: &[u8], out: &mut Vec<LogLine>) {
        let buf = match kind {
            LogStream::Stdout => &mut self.out,
            LogStream::Stderr => &mut self.err,
            LogStream::Console => &mut self.console,
        };
        buf.extend_from_slice(data);
        while let Some(pos) = buf.iter().position(|&c| c == b'\n') {
            let mut line: Vec<u8> = buf.drain(..=pos).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let (ts, text) = split_ts(&String::from_utf8_lossy(&line));
            out.push(LogLine {
                stream: kind,
                ts,
                text,
            });
        }
    }

    /// Emite las líneas sin salto final pendientes (al terminar el flujo).
    pub fn finish(&mut self, out: &mut Vec<LogLine>) {
        for (kind, buf) in [
            (LogStream::Stdout, &mut self.out),
            (LogStream::Stderr, &mut self.err),
            (LogStream::Console, &mut self.console),
        ] {
            if !buf.is_empty() {
                let (ts, text) = split_ts(&String::from_utf8_lossy(buf));
                out.push(LogLine {
                    stream: kind,
                    ts,
                    text,
                });
                buf.clear();
            }
        }
    }
}

// ------------------------------------------------------------------ estadísticas

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StatsSample {
    /// Porcentaje sobre una CPU (puede superar 100 con varias CPU), como `docker stats`.
    pub cpu_percent: Option<f64>,
    pub online_cpus: Option<u32>,
    #[ts(type = "number")]
    pub memory_used: u64,
    #[ts(type = "number")]
    pub memory_limit: u64,
    pub memory_percent: Option<f64>,
    #[ts(type = "number")]
    pub net_rx: u64,
    #[ts(type = "number")]
    pub net_tx: u64,
    #[ts(type = "number")]
    pub block_read: u64,
    #[ts(type = "number")]
    pub block_write: u64,
    #[ts(type = "number | null")]
    pub pids: Option<u64>,
    pub read: Option<String>,
}

/// Mismos cálculos que `docker stats` (CLI de Docker, `calculateCPUPercentUnix`
/// y `calculateMemUsageUnixNoCache`).
pub fn sample_from(s: &ContainerStatsResponse) -> StatsSample {
    let cpu = s.cpu_stats.clone().unwrap_or_default();
    let pre = s.precpu_stats.clone().unwrap_or_default();
    let total = cpu
        .cpu_usage
        .as_ref()
        .and_then(|u| u.total_usage)
        .unwrap_or(0);
    let pre_total = pre
        .cpu_usage
        .as_ref()
        .and_then(|u| u.total_usage)
        .unwrap_or(0);
    let sys = cpu.system_cpu_usage.unwrap_or(0);
    let pre_sys = pre.system_cpu_usage.unwrap_or(0);
    let online = cpu.online_cpus.filter(|n| *n > 0).or_else(|| {
        cpu.cpu_usage
            .as_ref()
            .and_then(|u| u.percpu_usage.as_ref())
            .map(|p| p.len() as u32)
            .filter(|n| *n > 0)
    });
    let cpu_delta = total.saturating_sub(pre_total) as f64;
    let sys_delta = sys.saturating_sub(pre_sys) as f64;
    let cpu_percent = if pre_sys > 0 && sys_delta > 0.0 && cpu_delta >= 0.0 {
        Some(cpu_delta / sys_delta * online.unwrap_or(1) as f64 * 100.0)
    } else {
        None
    };

    let mem = s.memory_stats.clone().unwrap_or_default();
    let usage = mem.usage.unwrap_or(0);
    let stats = mem.stats.clone().unwrap_or_default();
    let used = if let Some(v) = stats.get("total_inactive_file").filter(|v| **v < usage) {
        usage - v
    } else if let Some(v) = stats.get("inactive_file").filter(|v| **v < usage) {
        usage - v
    } else {
        usage
    };
    let limit = mem.limit.unwrap_or(0);

    let (mut rx, mut tx) = (0u64, 0u64);
    for n in s.networks.clone().unwrap_or_default().values() {
        rx += n.rx_bytes.unwrap_or(0);
        tx += n.tx_bytes.unwrap_or(0);
    }
    let (mut br, mut bw) = (0u64, 0u64);
    if let Some(entries) = s
        .blkio_stats
        .as_ref()
        .and_then(|b| b.io_service_bytes_recursive.as_ref())
    {
        for e in entries {
            match e.op.as_deref().map(str::to_ascii_lowercase).as_deref() {
                Some("read") => br += e.value.unwrap_or(0),
                Some("write") => bw += e.value.unwrap_or(0),
                _ => {}
            }
        }
    }
    StatsSample {
        cpu_percent,
        online_cpus: online,
        memory_used: used,
        memory_limit: limit,
        memory_percent: (limit > 0 && usage > 0).then(|| used as f64 / limit as f64 * 100.0),
        net_rx: rx,
        net_tx: tx,
        block_read: br,
        block_write: bw,
        pids: s.pids_stats.as_ref().and_then(|p| p.current),
        read: s.read.as_ref().map(|r| r.to_string()),
    }
}

// ------------------------------------------------------------------ terminal

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalInput {
    Data(Vec<u8>),
    Resize { cols: u16, rows: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ShellChoice {
    /// bash si existe; si no, sh.
    Auto,
    Bash,
    Sh,
}

/// Secuencia que la shell escribe al empezar con su PID dentro del contenedor
/// (`ESC ] 6973 ; PID BEL`). Es una secuencia OSC que las terminales ignoran;
/// el motor la retira de la salida. Con ese PID, que es el de la sesión de la
/// TTY, se pueden terminar todos los procesos de la sesión si la shell no sale
/// al cerrar la terminal (D-063).
pub const PID_MARK_START: &[u8] = b"\x1b]6973;";
const PID_MARK_END: u8 = 0x07;
/// Hasta dónde se busca la marca al principio de la salida.
const PID_MARK_WINDOW: usize = 4096;

impl ShellChoice {
    pub fn argv(self) -> Vec<String> {
        // `exec` mantiene el PID: el anunciado es el de la shell interactiva.
        let announce = r#"printf '\033]6973;%s\007' "$$"; "#;
        let (interp, then) = match self {
            Self::Auto => (
                "/bin/sh",
                "if [ -x /bin/bash ]; then exec /bin/bash; else exec /bin/sh; fi",
            ),
            Self::Bash => ("/bin/bash", "exec /bin/bash"),
            Self::Sh => ("/bin/sh", "exec /bin/sh"),
        };
        vec![interp.into(), "-c".into(), format!("{announce}{then}")]
    }
}

/// Retira la marca de PID de la salida de la terminal. Suele llegar la primera,
/// pero lo que se teclea antes de que la shell arranque aparece antes (eco de
/// la TTY): se busca en los primeros `PID_MARK_WINDOW` bytes.
#[derive(Debug, Default)]
pub struct PidMark {
    buf: Vec<u8>,
    seen: usize,
    done: bool,
    pub pid: Option<u32>,
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

impl PidMark {
    /// Devuelve lo que hay que mostrar (sin la marca). Retiene solo lo que
    /// podría ser el principio de una marca partida entre dos trozos.
    pub fn feed(&mut self, data: &[u8]) -> Vec<u8> {
        if self.done {
            return data.to_vec();
        }
        self.buf.extend_from_slice(data);
        if let Some(pos) = find(&self.buf, PID_MARK_START) {
            let body = pos + PID_MARK_START.len();
            if let Some(rel) = self.buf[body..].iter().position(|&c| c == PID_MARK_END) {
                let end = body + rel;
                self.pid = std::str::from_utf8(&self.buf[body..end])
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .filter(|&p: &u32| p > 0);
                self.done = true;
                let mut out = self.buf[..pos].to_vec();
                out.extend_from_slice(&self.buf[end + 1..]);
                self.buf.clear();
                return out;
            }
            if self.buf.len() - body > 12 {
                // Empieza como la marca pero no lo es: no se toca nada.
                self.done = true;
                return std::mem::take(&mut self.buf);
            }
            let out = self.buf[..pos].to_vec();
            self.seen += pos;
            self.buf.drain(..pos);
            return out;
        }
        // Sin marca todavía: se retiene el final si puede ser su principio.
        let keep = (1..PID_MARK_START.len())
            .rev()
            .find(|&k| self.buf.len() >= k && self.buf.ends_with(&PID_MARK_START[..k]))
            .unwrap_or(0);
        let cut = self.buf.len() - keep;
        let out: Vec<u8> = self.buf.drain(..cut).collect();
        self.seen += out.len();
        if self.seen > PID_MARK_WINDOW {
            self.done = true;
            let mut all = out;
            all.append(&mut self.buf);
            return all;
        }
        out
    }

    /// Lo retenido, si el flujo termina antes de completar la marca.
    pub fn flush(&mut self) -> Vec<u8> {
        self.done = true;
        std::mem::take(&mut self.buf)
    }
}

/// Termina, dentro del contenedor, todos los procesos de la sesión `$1` (la de
/// la TTY de la terminal). Solo usa órdenes internas de `sh` (dash, bash,
/// busybox): lee el campo «sesión» de /proc/PID/stat.
/// Primero los demás procesos y al final la shell: si la shell (líder de la
/// sesión) muriera antes, el núcleo enviaría SIGHUP al resto y el recuento
/// dependería de quién llega antes.
pub const KILL_SESSION_SCRIPT: &str = r#"s=$1; n=0
for d in /proc/[0-9]*; do
  p=${d#/proc/}; [ "$p" = "$s" ] && continue
  st=; read -r st < "$d/stat" 2>/dev/null || continue
  rest=${st##*) }; set -- $rest
  if [ "$1" != Z ] && [ "$4" = "$s" ]; then kill -KILL "$p" 2>/dev/null && n=$((n+1)); fi
done
st=; if read -r st < "/proc/$s/stat" 2>/dev/null; then
  rest=${st##*) }; set -- $rest
  if [ "$1" != Z ] && [ "$4" = "$s" ]; then kill -KILL "$s" 2>/dev/null && n=$((n+1)); fi
fi
echo "$n"
"#;

impl Engine {
    /// Sigue los logs de un contenedor (`docker logs --follow --timestamps`).
    /// `sink` recibe lotes de líneas; si devuelve `false`, el flujo se cierra.
    pub async fn stream_logs<F>(
        &self,
        id: &str,
        display: Option<&str>,
        tail: Option<u32>,
        cancel: CancellationToken,
        mut sink: F,
    ) -> Result<StreamEnd, EngineError>
    where
        F: FnMut(Vec<LogLine>) -> bool + Send,
    {
        validate_ref("contenedor", id)?;
        let tail_s = tail.map(|t| t.to_string()).unwrap_or_else(|| "all".into());
        let opts = LogsOptions {
            follow: true,
            stdout: true,
            stderr: true,
            since: 0,
            until: 0,
            timestamps: true,
            tail: tail_s.clone(),
        };
        let handle = self.activity().begin(OperationSpec {
            category: Category::Container,
            kind: "container.logs".into(),
            target: Some(display.unwrap_or(id).to_owned()),
            transport: Transport::Api {
                method: "GET".into(),
                path: path_with_query(&format!("/containers/{id}/logs"), &opts),
                endpoint: self.endpoint().socket_path.clone(),
            },
            request_body: None,
            equivalent_command: Some(self.docker_cli(&[
                "logs".into(),
                "--follow".into(),
                "--timestamps".into(),
                "--tail".into(),
                tail_s,
                id.into(),
            ])),
        });
        let mut stream = self.docker().logs(id, Some(opts));
        let mut asm = LineAssembler::default();
        let mut batch: Vec<LogLine> = Vec::new();
        let mut total: u64 = 0;
        let mut tick = tokio::time::interval(LOG_FLUSH);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        let mut flush = |batch: &mut Vec<LogLine>, total: &mut u64| -> bool {
            if batch.is_empty() {
                return true;
            }
            *total += batch.len() as u64;
            sink(std::mem::take(batch))
        };

        let reason = loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => break Ok(EndReason::Closed),
                item = stream.next() => match item {
                    Some(Ok(frame)) => {
                        let (kind, data) = match frame {
                            LogOutput::StdOut { message } => (LogStream::Stdout, message),
                            LogOutput::StdErr { message } => (LogStream::Stderr, message),
                            LogOutput::Console { message } => (LogStream::Console, message),
                            LogOutput::StdIn { .. } => continue,
                        };
                        asm.push(kind, &data, &mut batch);
                        if batch.len() >= LOG_BATCH && !flush(&mut batch, &mut total) {
                            break Ok(EndReason::Closed);
                        }
                    }
                    Some(Err(e)) => break Err(EngineError::from(e)),
                    None => break Ok(EndReason::Finished),
                },
                _ = tick.tick() => {
                    if !flush(&mut batch, &mut total) {
                        break Ok(EndReason::Closed);
                    }
                }
            }
        };
        asm.finish(&mut batch);
        let _ = flush(&mut batch, &mut total);
        match reason {
            Ok(reason) => {
                let summary = match reason {
                    EndReason::Closed => format!("Flujo cerrado por el usuario; {total} líneas recibidas"),
                    EndReason::Finished => format!("Docker cerró el flujo (el contenedor no está en ejecución); {total} líneas recibidas"),
                };
                handle.succeed(Outcome {
                    summary: Some(summary.clone()),
                    ..Default::default()
                });
                Ok(StreamEnd {
                    reason,
                    summary,
                    exit_code: None,
                    container: None,
                })
            }
            Err(e) => {
                handle.fail(Failure {
                    error: e.to_string(),
                    http_status: e.http_status(),
                    ..Default::default()
                });
                Err(e)
            }
        }
    }

    /// Estadísticas en tiempo real (`docker stats`), una muestra por segundo.
    pub async fn stream_stats<F>(
        &self,
        id: &str,
        display: Option<&str>,
        cancel: CancellationToken,
        mut sink: F,
    ) -> Result<StreamEnd, EngineError>
    where
        F: FnMut(StatsSample) -> bool + Send,
    {
        validate_ref("contenedor", id)?;
        let opts = StatsOptions {
            stream: true,
            one_shot: false,
        };
        let handle = self.activity().begin(OperationSpec {
            category: Category::Container,
            kind: "container.stats".into(),
            target: Some(display.unwrap_or(id).to_owned()),
            transport: Transport::Api {
                method: "GET".into(),
                path: path_with_query(&format!("/containers/{id}/stats"), &opts),
                endpoint: self.endpoint().socket_path.clone(),
            },
            request_body: None,
            equivalent_command: Some(self.docker_cli(&["stats".into(), id.into()])),
        });
        let mut stream = self.docker().stats(id, Some(opts));
        let mut n: u64 = 0;
        let reason = loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => break Ok(EndReason::Closed),
                item = stream.next() => match item {
                    Some(Ok(s)) => {
                        n += 1;
                        if !sink(sample_from(&s)) {
                            break Ok(EndReason::Closed);
                        }
                    }
                    Some(Err(e)) => break Err(EngineError::from(e)),
                    None => break Ok(EndReason::Finished),
                }
            }
        };
        match reason {
            Ok(reason) => {
                let summary = match reason {
                    EndReason::Closed => format!("Flujo cerrado por el usuario; {n} muestras"),
                    EndReason::Finished => format!("Docker cerró el flujo; {n} muestras"),
                };
                handle.succeed(Outcome {
                    summary: Some(summary.clone()),
                    ..Default::default()
                });
                Ok(StreamEnd {
                    reason,
                    summary,
                    exit_code: None,
                    container: None,
                })
            }
            Err(e) => {
                handle.fail(Failure {
                    error: e.to_string(),
                    http_status: e.http_status(),
                    ..Default::default()
                });
                Err(e)
            }
        }
    }

    /// Terminal interactiva: `POST /containers/{id}/exec` + `POST /exec/{id}/start`
    /// con la conexión secuestrada (equivalente a `docker exec -it`).
    #[allow(clippy::too_many_arguments)]
    pub async fn run_terminal<F>(
        &self,
        id: &str,
        display: Option<&str>,
        cmd: Vec<String>,
        size: (u16, u16),
        cancel: CancellationToken,
        mut input: mpsc::Receiver<TerminalInput>,
        mut output: F,
    ) -> Result<StreamEnd, EngineError>
    where
        F: FnMut(Bytes) -> bool + Send,
    {
        validate_ref("contenedor", id)?;
        if cmd.is_empty() || cmd.iter().any(|a| a.contains('\0')) || cmd[0].is_empty() {
            return Err(EngineError::InvalidInput {
                message: "orden de terminal no válida".into(),
            });
        }
        let target = display.unwrap_or(id).to_owned();
        let target_shown = target.clone();
        let cfg = ExecConfig {
            attach_stdin: Some(true),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            tty: Some(true),
            cmd: Some(cmd.clone()),
            env: Some(vec!["TERM=xterm-256color".into()]),
            ..Default::default()
        };
        let op = ApiOp {
            category: Category::Container,
            kind: "container.exec.create",
            target: Some(target.clone()),
            method: "POST",
            path: format!("/containers/{id}/exec"),
            body: Some(serde_json::to_string_pretty(&cfg).unwrap_or_default()),
            equivalent: None,
            timeout: None,
        };
        let d = self.docker().clone();
        let id_s = id.to_owned();
        let exec = self
            .call(op, async move { d.create_exec(&id_s, cfg).await }, |r| {
                Some(format!(
                    "Exec creado: {}",
                    r.id.chars().take(12).collect::<String>()
                ))
            })
            .await?;
        let exec_id = exec.id;

        let mut eq = vec![
            "exec".to_owned(),
            "--interactive".into(),
            "--tty".into(),
            id.into(),
        ];
        eq.extend(cmd.iter().cloned());
        let start_opts = bollard::exec::StartExecOptions {
            detach: false,
            tty: true,
            output_capacity: None,
        };
        let handle = self.activity().begin(OperationSpec {
            category: Category::Container,
            kind: "container.exec".into(),
            target: Some(target),
            transport: Transport::Api {
                method: "POST".into(),
                path: format!("/exec/{exec_id}/start"),
                endpoint: self.endpoint().socket_path.clone(),
            },
            request_body: Some(serde_json::to_string_pretty(&start_opts).unwrap_or_default()),
            equivalent_command: Some(self.docker_cli(&eq)),
        });
        let started = match self.docker().start_exec(&exec_id, Some(start_opts)).await {
            Ok(StartExecResults::Attached { output, input }) => (output, input),
            Ok(StartExecResults::Detached) => {
                let e = EngineError::InvalidResponse {
                    message: "Docker no adjuntó la sesión".into(),
                };
                handle.fail(Failure {
                    error: e.to_string(),
                    ..Default::default()
                });
                return Err(e);
            }
            Err(e) => {
                let e = EngineError::from(e);
                handle.fail(Failure {
                    error: e.to_string(),
                    http_status: e.http_status(),
                    ..Default::default()
                });
                return Err(e);
            }
        };
        let (mut out_stream, mut stdin) = started;
        let resize = |cols: u16, rows: u16| {
            let d = self.docker().clone();
            let ex = exec_id.clone();
            async move {
                d.resize_exec(
                    &ex,
                    ResizeExecOptions {
                        h: rows as i32,
                        w: cols as i32,
                    },
                )
                .await
                .is_ok()
            }
        };
        // Redimensionados: efecto técnico de la ventana, no se registran uno a uno (D-036).
        let mut resizes = u32::from(resize(size.0, size.1).await);
        let (mut bytes_out, mut bytes_in) = (0u64, 0u64);
        let mut mark = PidMark::default();

        let reason = loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => break Ok(EndReason::Closed),
                item = out_stream.next() => match item {
                    Some(Ok(frame)) => {
                        let data = match frame {
                            LogOutput::StdOut { message } | LogOutput::StdErr { message } | LogOutput::Console { message } => message,
                            LogOutput::StdIn { .. } => continue,
                        };
                        bytes_out += data.len() as u64;
                        let shown = mark.feed(&data);
                        if !shown.is_empty() && !output(Bytes::from(shown)) {
                            break Ok(EndReason::Closed);
                        }
                    }
                    Some(Err(e)) => break Err(EngineError::from(e)),
                    None => {
                        let rest = mark.flush();
                        if !rest.is_empty() {
                            output(Bytes::from(rest));
                        }
                        break Ok(EndReason::Finished);
                    }
                },
                msg = input.recv() => match msg {
                    Some(TerminalInput::Data(d)) => {
                        bytes_in += d.len() as u64;
                        if let Err(e) = async { stdin.write_all(&d).await?; stdin.flush().await }.await {
                            break Err(EngineError::Io { message: e.to_string() });
                        }
                    }
                    Some(TerminalInput::Resize { cols, rows }) => {
                        if resize(cols, rows).await {
                            resizes += 1;
                        }
                    }
                    None => break Ok(EndReason::Closed),
                }
            }
        };
        // Docker no termina los procesos exec cuando el cliente se desconecta.
        // Al cerrar se envía lo que haría el usuario para salir: Ctrl+C (descarta la
        // línea o detiene el programa en primer plano) y Ctrl+D (EOF: la shell sale),
        // y se espera a que el proceso termine (D-037).
        let mut ended = matches!(reason, Ok(EndReason::Finished));
        let mut exit_sent = false;
        if matches!(reason, Ok(EndReason::Closed)) {
            // Por separado: al recibir Ctrl+C el TTY vacía su búfer de entrada y se
            // perdería un Ctrl+D enviado en la misma escritura.
            exit_sent = async {
                stdin.write_all(&[0x03]).await?;
                stdin.flush().await?;
                tokio::time::sleep(Duration::from_millis(200)).await;
                stdin.write_all(&[0x04]).await?;
                stdin.flush().await
            }
            .await
            .is_ok();
            let drain = async {
                while let Some(item) = out_stream.next().await {
                    if item.is_err() {
                        break;
                    }
                }
            };
            ended = tokio::time::timeout(Duration::from_secs(3), drain)
                .await
                .is_ok();
        }
        let _ = stdin.shutdown().await;
        drop(stdin);
        drop(out_stream);

        // Código de salida: una consulta real, registrada.
        let exit_query = |exec_id: String| {
            let d = self.docker().clone();
            let op = ApiOp::get(
                Category::Container,
                "container.exec.inspect",
                format!("/exec/{exec_id}/json"),
            );
            async move {
                self.call(op, async move { d.inspect_exec(&exec_id).await }, |i| {
                    i.exit_code.map(|c| format!("Código de salida: {c}"))
                })
                .await
                .ok()
                .map(|i| (i.running == Some(true), i.exit_code))
            }
        };

        // Con el contenedor en marcha, se terminan los procesos que queden de la
        // sesión dentro del contenedor (D-063): la shell, si no salió (un programa
        // que ignora Ctrl+C), y lo que dejó en segundo plano (`orden &`), que ni
        // dash ni bash terminan al salir.
        let shell_exited = ended;
        let mut killed: Option<u32> = None;
        let mut container = None;
        let mut exit_code = None;
        match &reason {
            Ok(EndReason::Closed) => {
                if let Some(pid) = mark.pid {
                    killed = self
                        .kill_terminal_session(id, &target_shown, pid)
                        .await
                        .ok();
                }
                if ended || killed.is_some() {
                    // Tras terminar los procesos, Docker (24) puede tardar un
                    // momento en marcar el exec como terminado: se espera
                    // hasta ~2 s antes de concluir que sigue en marcha.
                    let mut tries = if killed.is_some() { 10 } else { 1 };
                    loop {
                        tries -= 1;
                        match exit_query(exec_id.clone()).await {
                            Some((true, _)) if tries > 0 => {
                                tokio::time::sleep(Duration::from_millis(200)).await;
                            }
                            Some((true, _)) => {
                                ended = false;
                                break;
                            }
                            Some((false, c)) => {
                                ended = true;
                                exit_code = c;
                                break;
                            }
                            None => break,
                        }
                    }
                }
            }
            Ok(EndReason::Finished) => {
                if let Some((_, c)) = exit_query(exec_id.clone()).await {
                    exit_code = c;
                }
                // Terminado por una señal (128 + n): lo normal es que se esté
                // deteniendo el contenedor, y Docker tarda un instante en
                // reflejarlo (D-031). Se confirma con inspect.
                let settle = exit_code.is_some_and(|c| c >= 128);
                container = self
                    .container_after_session(id, &target_shown, settle)
                    .await;
                if container.is_none() {
                    if let Some(pid) = mark.pid {
                        killed = self
                            .kill_terminal_session(id, &target_shown, pid)
                            .await
                            .ok();
                    }
                }
            }
            Err(_) => {}
        }
        let traffic = format!(
            "{} recibidos, {} enviados, {resizes} redimensionados",
            human_bytes(bytes_out),
            human_bytes(bytes_in)
        );
        match reason {
            Ok(reason) => {
                let gone = match container {
                    Some(ContainerAfter::Stopped) => "El contenedor se detuvo: ",
                    Some(ContainerAfter::Removed) => "El contenedor se eliminó: ",
                    None => "",
                };
                let leftovers = match killed {
                    Some(n) if n > 0 && shell_exited => {
                        format!("; se terminaron {n} procesos que seguían en segundo plano")
                    }
                    _ => String::new(),
                };
                let summary = match (reason, exit_code) {
                    (EndReason::Closed, c) if !shell_exited && killed.is_some() && ended => format!(
                        "Sesión cerrada por el usuario; la shell no salió con Ctrl+C y Ctrl+D y se terminaron sus {} procesos dentro del contenedor{}; {traffic}",
                        killed.unwrap_or(0),
                        c.map(|c| format!(" (código {c})")).unwrap_or_default()
                    ),
                    (EndReason::Closed, _) if !ended => format!(
                        "Sesión cerrada por el usuario{}; el proceso sigue en ejecución dentro del contenedor (Docker no lo termina al desconectar); {traffic}",
                        if exit_sent { " (se envió Ctrl+C y Ctrl+D)" } else { "" }
                    ),
                    (EndReason::Closed, Some(c)) => format!("Sesión cerrada por el usuario; la shell terminó con código {c}{leftovers}; {traffic}"),
                    (EndReason::Closed, None) => format!("Sesión cerrada por el usuario; la shell terminó{leftovers}; {traffic}"),
                    (EndReason::Finished, Some(c)) if gone.is_empty() => format!("El proceso terminó con código {c}{leftovers}; {traffic}"),
                    (EndReason::Finished, None) if gone.is_empty() => format!("El proceso terminó{leftovers}; {traffic}"),
                    (EndReason::Finished, Some(c)) => format!("{gone}el proceso terminó con código {c}; {traffic}"),
                    (EndReason::Finished, None) => format!("{gone}el proceso terminó; {traffic}"),
                };
                handle.succeed(Outcome {
                    summary: Some(summary.clone()),
                    exit_code: exit_code.map(|c| c as i32),
                    ..Default::default()
                });
                Ok(StreamEnd {
                    reason,
                    summary,
                    exit_code,
                    container,
                })
            }
            Err(e) => {
                handle.fail(Failure {
                    error: format!("{e}; {traffic}"),
                    http_status: e.http_status(),
                    ..Default::default()
                });
                Err(e)
            }
        }
    }
}

impl Engine {
    /// Termina dentro del contenedor todos los procesos de la sesión de una
    /// terminal (la shell y lo que lanzó), con un `exec` sin TTY. Se registra.
    /// Devuelve cuántos procesos se terminaron.
    pub async fn kill_terminal_session(
        &self,
        id: &str,
        target: &str,
        pid: u32,
    ) -> Result<u32, EngineError> {
        validate_ref("contenedor", id)?;
        let cmd: Vec<String> = vec![
            "/bin/sh".into(),
            "-c".into(),
            KILL_SESSION_SCRIPT.into(),
            "sh".into(),
            pid.to_string(),
        ];
        let cfg = ExecConfig {
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            tty: Some(false),
            cmd: Some(cmd.clone()),
            ..Default::default()
        };
        let create = ApiOp {
            category: Category::Container,
            kind: "container.exec.create",
            target: Some(target.to_owned()),
            method: "POST",
            path: format!("/containers/{id}/exec"),
            body: Some(serde_json::to_string_pretty(&cfg).unwrap_or_default()),
            equivalent: None,
            timeout: Some(Duration::from_secs(10)),
        };
        let d = self.docker().clone();
        let id_s = id.to_owned();
        let exec = self
            .call(
                create,
                async move { d.create_exec(&id_s, cfg).await },
                |r| {
                    Some(format!(
                        "Exec creado: {}",
                        r.id.chars().take(12).collect::<String>()
                    ))
                },
            )
            .await?;
        let mut eq = vec!["exec".to_owned(), id.to_owned()];
        eq.extend(cmd);
        let start = ApiOp {
            category: Category::Container,
            kind: "container.exec.cleanup",
            target: Some(target.to_owned()),
            method: "POST",
            path: format!("/exec/{}/start", exec.id),
            body: None,
            equivalent: Some(eq),
            timeout: Some(Duration::from_secs(10)),
        };
        let d = self.docker().clone();
        let eid = exec.id.clone();
        let out = self
            .call(
                start,
                async move {
                    let mut text = String::new();
                    if let StartExecResults::Attached { mut output, .. } =
                        d.start_exec(&eid, None).await?
                    {
                        while let Some(frame) = output.next().await {
                            if let LogOutput::StdOut { message } = frame? {
                                text.push_str(&String::from_utf8_lossy(&message));
                            }
                        }
                    }
                    Ok::<_, bollard::errors::Error>(text)
                },
                |t| {
                    Some(format!(
                        "Terminados {} procesos de la sesión {pid} de la terminal dentro del contenedor",
                        t.trim()
                    ))
                },
            )
            .await?;
        Ok(out.trim().parse().unwrap_or(0))
    }

    /// Cuando una sesión de terminal termina sola: ¿sigue el contenedor en
    /// ejecución? Una consulta real (`GET /containers/{id}/json`), registrada.
    pub async fn container_after_session(
        &self,
        id: &str,
        target: &str,
        settle: bool,
    ) -> Option<ContainerAfter> {
        // Si se espera que se esté deteniendo, se vuelve a mirar durante 3 s.
        let tries = if settle { 15 } else { 1 };
        for n in 0..tries {
            match self.container_state_once(id, target).await {
                Some(after) => return Some(after),
                None if n + 1 < tries => tokio::time::sleep(Duration::from_millis(200)).await,
                None => {}
            }
        }
        None
    }

    async fn container_state_once(&self, id: &str, target: &str) -> Option<ContainerAfter> {
        let mut op = ApiOp::get(
            Category::Container,
            "container.inspect",
            format!("/containers/{id}/json"),
        );
        op.target = Some(target.to_owned());
        op.timeout = Some(Duration::from_secs(10));
        let d = self.docker().clone();
        let id_s = id.to_owned();
        let r = self
            .call(
                op,
                async move {
                    d.inspect_container(
                        &id_s,
                        None::<bollard::query_parameters::InspectContainerOptions>,
                    )
                    .await
                },
                |c| {
                    let running = c.state.as_ref().and_then(|s| s.running) == Some(true);
                    Some(if running {
                        "El contenedor sigue en ejecución".to_owned()
                    } else {
                        "El contenedor ya no está en ejecución".to_owned()
                    })
                },
            )
            .await;
        use bollard::models::ContainerStateStatusEnum as St;
        match r {
            // `docker rm -f`: Docker lo marca como «removing» y no pasa por «exited».
            Ok(c) if c.state.as_ref().and_then(|s| s.status) == Some(St::REMOVING) => {
                Some(ContainerAfter::Removed)
            }
            Ok(c) if c.state.as_ref().and_then(|s| s.running) == Some(true) => None,
            Ok(_) => Some(ContainerAfter::Stopped),
            Err(e) if e.http_status() == Some(404) => Some(ContainerAfter::Removed),
            Err(_) => None,
        }
    }
}

/// Texto de la orden de terminal, para mostrarla.
pub fn render_exec(cmd: &[String]) -> String {
    match cmd.split_first() {
        Some((p, rest)) => render_argv(p, rest),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembler_joins_fragments_and_keeps_streams_apart() {
        let mut a = LineAssembler::default();
        let mut out = Vec::new();
        a.push(
            LogStream::Stdout,
            b"2026-11-05T10:00:00.000000001Z hola ",
            &mut out,
        );
        a.push(
            LogStream::Stderr,
            b"2026-11-05T10:00:00.000000002Z fallo\n",
            &mut out,
        );
        a.push(
            LogStream::Stdout,
            b"mundo\r\n2026-11-05T10:00:01.000000000Z segunda\nsin fin",
            &mut out,
        );
        assert_eq!(out.len(), 3);
        assert_eq!(
            out[0],
            LogLine {
                stream: LogStream::Stderr,
                ts: Some("2026-11-05T10:00:00.000000002Z".into()),
                text: "fallo".into()
            }
        );
        assert_eq!(
            out[1],
            LogLine {
                stream: LogStream::Stdout,
                ts: Some("2026-11-05T10:00:00.000000001Z".into()),
                text: "hola mundo".into()
            }
        );
        assert_eq!(out[2].text, "segunda");
        a.finish(&mut out);
        assert_eq!(
            out[3],
            LogLine {
                stream: LogStream::Stdout,
                ts: None,
                text: "sin fin".into()
            }
        );
    }

    #[test]
    fn assembler_handles_utf8_split_across_frames() {
        let mut a = LineAssembler::default();
        let mut out = Vec::new();
        let s = "2026-11-05T10:00:00.000000000Z señal ✓\n".as_bytes();
        let cut = s.iter().position(|&b| b == 0xC3).unwrap() + 1; // en medio de la ñ
        a.push(LogStream::Console, &s[..cut], &mut out);
        assert!(out.is_empty());
        a.push(LogStream::Console, &s[cut..], &mut out);
        assert_eq!(out[0].text, "señal ✓");
        assert_eq!(out[0].stream, LogStream::Console);
    }

    #[test]
    fn line_without_timestamp_is_kept_whole() {
        assert_eq!(split_ts("texto normal"), (None, "texto normal".into()));
        assert_eq!(
            split_ts("2026-11-05T10:00:00Z"),
            (Some("2026-11-05T10:00:00Z".into()), String::new())
        );
    }

    fn stats(json: &str) -> ContainerStatsResponse {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn stats_cgroup_v2_like_docker_cli() {
        // Forma de una muestra real de cgroup v2 (Debian 13).
        let s = stats(
            r#"{
            "read":"2026-11-05T10:00:01.000000000Z",
            "pids_stats":{"current":3},
            "cpu_stats":{"cpu_usage":{"total_usage":2000000000},"system_cpu_usage":20000000000,"online_cpus":4},
            "precpu_stats":{"cpu_usage":{"total_usage":1500000000},"system_cpu_usage":18000000000,"online_cpus":4},
            "memory_stats":{"usage":104857600,"limit":2147483648,"stats":{"inactive_file":4194304,"anon":90000000}},
            "networks":{"eth0":{"rx_bytes":1000,"tx_bytes":200},"eth1":{"rx_bytes":24,"tx_bytes":6}},
            "blkio_stats":{"io_service_bytes_recursive":[{"major":8,"minor":0,"op":"read","value":4096},{"major":8,"minor":0,"op":"write","value":8192}]}
        }"#,
        );
        let x = sample_from(&s);
        // (0.5e9 / 2e9) * 4 * 100 = 100 %
        assert!((x.cpu_percent.unwrap() - 100.0).abs() < 1e-9);
        assert_eq!(x.online_cpus, Some(4));
        assert_eq!(x.memory_used, 104857600 - 4194304);
        assert_eq!(x.memory_limit, 2147483648);
        assert!((x.memory_percent.unwrap() - 4.6875).abs() < 1e-9);
        assert_eq!((x.net_rx, x.net_tx), (1024, 206));
        assert_eq!((x.block_read, x.block_write), (4096, 8192));
        assert_eq!(x.pids, Some(3));
    }

    #[test]
    fn stats_cgroup_v1_and_first_sample() {
        let s = stats(
            r#"{
            "cpu_stats":{"cpu_usage":{"total_usage":500,"percpu_usage":[250,250]},"system_cpu_usage":1000},
            "precpu_stats":{"cpu_usage":{"total_usage":0}},
            "memory_stats":{"usage":1000,"limit":4000,"stats":{"total_inactive_file":200,"cache":300}},
            "blkio_stats":{"io_service_bytes_recursive":[{"op":"Read","value":10},{"op":"Write","value":20},{"op":"Total","value":30}]}
        }"#,
        );
        let x = sample_from(&s);
        assert_eq!(
            x.cpu_percent, None,
            "la primera muestra no tiene referencia anterior"
        );
        assert_eq!(x.online_cpus, Some(2));
        assert_eq!(x.memory_used, 800);
        assert_eq!((x.block_read, x.block_write), (10, 20));
        assert_eq!(x.pids, None);
    }

    #[test]
    fn shell_choices() {
        // Desde la 0.9.1 (D-063) la shell anuncia su PID antes de arrancar; el
        // resto de la orden no cambia: bash si existe, si no sh.
        assert_eq!(
            ShellChoice::Sh.argv(),
            vec![
                "/bin/sh",
                "-c",
                r#"printf '\033]6973;%s\007' "$$"; exec /bin/sh"#
            ]
        );
        assert_eq!(
            ShellChoice::Bash.argv(),
            vec![
                "/bin/bash",
                "-c",
                r#"printf '\033]6973;%s\007' "$$"; exec /bin/bash"#
            ]
        );
        assert_eq!(
            render_exec(&ShellChoice::Auto.argv()),
            r#"/bin/sh -c 'printf '\''\033]6973;%s\007'\'' "$$"; if [ -x /bin/bash ]; then exec /bin/bash; else exec /bin/sh; fi'"#
        );
    }

    #[test]
    fn pid_mark_is_removed_from_the_output() {
        // Llega en trozos, con texto pegado detrás.
        let mut m = PidMark::default();
        assert_eq!(m.feed(b"\x1b]69"), b"");
        assert_eq!(m.feed(b"73;4"), b"");
        assert_eq!(m.feed(b"2\x07$ "), b"$ ");
        assert_eq!(m.pid, Some(42));
        assert_eq!(m.feed(b"ls\r\n"), b"ls\r\n");

        // Detrás del eco de lo tecleado antes de que arrancara la shell.
        let mut m = PidMark::default();
        let mut shown = m.feed(b"echo hola\r\n\x1b]69");
        shown.extend(m.feed(b"73;7\x07# hola\r\n"));
        assert_eq!(shown, b"echo hola\r\n# hola\r\n");
        assert_eq!(m.pid, Some(7));

        // Sin marca: la salida pasa tal cual (salvo un posible principio de
        // marca al final, que se suelta en cuanto se ve que no lo es).
        let mut m = PidMark::default();
        assert_eq!(m.feed(b"hola"), b"hola");
        assert_eq!(m.pid, None);
        let mut m = PidMark::default();
        assert_eq!(m.feed(b"\x1b[31mrojo"), b"\x1b[31mrojo");
        let mut m = PidMark::default();
        assert_eq!(m.feed(b"abc\x1b"), b"abc");
        assert_eq!(m.feed(b"[0m"), b"\x1b[0m");

        // Marca no válida o sin cerrar: no se pierde nada.
        let mut m = PidMark::default();
        assert_eq!(m.feed(b"\x1b]6973;abc\x07x"), b"x");
        assert_eq!(m.pid, None);
        let mut m = PidMark::default();
        assert_eq!(m.feed(b"\x1b]6973;12"), b"");
        assert_eq!(m.flush(), b"\x1b]6973;12");
        let mut m = PidMark::default();
        let long = [&b"\x1b]6973;"[..], &[b'1'; 80]].concat();
        assert_eq!(m.feed(&long), long);

        // Pasada la ventana sin marca, deja de buscar.
        let mut m = PidMark::default();
        let big = vec![b'x'; PID_MARK_WINDOW + 10];
        assert_eq!(m.feed(&big), big);
        assert_eq!(m.feed(b"\x1b]6973;5\x07"), b"\x1b]6973;5\x07");
        assert_eq!(m.pid, None);
    }
}
