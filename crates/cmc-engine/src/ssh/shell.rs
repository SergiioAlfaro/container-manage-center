// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Terminal SSH del propio servidor (0.10.0): una shell interactiva en el
//! servidor, distinta de la terminal de un contenedor.
//!
//! `ssh -tt` va por la sesión maestra verificada (mismas opciones de
//! seguridad e identidad) y se ejecuta sobre una **PTY local**: así `ssh`
//! tiene terminal de control y los cambios de tamaño llegan al servidor como
//! en cualquier terminal. Al cerrar la sesión se termina el `ssh` del canal y
//! el servidor envía SIGHUP a la shell: no quedan procesos.
//!
//! No se ejecuta ninguna orden propia: solo se abre la shell de inicio de
//! sesión del usuario; lo que se escriba lo decide el usuario.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::Arc;
use std::time::Duration;

use cmc_activity::{Category, Failure, OperationSpec, Outcome, Transport};
use tokio::io::unix::AsyncFd;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::SshSession;
use crate::error::EngineError;
use crate::streams::{EndReason, StreamEnd, TerminalInput};

/// Tiempo para que `ssh` termine tras pedírselo antes de forzarlo.
const GRACE: Duration = Duration::from_secs(5);

fn io_err(what: &str, e: std::io::Error) -> EngineError {
    EngineError::Other {
        message: format!("{what}: {e}"),
    }
}

fn winsize(cols: u16, rows: u16) -> libc::winsize {
    libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    }
}

/// Par PTY (maestro, esclavo) con el tamaño indicado.
fn open_pty(cols: u16, rows: u16) -> Result<(OwnedFd, OwnedFd), EngineError> {
    let mut master: RawFd = -1;
    let mut slave: RawFd = -1;
    let ws = winsize(cols, rows);
    // SAFETY: punteros válidos a enteros y a `winsize`; nombre y termios nulos.
    let r = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null(),
            &ws,
        )
    };
    if r != 0 {
        return Err(io_err("openpty", std::io::Error::last_os_error()));
    }
    // SAFETY: openpty devolvió dos descriptores propios.
    let (m, s) = unsafe { (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(slave)) };
    for fd in [m.as_raw_fd(), s.as_raw_fd()] {
        // SAFETY: fd válido; solo se añade FD_CLOEXEC.
        unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) };
    }
    // SAFETY: fd válido; solo se añade O_NONBLOCK al maestro.
    unsafe {
        let fl = libc::fcntl(m.as_raw_fd(), libc::F_GETFL);
        libc::fcntl(m.as_raw_fd(), libc::F_SETFL, fl | libc::O_NONBLOCK);
    }
    Ok((m, s))
}

fn set_size(master: RawFd, cols: u16, rows: u16) {
    let ws = winsize(cols.clamp(10, 1000), rows.clamp(3, 500));
    // SAFETY: fd válido del maestro y puntero a `winsize`.
    unsafe { libc::ioctl(master, libc::TIOCSWINSZ, &ws) };
}

fn write_all(master: RawFd, mut data: &[u8]) -> std::io::Result<()> {
    while !data.is_empty() {
        // SAFETY: fd válido; puntero y longitud del búfer.
        let n = unsafe { libc::write(master, data.as_ptr().cast(), data.len()) };
        if n < 0 {
            let e = std::io::Error::last_os_error();
            if e.kind() == std::io::ErrorKind::WouldBlock {
                std::thread::sleep(Duration::from_millis(2));
                continue;
            }
            if e.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(e);
        }
        data = &data[n as usize..];
    }
    Ok(())
}

/// Orden `ssh` equivalente para el registro (sin opciones internas).
fn transport(p: &super::SshProfile) -> Transport {
    let mut args = Vec::new();
    if p.port != 22 {
        args.push("-p".to_owned());
        args.push(p.port.to_string());
    }
    if !p.options.proxy_jump.is_empty() {
        args.push("-J".to_owned());
        args.push(p.options.proxy_jump.clone());
    }
    args.push("-t".to_owned());
    args.push(format!("{}@{}", p.user, p.host));
    Transport::Cli {
        program: "ssh".into(),
        args,
    }
}

