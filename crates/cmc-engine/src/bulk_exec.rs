// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Comandos masivos (1.11.0, D-121 a D-124): el mismo comando dentro de varios
//! contenedores con Docker Exec.
//!
//! - El comando **nunca** pasa por una shell del equipo: se envía a Docker como
//!   una lista de argumentos (`Cmd`) y lo ejecuta el contenedor. No se crean
//!   archivos temporales ni scripts.
//! - Dos modalidades, sin suponer ninguna shell: un ejecutable con sus
//!   argumentos, o una secuencia de varias líneas para un intérprete que elige
//!   el usuario (`/bin/sh`, `bash`, `python3`…). Si el intérprete no existe en
//!   la imagen, Docker lo dice y se informa tal cual.
//! - Antes de crear la ejecución se comprueba con `inspect` (sin registrar) que
//!   el contenedor existe, es ese ID y está en ejecución.
//! - Tiempo máximo e interrupción: Docker Exec **no** tiene forma de detener un
//!   proceso ya lanzado. Al agotarse el tiempo o al pedir la interrupción se
//!   deja de esperar y se consulta si el proceso sigue vivo; nunca se mata ni
//!   se reinicia el contenedor.
//! - La salida se guarda con un límite por flujo; no se registra en la
//!   actividad (puede contener datos del usuario), solo el comando, el usuario,
//!   el directorio, el código de salida y la duración.

use std::time::Duration;

use bollard::container::LogOutput;
use bollard::exec::{StartExecOptions, StartExecResults};
use bollard::models::ExecConfig;
use chrono::{DateTime, Utc};
use cmc_activity::{quote::render_argv, Category, Failure, OperationSpec, Outcome, Transport};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::bulk::{is_full_id, BulkStage, ItemError, SkipReason};
use crate::client::{ApiOp, Engine};
use crate::error::EngineError;

/// Tiempo máximo que se puede pedir (1 hora).
pub const MAX_TIMEOUT_SECS: u32 = 3600;
/// Bytes que se guardan de cada flujo (salida y error) por contenedor.
pub const OUTPUT_LIMIT: usize = 128 * 1024;
/// Tamaño máximo de una secuencia de varias líneas.
pub const MAX_SCRIPT_BYTES: usize = 64 * 1024;
/// Tamaño máximo de todos los argumentos juntos.
pub const MAX_ARGV_BYTES: usize = 128 * 1024;
pub const MAX_ARGS: usize = 256;

/// Qué se ejecuta dentro del contenedor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "mode", rename_all = "camelCase")]
#[ts(export)]
pub enum ExecCommand {
    /// Un ejecutable con sus argumentos, sin intérprete.
    #[serde(rename_all = "camelCase")]
    Direct { program: String, args: Vec<String> },
    /// Una secuencia (puede tener varias líneas) para el intérprete elegido.
    /// Se ejecuta como `intérprete [opción] secuencia`; la opción habitual es
    /// `-c` (sh, bash, ash, python3).
    #[serde(rename_all = "camelCase")]
    Script {
        interpreter: String,
        flag: String,
        script: String,
    },
}

/// Comando masivo completo: qué, con qué usuario, en qué directorio y con qué
/// tiempo máximo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BulkCommand {
    pub command: ExecCommand,
    /// `usuario`, `uid`, `usuario:grupo` o `uid:gid`; vacío = el de la imagen.
    pub user: Option<String>,
    /// Ruta absoluta dentro del contenedor; vacío = la de la imagen.
    pub workdir: Option<String>,
    pub timeout_secs: u32,
}

fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidInput {
        message: message.into(),
    }
}

fn no_nul(what: &str, s: &str) -> Result<(), EngineError> {
    if s.contains('\0') {
        return Err(invalid(format!("{what}: contiene un carácter nulo")));
    }
    Ok(())
}

fn valid_user(u: &str) -> bool {
    let ok = |p: &str| {
        !p.is_empty()
            && p.len() <= 64
            && p.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    };
    match u.split_once(':') {
        Some((a, b)) => ok(a) && ok(b),
        None => ok(u),
    }
}

