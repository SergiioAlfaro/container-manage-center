//! Hacer una copia (D-096 a D-099).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use chrono::{SecondsFormat, Utc};
use dcc_activity::{Category, Failure, OperationSpec, Outcome, Transport};
use tokio_util::sync::CancellationToken;

use super::db;
use super::docker::{cancelled, ExecOut, HELPER_MOUNT};
use super::model::*;
use super::store::{file_stem, new_id, BackupTarget, PartialBackup};
use crate::client::Engine;
use crate::compose::project::ProjectRef;
use crate::containers::{ContainerAction, ContainerSummary};
use crate::error::EngineError;
use crate::volumes::VolumeSummary;

/// Tamaño máximo de un archivo de Compose o de configuración.
const SMALL_FILE_MAX: u64 = 10 * 1024 * 1024;
/// Elementos máximos en una copia.
const MAX_ITEMS: usize = 500;
/// Cada cuántos bytes se informa del avance.
const PROGRESS_STEP: u64 = 4 * 1024 * 1024;

/// Lo que la copia necesita de fuera del motor.
#[derive(Debug, Clone, Default)]
pub struct BackupContext {
    /// Id del servidor (`local` o `srv-…`).
    pub server: String,
    /// ¿Es el Docker de este equipo? Los archivos de Compose solo se leen en local.
    pub local: bool,
    /// Tarea programada que la lanza.
    pub job: Option<String>,
    /// Proyectos Compose conocidos.
    pub compose_projects: Vec<ProjectRef>,
    /// Archivos de configuración de la aplicación (nombre, contenido), ya sin secretos.
    pub app_config: Vec<(String, Vec<u8>)>,
    pub app_version: String,
}

pub(crate) fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidInput {
        message: message.into(),
    }
}

fn io(e: std::io::Error) -> EngineError {
    EngineError::Io {
        message: e.to_string(),
    }
}

/// Validación de la petición, sin Docker.
pub fn validate_request(req: &BackupRequest, local: bool) -> Result<(), EngineError> {
    if req.is_empty() {
        return Err(invalid("backup.emptyRequest"));
    }
    let n = req.volumes.len() + req.databases.len() + req.containers.len() + req.compose.len();
    if n > MAX_ITEMS {
        return Err(invalid("demasiados elementos en la copia"));
    }
    for v in &req.volumes {
        if !crate::volumes::valid_volume_name(&v.name) {
            return Err(invalid(format!(
                "nombre de volumen no válido: {:?}",
                v.name
            )));
        }
    }
    for c in req.databases.iter().chain(&req.containers) {
        crate::resources::validate_ref("contenedor", c)?;
    }
    for c in &req.compose {
        if !crate::compose::project::valid_project_name(&c.project) {
            return Err(invalid(format!("proyecto no válido: {:?}", c.project)));
        }
        if !local {
            return Err(invalid("backup.composeRemote"));
        }
    }
    Ok(())
}

/// Nombres de archivo únicos dentro de la copia.
#[derive(Default)]
struct Names(BTreeSet<String>);

impl Names {
    fn unique(&mut self, dir: &str, stem: &str, ext: &str) -> String {
        let mut n = 1;
        loop {
            let s = if n == 1 {
                format!("{dir}/{stem}{ext}")
            } else {
                format!("{dir}/{stem}-{n}{ext}")
            };
            if self.0.insert(s.clone()) {
                return s;
            }
            n += 1;
        }
    }
}

struct Run<'a, F> {
    engine: &'a Engine,
    cancel: &'a CancellationToken,
    progress: F,
    done: u32,
    total: u32,
    names: Names,
    containers: Vec<ContainerSummary>,
}

