// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Conexión maestra SSH con un servidor y canales multiplexados sobre ella.
//!
//! - La maestra (`ssh -M -N`) autentica una vez y queda como proceso hijo de la
//!   aplicación; al cerrar la sesión (o al liberarla) se termina.
//! - Cada conexión HTTP de bollard, cada flujo y cada terminal es un canal
//!   (`ssh -S <control> -- host <orden>`) con `kill_on_drop`: no quedan
//!   procesos huérfanos.
//! - Si el perfil necesita un secreto, el servidor askpass vive mientras la
//!   sesión: si el servidor SSH rechaza más canales en la misma conexión
//!   (`MaxSessions`), OpenSSH abre una conexión aparte que también se autentica.

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

#[cfg(windows)]
use super::args::direct_args;
use super::args::{channel_args, control_args, master_args};
use super::askpass::{private_dir, random_hex, AskpassServer, PromptTarget, SOCKET_ENV};
use super::hostkey::{run, Run};
use super::problem::{classify_ssh, RemoteProblem};
use super::profile::{expand_home, validate, SshProfile};
use super::{SshContext, SshPaths};

/// Respuestas de askpass durante una sesión: la maestra y, si el servidor SSH
/// rechaza más canales, las conexiones adicionales que abra OpenSSH.
const ASKPASS_ANSWERS: usize = 64;

/// Máximo de salida de error que se conserva de un proceso `ssh`.
const STDERR_CAP: usize = 16 * 1024;

/// Salida de error de un proceso, leída en segundo plano.
#[derive(Clone, Default)]
pub struct StderrBuf(Arc<parking_lot::Mutex<String>>);

impl StderrBuf {
    pub fn text(&self) -> String {
        self.0.lock().clone()
    }

    fn drain(&self, mut r: tokio::process::ChildStderr) {
        let buf = self.0.clone();
        tokio::spawn(async move {
            let mut chunk = [0u8; 1024];
            while let Ok(n) = r.read(&mut chunk).await {
                if n == 0 {
                    break;
                }
                let mut b = buf.lock();
                if b.len() < STDERR_CAP {
                    b.push_str(&String::from_utf8_lossy(&chunk[..n]));
                }
            }
        });
    }
}

pub struct SshSession {
    profile: SshProfile,
    paths: SshPaths,
    control: PathBuf,
    master: parking_lot::Mutex<Option<Child>>,
    master_err: StderrBuf,
    /// Mientras viva la sesión (ver la nota del módulo).
    _askpass: Option<AskpassServer>,
}

impl std::fmt::Debug for SshSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SshSession")
            .field("server", &self.profile.id)
            .field("control", &self.control)
            .finish()
    }
}

/// Nombre efectivo del destino según `ssh -G` (`HostName` de `~/.ssh/config`):
/// es el que OpenSSH pone en la pregunta de la contraseña.
async fn effective_hostname(paths: &SshPaths, p: &SshProfile) -> Option<String> {
    let r = super::hostkey::run(
        &paths.ssh,
        &super::args::effective_config_args(p),
        std::time::Duration::from_secs(5),
    )
    .await
    .ok()?;
    r.stdout
        .lines()
        .find_map(|l| l.strip_prefix("hostname "))
        .map(|h| h.trim().to_owned())
}

/// Variables de entorno de un `ssh`: askpass si hace falta y nada heredado que
/// pueda alterar la autenticación.
fn ssh_env(c: &mut Command, paths: &SshPaths, askpass: Option<&AskpassServer>) {
    crate::sys::no_console(c);
    c.env_remove("SSH_ASKPASS")
        .env_remove("SSH_ASKPASS_REQUIRE")
        .env_remove(SOCKET_ENV)
        // OpenSSH ejecuta `ProxyCommand` con `$SHELL -c`: el del servidor
        // intermedio está entrecomillado para un shell POSIX (D-087).
        .env("SHELL", "/bin/sh");
    if let Some(a) = askpass {
        c.env("SSH_ASKPASS", &paths.askpass)
            .env("SSH_ASKPASS_REQUIRE", "force")
            .env(SOCKET_ENV, a.socket());
        #[cfg(windows)]
        c.env(super::askpass::TOKEN_ENV, a.token());
    }
}