fn opt(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

impl BulkCommand {
    pub fn user(&self) -> Option<&str> {
        opt(&self.user)
    }

    pub fn workdir(&self) -> Option<&str> {
        opt(&self.workdir)
    }

    /// Comprueba el comando antes de enviar nada.
    pub fn validate(&self) -> Result<(), EngineError> {
        if self.timeout_secs == 0 || self.timeout_secs > MAX_TIMEOUT_SECS {
            return Err(invalid(format!(
                "el tiempo máximo debe estar entre 1 y {MAX_TIMEOUT_SECS} segundos"
            )));
        }
        if let Some(u) = self.user() {
            if !valid_user(u) {
                return Err(invalid(
                    "usuario no válido: usa usuario, uid, usuario:grupo o uid:gid",
                ));
            }
        }
        if let Some(w) = self.workdir() {
            no_nul("directorio de trabajo", w)?;
            if !w.starts_with('/') || w.len() > 4096 {
                return Err(invalid(
                    "el directorio de trabajo debe ser una ruta absoluta del contenedor",
                ));
            }
        }
        match &self.command {
            ExecCommand::Direct { program, args } => {
                if program.trim().is_empty() {
                    return Err(invalid("falta el ejecutable"));
                }
                if program.trim() != program {
                    return Err(invalid(
                        "el ejecutable no puede empezar ni acabar en espacio",
                    ));
                }
                if args.len() > MAX_ARGS {
                    return Err(invalid(format!("como máximo {MAX_ARGS} argumentos")));
                }
                no_nul("ejecutable", program)?;
                for a in args {
                    no_nul("argumento", a)?;
                }
            }
            ExecCommand::Script {
                interpreter,
                flag,
                script,
            } => {
                if interpreter.trim().is_empty() || interpreter.trim() != interpreter {
                    return Err(invalid(
                        "falta el intérprete o tiene espacios al principio o al final",
                    ));
                }
                if flag.chars().any(char::is_whitespace) {
                    return Err(invalid("la opción del intérprete no puede tener espacios"));
                }
                if script.trim().is_empty() {
                    return Err(invalid("la secuencia está vacía"));
                }
                if script.len() > MAX_SCRIPT_BYTES {
                    return Err(invalid(format!(
                        "la secuencia supera {} KiB",
                        MAX_SCRIPT_BYTES / 1024
                    )));
                }
                no_nul("intérprete", interpreter)?;
                no_nul("opción", flag)?;
                no_nul("secuencia", script)?;
            }
        }
        if self.argv().iter().map(String::len).sum::<usize>() > MAX_ARGV_BYTES {
            return Err(invalid(format!(
                "el comando supera {} KiB",
                MAX_ARGV_BYTES / 1024
            )));
        }
        Ok(())
    }

    /// Lista exacta de argumentos que recibe Docker (`Cmd`). Sin shell del
    /// equipo, sin comillas que interpretar: cada elemento llega tal cual.
    pub fn argv(&self) -> Vec<String> {
        match &self.command {
            ExecCommand::Direct { program, args } => {
                let mut v = vec![program.clone()];
                v.extend(args.iter().cloned());
                v
            }
            ExecCommand::Script {
                interpreter,
                flag,
                script,
            } => {
                let mut v = vec![interpreter.clone()];
                if !flag.is_empty() {
                    v.push(flag.clone());
                }
                v.push(script.clone());
                v
            }
        }
    }

    /// Argumentos de `docker exec` equivalentes (para mostrar y registrar).
    pub fn exec_args(&self, id: &str) -> Vec<String> {
        let mut eq = vec!["exec".to_owned()];
        if let Some(u) = self.user() {
            eq.push("--user".into());
            eq.push(u.into());
        }
        if let Some(w) = self.workdir() {
            eq.push("--workdir".into());
            eq.push(w.into());
        }
        eq.push(id.into());
        eq.extend(self.argv());
        eq
    }
}

// ------------------------------------------------------------------ riesgo

/// Nivel de riesgo de un comando, de menor a mayor. Es una **ayuda** para
/// avisar: no sustituye a la confirmación ni a ningún otro control (D-123).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CommandRiskLevel {
    Informative,
    Modification,
    Administrative,
    Destructive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommandRisk {
    pub level: CommandRiskLevel,
    /// Códigos de los motivos (la interfaz los traduce), sin repetir.
    pub reasons: Vec<String>,
    /// El comando puede mostrar variables de entorno u otros secretos.
    pub exposes_secrets: bool,
}

/// Palabras del comando: separa por espacios y por los signos de la shell, y
/// quita la ruta de los ejecutables (`/usr/bin/apt` → `apt`).
fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for raw in text.split(|c: char| {
        c.is_whitespace()
            || matches!(
                c,
                ';' | '|' | '&' | '(' | ')' | '`' | '{' | '}' | '"' | '\''
            )
    }) {
        if raw.is_empty() {
            continue;
        }
        let w = raw
            .rsplit('/')
            .next()
            .filter(|s| !s.is_empty() && !raw.starts_with('-'))
            .unwrap_or(raw);
        out.push(w.to_ascii_lowercase());
    }
    out
}

