// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Restaurar una copia (D-099): completa o selectiva, siempre con vista
//! previa, aviso de pérdida de datos y confirmación.
//!
//! Reglas:
//! - Antes de restaurar se comprueba la integridad; una copia alterada,
//!   incompleta o ilegible no se restaura.
//! - **Volúmenes**: a un volumen nuevo, o a uno existente solo si está vacío.
//!   Nunca se sobrescribe un volumen con datos.
//! - **Bases de datos**: sobre el contenedor que se elija (en marcha y del
//!   mismo motor). Sustituye sus datos: exige confirmación expresa.
//! - **Contenedores**: se crea uno nuevo, con otro nombre y **sin arrancar**,
//!   sin las variables secretas que se quitaron al copiar y con la aceptación
//!   de sus riesgos.
//! - **Compose y configuración**: los archivos se escriben en una carpeta
//!   nueva; nada se sobrescribe.

use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use bollard::models::ContainerCreateBody;
use cmc_activity::{Category, Failure, OperationSpec, Outcome, Transport};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use super::db;
use super::docker::{file_stream, single_file_tar, tar_is_empty_dir, ExecOut, HELPER_MOUNT};
use super::model::*;
use super::run::invalid;
use super::store::{read_manifest, verify};
use crate::client::Engine;
use crate::containers::ContainerAction;
use crate::error::EngineError;
use crate::risk::{self, Risk};

/// Qué restaurar de una copia y adónde.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RestoreItem {
    pub kind: ItemKind,
    /// Nombre del elemento en el manifiesto.
    pub name: String,
    /// Destino: volumen, contenedor de la base de datos, nombre del contenedor
    /// nuevo o carpeta nueva (ruta absoluta).
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RestoreRequest {
    pub items: Vec<RestoreItem>,
    /// Confirmación expresa (y, en la interfaz, escrita) de la pérdida de datos.
    pub confirmed: bool,
    /// Riesgos aceptados de los contenedores que se crean (por `key`).
    pub accepted_risks: Vec<String>,
}

