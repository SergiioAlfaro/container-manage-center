// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Operaciones de Docker de las copias: contenedor auxiliar, archivos por la
//! API de archivos (`/containers/{id}/archive`) y `exec` con la salida
//! volcada a un archivo. Todas quedan en el panel de actividad.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use bollard::container::LogOutput;
use bollard::exec::StartExecResults;
use bollard::models::{ContainerCreateBody, ExecConfig, HostConfig};
use bollard::query_parameters::{
    CreateContainerOptions, DownloadFromContainerOptions, RemoveContainerOptions,
    UploadToContainerOptions,
};
use bytes::Bytes;
use cmc_activity::Category;
use futures_util::StreamExt;
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

use super::store::HashedFile;
use crate::client::{ApiOp, Engine};
use crate::error::EngineError;
use crate::resources::path_with_query;

/// Ruta donde el contenedor auxiliar monta el volumen.
pub const HELPER_MOUNT: &str = "/cmc-volume";
/// Etiqueta de los contenedores auxiliares.
pub const HELPER_LABEL: &str = "cmc.backup.helper";
/// Orden del auxiliar: **nunca se arranca**; solo hace falta para crearlo.
const HELPER_CMD: &str = "/cmc-helper-never-started";
/// Tiempo máximo de una subida (restauración) grande.
const UPLOAD_TIMEOUT: Duration = Duration::from_secs(24 * 3600);
/// Salida de error que se conserva de un `exec`.
const STDERR_MAX: usize = 64 * 1024;

pub(crate) fn cancelled() -> EngineError {
    EngineError::Other {
        message: "operación cancelada".into(),
    }
}

fn io(e: std::io::Error) -> EngineError {
    EngineError::Io {
        message: e.to_string(),
    }
}

/// Salida estándar máxima que se guarda en memoria.
const CAPTURE_MAX: usize = 64 * 1024;

