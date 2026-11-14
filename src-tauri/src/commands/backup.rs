//! E4 (1.4.0): copias de seguridad, restauración y tareas programadas
//! (D-096–D-101).

use std::path::PathBuf;

use dcc_activity::{Category, Failure, OperationSpec, Outcome, Transport};
use dcc_engine::backup::store::{self, BackupTarget, LocalTarget};
use dcc_engine::backup::{
    BackupEntry, BackupProgress, BackupRequest, IntegrityReport, RestoreOutcome, RestorePreview,
    RestoreRequest,
};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_dialog::DialogExt;
use ts_rs::TS;

use crate::backups::{self, backup_error, BackupJob, BackupRun};
use crate::error::{CmdResult, CommandError};
use crate::state::AppState;
use crate::systemd::{Systemd, TimerPreview, TimerStatus};

#[derive(Debug, Clone, Serialize, TS)]
#[serde(
    tag = "event",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum BackupEvent {
    Progress {
        progress: BackupProgress,
    },
    BackupEnd {
        entry: Option<Box<BackupEntry>>,
        error: Option<CommandError>,
    },
    RestoreEnd {
        outcome: Option<RestoreOutcome>,
        error: Option<CommandError>,
    },
}

/// Una tarea con el estado de su temporizador.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BackupJobView {
    pub job: BackupJob,
    pub timer: TimerStatus,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BackupSettings {
    pub destination: Option<String>,
    pub jobs: Vec<BackupJobView>,
    /// `systemctl --user` disponible en este equipo.
    pub systemd: bool,
}

fn exe() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("/usr/bin/docker-control-center"))
}

fn systemd() -> CmdResult<Systemd> {
    Systemd::from_env()
        .ok_or_else(|| CommandError::new("backup.noSystemd", "no se encuentra systemctl"))
}

fn destination(state: &AppState, dest: Option<String>) -> CmdResult<String> {
    dest.or_else(|| backups::default_destination(state.store.path()))
        .ok_or_else(|| CommandError::new("backup.noDestination", "elige una carpeta de destino"))
}

fn target(dest: &str) -> CmdResult<LocalTarget> {
    let d = backups::check_destination(dest)?;
    LocalTarget::new(&d).map_err(|e| CommandError::new("backup.io", e))
}

fn known(state: &AppState, server: &str) -> bool {
    state.server(server).is_ok()
}

async fn job_view(state: &AppState, sd: Option<&Systemd>, job: BackupJob) -> BackupJobView {
    let timer = match sd {
        Some(s) => s.status(&state.activity, &job, &exe()).await,
        None => TimerStatus::default(),
    };
    BackupJobView { job, timer }
}

#[tauri::command]
pub async fn backup_settings(state: State<'_, AppState>) -> CmdResult<BackupSettings> {
    let sd = Systemd::from_env();
    let mut jobs = Vec::new();
    for j in backups::load_jobs(state.store.path()) {
        jobs.push(job_view(&state, sd.as_ref(), j).await);
    }
    Ok(BackupSettings {
        destination: backups::default_destination(state.store.path()),
        jobs,
        systemd: sd.is_some(),
    })
}

/// Elige una carpeta con el diálogo nativo. `save_default`: la guarda como
/// destino por defecto de las copias.
#[tauri::command]
pub async fn backup_pick_folder<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    save_default: bool,
) -> CmdResult<Option<String>> {
    let picked =
        tauri::async_runtime::spawn_blocking(move || app.dialog().file().blocking_pick_folder())
            .await
            .map_err(|e| CommandError::new("backup.io", e.to_string()))?;
    let Some(fp) = picked else { return Ok(None) };
    let p = fp
        .into_path()
        .map_err(|e| CommandError::new("backup.io", e.to_string()))?;
    let s = p.display().to_string();
    if save_default {
        let before = backups::default_destination(state.store.path()).unwrap_or_default();
        backups::set_default_destination(state.store.path(), &s)?;
        crate::state::record_preference(
            &state.activity,
            "backup.destination",
            "Destino de las copias",
            &before,
            &s,
        );
    }
    Ok(Some(s))
}

#[tauri::command]
pub fn backup_list(
    state: State<'_, AppState>,
    server: String,
    destination: Option<String>,
) -> CmdResult<Vec<BackupEntry>> {
    state.server(&server)?;
    let dest = match destination.or_else(|| backups::default_destination(state.store.path())) {
        Some(d) => d,
        None => return Ok(vec![]),
    };
    target(&dest)?
        .list(&server)
        .map_err(|e| CommandError::new("backup.io", e))
}

fn open_backup(
    state: &AppState,
    server: &str,
    dest: Option<String>,
    id: &str,
) -> CmdResult<PathBuf> {
    state.server(server)?;
    let dest = destination(state, dest)?;
    target(&dest)?
        .open(server, id)
        .map_err(|e| CommandError::new("backup.notFound", format!("{id}: {e}")))
}