/// Plan de un elemento, para la vista previa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RestorePlanItem {
    pub kind: ItemKind,
    pub name: String,
    pub target: String,
    /// Código traducible de la acción (`restore.action.newVolume`…).
    pub action: String,
    /// Sustituye datos que ya existen.
    pub data_loss: bool,
    /// Motivo por el que no se puede restaurar (código traducible).
    pub blocked: Option<String>,
    pub warnings: Vec<String>,
    #[ts(type = "number")]
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RestorePreview {
    pub backup_id: String,
    pub integrity: IntegrityReport,
    pub items: Vec<RestorePlanItem>,
    #[ts(as = "Vec<crate::risk::Risk>")]
    pub risks: Vec<Risk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RestoreItemResult {
    pub kind: ItemKind,
    pub name: String,
    pub target: String,
    pub ok: bool,
    pub error: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RestoreOutcome {
    pub backup_id: String,
    pub items: Vec<RestoreItemResult>,
}

fn io(e: std::io::Error) -> EngineError {
    EngineError::Io {
        message: e.to_string(),
    }
}

/// Error de un elemento: el código (`restore.volumeNotEmpty`…) si lo hay, para
/// traducirlo en la interfaz; si no, el mensaje.
fn item_error(e: &EngineError) -> String {
    if let EngineError::InvalidInput { message } = e {
        if (message.starts_with("backup.") || message.starts_with("restore."))
            && !message.contains(' ')
        {
            return message.clone();
        }
    }
    e.to_string()
}

/// Riesgos para el equipo del cuerpo de creación de un contenedor (las mismas
/// reglas que al crear y editar).
pub fn body_risks(body: &ContainerCreateBody, local: bool) -> Vec<Risk> {
    let mut out = Vec::new();
    let Some(h) = &body.host_config else {
        return out;
    };
    if h.privileged == Some(true) {
        out.push(risk::privileged_risk("privileged"));
    }
    out.extend(risk::cap_risks(
        h.cap_add.as_deref().unwrap_or_default(),
        "capAdd",
    ));
    for (i, b) in h.binds.iter().flatten().enumerate() {
        let mut parts = b.split(':');
        let src = parts.next().unwrap_or("");
        let _dst = parts.next();
        let ro = parts
            .next()
            .is_some_and(|o| o.split(',').any(|x| x == "ro"));
        if src.starts_with('/') {
            out.extend(risk::mount_risk(src, ro, local, &format!("binds.{i}")));
        }
    }
    for (i, m) in h.mounts.iter().flatten().enumerate() {
        let bind = matches!(m.typ, Some(bollard::models::MountType::BIND));
        if let (true, Some(src)) = (bind, &m.source) {
            out.extend(risk::mount_risk(
                src,
                m.read_only == Some(true),
                local,
                &format!("mounts.{i}"),
            ));
        }
    }
    if let Some(n) = &h.network_mode {
        out.extend(risk::network_risk(n, "network"));
    }
    for (kind, mode) in [
        ("pid", &h.pid_mode),
        ("ipc", &h.ipc_mode),
        ("userns", &h.userns_mode),
        ("uts", &h.uts_mode),
    ] {
        if let Some(m) = mode {
            out.extend(risk::namespace_risk(kind, m, kind));
        }
    }
    for o in h.security_opt.iter().flatten() {
        out.extend(risk::security_opt_risk(o, "securityOpt"));
    }
    for (i, d) in h.devices.iter().flatten().enumerate() {
        if let Some(p) = &d.path_on_host {
            out.extend(risk::device_risk(p, &format!("devices.{i}")));
        }
    }
    for (port, binds) in h.port_bindings.iter().flatten() {
        for b in binds.iter().flatten() {
            out.extend(risk::port_risk(
                b.host_ip.as_deref().unwrap_or(""),
                b.host_port.as_deref().unwrap_or(""),
                port,
                "ports",
            ));
        }
    }
    out
}

/// Nombre de contenedor válido para Docker.
fn valid_container_name(s: &str) -> bool {
    let mut c = s.chars();
    matches!(c.next(), Some(f) if f.is_ascii_alphanumeric())
        && s.len() <= 128
        && c.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-'))
}

/// Carpeta nueva para restaurar archivos: absoluta, que no exista, con la
/// carpeta superior existente y que cumpla la política de rutas de Compose.
pub fn check_new_folder(target: &str) -> Result<PathBuf, EngineError> {
    let p = Path::new(target);
    if !p.is_absolute() || target.contains('\0') {
        return Err(invalid("restore.folderNotAbsolute"));
    }
    crate::compose::file::check_compose_path(&p.join("compose.yaml").display().to_string())?;
    if std::fs::symlink_metadata(p).is_ok() {
        return Err(EngineError::AlreadyExists {
            path: target.to_owned(),
        });
    }
    match p.parent() {
        Some(parent) if parent.is_dir() => Ok(p.to_path_buf()),
        _ => Err(invalid("restore.folderNoParent")),
    }
}

fn item<'m>(m: &'m Manifest, r: &RestoreItem) -> Result<&'m BackupItem, EngineError> {
    m.items
        .iter()
        .find(|i| i.kind == r.kind && i.name == r.name)
        .ok_or_else(|| invalid(format!("la copia no tiene {}", r.name)))
}

fn single_file(i: &BackupItem) -> Result<&ManifestFile, EngineError> {
    match i.files.as_slice() {
        [f] => Ok(f),
        _ => Err(invalid("elemento de la copia no válido")),
    }
}

fn same_family(a: DbEngine, b: DbEngine) -> bool {
    let fam = |e| match e {
        DbEngine::Mariadb | DbEngine::Mysql => 0,
        DbEngine::Postgres => 1,
        DbEngine::Redis => 2,
        DbEngine::Mongo => 3,
    };
    fam(a) == fam(b)
}

fn engine_of(i: &BackupItem) -> Option<DbEngine> {
    match i.details.get("engine").map(String::as_str) {
        Some("mariadb") => Some(DbEngine::Mariadb),
        Some("mysql") => Some(DbEngine::Mysql),
        Some("postgres") => Some(DbEngine::Postgres),
        Some("redis") => Some(DbEngine::Redis),
        Some("mongo") => Some(DbEngine::Mongo),
        _ => None,
    }
}

/// Lee el cuerpo de creación guardado de un contenedor.
fn read_body(dir: &Path, f: &ManifestFile) -> Result<ContainerCreateBody, EngineError> {
    let text = std::fs::read(dir.join(&f.path)).map_err(io)?;
    serde_json::from_slice(&text).map_err(|e| invalid(format!("configuración no válida: {e}")))
}

