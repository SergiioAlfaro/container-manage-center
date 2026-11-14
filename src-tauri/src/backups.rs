//! Copias de seguridad (E4, 1.4.0): tareas, historial y ejecución común a la
//! interfaz y al modo sin ventana de los temporizadores (`--run-backup`).
//!
//! - Las tareas se guardan en `copias.json` junto a `config.toml` (0600).
//! - El historial de ejecuciones, en `copias.jsonl` en la carpeta de estado
//!   (0600). Se lee del disco en cada consulta: una copia programada lo
//!   escribe desde otro proceso.
//! - La retención solo borra copias **completas** de **esa** tarea (D-097).

use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use dcc_activity::{Category, Failure, OperationSpec, Outcome, Transport};
use dcc_engine::backup::store::{self, BackupTarget, LocalTarget};
use dcc_engine::backup::{BackupContext, BackupEntry, BackupProgress, BackupRequest, Retention};
use dcc_engine::EngineManager;
use dcc_store::journal::Journal;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::error::{CmdResult, CommandError};
use crate::state::AppState;

pub const JOBS_FILE: &str = "copias.json";
pub const RUNS_FILE: &str = "copias.jsonl";
pub const RUNS_MAX: usize = 1000;
const MAX_JOBS: usize = 100;

/// Cuándo se ejecuta una tarea (hora local del equipo).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export, rename = "BackupSchedule")]
pub enum Schedule {
    Daily {
        hour: u8,
        minute: u8,
    },
    /// `weekday`: 1 = lunes … 7 = domingo.
    Weekly {
        weekday: u8,
        hour: u8,
        minute: u8,
    },
    /// `day`: 1–28 (existe todos los meses).
    Monthly {
        day: u8,
        hour: u8,
        minute: u8,
    },
}

impl Schedule {
    fn validate(&self) -> Result<(), String> {
        let (h, m) = match *self {
            Self::Daily { hour, minute } => (hour, minute),
            Self::Weekly {
                weekday,
                hour,
                minute,
            } => {
                if !(1..=7).contains(&weekday) {
                    return Err("día de la semana no válido".into());
                }
                (hour, minute)
            }
            Self::Monthly { day, hour, minute } => {
                if !(1..=28).contains(&day) {
                    return Err("día del mes no válido (1–28)".into());
                }
                (hour, minute)
            }
        };
        if h > 23 || m > 59 {
            return Err("hora no válida".into());
        }
        Ok(())
    }
}

/// Una tarea de copia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BackupJob {
    /// `job-` y 8 caracteres hexadecimales. Vacío al crearla.
    pub id: String,
    pub name: String,
    pub server: String,
    /// Carpeta de destino (absoluta).
    pub destination: String,
    pub request: BackupRequest,
    pub retention: Retention,
    pub schedule: Schedule,
}

impl BackupJob {
    #[cfg(test)]
    pub fn new_for_tests(id: &str) -> Self {
        Self {
            id: id.into(),
            name: "Prueba".into(),
            server: "local".into(),
            destination: "/tmp".into(),
            request: BackupRequest {
                app_config: true,
                ..Default::default()
            },
            retention: Retention::default(),
            schedule: Schedule::Daily { hour: 3, minute: 0 },
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JobsFile {
    version: u32,
    #[serde(default)]
    destination: Option<String>,
    #[serde(default)]
    jobs: Vec<BackupJob>,
}

/// Una ejecución, manual o programada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BackupRun {
    pub server: String,
    pub job: Option<String>,
    pub job_name: Option<String>,
    /// `manual`, `job` (desde la aplicación) o `timer` (temporizador).
    pub trigger: String,
    pub destination: String,
    pub started_at: String,
    pub finished_at: String,
    pub ok: bool,
    pub backup_id: Option<String>,
    #[ts(type = "number")]
    pub size: u64,
    pub items: u32,
    pub warnings: u32,
    pub error: Option<String>,
    /// Copias eliminadas por la retención.
    pub deleted: Vec<String>,
}

pub fn jobs_path(config: &Path) -> PathBuf {
    config.with_file_name(JOBS_FILE)
}

fn load_file(config: &Path) -> JobsFile {
    match std::fs::read_to_string(jobs_path(config)) {
        Ok(t) => serde_json::from_str(&t).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "copias.json no válido; se ignora");
            JobsFile::default()
        }),
        Err(_) => JobsFile::default(),
    }
}

fn save_file(config: &Path, f: &JobsFile) -> CmdResult<()> {
    let mut f = f.clone();
    f.version = 1;
    let text = serde_json::to_string_pretty(&f).map_err(|e| CommandError::new("backup.io", e))?;
    dcc_store::write_atomic(&jobs_path(config), &text)
        .map_err(|e| CommandError::new("backup.io", e))
}