/// Comprueba las sumas de una copia (solo lectura), y lo registra.
#[tauri::command]
pub async fn backup_verify(
    state: State<'_, AppState>,
    server: String,
    destination: Option<String>,
    id: String,
) -> CmdResult<IntegrityReport> {
    let dir = open_backup(&state, &server, destination, &id)?;
    let h = state.activity.begin(OperationSpec {
        category: Category::Backup,
        kind: "backup.verify".into(),
        target: Some(id.clone()),
        transport: Transport::App {
            action: format!("Comprobar la integridad de {}", dir.display()),
        },
        request_body: None,
        equivalent_command: None,
    });
    let r = tauri::async_runtime::spawn_blocking(move || store::verify(&dir))
        .await
        .map_err(|e| CommandError::new("backup.io", e.to_string()))?
        .map_err(|e| CommandError::new("backup.io", e));
    match &r {
        Ok(rep) if rep.ok => h.succeed(Outcome {
            summary: Some(format!("{} archivos correctos", rep.checked)),
            ..Default::default()
        }),
        Ok(rep) => h.fail(Failure {
            error: format!(
                "{} alterados, {} faltan, {} de más",
                rep.altered.len(),
                rep.missing.len(),
                rep.unexpected.len()
            ),
            ..Default::default()
        }),
        Err(e) => h.fail(Failure {
            error: e.message.clone(),
            ..Default::default()
        }),
    }
    r
}

/// Elimina una copia (completa o incompleta). Requiere confirmación.
#[tauri::command]
pub fn backup_delete(
    state: State<'_, AppState>,
    server: String,
    destination: Option<String>,
    id: String,
    confirmed: bool,
) -> CmdResult<()> {
    if !confirmed {
        return Err(dcc_engine::EngineError::NotConfirmed.into());
    }
    state.server(&server)?;
    let dest = destination_or(&state, destination)?;
    let t = target(&dest)?;
    let h = state.activity.begin(OperationSpec {
        category: Category::Backup,
        kind: "backup.delete".into(),
        target: Some(id.clone()),
        transport: Transport::App {
            action: format!("Eliminar la copia {id} de {}", t.describe()),
        },
        request_body: None,
        equivalent_command: None,
    });
    match t.delete(&server, &id) {
        Ok(()) => {
            h.succeed(Outcome {
                summary: Some("Copia eliminada".into()),
                ..Default::default()
            });
            Ok(())
        }
        Err(e) => {
            h.fail(Failure {
                error: e.to_string(),
                ..Default::default()
            });
            Err(CommandError::new("backup.io", e))
        }
    }
}

fn destination_or(state: &AppState, d: Option<String>) -> CmdResult<String> {
    destination(state, d)
}

/// Hace una copia manual (o de una tarea, con `job`) con avance por canal.
#[tauri::command]
pub async fn backup_run_open<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    server: String,
    destination: Option<String>,
    request: BackupRequest,
    job: Option<String>,
    on_event: Channel<BackupEvent>,
) -> CmdResult<u64> {
    let (server, dest, request, job) = match job {
        Some(id) => {
            let j = backups::find_job(state.store.path(), &id)?;
            (
                j.server.clone(),
                j.destination.clone(),
                j.request.clone(),
                Some(j),
            )
        }
        None => (server, destination_or(&state, destination)?, request, None),
    };
    let mgr = state.server(&server)?;
    backups::check_destination(&dest)?;
    dcc_engine::backup::run::validate_request(&request, !mgr.is_remote()).map_err(backup_error)?;
    let registry = state.streams.clone();
    let (sid, cancel) = registry.register(&server, None);
    tauri::async_runtime::spawn(async move {
        let ch = on_event.clone();
        let trigger = if job.is_some() { "job" } else { "manual" };
        let app_state = app.state::<AppState>();
        let r = backups::run_backup(
            &app_state,
            &mgr,
            &server,
            &dest,
            &request,
            job.as_ref(),
            trigger,
            cancel,
            move |p| {
                let _ = ch.send(BackupEvent::Progress { progress: p });
            },
        )
        .await;
        let end = match r {
            Ok(e) => BackupEvent::BackupEnd {
                entry: Some(Box::new(e)),
                error: None,
            },
            Err(e) => BackupEvent::BackupEnd {
                entry: None,
                error: Some(e),
            },
        };
        let _ = on_event.send(end);
        registry.remove(sid);
    });
    Ok(sid)
}

/// Vista previa de una restauración (solo lectura).
#[tauri::command]
pub async fn backup_restore_preview(
    state: State<'_, AppState>,
    server: String,
    destination: Option<String>,
    id: String,
    request: RestoreRequest,
) -> CmdResult<RestorePreview> {
    let dir = open_backup(&state, &server, destination, &id)?;
    let mgr = state.server(&server)?;
    let engine = mgr.engine().await?;
    engine
        .restore_preview(&dir, &request, !mgr.is_remote())
        .await
        .map_err(backup_error)
}