impl Engine {
    /// ¿Existe el volumen y tiene datos? `None`: no existe.
    async fn volume_has_data(&self, name: &str, image: &str) -> Result<Option<bool>, EngineError> {
        let vols = self.list_volumes(Some(&[])).await?;
        if !vols.iter().any(|v| v.name == name) {
            return Ok(None);
        }
        let helper = self.helper_create(name, image, true).await?;
        let r = self
            .archive_peek(&helper, name, HELPER_MOUNT, 1024 * 1024)
            .await;
        let _ = self.helper_remove(&helper).await;
        let (bytes, complete) = r?;
        Ok(Some(!tar_is_empty_dir(&bytes, complete)))
    }

    async fn any_image(&self) -> Result<String, EngineError> {
        let images = self.list_images(Some(&[])).await?;
        images
            .into_iter()
            .map(|i| i.id)
            .find(|i| !i.is_empty())
            .ok_or_else(|| invalid("backup.noHelperImage"))
    }

    /// Vista previa: integridad, qué se hará con cada elemento y los riesgos.
    /// Solo lectura.
    pub async fn restore_preview(
        &self,
        dir: &Path,
        req: &RestoreRequest,
        local: bool,
    ) -> Result<RestorePreview, EngineError> {
        let m = read_manifest(dir).map_err(io)?;
        let integrity = verify(dir).map_err(io)?;
        let mut items = Vec::new();
        let mut risks = Vec::new();
        let mut image: Option<String> = None;
        let containers = self.list_containers().await?;
        for r in &req.items {
            let it = item(&m, r)?;
            let size = it.files.iter().map(|f| f.size).sum();
            let mut p = RestorePlanItem {
                kind: r.kind,
                name: r.name.clone(),
                target: r.target.clone(),
                action: String::new(),
                data_loss: false,
                blocked: None,
                warnings: it.warnings.clone(),
                size,
            };
            match r.kind {
                ItemKind::Volume => {
                    if !crate::volumes::valid_volume_name(&r.target) {
                        p.blocked = Some("restore.invalidName".into());
                    } else {
                        if image.is_none() {
                            image = Some(self.any_image().await?);
                        }
                        match self
                            .volume_has_data(&r.target, image.as_deref().unwrap_or(""))
                            .await?
                        {
                            None => p.action = "restore.action.newVolume".into(),
                            Some(false) => p.action = "restore.action.emptyVolume".into(),
                            Some(true) => p.blocked = Some("restore.volumeNotEmpty".into()),
                        }
                    }
                }
                ItemKind::Database => {
                    p.action = "restore.action.database".into();
                    p.data_loss = true;
                    match containers
                        .iter()
                        .find(|c| c.name == r.target || c.id == r.target)
                    {
                        None => p.blocked = Some("restore.containerMissing".into()),
                        Some(c) if c.state != "running" => {
                            p.blocked = Some("backup.dbNotRunning".into())
                        }
                        Some(c) => {
                            let want = engine_of(it);
                            let have = db::detect(&c.image, &[]);
                            if let (Some(w), Some(h)) = (want, have) {
                                if !same_family(w, h) {
                                    p.blocked = Some("restore.engineMismatch".into());
                                }
                            }
                            if want == Some(DbEngine::Redis) {
                                p.warnings.push("restore.warn.redisRestart".into());
                            }
                        }
                    }
                }
                ItemKind::Container => {
                    p.action = "restore.action.newContainer".into();
                    if !valid_container_name(&r.target) {
                        p.blocked = Some("restore.invalidName".into());
                    } else if containers.iter().any(|c| c.name == r.target) {
                        p.blocked = Some("restore.containerExists".into());
                    } else {
                        let body = read_body(dir, single_file(it)?)?;
                        risks.extend(body_risks(&body, local));
                    }
                }
                ItemKind::Compose | ItemKind::AppConfig => {
                    p.action = "restore.action.newFolder".into();
                    if let Err(e) = check_new_folder(&r.target) {
                        p.blocked = Some(match e {
                            EngineError::AlreadyExists { .. } => "restore.folderExists".into(),
                            EngineError::PathNotAllowed { .. } => "engine.pathNotAllowed".into(),
                            EngineError::InvalidInput { message } => message,
                            _ => "restore.folderInvalid".into(),
                        });
                    }
                }
            }
            items.push(p);
        }
        Ok(RestorePreview {
            backup_id: m.id,
            integrity,
            items,
            risks,
        })
    }