impl<F: FnMut(BackupProgress) + Send> Run<'_, F> {
    fn report(&mut self, kind: ItemKind, name: &str, bytes: u64) {
        (self.progress)(BackupProgress {
            kind,
            name: name.to_owned(),
            done: self.done,
            total: self.total,
            bytes,
        });
    }

    fn check(&self) -> Result<(), EngineError> {
        if self.cancel.is_cancelled() {
            Err(cancelled())
        } else {
            Ok(())
        }
    }

    /// Imagen ya presente para el contenedor auxiliar: la de un contenedor
    /// que use el volumen o, si no, cualquiera del equipo.
    async fn helper_image(&self, vol: &VolumeSummary) -> Result<String, EngineError> {
        for u in &vol.containers {
            if let Some(c) = self.containers.iter().find(|c| c.id == u.id) {
                if !c.image_id.is_empty() {
                    return Ok(c.image_id.clone());
                }
            }
        }
        if let Some(c) = self.containers.iter().find(|c| !c.image_id.is_empty()) {
            return Ok(c.image_id.clone());
        }
        let images = self.engine.list_images(Some(&self.containers)).await?;
        images
            .into_iter()
            .map(|i| i.id)
            .find(|i| !i.is_empty())
            .ok_or_else(|| invalid("backup.noHelperImage"))
    }

    async fn volume(
        &mut self,
        pb: &mut PartialBackup,
        sel: &VolumeSelection,
        vols: &[VolumeSummary],
    ) -> Result<BackupItem, EngineError> {
        let vol = vols
            .iter()
            .find(|v| v.name == sel.name)
            .ok_or_else(|| invalid(format!("no existe el volumen {}", sel.name)))?;
        let image = self.helper_image(vol).await?;
        let running: Vec<(String, String)> = vol
            .containers
            .iter()
            .filter(|u| u.state == "running")
            .map(|u| (u.id.clone(), u.name.clone()))
            .collect();
        let mut warnings = Vec::new();
        let mut stopped = Vec::new();
        if sel.mode == VolumeMode::Stop {
            for (id, name) in &running {
                // Si falla una parada, se vuelven a arrancar las ya paradas.
                if let Err(e) = self
                    .engine
                    .run_action(id, Some(name), ContainerAction::Stop)
                    .await
                {
                    self.restart(&stopped, &mut warnings).await;
                    return Err(e);
                }
                stopped.push((id.clone(), name.clone()));
            }
        }
        let rel = self.names.unique("volumes", &file_stem(&vol.name), ".tar");
        let r = self.copy_volume(pb, &vol.name, &image, &rel).await;
        self.restart(&stopped, &mut warnings).await;
        let file = r?;
        let consistency = match (sel.mode, running.is_empty()) {
            (_, true) => Consistency::Idle,
            (VolumeMode::Stop, false) => Consistency::Stopped,
            (VolumeMode::Hot, false) => {
                warnings.push("backup.warn.hot".into());
                Consistency::Hot
            }
        };
        let mut details = BTreeMap::new();
        details.insert("driver".into(), vol.driver.clone());
        if !vol.labels.is_empty() {
            details.insert(
                "labels".into(),
                serde_json::to_string(&vol.labels).unwrap_or_default(),
            );
        }
        if !vol.options.is_empty() {
            details.insert(
                "driverOpts".into(),
                serde_json::to_string(&vol.options).unwrap_or_default(),
            );
        }
        let users: Vec<String> = vol
            .containers
            .iter()
            .map(|u| format!("{}:{}", u.name, u.destination))
            .collect();
        if !users.is_empty() {
            details.insert("usedBy".into(), users.join(", "));
        }
        if vol.driver != "local" {
            warnings.push("backup.warn.driver".into());
        }
        Ok(BackupItem {
            kind: ItemKind::Volume,
            name: vol.name.clone(),
            files: vec![file],
            consistency,
            details,
            warnings,
        })
    }

    async fn copy_volume(
        &mut self,
        pb: &mut PartialBackup,
        volume: &str,
        image: &str,
        rel: &str,
    ) -> Result<ManifestFile, EngineError> {
        let helper = self.engine.helper_create(volume, image, true).await?;
        let mut f = match pb.create(rel).await {
            Ok(f) => f,
            Err(e) => {
                let _ = self.engine.helper_remove(&helper).await;
                return Err(io(e));
            }
        };
        let (done, total) = (self.done, self.total);
        let mut last = 0u64;
        let progress = &mut self.progress;
        let r = self
            .engine
            .archive_download(&helper, volume, HELPER_MOUNT, &mut f, self.cancel, |n| {
                if n - last >= PROGRESS_STEP {
                    last = n;
                    progress(BackupProgress {
                        kind: ItemKind::Volume,
                        name: volume.to_owned(),
                        done,
                        total,
                        bytes: n,
                    });
                }
            })
            .await;
        let removed = self.engine.helper_remove(&helper).await;
        r?;
        removed?;
        f.finish().await.map_err(io)
    }

    async fn restart(&self, stopped: &[(String, String)], warnings: &mut Vec<String>) {
        for (id, name) in stopped {
            if self
                .engine
                .run_action(id, Some(name), ContainerAction::Start)
                .await
                .is_err()
            {
                warnings.push(format!("backup.warn.restartFailed:{name}"));
            }
        }
    }

    async fn database(
        &mut self,
        pb: &mut PartialBackup,
        container: &str,
    ) -> Result<BackupItem, EngineError> {
        let c = self.engine.inspect_raw(container, Some(container)).await?;
        let name = c
            .name
            .clone()
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_owned();
        let cfg = c.config.clone().unwrap_or_default();
        let image = cfg.image.clone().unwrap_or_default();
        let env = cfg.env.clone().unwrap_or_default();
        let engine = db::detect(&image, &env).ok_or_else(|| invalid("backup.dbUnknown"))?;
        let running = c.state.as_ref().and_then(|s| s.running).unwrap_or(false);
        if !running {
            return Err(invalid("backup.dbNotRunning"));
        }
        let rel = self.names.unique(
            "databases",
            &file_stem(&name),
            &format!(".{}", engine.extension()),
        );
        let mut f = pb.create(&rel).await.map_err(io)?;
        let id = c.id.clone().unwrap_or_else(|| container.to_owned());
        let r = self
            .engine
            .exec_script(
                &id,
                &name,
                "backup.db.dump",
                &db::dump_script(engine),
                &[],
                None,
                ExecOut::File(&mut f),
                self.cancel,
            )
            .await?;
        if r.stderr.contains("DCC_NO_CREDENTIALS") {
            return Err(invalid("backup.dbNoCredentials"));
        }
        if r.exit_code != Some(0) {
            return Err(EngineError::Process {
                message: format!(
                    "el volcado terminó con código {}: {}",
                    r.exit_code.map_or("?".into(), |c| c.to_string()),
                    r.stderr.trim()
                ),
            });
        }
        if f.size() == 0 {
            return Err(invalid("backup.dbEmptyDump"));
        }
        let file = f.finish().await.map_err(io)?;
        let mut details = db::details(engine, &image);
        details.insert("container".into(), name.clone());
        Ok(BackupItem {
            kind: ItemKind::Database,
            name,
            files: vec![file],
            consistency: Consistency::Dump,
            details,
            warnings: vec![],
        })
    }

    async fn container(
        &mut self,
        pb: &mut PartialBackup,
        container: &str,
    ) -> Result<BackupItem, EngineError> {
        let c = self.engine.inspect_raw(container, Some(container)).await?;
        let name = c
            .name
            .clone()
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_owned();
        let image = c
            .config
            .as_ref()
            .and_then(|x| x.image.clone())
            .unwrap_or_default();
        let cur = crate::edit::current_request(&c, None);
        let (mut body, _plan) =
            crate::edit::recreate_body(&c, &image, &cur, &cur, &HashMap::new()).map_err(invalid)?;
        let (env, removed) = strip_secret_env(body.env.take().unwrap_or_default());
        body.env = Some(env);
        let mut warnings = Vec::new();
        let mut details = BTreeMap::new();
        details.insert("image".into(), image);
        if let Some(i) = &c.image {
            details.insert("imageId".into(), i.clone());
        }
        if !removed.is_empty() {
            warnings.push("backup.warn.secretsRemoved".into());
            details.insert("removedEnv".into(), removed.join(","));
        }
        let nets: Vec<String> = c
            .network_settings
            .as_ref()
            .and_then(|n| n.networks.as_ref())
            .map(|n| n.keys().cloned().collect())
            .unwrap_or_default();
        if nets.len() > 1 {
            warnings.push("backup.warn.extraNetworks".into());
        }
        if !nets.is_empty() {
            details.insert("networks".into(), nets.join(","));
        }
        let text = serde_json::to_vec_pretty(&body).map_err(|e| invalid(e.to_string()))?;
        let rel = self.names.unique("containers", &file_stem(&name), ".json");
        let file = pb.write_all(&rel, &text).await.map_err(io)?;
        Ok(BackupItem {
            kind: ItemKind::Container,
            name,
            files: vec![file],
            consistency: Consistency::NotApplicable,
            details,
            warnings,
        })
    }

    async fn compose(
        &mut self,
        pb: &mut PartialBackup,
        sel: &ComposeSelection,
        projects: &[ProjectRef],
    ) -> Result<BackupItem, EngineError> {
        let p = projects
            .iter()
            .find(|p| p.name == sel.project)
            .ok_or_else(|| invalid(format!("no existe el proyecto {}", sel.project)))?;
        p.validate()?;
        if p.files.is_empty() {
            return Err(invalid("backup.composeNoFiles"));
        }
        let stem = file_stem(&p.name);
        let mut files = Vec::new();
        let mut details = BTreeMap::new();
        let mut warnings = Vec::new();
        for (i, path) in p.files.iter().enumerate() {
            let data = read_small(Path::new(path))?;
            let rel = self.names.unique(
                &format!("compose/{stem}"),
                &format!("{i}-{}", base(path)),
                "",
            );
            files.push(pb.write_all(&rel, &data).await.map_err(io)?);
            details.insert(format!("file.{i}"), path.clone());
        }
        if let Some(d) = p.project_dir() {
            details.insert("workingDir".into(), d.clone());
        }
        let mut envs: Vec<String> = p.env_files.clone();
        if let Some(d) = p.project_dir() {
            let dot = Path::new(&d).join(".env");
            if dot.is_file() && !envs.iter().any(|e| Path::new(e) == dot) {
                envs.push(dot.display().to_string());
            }
        }
        if sel.include_env {
            for (i, path) in envs.iter().enumerate() {
                if !Path::new(path).is_file() {
                    continue;
                }
                let data = read_small(Path::new(path))?;
                let rel = self.names.unique(
                    &format!("compose/{stem}/env"),
                    &format!("{i}-{}", base(path)),
                    "",
                );
                files.push(pb.write_all(&rel, &data).await.map_err(io)?);
                details.insert(format!("env.{i}"), path.clone());
            }
            if !envs.is_empty() {
                warnings.push("backup.warn.envIncluded".into());
            }
        } else if !envs.is_empty() {
            warnings.push("backup.warn.envExcluded".into());
        }
        Ok(BackupItem {
            kind: ItemKind::Compose,
            name: p.name.clone(),
            files,
            consistency: Consistency::NotApplicable,
            details,
            warnings,
        })
    }
}