/// Dónde va la salida estándar de un `exec`.
pub(crate) enum ExecOut<'a> {
    File(&'a mut HashedFile),
    Buffer(&'a mut Vec<u8>),
    Discard,
}

/// Resultado de un `exec`.
#[derive(Debug, Default)]
pub struct ExecResult {
    pub exit_code: Option<i64>,
    pub stderr: String,
    pub stdout_bytes: u64,
}

impl Engine {
    /// Crea (sin arrancarlo) un contenedor auxiliar que monta `volume` en
    /// [`HELPER_MOUNT`]. Usa una imagen que ya está en el equipo: no se
    /// descarga nada ni se ejecuta ningún código (D-097).
    pub(crate) async fn helper_create(
        &self,
        volume: &str,
        image: &str,
        read_only: bool,
    ) -> Result<String, EngineError> {
        let name = format!("cmc-backup-helper-{}", crate::ssh::askpass::random_hex(6));
        let mode = if read_only { ":ro" } else { "" };
        let bind = format!("{volume}:{HELPER_MOUNT}{mode}");
        let body = ContainerCreateBody {
            image: Some(image.to_owned()),
            cmd: Some(vec![HELPER_CMD.into()]),
            entrypoint: Some(vec![HELPER_CMD.into()]),
            network_disabled: Some(true),
            labels: Some(HashMap::from([(HELPER_LABEL.to_owned(), "1".to_owned())])),
            host_config: Some(HostConfig {
                binds: Some(vec![bind.clone()]),
                network_mode: Some("none".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let opts = CreateContainerOptions {
            name: Some(name.clone()),
            platform: String::new(),
        };
        let op = ApiOp {
            category: Category::Backup,
            kind: "backup.helper.create",
            target: Some(volume.to_owned()),
            method: "POST",
            path: path_with_query("/containers/create", &opts),
            body: Some(serde_json::to_string_pretty(&body).unwrap_or_default()),
            equivalent: Some(vec![
                "create".into(),
                "--name".into(),
                name.clone(),
                "--network".into(),
                "none".into(),
                "--label".into(),
                format!("{HELPER_LABEL}=1"),
                "-v".into(),
                bind,
                "--entrypoint".into(),
                HELPER_CMD.into(),
                image.to_owned(),
            ]),
            timeout: Some(Duration::from_secs(60)),
        };
        let d = self.docker().clone();
        let r = self
            .call(
                op,
                async move { d.create_container(Some(opts), body).await },
                |_| Some("Contenedor auxiliar creado (no se arranca)".into()),
            )
            .await?;
        Ok(r.id)
    }

    /// Elimina un contenedor auxiliar con `v=true`: así se van también los
    /// volúmenes **anónimos** que crea Docker si su imagen declara `VOLUME`.
    /// Docker nunca elimina un volumen con nombre al quitar un contenedor: el
    /// volumen copiado o restaurado se queda (lo comprueba `live_backup`).
    pub(crate) async fn helper_remove(&self, id: &str) -> Result<(), EngineError> {
        let opts = RemoveContainerOptions {
            v: true,
            force: true,
            link: false,
        };
        let op = ApiOp {
            category: Category::Backup,
            kind: "backup.helper.remove",
            target: Some(id.chars().take(12).collect()),
            method: "DELETE",
            path: path_with_query(&format!("/containers/{id}"), &opts),
            body: None,
            equivalent: Some(vec![
                "rm".into(),
                "--force".into(),
                "--volumes".into(),
                id.into(),
            ]),
            timeout: Some(Duration::from_secs(60)),
        };
        let d = self.docker().clone();
        let id_s = id.to_owned();
        self.call(
            op,
            async move { d.remove_container(&id_s, Some(opts)).await },
            |_| Some("Contenedor auxiliar eliminado".into()),
        )
        .await
    }

    /// Descarga `path` de un contenedor como `tar` y lo escribe en `out`.
    pub(crate) async fn archive_download<F>(
        &self,
        container: &str,
        target: &str,
        path: &str,
        out: &mut HashedFile,
        cancel: &CancellationToken,
        mut on_bytes: F,
    ) -> Result<u64, EngineError>
    where
        F: FnMut(u64) + Send,
    {
        let opts = DownloadFromContainerOptions { path: path.into() };
        let op = ApiOp {
            category: Category::Backup,
            kind: "backup.archive.get",
            target: Some(target.to_owned()),
            method: "GET",
            path: path_with_query(&format!("/containers/{container}/archive"), &opts),
            body: None,
            equivalent: Some(vec!["cp".into(), format!("{container}:{path}"), "-".into()]),
            timeout: None,
        };
        let d = self.docker().clone();
        let id = container.to_owned();
        self.call(
            op,
            async {
                let mut s = d.download_from_container(&id, Some(opts));
                let mut n = 0u64;
                loop {
                    let chunk = tokio::select! {
                        () = cancel.cancelled() => return Err(cancelled()),
                        c = s.next() => c,
                    };
                    let Some(chunk) = chunk else { break };
                    let chunk = chunk.map_err(EngineError::from)?;
                    out.write(&chunk).await.map_err(io)?;
                    n += chunk.len() as u64;
                    on_bytes(n);
                }
                Ok::<_, EngineError>(n)
            },
            |n| Some(format!("{n} bytes copiados")),
        )
        .await
    }

    /// Los primeros bytes (como mucho `max`) de un `tar` de `path`, y si se
    /// leyó entero.
    pub(crate) async fn archive_peek(
        &self,
        container: &str,
        target: &str,
        path: &str,
        max: usize,
    ) -> Result<(Vec<u8>, bool), EngineError> {
        let opts = DownloadFromContainerOptions { path: path.into() };
        let op = ApiOp {
            category: Category::Backup,
            kind: "backup.archive.peek",
            target: Some(target.to_owned()),
            method: "GET",
            path: path_with_query(&format!("/containers/{container}/archive"), &opts),
            body: None,
            equivalent: None,
            timeout: Some(Duration::from_secs(60)),
        };
        let d = self.docker().clone();
        let id = container.to_owned();
        self.call(
            op,
            async move {
                let mut s = d.download_from_container(&id, Some(opts));
                let mut buf = Vec::new();
                while let Some(c) = s.next().await {
                    buf.extend_from_slice(&c?);
                    if buf.len() >= max {
                        return Ok::<_, bollard::errors::Error>((buf, false));
                    }
                }
                Ok((buf, true))
            },
            |(b, all)| {
                Some(format!(
                    "{} bytes leídos{}",
                    b.len(),
                    if *all { "" } else { " (parcial)" }
                ))
            },
        )
        .await
    }

    /// Sube un `tar` y lo extrae en `path` del contenedor.
    pub(crate) async fn archive_upload<S>(
        &self,
        container: &str,
        target: &str,
        path: &str,
        tar: S,
        size: u64,
        cancel: &CancellationToken,
    ) -> Result<(), EngineError>
    where
        S: futures_util::Stream<Item = Bytes> + Send + 'static,
    {
        let opts = UploadToContainerOptions {
            path: path.into(),
            no_overwrite_dir_non_dir: Some("true".into()),
            copy_uidgid: None,
        };
        let op = ApiOp {
            category: Category::Backup,
            kind: "backup.archive.put",
            target: Some(target.to_owned()),
            method: "PUT",
            path: path_with_query(&format!("/containers/{container}/archive"), &opts),
            body: Some(format!("<tar de {size} bytes>")),
            equivalent: Some(vec!["cp".into(), "-".into(), format!("{container}:{path}")]),
            timeout: None,
        };
        // El tiempo máximo del cliente cuenta hasta la respuesta, que llega
        // después de enviar todo el cuerpo: en las subidas se amplía.
        let d = self.docker().clone().with_timeout(UPLOAD_TIMEOUT);
        let id = container.to_owned();
        self.call(
            op,
            async move {
                tokio::select! {
                    () = cancel.cancelled() => Err(cancelled()),
                    r = d.upload_to_container(&id, Some(opts), bollard::body_stream(tar)) => r.map_err(EngineError::from),
                }
            },
            |_| Some(format!("{size} bytes restaurados")),
        )
        .await
    }

    /// Ejecuta `sh -c SCRIPT sh ARGS…` dentro de un contenedor en marcha. La
    /// salida estándar va a `out` (si se da) o se descarta; se conserva un
    /// trozo de la de error. `user`: `Some("0")` para root.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn exec_script(
        &self,
        container: &str,
        target: &str,
        kind: &'static str,
        script: &str,
        args: &[String],
        user: Option<&str>,
        mut out: ExecOut<'_>,
        cancel: &CancellationToken,
    ) -> Result<ExecResult, EngineError> {
        let mut cmd: Vec<String> = vec!["/bin/sh".into(), "-c".into(), script.into(), "sh".into()];
        cmd.extend(args.iter().cloned());
        let cfg = ExecConfig {
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            tty: Some(false),
            user: user.map(str::to_owned),
            cmd: Some(cmd.clone()),
            ..Default::default()
        };
        let create = ApiOp {
            category: Category::Backup,
            kind: "backup.exec.create",
            target: Some(target.to_owned()),
            method: "POST",
            path: format!("/containers/{container}/exec"),
            // El guion es constante y no lleva secretos: se registra entero.
            body: Some(serde_json::to_string_pretty(&cfg).unwrap_or_default()),
            equivalent: None,
            timeout: Some(Duration::from_secs(30)),
        };
        let d = self.docker().clone();
        let id_s = container.to_owned();
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
        let mut eq = vec!["exec".to_owned()];
        if let Some(u) = user {
            eq.push("--user".into());
            eq.push(u.into());
        }
        eq.push(container.to_owned());
        eq.extend(cmd);
        let start = ApiOp {
            category: Category::Backup,
            kind,
            target: Some(target.to_owned()),
            method: "POST",
            path: format!("/exec/{}/start", exec.id),
            body: None,
            equivalent: Some(eq),
            timeout: None,
        };
        let d = self.docker().clone();
        let eid = exec.id.clone();
        let mut res = self
            .call(
                start,
                async {
                    let mut r = ExecResult::default();
                    if let StartExecResults::Attached { mut output, .. } =
                        d.start_exec(&eid, None).await?
                    {
                        loop {
                            let frame = tokio::select! {
                                () = cancel.cancelled() => return Err(cancelled()),
                                f = output.next() => f,
                            };
                            let Some(frame) = frame else { break };
                            match frame? {
                                LogOutput::StdOut { message } => {
                                    r.stdout_bytes += message.len() as u64;
                                    match &mut out {
                                        ExecOut::File(o) => o.write(&message).await.map_err(io)?,
                                        ExecOut::Buffer(b) => {
                                            if b.len() < CAPTURE_MAX {
                                                b.extend_from_slice(&message);
                                            }
                                        }
                                        ExecOut::Discard => {}
                                    }
                                }
                                LogOutput::StdErr { message } if r.stderr.len() < STDERR_MAX => {
                                    r.stderr.push_str(&String::from_utf8_lossy(&message));
                                }
                                _ => {}
                            }
                        }
                    }
                    Ok::<_, EngineError>(r)
                },
                |r| Some(format!("{} bytes de salida", r.stdout_bytes)),
            )
            .await?;
        res.stderr = cmc_activity::redact::redact_text(&res.stderr).into_owned();
        // Docker puede tardar en publicar el código de salida (Docker 24).
        for _ in 0..20 {
            let d = self.docker().clone();
            let eid = exec.id.clone();
            let i = self
                .call_silent(Some(Duration::from_secs(10)), async move {
                    d.inspect_exec(&eid).await
                })
                .await?;
            if i.running != Some(true) && i.exit_code.is_some() {
                res.exit_code = i.exit_code;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok(res)
    }

    /// Ejecuta un guion corto y devuelve su salida estándar (máx. 64 KiB).
    pub(crate) async fn exec_capture(
        &self,
        container: &str,
        target: &str,
        kind: &'static str,
        script: &str,
        cancel: &CancellationToken,
    ) -> Result<(String, ExecResult), EngineError> {
        let mut buf = Vec::new();
        let r = self
            .exec_script(
                container,
                target,
                kind,
                script,
                &[],
                None,
                ExecOut::Buffer(&mut buf),
                cancel,
            )
            .await?;
        Ok((String::from_utf8_lossy(&buf).into_owned(), r))
    }
}

/// Contenido de un archivo como flujo, en trozos de 256 KiB. Un error de
/// lectura corta el flujo; la subida queda incompleta y Docker la rechaza.
pub(crate) fn file_stream(f: tokio::fs::File) -> impl futures_util::Stream<Item = Bytes> + Send {
    futures_util::stream::unfold(Some(f), |f| async move {
        let mut f = f?;
        let mut buf = vec![0u8; 256 * 1024];
        match f.read(&mut buf).await {
            Ok(0) | Err(_) => None,
            Ok(n) => {
                buf.truncate(n);
                Some((Bytes::from(buf), Some(f)))
            }
        }
    })
}

/// `tar` con un solo archivo (`name`, `mode`) cuyo contenido es `path`, como
/// flujo: cabecera, contenido, relleno y fin de archivo.
pub(crate) async fn single_file_tar(
    path: &Path,
    name: &str,
    mode: u32,
) -> std::io::Result<(impl futures_util::Stream<Item = Bytes> + Send, u64)> {
    let f = tokio::fs::File::open(path).await?;
    let size = f.metadata().await?.len();
    let mut h = tar::Header::new_ustar();
    h.set_path(name)?;
    h.set_size(size);
    h.set_mode(mode);
    h.set_uid(0);
    h.set_gid(0);
    h.set_mtime(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    );
    h.set_entry_type(tar::EntryType::Regular);
    h.set_cksum();
    let head = Bytes::copy_from_slice(h.as_bytes());
    let pad = (512 - (size % 512) as usize) % 512;
    let tail = Bytes::from(vec![0u8; pad + 1024]);
    let s = futures_util::stream::once(async move { head })
        .chain(file_stream(f))
        .chain(futures_util::stream::once(async move { tail }));
    Ok((s, size))
}

/// ¿Es el `tar` de una carpeta vacía? Solo con el archivo leído entero y una
/// sola entrada (la propia carpeta). Ante cualquier duda: no vacía.
pub fn tar_is_empty_dir(bytes: &[u8], complete: bool) -> bool {
    if !complete {
        return false;
    }
    let mut a = tar::Archive::new(bytes);
    let Ok(entries) = a.entries() else {
        return false;
    };
    let mut n = 0;
    for e in entries {
        if e.is_err() {
            return false;
        }
        n += 1;
        if n > 1 {
            return false;
        }
    }
    n == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tar_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut b = tar::Builder::new(Vec::new());
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(tar::EntryType::Directory);
        h.set_mode(0o755);
        h.set_size(0);
        b.append_data(&mut h, "cmc-volume/", &[][..]).unwrap();
        for (n, d) in files {
            let mut h = tar::Header::new_gnu();
            h.set_mode(0o644);
            h.set_size(d.len() as u64);
            b.append_data(&mut h, format!("cmc-volume/{n}"), *d)
                .unwrap();
        }
        b.into_inner().unwrap()
    }

    #[test]
    fn empty_volume_detection_is_conservative() {
        assert!(tar_is_empty_dir(&tar_of(&[]), true));
        assert!(!tar_is_empty_dir(&tar_of(&[("a", b"x")]), true));
        // Leído a medias: nunca se da por vacío.
        assert!(!tar_is_empty_dir(&tar_of(&[]), false));
        assert!(!tar_is_empty_dir(b"basura", true));
    }
}