/// Clasifica un comando por palabras clave. Es aproximado a propósito: no
/// intenta entender la shell (D-123).
pub fn classify(cmd: &BulkCommand) -> CommandRisk {
    let text = cmd.argv().join(" ");
    let lower = text.to_ascii_lowercase();
    let w = words(&text);
    let has = |x: &str| w.iter().any(|y| y == x);
    let mut level = CommandRiskLevel::Informative;
    let mut reasons: Vec<String> = Vec::new();
    let mut add = |l: CommandRiskLevel, r: &str| {
        level = level.max(l);
        if !reasons.iter().any(|x| x == r) {
            reasons.push(r.to_owned());
        }
    };
    use CommandRiskLevel::*;

    // Potencialmente destructivo.
    let rm_recursive = w.windows(2).any(|p| {
        p[0] == "rm"
            && p[1].starts_with('-')
            && !p[1].starts_with("--")
            && (p[1].contains('r') || p[1].contains('f'))
    }) || (has("rm") && (has("--recursive") || has("--force")));
    if rm_recursive {
        add(Destructive, "rmRecursive");
    }
    if w.iter()
        .any(|x| x.starts_with("mkfs") || x == "wipefs" || x == "fdisk" || x == "parted")
    {
        add(Destructive, "filesystem");
    }
    if has("dd") || has("shred") || lower.contains("> /dev/sd") || lower.contains(">/dev/sd") {
        add(Destructive, "rawDisk");
    }
    if lower.contains("drop database")
        || lower.contains("drop table")
        || lower.contains("truncate table")
        || has("dropdb")
    {
        add(Destructive, "database");
    }
    if (has("kill") || has("pkill") || has("killall")) && (has("1") || has("-1") || has("all")) {
        add(Destructive, "killAll");
    }
    if has("truncate") || lower.contains(":(){") {
        add(Destructive, "truncate");
    }
    if (has("chmod") || has("chown")) && (has("-r") || has("--recursive")) {
        add(Destructive, "recursivePermissions");
    }

    // Administrativo.
    for (words_, reason) in [
        (
            &[
                "apt", "apt-get", "apk", "yum", "dnf", "zypper", "pacman", "dpkg", "rpm", "pip",
                "pip3", "npm", "gem",
            ][..],
            "packages",
        ),
        (
            &[
                "systemctl",
                "service",
                "rc-service",
                "supervisorctl",
                "nginx",
                "apachectl",
            ][..],
            "services",
        ),
        (
            &[
                "useradd", "userdel", "usermod", "adduser", "deluser", "passwd", "chpasswd",
                "groupadd",
            ][..],
            "users",
        ),
        (&["chmod", "chown", "chgrp", "setfacl"][..], "permissions"),
        (&["kill", "pkill", "killall"][..], "signals"),
        (
            &["reboot", "shutdown", "halt", "poweroff", "init"][..],
            "power",
        ),
        (
            &[
                "mount",
                "umount",
                "iptables",
                "nft",
                "ip6tables",
                "sysctl",
                "modprobe",
            ][..],
            "system",
        ),
        (&["crontab"][..], "scheduler"),
    ] {
        if words_.iter().any(|x| has(x)) {
            add(Administrative, reason);
        }
    }

    // Modificación.
    for (words_, reason) in [
        (&["rm", "rmdir", "unlink"][..], "delete"),
        (&["mv", "cp", "rsync", "ln", "install"][..], "files"),
        (&["touch", "mkdir", "tee"][..], "create"),
        (&["sed", "perl"][..], "edit"),
        (
            &["git", "curl", "wget", "tar", "unzip", "gzip", "gunzip"][..],
            "download",
        ),
    ] {
        if words_.iter().any(|x| has(x)) {
            // `sed` sin `-i` solo lee; `curl` sin `-o` solo muestra.
            let real = match reason {
                "edit" => w
                    .iter()
                    .any(|x| x == "-i" || x.starts_with("-i") || x == "--in-place"),
                _ => true,
            };
            if real {
                add(Modification, reason);
            }
        }
    }
    // `>` que escribe en un archivo; no cuentan `2>&1`, `>&2` ni `> /dev/null`.
    let to_streams = regex::Regex::new(r"\d*>>?&\d|\d*>>?\s*/dev/(null|stdout|stderr)\b")
        .expect("expresión fija");
    if to_streams.replace_all(&text, "").contains('>') {
        add(Modification, "redirect");
    }

    let exposes_secrets = has("env")
        || has("printenv")
        || has("set")
        || has("export")
        || lower.contains("/environ")
        || lower.contains("/etc/shadow")
        || lower.contains(".env")
        || w.iter()
            .any(|x| x.contains("secret") || x.contains("password") || x.contains("token"));
    CommandRisk {
        level,
        reasons,
        exposes_secrets,
    }
}