/// Nombre de archivo original, saneado; conserva el punto inicial (`.env`).
fn base(path: &str) -> String {
    let name = Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .take(120)
        .collect();
    if s.is_empty() || s == "." || s == ".." {
        "archivo".into()
    } else {
        s
    }
}

fn read_small(p: &Path) -> Result<Vec<u8>, EngineError> {
    let meta = std::fs::metadata(p).map_err(io)?;
    if !meta.is_file() {
        return Err(invalid(format!("{} no es un archivo", p.display())));
    }
    if meta.len() > SMALL_FILE_MAX {
        return Err(invalid(format!("{} es demasiado grande", p.display())));
    }
    std::fs::read(p).map_err(io)
}

/// Quita del entorno las variables con nombre de secreto (D-085). Devuelve el
/// entorno limpio y los nombres quitados.
pub fn strip_secret_env(env: Vec<String>) -> (Vec<String>, Vec<String>) {
    let mut keep = Vec::new();
    let mut removed = Vec::new();
    for e in env {
        let key = e.split_once('=').map_or(e.as_str(), |(k, _)| k).to_owned();
        if crate::spec::is_secret_key(&key) {
            removed.push(key);
        } else {
            keep.push(e);
        }
    }
    (keep, removed)
}

impl Engine {
    /// Hace una copia en `target`. Si falla o se cancela, la carpeta parcial
    /// se elimina y no queda ninguna copia que pueda tomarse por buena.
    pub async fn backup<F>(
        &self,
        target: &dyn BackupTarget,
        req: &BackupRequest,
        ctx: &BackupContext,
        cancel: CancellationToken,
        progress: F,
    ) -> Result<BackupEntry, EngineError>
    where
        F: FnMut(BackupProgress) + Send,
    {
        validate_request(req, ctx.local)?;
        let started = Utc::now();
        let id = new_id(started);
        let handle = self.activity().begin(OperationSpec {
            category: Category::Backup,
            kind: "backup.run".into(),
            target: Some(id.clone()),
            transport: Transport::App {
                action: format!("Copia de seguridad en {}", target.describe()),
            },
            request_body: serde_json::to_string_pretty(req).ok(),
            equivalent_command: None,
        });
        let mut pb = match target.begin(&ctx.server, &id) {
            Ok(p) => p,
            Err(e) => {
                handle.fail(Failure {
                    error: e.to_string(),
                    ..Default::default()
                });
                return Err(io(e));
            }
        };
        let r = self
            .backup_inner(&mut pb, req, ctx, &cancel, progress)
            .await;
        let r = match r {
            Ok(items) => {
                let manifest = Manifest {
                    format: FORMAT,
                    id: id.clone(),
                    server: ctx.server.clone(),
                    created_at: started.to_rfc3339_opts(SecondsFormat::Secs, true),
                    finished_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
                    app_version: ctx.app_version.clone(),
                    job: ctx.job.clone(),
                    items,
                };
                match pb.finalize(&manifest) {
                    Ok(dir) => Ok(BackupEntry {
                        id: id.clone(),
                        path: dir.display().to_string(),
                        state: BackupState::Complete,
                        size: manifest.size(),
                        manifest: Some(manifest),
                    }),
                    Err(e) => Err(io(e)),
                }
            }
            Err(e) => Err(e),
        };
        match &r {
            Ok(entry) => {
                let m = entry.manifest.as_ref();
                let warnings: usize =
                    m.map_or(0, |m| m.items.iter().map(|i| i.warnings.len()).sum());
                handle.succeed(Outcome {
                    summary: Some(format!(
                        "Copia {} completa: {} elementos, {} bytes, {} avisos",
                        entry.id,
                        m.map_or(0, |m| m.items.len()),
                        entry.size,
                        warnings
                    )),
                    ..Default::default()
                });
            }
            Err(e) => {
                // Nunca queda una copia a medias que parezca buena.
                let _ = target.delete(&ctx.server, &id);
                if matches!(e, EngineError::Other { message } if message == "operación cancelada")
                {
                    handle.cancel(Failure {
                        error: "copia cancelada".into(),
                        ..Default::default()
                    });
                } else {
                    handle.fail(Failure {
                        error: e.to_string(),
                        ..Default::default()
                    });
                }
            }
        }
        r
    }