pub fn load_jobs(config: &Path) -> Vec<BackupJob> {
    load_file(config).jobs
}

pub fn default_destination(config: &Path) -> Option<String> {
    load_file(config).destination
}

pub fn set_default_destination(config: &Path, dest: &str) -> CmdResult<()> {
    check_destination(dest)?;
    let mut f = load_file(config);
    f.destination = Some(dest.to_owned());
    save_file(config, &f)
}

/// Carpetas del sistema donde no se guardan copias.
const FORBIDDEN: [&str; 12] = [
    "/",
    "/bin",
    "/boot",
    "/dev",
    "/etc",
    "/lib",
    "/proc",
    "/run",
    "/sbin",
    "/sys",
    "/usr",
    "/var/lib/docker",
];

/// Destino válido: ruta absoluta de una carpeta existente, fuera del sistema.
pub fn check_destination(dest: &str) -> CmdResult<PathBuf> {
    let norm = dcc_engine::risk::normalize(dest).ok_or_else(|| {
        CommandError::new(
            "backup.destinationInvalid",
            format!("ruta no válida: {dest:?}"),
        )
    })?;
    if !Path::new(&norm).is_absolute() {
        return Err(CommandError::new(
            "backup.destinationInvalid",
            "la ruta debe ser absoluta",
        ));
    }
    if FORBIDDEN
        .iter()
        .any(|f| norm == *f || (*f != "/" && norm.starts_with(&format!("{f}/"))))
    {
        return Err(CommandError::new(
            "backup.destinationSystem",
            format!("{norm} es una carpeta del sistema"),
        ));
    }
    if !Path::new(&norm).is_dir() {
        return Err(CommandError::new(
            "backup.destinationMissing",
            format!("no existe la carpeta {norm}"),
        ));
    }
    Ok(PathBuf::from(norm))
}

pub fn validate_job(job: &BackupJob, known_server: bool) -> CmdResult<()> {
    let bad = |m: &str| Err(CommandError::new("backup.jobInvalid", m));
    if job.name.trim().is_empty() || job.name.chars().count() > 80 {
        return bad("el nombre debe tener entre 1 y 80 caracteres");
    }
    if !known_server {
        return bad("servidor desconocido");
    }
    check_destination(&job.destination)?;
    job.schedule
        .validate()
        .map_err(|m| CommandError::new("backup.jobInvalid", m))?;
    store::check_retention(&job.retention)
        .map_err(|m| CommandError::new("backup.jobInvalid", m))?;
    dcc_engine::backup::run::validate_request(&job.request, job.server == "local")
        .map_err(backup_error)?;
    Ok(())
}