    /// Restaura. Cada elemento se intenta por separado y se informa de cada uno.
    pub async fn restore(
        &self,
        dir: &Path,
        req: &RestoreRequest,
        local: bool,
        cancel: CancellationToken,
    ) -> Result<RestoreOutcome, EngineError> {
        if !req.confirmed {
            return Err(EngineError::NotConfirmed);
        }
        if req.items.is_empty() {
            return Err(invalid("restore.nothingSelected"));
        }
        let m = read_manifest(dir).map_err(io)?;
        let handle = self.activity().begin(OperationSpec {
            category: Category::Backup,
            kind: "backup.restore".into(),
            target: Some(m.id.clone()),
            transport: Transport::App {
                action: format!("Restauración de la copia {}", m.id),
            },
            request_body: serde_json::to_string_pretty(req).ok(),
            equivalent_command: None,
        });
        // Nunca se restaura una copia alterada.
        let integrity = verify(dir).map_err(io)?;
        if !integrity.ok {
            let e = invalid("restore.integrityFailed");
            handle.fail(Failure {
                error: format!(
                    "integridad: {} alterados, {} faltan, {} de más",
                    integrity.altered.len(),
                    integrity.missing.len(),
                    integrity.unexpected.len()
                ),
                ..Default::default()
            });
            return Err(e);
        }
        // Los riesgos de todos los contenedores se aceptan antes de empezar.
        for r in req.items.iter().filter(|r| r.kind == ItemKind::Container) {
            let body = read_body(dir, single_file(item(&m, r)?)?)?;
            let risks = body_risks(&body, local);
            let missing: Vec<Risk> = risk::unaccepted(&risks, &req.accepted_risks)
                .into_iter()
                .cloned()
                .collect();
            if !missing.is_empty() {
                handle.fail(Failure {
                    error: "riesgos sin aceptar".into(),
                    ..Default::default()
                });
                return Err(EngineError::RisksNotAccepted { risks: missing });
            }
        }
        let mut results = Vec::new();
        for r in &req.items {
            if cancel.is_cancelled() {
                break;
            }
            let it = item(&m, r)?;
            let mut warnings = Vec::new();
            let res = match r.kind {
                ItemKind::Volume => {
                    self.restore_volume(dir, it, &r.target, &m.id, &cancel)
                        .await
                }
                ItemKind::Database => {
                    self.restore_database(dir, it, &r.target, &mut warnings, &cancel)
                        .await
                }
                ItemKind::Container => {
                    self.restore_container(dir, it, &r.target, &mut warnings)
                        .await
                }
                ItemKind::Compose | ItemKind::AppConfig => restore_files(dir, it, &r.target),
            };
            results.push(RestoreItemResult {
                kind: r.kind,
                name: r.name.clone(),
                target: r.target.clone(),
                ok: res.is_ok(),
                error: res.err().map(|e| item_error(&e)),
                warnings,
            });
        }
        let failed = results.iter().filter(|r| !r.ok).count();
        if cancel.is_cancelled() {
            handle.cancel(Failure {
                error: "restauración cancelada".into(),
                ..Default::default()
            });
        } else if failed == 0 {
            handle.succeed(Outcome {
                summary: Some(format!(
                    "{} elementos restaurados de la copia {}",
                    results.len(),
                    m.id
                )),
                ..Default::default()
            });
        } else {
            handle.fail(Failure {
                error: format!(
                    "{failed} de {} elementos no se pudieron restaurar",
                    results.len()
                ),
                ..Default::default()
            });
        }
        Ok(RestoreOutcome {
            backup_id: m.id,
            items: results,
        })
    }

    async fn restore_volume(
        &self,
        dir: &Path,
        it: &BackupItem,
        target: &str,
        backup_id: &str,
        cancel: &CancellationToken,
    ) -> Result<(), EngineError> {
        if !crate::volumes::valid_volume_name(target) {
            return Err(invalid("restore.invalidName"));
        }
        let f = single_file(it)?;
        let image = self.any_image().await?;
        let _lock = self.locks().acquire(&format!("volume:{target}"))?;
        match self.volume_has_data(target, &image).await? {
            Some(true) => return Err(invalid("restore.volumeNotEmpty")),
            Some(false) => {}
            None => {
                let labels = vec![crate::spec::KeyValue {
                    key: "cmc.restored-from".to_owned(),
                    value: backup_id.to_owned(),
                }];
                let spec = crate::volumes::VolumeSpec {
                    name: target.to_owned(),
                    labels,
                    ..Default::default()
                };
                self.create_volume(&spec).await?;
            }
        }
        let helper = self.helper_create(target, &image, false).await?;
        let r = async {
            let file = tokio::fs::File::open(dir.join(&f.path)).await.map_err(io)?;
            // El `tar` empieza por `cmc-volume/`: se extrae en `/`.
            self.archive_upload(&helper, target, "/", file_stream(file), f.size, cancel)
                .await
        }
        .await;
        let removed = self.helper_remove(&helper).await;
        r?;
        removed
    }