/// Abre la shell del servidor y la mantiene hasta que termina o se cancela.
///
/// Recibe el motor del servidor: la sesión y el registro de actividad (ya
/// etiquetado con su servidor) salen de él, así que no se pueden mezclar.
pub async fn run_server_shell<F>(
    engine: &crate::client::Engine,
    size: (u16, u16),
    cancel: CancellationToken,
    mut input: mpsc::Receiver<TerminalInput>,
    mut on_output: F,
) -> Result<StreamEnd, EngineError>
where
    F: FnMut(Vec<u8>) -> bool,
{
    let session: Arc<SshSession> =
        engine
            .ssh_session()
            .cloned()
            .ok_or_else(|| EngineError::Other {
                message: "la terminal SSH solo existe para servidores remotos".into(),
            })?;
    let activity = engine.activity();
    let p = session.profile().clone();
    let handle = activity.begin(OperationSpec {
        category: Category::System,
        kind: "server.shell".into(),
        target: Some(p.name.clone()),
        transport: transport(&p),
        request_body: None,
        equivalent_command: None,
    });
    let (master, slave) = match open_pty(size.0.clamp(10, 1000), size.1.clamp(3, 500)) {
        Ok(x) => x,
        Err(e) => {
            handle.fail(Failure {
                error: e.to_string(),
                ..Default::default()
            });
            return Err(e);
        }
    };
    let mut cmd = session.channel(&[], true);
    let dup = |fd: &OwnedFd| fd.try_clone().map(std::process::Stdio::from);
    let (si, so, se) = match (dup(&slave), dup(&slave), dup(&slave)) {
        (Ok(a), Ok(b), Ok(c)) => (a, b, c),
        _ => {
            let e = io_err("pty", std::io::Error::last_os_error());
            handle.fail(Failure {
                error: e.to_string(),
                ..Default::default()
            });
            return Err(e);
        }
    };
    cmd.stdin(si).stdout(so).stderr(se).kill_on_drop(true);
    // SAFETY: entre fork y exec solo se llaman funciones async-signal-safe:
    // nueva sesión y la PTY como terminal de control (para SIGWINCH).
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let e = io_err("ssh", e);
            handle.fail(Failure {
                error: e.to_string(),
                ..Default::default()
            });
            return Err(e);
        }
    };
    drop(slave);
    let pid = child.id();
    let master_fd = master.as_raw_fd();
    // SAFETY: `master` es un OwnedFd abierto que AsyncFd posee hasta el
    // final; su descriptor no cambia mientras está registrado.
    let afd = unsafe { AsyncFd::register(master) }.map_err(|e| io_err("pty", e.into()))?;
    let mut buf = vec![0u8; 16 * 1024];
    let mut closed_by_user = false;
    let mut listener_gone = false;
    let mut status: Option<std::process::ExitStatus> = None;

    loop {
        tokio::select! {
            r = afd.readable() => {
                let mut guard = r.map_err(|e| io_err("pty", e))?;
                // SAFETY: fd válido; búfer propio.
                let n = unsafe { libc::read(master_fd, buf.as_mut_ptr().cast(), buf.len()) };
                if n > 0 {
                    if !on_output(buf[..n as usize].to_vec()) {
                        listener_gone = true;
                        closed_by_user = true;
                        break;
                    }
                } else if n == 0 {
                    break;
                } else {
                    let e = std::io::Error::last_os_error();
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        guard.clear_ready();
                    } else {
                        // EIO: el otro extremo se cerró (ssh terminó).
                        break;
                    }
                }
            }
            msg = input.recv() => match msg {
                Some(TerminalInput::Data(d)) => {
                    if write_all(master_fd, &d).is_err() {
                        break;
                    }
                }
                Some(TerminalInput::Resize { cols, rows }) => set_size(master_fd, cols, rows),
                None => {}
            },
            s = child.wait() => {
                status = s.ok();
                // Lo que quede en la PTY.
                loop {
                    // SAFETY: fd válido; búfer propio.
                    let n = unsafe { libc::read(master_fd, buf.as_mut_ptr().cast(), buf.len()) };
                    if n <= 0 || !on_output(buf[..n as usize].to_vec()) {
                        break;
                    }
                }
                break;
            }
            _ = cancel.cancelled() => {
                closed_by_user = true;
                break;
            }
        }
    }

    if status.is_none() {
        // Cerrar: SIGHUP a la sesión de ssh (como al cerrar una terminal) y,
        // si no termina, SIGKILL.
        if let Some(pid) = pid {
            // SAFETY: señal a la sesión (grupo) propia del ssh lanzado.
            unsafe { libc::kill(-(pid as i32), libc::SIGHUP) };
        }
        status = match tokio::time::timeout(GRACE, child.wait()).await {
            Ok(s) => s.ok(),
            Err(_) => {
                let _ = child.kill().await;
                child.wait().await.ok()
            }
        };
    }
    drop(afd);
    let code = status.and_then(|s| s.code());
    let end = if closed_by_user {
        StreamEnd {
            reason: EndReason::Closed,
            summary: if listener_gone {
                "Sesión SSH cerrada (la ventana dejó de escuchar)".into()
            } else {
                "Sesión SSH cerrada por el usuario".into()
            },
            exit_code: code.map(i64::from),
            container: None,
        }
    } else {
        StreamEnd {
            reason: EndReason::Finished,
            summary: match code {
                Some(0) => "La sesión SSH terminó".into(),
                Some(c) => format!("La sesión SSH terminó con código {c}"),
                None => "La sesión SSH terminó por una señal".into(),
            },
            exit_code: code.map(i64::from),
            container: None,
        }
    };
    handle.succeed(Outcome {
        summary: Some(end.summary.clone()),
        exit_code: code,
        ..Default::default()
    });
    Ok(end)
}