// ------------------------------------------------------------------ ejecución

/// Cómo terminó la espera de una ejecución.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ExecEnd {
    /// El proceso terminó y Docker dio su código de salida.
    Exited,
    /// Se agotó el tiempo máximo; se dejó de esperar.
    TimedOut,
    /// Se pidió la interrupción; se dejó de esperar.
    Interrupted,
}

/// Lo que devolvió una ejecución.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExecOutput {
    pub end: ExecEnd,
    #[ts(type = "number | null")]
    pub exit_code: Option<i64>,
    /// Si el proceso seguía vivo al dejar de esperar (`null` si no se pudo
    /// saber). Solo tiene sentido con `timedOut` o `interrupted`.
    pub still_running: Option<bool>,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    /// Docker no pudo lanzar el proceso (ejecutable, intérprete o directorio
    /// inexistente): el mensaje de Docker viene en la salida.
    pub launch_failed: bool,
    #[ts(type = "string")]
    pub started_at: DateTime<Utc>,
}

/// Resultado de un contenedor.
#[derive(Debug, Clone, PartialEq)]
pub enum ExecDone {
    Ran(ExecOutput),
    /// No se ejecutó: no existe o no está en ejecución.
    NotRun {
        reason: SkipReason,
    },
}

/// Guarda hasta `OUTPUT_LIMIT` bytes; lo demás se cuenta y se descarta.
#[derive(Debug, Default)]
pub struct Capture {
    buf: Vec<u8>,
    pub truncated: bool,
}

impl Capture {
    pub fn push(&mut self, data: &[u8]) {
        let room = OUTPUT_LIMIT.saturating_sub(self.buf.len());
        if data.len() > room {
            self.truncated = true;
        }
        self.buf.extend_from_slice(&data[..data.len().min(room)]);
    }

    pub fn text(self) -> String {
        String::from_utf8_lossy(&self.buf).into_owned()
    }
}

/// Mensaje con el que Docker (runc) informa de que no pudo lanzar el proceso.
pub fn is_launch_failure(exit_code: Option<i64>, stdout: &str, stderr: &str) -> bool {
    matches!(exit_code, Some(126 | 127))
        && (stdout.contains("OCI runtime exec failed")
            || stderr.contains("OCI runtime exec failed"))
}