    async fn restore_database(
        &self,
        dir: &Path,
        it: &BackupItem,
        target: &str,
        warnings: &mut Vec<String>,
        cancel: &CancellationToken,
    ) -> Result<(), EngineError> {
        crate::resources::validate_ref("contenedor", target)?;
        let f = single_file(it)?;
        let want = engine_of(it).ok_or_else(|| invalid("backup.dbUnknown"))?;
        let c = self.inspect_raw(target, Some(target)).await?;
        let id = c.id.clone().unwrap_or_else(|| target.to_owned());
        let cfg = c.config.clone().unwrap_or_default();
        if let Some(have) = db::detect(
            cfg.image.as_deref().unwrap_or(""),
            &cfg.env.clone().unwrap_or_default(),
        ) {
            if !same_family(want, have) {
                return Err(invalid("restore.engineMismatch"));
            }
        }
        if !c.state.as_ref().and_then(|s| s.running).unwrap_or(false) {
            return Err(invalid("backup.dbNotRunning"));
        }
        let _lock = self.locks().acquire(&format!("container:{id}"))?;
        let path = dir.join(&f.path);
        if want == DbEngine::Redis {
            return self.restore_redis(&id, target, &path, cancel).await;
        }
        let script = db::restore_script(want).ok_or_else(|| invalid("backup.dbUnknown"))?;
        let name = format!(
            "cmc-restore-{}.{}",
            crate::ssh::askpass::random_hex(6),
            want.extension()
        );
        let (tar, size) = single_file_tar(&path, &name, 0o600).await.map_err(io)?;
        self.archive_upload(&id, target, "/tmp", tar, size, cancel)
            .await?;
        let r = self
            .exec_script(
                &id,
                target,
                "backup.db.restore",
                &script,
                &[format!("/tmp/{name}")],
                Some("0"),
                ExecOut::Discard,
                cancel,
            )
            .await?;
        if r.stderr.contains("CMC_NO_CREDENTIALS") {
            return Err(invalid("backup.dbNoCredentials"));
        }
        if r.exit_code != Some(0) {
            return Err(EngineError::Process {
                message: format!(
                    "la restauración terminó con código {}: {}",
                    r.exit_code.map_or("?".into(), |c| c.to_string()),
                    r.stderr.trim()
                ),
            });
        }
        if want == DbEngine::Postgres {
            let errors = r
                .stderr
                .lines()
                .filter(|l| l.contains("ERROR:") && !db::pg_expected_error(l))
                .count();
            if errors > 0 {
                warnings.push(format!("restore.warn.sqlErrors:{errors}"));
            }
        }
        Ok(())
    }

    /// Redis: se sustituye su archivo RDB con el contenedor detenido. Con AOF
    /// activado el RDB no se cargaría: se rechaza.
    async fn restore_redis(
        &self,
        id: &str,
        target: &str,
        rdb: &Path,
        cancel: &CancellationToken,
    ) -> Result<(), EngineError> {
        let (out, r) = self
            .exec_capture(
                id,
                target,
                "backup.redis.config",
                &db::redis_config_script(),
                cancel,
            )
            .await?;
        if r.exit_code != Some(0) {
            return Err(EngineError::Process {
                message: format!(
                    "no se pudo leer la configuración de Redis: {}",
                    r.stderr.trim()
                ),
            });
        }
        let mut lines = out.lines().map(str::trim);
        let dir = lines.next().unwrap_or_default().to_owned();
        let file = lines.next().unwrap_or_default().to_owned();
        let aof = lines.next().unwrap_or_default().to_owned();
        if aof == "yes" {
            return Err(invalid("restore.redisAof"));
        }
        let ok_name = !file.is_empty() && !file.contains('/') && file != "." && file != "..";
        if !dir.starts_with('/') || !ok_name {
            return Err(invalid("restore.redisConfig"));
        }
        self.run_action(id, Some(target), ContainerAction::Stop)
            .await?;
        let (tar, size) = single_file_tar(rdb, &file, 0o644).await.map_err(io)?;
        let up = self
            .archive_upload(id, target, &dir, tar, size, cancel)
            .await;
        // Se vuelve a arrancar pase lo que pase.
        let started = self
            .run_action(id, Some(target), ContainerAction::Start)
            .await;
        up?;
        started
    }