impl SshSession {
    /// Autentica con el servidor y deja la conexión maestra preparada.
    /// No comprueba Docker (eso lo hace quien conecta).
    // En Windows, la rama propia termina con `return` (sin ControlMaster).
    #[cfg_attr(windows, allow(unreachable_code))]
    pub async fn open(ctx: &SshContext, p: &SshProfile) -> Result<Arc<Self>, RemoteProblem> {
        let issues = validate(p);
        if !issues.is_empty() {
            return Err(RemoteProblem::InvalidProfile {
                codes: issues.into_iter().map(|i| i.code).collect(),
            });
        }
        let paths = ctx.paths.clone();
        if let Some(key) = p.auth.key_path() {
            let path = expand_home(key);
            // Solo se comprueba que exista: la clave no se lee.
            if std::fs::metadata(&path).is_err() {
                return Err(RemoteProblem::KeyUnreadable { path });
            }
        }
        private_dir(&paths.runtime_dir).map_err(|e| RemoteProblem::Other {
            message: format!("{}: {e}", paths.runtime_dir.display()),
        })?;
        let askpass = match p.auth.secret_kind() {
            None => None,
            Some(kind) => {
                let secret = ctx
                    .secrets
                    .secret(&p.id, kind)
                    .ok_or(RemoteProblem::SecretRequired { secret: kind })?;
                // Solo se responde a las preguntas de este destino (D-087).
                let mut hosts = vec![p.host.clone()];
                if let Some(h) = effective_hostname(&paths, p).await {
                    if !hosts.contains(&h) {
                        hosts.push(h);
                    }
                }
                let target = PromptTarget {
                    user: p.user.clone(),
                    hosts,
                    key_path: p.auth.key_path().map(expand_home),
                };
                Some(
                    AskpassServer::start(&paths.runtime_dir, kind, secret, ASKPASS_ANSWERS, target)
                        .await
                        .map_err(|e| RemoteProblem::Other {
                            message: e.to_string(),
                        })?,
                )
            }
        };
        // Windows: sin conexión maestra. Se comprueba que se autentica con una
        // conexión directa que solo ejecuta `true`; después, cada canal es una
        // conexión directa con las mismas opciones (1.9.0, D-114).
        #[cfg(windows)]
        {
            if !p.options.proxy_jump.is_empty() {
                return Err(RemoteProblem::Other {
                    message: "Los servidores intermedios (ProxyJump) todavía no están disponibles en Windows.".into(),
                });
            }
            let mut cmd = Command::new(&paths.ssh);
            cmd.args(direct_args(p, &paths, &["true".to_owned()], false))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            ssh_env(&mut cmd, &paths, askpass.as_ref());
            let child = cmd.spawn().map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    RemoteProblem::SshMissing
                } else {
                    RemoteProblem::Other {
                        message: e.to_string(),
                    }
                }
            })?;
            let limit = Duration::from_secs(u64::from(p.options.connect_timeout) + 20);
            let out = match tokio::time::timeout(limit, child.wait_with_output()).await {
                Ok(Ok(o)) => o,
                Ok(Err(e)) => {
                    return Err(RemoteProblem::Other {
                        message: e.to_string(),
                    })
                }
                Err(_) => return Err(RemoteProblem::Timeout),
            };
            if !out.status.success() {
                return Err(classify_ssh(
                    out.status.code(),
                    &String::from_utf8_lossy(&out.stderr),
                    &p.host,
                    p.port,
                ));
            }
            #[allow(clippy::needless_return)]
            return Ok(Arc::new(Self {
                profile: p.clone(),
                paths,
                control: PathBuf::new(),
                master: parking_lot::Mutex::new(None),
                master_err: StderrBuf::default(),
                _askpass: askpass,
            }));
        }
        let short: String =
            p.id.chars()
                .rev()
                .take(6)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
        // El PID de esta instancia en el nombre: al arrancar se cierran solo las
        // sesiones de instancias que ya no existen (`close_orphans`).
        let control = paths.runtime_dir.join(format!(
            "c-{}-{short}-{}",
            std::process::id(),
            random_hex(3)
        ));
        let mut cmd = Command::new(&paths.ssh);
        cmd.args(master_args(p, &paths, &control))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        ssh_env(&mut cmd, &paths, askpass.as_ref());
        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                RemoteProblem::SshMissing
            } else {
                RemoteProblem::Other {
                    message: e.to_string(),
                }
            }
        })?;
        let err = StderrBuf::default();
        if let Some(e) = child.stderr.take() {
            err.drain(e);
        }
        let deadline = tokio::time::Instant::now()
            + Duration::from_secs(u64::from(p.options.connect_timeout) + 20);
        loop {
            if let Ok(Some(status)) = child.try_wait() {
                // Deja que termine de leerse la salida de error.
                tokio::time::sleep(Duration::from_millis(50)).await;
                let _ = std::fs::remove_file(&control);
                return Err(classify_ssh(status.code(), &err.text(), &p.host, p.port));
            }
            if control.exists() {
                let check = run(
                    &paths.ssh,
                    &control_args(p, &control, "check"),
                    Duration::from_secs(5),
                )
                .await;
                if matches!(check, Ok(Run { code: Some(0), .. })) {
                    break;
                }
            }
            if tokio::time::Instant::now() >= deadline {
                let _ = child.start_kill();
                let _ = std::fs::remove_file(&control);
                return Err(RemoteProblem::Timeout);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok(Arc::new(Self {
            profile: p.clone(),
            paths,
            control,
            master: parking_lot::Mutex::new(Some(child)),
            master_err: err,
            _askpass: askpass,
        }))
    }

    pub fn profile(&self) -> &SshProfile {
        &self.profile
    }

    pub fn control_path(&self) -> &Path {
        &self.control
    }

    /// Orden `ssh` para un canal que ejecuta `remote` en el servidor.
    // En Windows, la rama propia termina con `return` (sin ControlMaster).
    #[cfg_attr(windows, allow(unreachable_code))]
    pub fn channel(&self, remote: &[String], tty: bool) -> Command {
        let mut c = Command::new(&self.paths.ssh);
        #[cfg(windows)]
        {
            c.args(direct_args(&self.profile, &self.paths, remote, tty))
                .kill_on_drop(true);
            ssh_env(&mut c, &self.paths, self._askpass.as_ref());
            return c;
        }
        c.args(channel_args(
            &self.profile,
            &self.paths,
            &self.control,
            remote,
            tty,
        ))
        .kill_on_drop(true);
        ssh_env(&mut c, &self.paths, self._askpass.as_ref());
        c
    }

    /// Ejecuta una orden corta en el servidor y recoge su salida.
    pub async fn run(
        &self,
        remote: &[String],
        timeout: Duration,
    ) -> Result<RemoteOutput, RemoteProblem> {
        let mut c = self.channel(remote, false);
        c.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let child = c.spawn().map_err(|e| RemoteProblem::Other {
            message: e.to_string(),
        })?;
        match tokio::time::timeout(timeout, child.wait_with_output()).await {
            Ok(Ok(o)) => Ok(RemoteOutput {
                code: o.status.code(),
                stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
            }),
            Ok(Err(e)) => Err(RemoteProblem::Other {
                message: e.to_string(),
            }),
            Err(_) => Err(RemoteProblem::Timeout),
        }
    }

    /// Abre un canal `docker system dial-stdio`: un flujo hacia la API remota.
    pub async fn dial(&self) -> std::io::Result<SshStream> {
        let remote = super::args::dial_stdio(&self.profile.docker);
        let mut c = self.channel(&remote, false);
        c.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = c.spawn()?;
        let err = StderrBuf::default();
        if let Some(e) = child.stderr.take() {
            err.drain(e);
        }
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| std::io::Error::other("sin entrada"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| std::io::Error::other("sin salida"))?;
        Ok(SshStream {
            _child: child,
            stdin: Some(stdin),
            stdout,
            stderr: err,
        })
    }

    /// La conexión maestra sigue viva (en Windows, que se puede conectar).
    // En Windows, la rama propia termina con `return` (sin ControlMaster).
    #[cfg_attr(windows, allow(unreachable_code))]
    pub async fn alive(&self) -> bool {
        #[cfg(windows)]
        return matches!(
            self.run(&["true".to_owned()], Duration::from_secs(20))
                .await,
            Ok(RemoteOutput { code: Some(0), .. })
        );
        if let Some(c) = self.master.lock().as_mut() {
            if !matches!(c.try_wait(), Ok(None)) {
                return false;
            }
        } else {
            return false;
        }
        matches!(
            run(
                &self.paths.ssh,
                &control_args(&self.profile, &self.control, "check"),
                Duration::from_secs(5)
            )
            .await,
            Ok(Run { code: Some(0), .. })
        )
    }

    /// Salida de error de la conexión maestra (diagnóstico de una pérdida).
    pub fn master_stderr(&self) -> String {
        self.master_err.text()
    }

    /// Cierra la conexión maestra y con ella todos sus canales.
    pub async fn close(&self) {
        // Windows: no hay maestra; los canales terminan con sus procesos.
        #[cfg(windows)]
        if self.control.as_os_str().is_empty() {
            return;
        }
        let _ = run(
            &self.paths.ssh,
            &control_args(&self.profile, &self.control, "exit"),
            Duration::from_secs(5),
        )
        .await;
        let child = self.master.lock().take();
        if let Some(mut c) = child {
            let _ = c.start_kill();
            let _ = tokio::time::timeout(Duration::from_secs(5), c.wait()).await;
        }
        let _ = std::fs::remove_file(&self.control);
    }
}