    async fn backup_inner<F>(
        &self,
        pb: &mut PartialBackup,
        req: &BackupRequest,
        ctx: &BackupContext,
        cancel: &CancellationToken,
        progress: F,
    ) -> Result<Vec<BackupItem>, EngineError>
    where
        F: FnMut(BackupProgress) + Send,
    {
        let total = (req.volumes.len()
            + req.databases.len()
            + req.containers.len()
            + req.compose.len()
            + usize::from(req.app_config)) as u32;
        let containers = if req.volumes.is_empty() {
            Vec::new()
        } else {
            self.list_containers().await?
        };
        let vols = if req.volumes.is_empty() {
            Vec::new()
        } else {
            self.list_volumes(Some(&containers)).await?
        };
        let mut run = Run {
            engine: self,
            cancel,
            progress,
            done: 0,
            total,
            names: Names::default(),
            containers,
        };
        let mut items = Vec::new();
        for v in &req.volumes {
            run.check()?;
            run.report(ItemKind::Volume, &v.name, 0);
            items.push(run.volume(pb, v, &vols).await?);
            run.done += 1;
        }
        for d in &req.databases {
            run.check()?;
            run.report(ItemKind::Database, d, 0);
            items.push(run.database(pb, d).await?);
            run.done += 1;
        }
        for c in &req.containers {
            run.check()?;
            run.report(ItemKind::Container, c, 0);
            items.push(run.container(pb, c).await?);
            run.done += 1;
        }
        for c in &req.compose {
            run.check()?;
            run.report(ItemKind::Compose, &c.project, 0);
            items.push(run.compose(pb, c, &ctx.compose_projects).await?);
            run.done += 1;
        }
        if req.app_config {
            run.check()?;
            run.report(ItemKind::AppConfig, "app", 0);
            let mut files = Vec::new();
            for (name, data) in &ctx.app_config {
                let rel = run.names.unique("app", &file_stem(name), "");
                files.push(pb.write_all(&rel, data).await.map_err(io)?);
            }
            items.push(BackupItem {
                kind: ItemKind::AppConfig,
                name: "app".into(),
                files,
                consistency: Consistency::NotApplicable,
                details: BTreeMap::new(),
                warnings: vec![],
            });
            run.done += 1;
        }
        run.check()?;
        run.report(ItemKind::AppConfig, "", 0);
        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_validated_and_secrets_stripped() {
        assert!(validate_request(&BackupRequest::default(), true).is_err());
        let mut r = BackupRequest {
            volumes: vec![VolumeSelection {
                name: "../x".into(),
                mode: VolumeMode::Hot,
            }],
            ..Default::default()
        };
        assert!(validate_request(&r, true).is_err());
        r.volumes[0].name = "datos".into();
        assert!(validate_request(&r, true).is_ok());
        r.compose.push(ComposeSelection {
            project: "tienda".into(),
            include_env: false,
        });
        assert!(validate_request(&r, true).is_ok());
        // Compose en un servidor remoto: los archivos no están en este equipo.
        assert!(validate_request(&r, false).is_err());
        let (env, removed) = strip_secret_env(vec![
            "PATH=/bin".into(),
            "POSTGRES_PASSWORD=x".into(),
            "API_TOKEN=y".into(),
            "MODO".into(),
        ]);
        assert_eq!(env, vec!["PATH=/bin", "MODO"]);
        assert_eq!(removed, vec!["POSTGRES_PASSWORD", "API_TOKEN"]);
        let mut n = Names::default();
        assert_eq!(n.unique("volumes", "a_b", ".tar"), "volumes/a_b.tar");
        assert_eq!(n.unique("volumes", "a_b", ".tar"), "volumes/a_b-2.tar");
        assert_eq!(base("/srv/app/.env"), ".env");
        assert_eq!(base("/srv/app/docker compose.yml"), "docker_compose.yml");
        assert_eq!(base("/"), "archivo");
    }
}