impl Engine {
    /// Ejecuta `cmd` dentro del contenedor `id` (ID completo):
    ///
    /// 1. comprueba el comando, el ID y, con `inspect` sin registrar, que el
    ///    contenedor existe, es ese y está en ejecución (si no, no se ejecuta);
    /// 2. crea la ejecución (registrada) y la arranca adjuntando la salida;
    /// 3. lee la salida hasta que termina, se agota `timeout_secs` o se pide la
    ///    interrupción (`interrupt`); en los dos últimos casos solo se deja de
    ///    esperar: Docker no permite detener el proceso;
    /// 4. consulta el código de salida (y si sigue vivo) con `inspect` del exec.
    ///
    /// Nunca reintenta: un comando puede tener efectos aunque la conexión se
    /// corte.
    pub async fn bulk_exec(
        &self,
        cmd: &BulkCommand,
        id: &str,
        display: &str,
        interrupt: CancellationToken,
    ) -> Result<ExecDone, ItemError> {
        let check = |error| ItemError {
            stage: BulkStage::Check,
            error,
        };
        let exec = |error| ItemError {
            stage: BulkStage::Execute,
            error,
        };
        if !is_full_id(id) {
            return Err(check(invalid("se necesita el ID completo del contenedor")));
        }
        cmd.validate().map_err(check)?;

        let info = match self.inspect_silent(id).await {
            Ok(i) => i,
            Err(EngineError::Docker { status: 404, .. }) => {
                return Ok(ExecDone::NotRun {
                    reason: SkipReason::Gone,
                })
            }
            Err(e) => return Err(check(e)),
        };
        if info.id.as_deref() != Some(id) {
            return Err(check(EngineError::InvalidResponse {
                message: "Docker devolvió otro contenedor".into(),
            }));
        }
        let state = info
            .state
            .as_ref()
            .and_then(|s| s.status)
            .and_then(|s| serde_json::to_value(s).ok())
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        if state != "running" {
            return Ok(ExecDone::NotRun {
                reason: SkipReason::State { state },
            });
        }

        let argv = cmd.argv();
        let cfg = ExecConfig {
            attach_stdin: Some(false),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            tty: Some(false),
            cmd: Some(argv.clone()),
            user: cmd.user().map(str::to_owned),
            working_dir: cmd.workdir().map(str::to_owned),
            ..Default::default()
        };
        let create = ApiOp {
            category: Category::Container,
            kind: "container.exec.prepare",
            target: Some(display.to_owned()),
            method: "POST",
            path: format!("/containers/{id}/exec"),
            body: Some(serde_json::to_string_pretty(&cfg).unwrap_or_default()),
            equivalent: None,
            timeout: Some(Duration::from_secs(30)),
        };
        let d = self.docker().clone();
        let id_s = id.to_owned();
        // Un error al crear (usuario inexistente, contenedor pausado…) llega
        // con su código de Docker: el proceso no llegó a lanzarse.
        let created = self
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
            .await
            .map_err(|e| match e {
                EngineError::Docker { .. } | EngineError::InvalidInput { .. } => check(e),
                other => exec(other),
            })?;
        let exec_id = created.id;

        let start_opts = StartExecOptions {
            detach: false,
            tty: false,
            output_capacity: None,
        };
        let handle = self.activity().begin(OperationSpec {
            category: Category::Container,
            kind: "container.exec.run".into(),
            target: Some(display.to_owned()),
            transport: Transport::Api {
                method: "POST".into(),
                path: format!("/exec/{exec_id}/start"),
                endpoint: self.endpoint().socket_path.clone(),
            },
            request_body: Some(serde_json::to_string_pretty(&start_opts).unwrap_or_default()),
            equivalent_command: Some(self.docker_cli(&cmd.exec_args(id))),
        });
        let started_at = Utc::now();
        let mut output = match self.docker().start_exec(&exec_id, Some(start_opts)).await {
            Ok(StartExecResults::Attached { output, .. }) => output,
            Ok(StartExecResults::Detached) => {
                let e = EngineError::InvalidResponse {
                    message: "Docker no adjuntó la salida".into(),
                };
                handle.fail(Failure {
                    error: e.to_string(),
                    ..Default::default()
                });
                return Err(exec(e));
            }
            Err(e) => {
                let e = EngineError::from(e);
                handle.fail(Failure {
                    error: e.to_string(),
                    http_status: e.http_status(),
                    ..Default::default()
                });
                return Err(exec(e));
            }
        };

        let mut out = Capture::default();
        let mut err = Capture::default();
        let deadline = tokio::time::sleep(Duration::from_secs(u64::from(cmd.timeout_secs)));
        tokio::pin!(deadline);
        let end = loop {
            tokio::select! {
                biased;
                _ = interrupt.cancelled() => break Ok(ExecEnd::Interrupted),
                _ = &mut deadline => break Ok(ExecEnd::TimedOut),
                next = output.next() => match next {
                    None => break Ok(ExecEnd::Exited),
                    Some(Ok(LogOutput::StdOut { message } | LogOutput::Console { message })) => out.push(&message),
                    Some(Ok(LogOutput::StdErr { message })) => err.push(&message),
                    Some(Ok(LogOutput::StdIn { .. })) => {}
                    Some(Err(e)) => break Err(EngineError::from(e)),
                },
            }
        };
        // Se suelta la conexión: con tiempo agotado o interrupción el proceso
        // puede seguir dentro del contenedor (Docker no permite pararlo).
        drop(output);
        let end = match end {
            Ok(end) => end,
            Err(e) => {
                handle.fail(Failure {
                    error: format!("conexión perdida durante la ejecución: {e}"),
                    ..Default::default()
                });
                return Err(exec(e));
            }
        };

        let d = self.docker().clone();
        let eid = exec_id.clone();
        let inspected = self
            .call_silent(Some(Duration::from_secs(15)), async move {
                d.inspect_exec(&eid).await
            })
            .await;
        // Si la salida terminó pero Docker no responde, o dice que el proceso
        // sigue vivo, lo que se cerró fue la conexión, no el proceso: el
        // resultado es desconocido (nunca «terminado»).
        if end == ExecEnd::Exited {
            let lost = match &inspected {
                Err(e) => Some(format!("conexión perdida durante la ejecución: {e}")),
                Ok(i) if i.running == Some(true) => {
                    Some("la conexión se cerró antes de que el proceso terminara".to_owned())
                }
                Ok(_) => None,
            };
            if let Some(message) = lost {
                handle.fail(Failure {
                    error: message.clone(),
                    ..Default::default()
                });
                return Err(exec(EngineError::Io { message }));
            }
        }
        let (exit_code, still_running) = match &inspected {
            Ok(i) => (
                if i.running == Some(true) {
                    None
                } else {
                    i.exit_code
                },
                i.running,
            ),
            Err(_) => (None, None),
        };
        let stdout_truncated = out.truncated;
        let stderr_truncated = err.truncated;
        let stdout = out.text();
        let stderr = err.text();
        let launch_failed = is_launch_failure(exit_code, &stdout, &stderr);
        let summary = match (end, exit_code) {
            (ExecEnd::Exited, Some(c)) => format!("Código de salida {c}"),
            (ExecEnd::Exited, None) => "Terminado sin código de salida".to_owned(),
            (ExecEnd::TimedOut, _) => format!(
                "Tiempo máximo agotado ({} s); {}",
                cmd.timeout_secs,
                running_text(still_running)
            ),
            (ExecEnd::Interrupted, _) => {
                format!("Interrupción solicitada; {}", running_text(still_running))
            }
        };
        match (end, exit_code) {
            (ExecEnd::Exited, Some(0)) => handle.succeed(Outcome {
                summary: Some(summary),
                exit_code: Some(0),
                ..Default::default()
            }),
            _ => handle.fail(Failure {
                error: summary,
                exit_code: exit_code.map(|c| c as i32),
                ..Default::default()
            }),
        }
        Ok(ExecDone::Ran(ExecOutput {
            end,
            exit_code,
            still_running,
            stdout,
            stderr,
            stdout_truncated,
            stderr_truncated,
            launch_failed,
            started_at,
        }))
    }
}

fn running_text(r: Option<bool>) -> &'static str {
    match r {
        Some(true) => "el proceso sigue en el contenedor",
        Some(false) => "el proceso ya terminó",
        None => "no se pudo saber si el proceso sigue",
    }
}

/// Texto del comando para mostrarlo (`argv` con comillas donde hacen falta).
pub fn render_command(cmd: &BulkCommand) -> String {
    let argv = cmd.argv();
    match argv.split_first() {
        Some((p, rest)) => render_argv(p, rest),
        None => String::new(),
    }
}

#[cfg(test)]
#[path = "bulk_exec_tests.rs"]
mod tests;