impl Drop for SshSession {
    fn drop(&mut self) {
        if let Some(mut c) = self.master.lock().take() {
            let _ = c.start_kill();
        }
        if !self.control.as_os_str().is_empty() {
            let _ = std::fs::remove_file(&self.control);
        }
    }
}

/// Resultado de una orden remota corta.
#[derive(Debug, Clone)]
pub struct RemoteOutput {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Flujo bidireccional sobre un canal SSH (`docker system dial-stdio`).
/// Al liberarse termina el proceso `ssh` del canal.
pub struct SshStream {
    _child: Child,
    stdin: Option<ChildStdin>,
    stdout: ChildStdout,
    stderr: StderrBuf,
}

impl SshStream {
    /// Lo que el canal escribió en su salida de error (p. ej. por qué falló el CLI remoto).
    pub fn stderr(&self) -> String {
        self.stderr.text()
    }
}

impl AsyncRead for SshStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.stdout).poll_read(cx, buf)
    }
}

impl AsyncWrite for SshStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.stdin.as_mut() {
            Some(w) => Pin::new(w).poll_write(cx, buf),
            None => Poll::Ready(Err(std::io::ErrorKind::BrokenPipe.into())),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.stdin.as_mut() {
            Some(w) => Pin::new(w).poll_flush(cx),
            None => Poll::Ready(Ok(())),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.stdin.as_mut() {
            Some(w) => {
                let r = Pin::new(w).poll_shutdown(cx);
                if r.is_ready() {
                    self.stdin = None;
                }
                r
            }
            None => Poll::Ready(Ok(())),
        }
    }
}