/// Restaura (con confirmación) y envía el resultado por el canal.
#[tauri::command]
pub async fn backup_restore_open(
    state: State<'_, AppState>,
    server: String,
    destination: Option<String>,
    id: String,
    request: RestoreRequest,
    on_event: Channel<BackupEvent>,
) -> CmdResult<u64> {
    if !request.confirmed {
        return Err(dcc_engine::EngineError::NotConfirmed.into());
    }
    let dir = open_backup(&state, &server, destination, &id)?;
    let mgr = state.server(&server)?;
    let engine = mgr.engine().await?;
    let registry = state.streams.clone();
    let (sid, cancel) = registry.register(&server, None);
    let local = !mgr.is_remote();
    tauri::async_runtime::spawn(async move {
        let r = engine.restore(&dir, &request, local, cancel).await;
        if matches!(&r, Err(e) if e.is_connection_error()) {
            mgr.report_connection_error();
        }
        let end = match r {
            Ok(o) => BackupEvent::RestoreEnd {
                outcome: Some(o),
                error: None,
            },
            Err(e) => BackupEvent::RestoreEnd {
                outcome: None,
                error: Some(backup_error(e)),
            },
        };
        let _ = on_event.send(end);
        registry.remove(sid);
    });
    Ok(sid)
}

#[tauri::command]
pub fn backup_history(
    state: State<'_, AppState>,
    server: Option<String>,
) -> CmdResult<Vec<BackupRun>> {
    let mut v: Vec<BackupRun> = backups::runs(&state)
        .into_iter()
        .filter(|r| server.as_deref().map_or(true, |s| r.server == s))
        .collect();
    v.reverse();
    Ok(v)
}

// ------------------------------------------------------------------ tareas

#[tauri::command]
pub async fn backup_job_save(
    state: State<'_, AppState>,
    job: BackupJob,
) -> CmdResult<BackupJobView> {
    let known = known(&state, &job.server);
    let is_new = job.id.is_empty();
    let saved = backups::save_job(state.store.path(), job, known)?;
    crate::state::record_preference(
        &state.activity,
        "backup.job.save",
        if is_new {
            "Crear una tarea de copia"
        } else {
            "Cambiar una tarea de copia"
        },
        "",
        &saved.name,
    );
    Ok(job_view(&state, Systemd::from_env().as_ref(), saved).await)
}

/// Elimina una tarea. Si tiene temporizador, primero hay que eliminarlo (con
/// su propia autorización). Las copias ya hechas no se tocan.
#[tauri::command]
pub async fn backup_job_delete(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let job = backups::find_job(state.store.path(), &id)?;
    if let Some(sd) = Systemd::from_env() {
        let st = sd.status(&state.activity, &job, &exe()).await;
        if st.installed {
            return Err(CommandError::new(
                "backup.timerInstalled",
                "elimina antes la programación",
            ));
        }
    }
    backups::delete_job(state.store.path(), &id)?;
    crate::state::record_preference(
        &state.activity,
        "backup.job.delete",
        "Eliminar una tarea de copia",
        &job.name,
        "",
    );
    Ok(())
}

/// Archivos y órdenes que se crearían, para pedir la autorización.
#[tauri::command]
pub fn backup_timer_preview(state: State<'_, AppState>, id: String) -> CmdResult<TimerPreview> {
    let job = backups::find_job(state.store.path(), &id)?;
    Ok(systemd()?.preview(&job, &exe()))
}

/// Crea o actualiza el temporizador. Solo con `authorized` (el usuario ha
/// visto los archivos y lo ha autorizado expresamente).
#[tauri::command]
pub async fn backup_timer_install(
    state: State<'_, AppState>,
    id: String,
    authorized: bool,
) -> CmdResult<BackupJobView> {
    if !authorized {
        return Err(CommandError::new(
            "backup.timerNotAuthorized",
            "la programación necesita tu autorización",
        ));
    }
    let job = backups::find_job(state.store.path(), &id)?;
    let sd = systemd()?;
    sd.install(&state.activity, &job, &exe())
        .await
        .map_err(|e| CommandError::new("backup.timer", e))?;
    Ok(job_view(&state, Some(&sd), job).await)
}

#[tauri::command]
pub async fn backup_timer_set_enabled(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> CmdResult<BackupJobView> {
    let job = backups::find_job(state.store.path(), &id)?;
    let sd = systemd()?;
    sd.set_enabled(&state.activity, &id, enabled)
        .await
        .map_err(|e| CommandError::new("backup.timer", e))?;
    Ok(job_view(&state, Some(&sd), job).await)
}

/// Elimina el temporizador (solo los archivos propios). Requiere confirmación.
#[tauri::command]
pub async fn backup_timer_remove(
    state: State<'_, AppState>,
    id: String,
    confirmed: bool,
) -> CmdResult<BackupJobView> {
    if !confirmed {
        return Err(dcc_engine::EngineError::NotConfirmed.into());
    }
    let job = backups::find_job(state.store.path(), &id)?;
    let sd = systemd()?;
    sd.remove(&state.activity, &id)
        .await
        .map_err(|e| CommandError::new("backup.timer", e))?;
    Ok(job_view(&state, Some(&sd), job).await)
}