fn valid_job_id(id: &str) -> bool {
    id.len() == 12 && id.starts_with("job-") && id[4..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Guarda una tarea (nueva si no tiene id).
pub fn save_job(config: &Path, mut job: BackupJob, known_server: bool) -> CmdResult<BackupJob> {
    job.name = job.name.trim().to_owned();
    validate_job(&job, known_server)?;
    let mut f = load_file(config);
    if job.id.is_empty() {
        if f.jobs.len() >= MAX_JOBS {
            return Err(CommandError::new("backup.jobInvalid", "demasiadas tareas"));
        }
        job.id = format!("job-{}", dcc_engine::ssh::askpass::random_hex(4));
        f.jobs.push(job.clone());
    } else if !valid_job_id(&job.id) {
        return Err(CommandError::new("backup.jobInvalid", "id no válido"));
    } else if let Some(j) = f.jobs.iter_mut().find(|j| j.id == job.id) {
        *j = job.clone();
    } else {
        return Err(CommandError::new(
            "backup.jobNotFound",
            format!("no existe la tarea {}", job.id),
        ));
    }
    save_file(config, &f)?;
    Ok(job)
}

pub fn delete_job(config: &Path, id: &str) -> CmdResult<()> {
    let mut f = load_file(config);
    let before = f.jobs.len();
    f.jobs.retain(|j| j.id != id);
    if f.jobs.len() == before {
        return Err(CommandError::new(
            "backup.jobNotFound",
            format!("no existe la tarea {id}"),
        ));
    }
    save_file(config, &f)
}

pub fn find_job(config: &Path, id: &str) -> CmdResult<BackupJob> {
    load_jobs(config)
        .into_iter()
        .find(|j| j.id == id)
        .ok_or_else(|| CommandError::new("backup.jobNotFound", format!("no existe la tarea {id}")))
}

/// Errores del motor con código propio (`backup.*`, `restore.*`) como código
/// de la interfaz.
pub fn backup_error(e: dcc_engine::EngineError) -> CommandError {
    if let dcc_engine::EngineError::InvalidInput { message } = &e {
        let code = message.as_str();
        let is_code = (code.starts_with("backup.") || code.starts_with("restore."))
            && code.len() < 64
            && code[code.find('.').unwrap_or(0) + 1..]
                .chars()
                .all(|c| c.is_ascii_alphanumeric());
        if is_code {
            return CommandError::new(code, code);
        }
    }
    e.into()
}

// ------------------------------------------------------------------ historial

pub fn runs_path(state_dir: &Path) -> PathBuf {
    state_dir.join(RUNS_FILE)
}

pub fn record_run(state: &AppState, run: BackupRun) {
    let Some(dir) = &state.journals.dir else {
        return;
    };
    match Journal::open(runs_path(dir), RUNS_MAX) {
        Ok((j, _)) => {
            if let Err(e) = j.append(run) {
                tracing::warn!(error = %e, "no se pudo guardar el historial de copias");
            }
        }
        Err(e) => tracing::warn!(error = %e, "no se pudo abrir el historial de copias"),
    }
}

pub fn runs(state: &AppState) -> Vec<BackupRun> {
    let Some(dir) = &state.journals.dir else {
        return vec![];
    };
    Journal::<BackupRun>::open(runs_path(dir), RUNS_MAX)
        .map(|(j, _)| j.entries())
        .unwrap_or_default()
}

// ------------------------------------------------------------------ ejecución

/// Archivos de configuración de la aplicación que entran en la copia. No
/// tienen secretos: las contraseñas y frases están en el llavero (D-076).
pub fn app_config_files(config: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for name in [dcc_store::CONFIG_FILE, "servers.toml", JOBS_FILE] {
        let p = config.with_file_name(name);
        if let Ok(d) = std::fs::read(&p) {
            let text = String::from_utf8_lossy(&d);
            // Por si acaso: se ocultan los secretos que pudiera haber.
            out.push((
                name.to_owned(),
                dcc_activity::redact::redact_text(&text)
                    .into_owned()
                    .into_bytes(),
            ));
        }
    }
    out
}

/// Contexto de la copia: proyectos Compose (si se piden) y configuración.
pub async fn context(
    state: &AppState,
    mgr: &EngineManager,
    server: &str,
    req: &BackupRequest,
    job: Option<&str>,
) -> CmdResult<BackupContext> {
    let engine = mgr.engine().await?;
    let compose_projects = if req.compose.is_empty() {
        vec![]
    } else {
        let reg = crate::commands::compose::registered(state, server)?;
        engine
            .compose_projects(&reg)
            .await
            .map(|ps| ps.into_iter().map(|p| p.reference).collect())
            .unwrap_or(reg)
    };
    Ok(BackupContext {
        server: server.to_owned(),
        local: !mgr.is_remote(),
        job: job.map(str::to_owned),
        compose_projects,
        app_config: if req.app_config {
            app_config_files(state.store.path())
        } else {
            vec![]
        },
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
    })
}

/// Aplica la retención de una tarea en su destino. Devuelve lo eliminado.
pub fn apply_retention(state: &AppState, job: &BackupJob, target: &LocalTarget) -> Vec<String> {
    let Ok(entries) = target.list(&job.server) else {
        return vec![];
    };
    let victims = store::retention_victims(&entries, &job.id, job.retention);
    let mut deleted = Vec::new();
    for id in victims {
        let h = state.activity.begin(OperationSpec {
            category: Category::Backup,
            kind: "backup.retention.delete".into(),
            target: Some(id.clone()),
            transport: Transport::App {
                action: format!("Retención de «{}»: eliminar la copia {id}", job.name),
            },
            request_body: None,
            equivalent_command: None,
        });
        match target.delete(&job.server, &id) {
            Ok(()) => {
                h.succeed(Outcome {
                    summary: Some("Copia eliminada por la retención".into()),
                    ..Default::default()
                });
                deleted.push(id);
            }
            Err(e) => h.fail(Failure {
                error: e.to_string(),
                ..Default::default()
            }),
        }
    }
    deleted
}

/// Hace una copia y la apunta en el historial. Con `job`, aplica después su
/// retención (solo si la copia salió bien).
#[allow(clippy::too_many_arguments)]
pub async fn run_backup<F>(
    state: &AppState,
    mgr: &EngineManager,
    server: &str,
    destination: &str,
    req: &BackupRequest,
    job: Option<&BackupJob>,
    trigger: &str,
    cancel: CancellationToken,
    progress: F,
) -> CmdResult<BackupEntry>
where
    F: FnMut(BackupProgress) + Send,
{
    let started = Utc::now();
    let mut run = BackupRun {
        server: server.to_owned(),
        job: job.map(|j| j.id.clone()),
        job_name: job.map(|j| j.name.clone()),
        trigger: trigger.to_owned(),
        destination: destination.to_owned(),
        started_at: started.to_rfc3339_opts(SecondsFormat::Secs, true),
        finished_at: String::new(),
        ok: false,
        backup_id: None,
        size: 0,
        items: 0,
        warnings: 0,
        error: None,
        deleted: vec![],
    };
    let result = async {
        let dest = check_destination(destination)?;
        let target = LocalTarget::new(&dest).map_err(|e| CommandError::new("backup.io", e))?;
        let ctx = context(state, mgr, server, req, job.map(|j| j.id.as_str())).await?;
        let engine = mgr.engine().await?;
        let r = engine.backup(&target, req, &ctx, cancel, progress).await;
        if matches!(&r, Err(e) if e.is_connection_error()) {
            mgr.report_connection_error();
        }
        let entry = r.map_err(backup_error)?;
        let deleted = job
            .map(|j| apply_retention(state, j, &target))
            .unwrap_or_default();
        Ok::<_, CommandError>((entry, deleted))
    }
    .await;
    run.finished_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let out = match result {
        Ok((entry, deleted)) => {
            run.ok = true;
            run.backup_id = Some(entry.id.clone());
            run.size = entry.size;
            if let Some(m) = &entry.manifest {
                run.items = m.items.len() as u32;
                run.warnings = m.items.iter().map(|i| i.warnings.len() as u32).sum();
            }
            run.deleted = deleted;
            Ok(entry)
        }
        Err(e) => {
            run.error = Some(
                dcc_activity::redact::redact_text(&format!("{}: {}", e.code, e.message))
                    .into_owned(),
            );
            Err(e)
        }
    };
    record_run(state, run);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jobs_are_validated_saved_private_and_survive_a_restart() {
        let d = tempfile::Builder::new()
            .prefix("dcc-test")
            .tempdir()
            .unwrap();
        let cfg = d.path().join("config.toml");
        let dest = d.path().join("destino");
        std::fs::create_dir(&dest).unwrap();
        let mut j = BackupJob::new_for_tests("");
        j.destination = dest.display().to_string();
        let saved = save_job(&cfg, j.clone(), true).unwrap();
        assert!(valid_job_id(&saved.id), "{}", saved.id);
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(jobs_path(&cfg))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        // Otra «sesión» lee lo mismo del disco.
        assert_eq!(load_jobs(&cfg), vec![saved.clone()]);
        let mut upd = saved.clone();
        upd.schedule = Schedule::Weekly {
            weekday: 3,
            hour: 1,
            minute: 30,
        };
        save_job(&cfg, upd.clone(), true).unwrap();
        assert_eq!(find_job(&cfg, &saved.id).unwrap().schedule, upd.schedule);
        // Validación.
        let mut bad = j.clone();
        bad.schedule = Schedule::Daily {
            hour: 24,
            minute: 0,
        };
        assert!(save_job(&cfg, bad, true).is_err());
        let mut bad = j.clone();
        bad.destination = "/etc".into();
        assert_eq!(
            save_job(&cfg, bad, true).unwrap_err().code,
            "backup.destinationSystem"
        );
        let mut bad = j.clone();
        bad.destination = "/var/lib/docker/volumes".into();
        assert!(save_job(&cfg, bad, true).is_err());
        let mut bad = j.clone();
        bad.request = BackupRequest::default();
        assert_eq!(
            save_job(&cfg, bad, true).unwrap_err().code,
            "backup.emptyRequest"
        );
        assert!(
            save_job(&cfg, j.clone(), false).is_err(),
            "servidor desconocido"
        );
        let mut bad = j.clone();
        bad.retention = Retention {
            daily: 0,
            weekly: 0,
            monthly: 0,
        };
        assert!(save_job(&cfg, bad, true).is_err());
        let mut forged = j.clone();
        forged.id = "../../x".into();
        assert!(save_job(&cfg, forged, true).is_err());
        delete_job(&cfg, &saved.id).unwrap();
        assert!(load_jobs(&cfg).is_empty());
        assert!(delete_job(&cfg, &saved.id).is_err());
        // Destino por defecto.
        set_default_destination(&cfg, dest.to_str().unwrap()).unwrap();
        assert_eq!(default_destination(&cfg), Some(dest.display().to_string()));
        assert!(set_default_destination(&cfg, "/proc").is_err());
    }

    #[test]
    fn engine_codes_become_interface_codes() {
        let e = backup_error(dcc_engine::EngineError::InvalidInput {
            message: "backup.dbNotRunning".into(),
        });
        assert_eq!(e.code, "backup.dbNotRunning");
        let e = backup_error(dcc_engine::EngineError::InvalidInput {
            message: "otra cosa".into(),
        });
        assert_eq!(e.code, "engine.invalidInput");
    }
}