impl hyper_util::client::legacy::connect::Connection for SshStream {
    fn connected(&self) -> hyper_util::client::legacy::connect::Connected {
        hyper_util::client::legacy::connect::Connected::new()
    }
}

/// Conector de hyper para bollard: cada conexión es un canal `dial-stdio`.
#[derive(Clone)]
pub struct SshConnector {
    pub session: Arc<SshSession>,
}

impl tower_service::Service<hyper::Uri> for SshConnector {
    type Response = hyper_util::rt::TokioIo<SshStream>;
    type Error = std::io::Error;
    type Future =
        Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: hyper::Uri) -> Self::Future {
        let s = self.session.clone();
        Box::pin(async move { s.dial().await.map(hyper_util::rt::TokioIo::new) })
    }
}

/// Cierra las conexiones maestras que dejó una instancia de la aplicación que
/// terminó sin poder cerrarlas (p. ej. por un cierre forzado). Solo actúa
/// sobre sockets de control del directorio privado de la aplicación cuyo PID
/// ya no existe; nunca sobre los de otra instancia en marcha. Devuelve cuántos.
pub async fn close_orphans(paths: &SshPaths) -> usize {
    // Windows: no hay conexiones maestras.
    #[cfg(windows)]
    {
        let _ = paths;
        #[allow(clippy::needless_return)]
        return 0;
    }
    #[cfg(unix)]
    close_orphans_unix(paths).await
}

#[cfg(unix)]
async fn close_orphans_unix(paths: &SshPaths) -> usize {
    let Ok(dir) = std::fs::read_dir(&paths.runtime_dir) else {
        return 0;
    };
    let mut closed = 0;
    for entry in dir.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(pid) = name
            .strip_prefix("c-")
            .and_then(|r| r.split('-').next())
            .and_then(|p| p.parse::<i32>().ok())
        else {
            continue;
        };
        if pid <= 1 || process_alive(pid) {
            continue;
        }
        let sock = entry.path();
        let args = vec![
            "-S".to_owned(),
            sock.display().to_string(),
            "-O".into(),
            "exit".into(),
            "--".into(),
            "cmc-orphan".into(),
        ];
        let _ = super::hostkey::run(&paths.ssh, &args, Duration::from_secs(5)).await;
        let _ = std::fs::remove_file(&sock);
        closed += 1;
    }
    closed
}

#[cfg(unix)]
fn process_alive(pid: i32) -> bool {
    // SAFETY: kill con la señal 0 solo comprueba si el proceso existe.
    let r = unsafe { libc::kill(pid, 0) };
    r == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::ssh::profile::{RemoteDocker, SshAuth, SshOptions};

    pub(crate) fn profile_for(host: &str, port: u16, user: &str) -> SshProfile {
        SshProfile {
            id: "srv-0123456789abcdef".into(),
            name: "Prueba".into(),
            host: host.into(),
            port,
            user: user.into(),
            description: String::new(),
            auth: SshAuth::Agent,
            docker: RemoteDocker::Default,
            options: SshOptions::default(),
        }
    }
}