    async fn restore_container(
        &self,
        dir: &Path,
        it: &BackupItem,
        target: &str,
        warnings: &mut Vec<String>,
    ) -> Result<(), EngineError> {
        if !valid_container_name(target) {
            return Err(invalid("restore.invalidName"));
        }
        let body = read_body(dir, single_file(it)?)?;
        if let Some(r) = it.details.get("removedEnv") {
            warnings.push(format!("restore.warn.secretsMissing:{r}"));
        }
        if it.details.get("networks").is_some_and(|n| n.contains(',')) {
            warnings.push("backup.warn.extraNetworks".into());
        }
        self.op_create_raw(target, body).await.map(|_| ())
    }
}

/// Escribe los archivos de Compose o de configuración en una carpeta nueva
/// (0700, archivos 0600). Nunca sobrescribe.
fn restore_files(dir: &Path, it: &BackupItem, target: &str) -> Result<(), EngineError> {
    let dest = check_new_folder(target)?;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&dest)
        .map_err(io)?;
    for f in &it.files {
        // `compose/<proyecto>/[env/]<n>-<nombre>`: se conserva la parte tras el
        // proyecto, sin el prefijo numérico.
        let rel = Path::new(&f.path);
        let comps: Vec<_> = rel
            .components()
            .skip(if it.kind == ItemKind::AppConfig { 1 } else { 2 })
            .collect();
        let mut out = dest.clone();
        for (i, c) in comps.iter().enumerate() {
            let s = c.as_os_str().to_string_lossy();
            if i + 1 == comps.len() {
                let name = match s.split_once('-') {
                    Some((n, rest))
                        if it.kind == ItemKind::Compose
                            && n.chars().all(|c| c.is_ascii_digit()) =>
                    {
                        rest.to_owned()
                    }
                    _ => s.into_owned(),
                };
                out.push(name);
            } else {
                out.push(s.as_ref());
                if !out.is_dir() {
                    std::fs::DirBuilder::new()
                        .mode(0o700)
                        .create(&out)
                        .map_err(io)?;
                }
            }
        }
        let data = std::fs::read(dir.join(&f.path)).map_err(io)?;
        let mut w = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&out)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    EngineError::AlreadyExists {
                        path: out.display().to_string(),
                    }
                } else {
                    io(e)
                }
            })?;
        std::io::Write::write_all(&mut w, &data).map_err(io)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bollard::models::{HostConfig, PortBinding};
    use std::collections::HashMap;

    #[test]
    fn body_risks_use_the_same_rules_as_create() {
        let body = ContainerCreateBody {
            host_config: Some(HostConfig {
                privileged: Some(true),
                binds: Some(vec![
                    "/var/run/docker.sock:/var/run/docker.sock:ro".into(),
                    "datos:/d".into(),
                ]),
                network_mode: Some("host".into()),
                pid_mode: Some("host".into()),
                port_bindings: Some(HashMap::from([(
                    "80/tcp".to_owned(),
                    Some(vec![PortBinding {
                        host_ip: Some(String::new()),
                        host_port: Some("8080".into()),
                    }]),
                )])),
                ..Default::default()
            }),
            ..Default::default()
        };
        let codes: Vec<String> = body_risks(&body, true)
            .into_iter()
            .map(|r| r.code)
            .collect();
        for c in [
            "risk.privileged",
            "risk.runtimeSocket",
            "risk.networkHost",
            "risk.hostNamespace",
            "risk.portAllInterfaces",
        ] {
            assert!(codes.iter().any(|x| x == c), "{c}: {codes:?}");
        }
        assert!(body_risks(&ContainerCreateBody::default(), true).is_empty());
    }

    #[test]
    fn new_folders_are_never_existing_ones() {
        let d = tempfile::Builder::new()
            .prefix("cmc-test")
            .tempdir()
            .unwrap();
        let ok = d.path().join("nuevo");
        assert!(check_new_folder(ok.to_str().unwrap()).is_ok());
        assert!(matches!(
            check_new_folder(d.path().to_str().unwrap()),
            Err(EngineError::AlreadyExists { .. })
        ));
        assert!(check_new_folder("relativa/x").is_err());
        assert!(check_new_folder(d.path().join("no/existe").to_str().unwrap()).is_err());
        assert!(valid_container_name("web-restaurado_1"));
        assert!(!valid_container_name("-x"));
        assert!(!valid_container_name("a/b"));
    }
}
