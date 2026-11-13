//! Fase 7: Docker Compose (CLI oficial sin shell, D-053).

use dcc_engine::cli::{OutputLine, RunEnd};
use dcc_engine::compose::file as cfile;
use dcc_engine::compose::ops::action_command;
use dcc_engine::compose::project::valid_project_name;
use dcc_engine::{
    ComposeAction, ComposeConfig, ComposeFile, ComposeProject, EngineError, ProjectRef,
};
use dcc_store::SavedProject;
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::State;
use ts_rs::TS;

use crate::error::{CmdResult, CommandError};
use crate::state::AppState;

/// Proyectos añadidos **en ese servidor**: los del equipo local en su
/// configuración; los de un servidor remoto en su perfil, con rutas del
/// servidor (0.10.0). Nunca se mezclan.
pub(crate) fn registered(state: &AppState, server: &str) -> CmdResult<Vec<ProjectRef>> {
    let saved = if server == crate::servers::LOCAL {
        state.config().compose.projects
    } else {
        state.servers.config(server)?.compose_projects
    };
    Ok(saved
        .into_iter()
        .map(|p| ProjectRef {
            name: p.name,
            files: p.files,
            working_dir: None,
            env_files: vec![],
        })
        .collect())
}

/// Cambia la lista de proyectos de ese servidor.
fn update_registered<T>(
    state: &AppState,
    server: &str,
    f: impl FnOnce(&mut Vec<SavedProject>) -> CmdResult<T>,
) -> CmdResult<T> {
    if server == crate::servers::LOCAL {
        Ok(state.update_compose(f)?.0)
    } else {
        state.servers.update_compose(server, f)
    }
}

/// Proyectos con contenedores (por sus etiquetas) y los añadidos por el usuario.
#[tauri::command]
pub async fn compose_projects(
    state: State<'_, AppState>,
    server: String,
) -> CmdResult<Vec<ComposeProject>> {
    let mgr = state.server(&server)?;
    let reg = registered(&state, &server)?;
    Ok(mgr
        .with_engine(|e| async move { e.compose_projects(&reg).await })
        .await?)
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UnsavedFile {
    /// Posición del archivo en `project.files`.
    pub index: u32,
    pub content: String,
}

/// Modelo y configuración resuelta (`docker compose config`). Con `unsaved`
/// valida el contenido del editor sin guardarlo.
#[tauri::command]
pub async fn compose_config(
    state: State<'_, AppState>,
    server: String,
    project: ProjectRef,
    unsaved: Option<UnsavedFile>,
) -> CmdResult<ComposeConfig> {
    let mgr = state.server(&server)?;
    let engine = mgr.engine().await?;
    let cli = mgr.compose_cli().await?;
    Ok(engine
        .compose_config(
            &cli,
            &project,
            unsaved.map(|u| (u.index as usize, u.content)),
        )
        .await?)
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ComposePreview {
    /// Comando exacto que se ejecutará.
    pub command: String,
    /// Riesgos para el equipo de los servicios que arrancaría `up` (D-086).
    /// Los altos y críticos se aceptan con su `key` al ejecutar.
    pub risks: Vec<dcc_engine::risk::Risk>,
}

/// Comando exacto de una acción, sin ejecutarla. Valida la entrada.
#[tauri::command]
pub async fn compose_preview(
    state: State<'_, AppState>,
    server: String,
    project: ProjectRef,
    action: ComposeAction,
) -> CmdResult<ComposePreview> {
    let mgr = state.server(&server)?;
    project.validate()?;
    action.validate()?;
    let cli = mgr.compose_cli().await?;
    let risks = mgr
        .engine()
        .await?
        .compose_up_risks(&cli, &project, &action)
        .await?;
    Ok(ComposePreview {
        command: action_command(&cli, &project, &action),
        risks,
    })
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunClosed {
    pub end: Option<RunEnd>,
    pub error: Option<CommandError>,
    /// Final de la salida de errores (mensaje de Compose si falló).
    pub stderr_tail: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(
    tag = "event",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ComposeEvent {
    Lines { lines: Vec<OutputLine> },
    End { closed: RunClosed },
}

fn closed(r: Result<dcc_engine::cli::StreamedRun, EngineError>) -> RunClosed {
    match r {
        Ok(run) => RunClosed {
            end: Some(run.end),
            error: None,
            stderr_tail: run
                .stderr_tail
                .lines()
                .rev()
                .take(40)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n"),
        },
        Err(e) => RunClosed {
            end: None,
            error: Some(e.into()),
            stderr_tail: String::new(),
        },
    }
}

/// Ejecuta una acción con la salida en vivo por `on_event`. Los errores de
/// validación, confirmación o archivos se devuelven antes de abrir nada.
/// Se detiene con `stream_close`.
#[tauri::command]
pub async fn compose_run_open(
    state: State<'_, AppState>,
    server: String,
    project: ProjectRef,
    action: ComposeAction,
    confirmed: bool,
    accepted_risks: Option<Vec<String>>,
    on_event: Channel<ComposeEvent>,
) -> CmdResult<u64> {
    let mgr = state.server(&server)?;
    let engine = mgr.engine().await?;
    // Comprobación previa (también de los archivos, donde esté el proyecto)
    // para responder antes de abrir la salida en vivo.
    let missing = engine.files_missing(&project.files).await?;
    engine.compose_check(&project, &action, confirmed, &missing)?;
    let cli = mgr.compose_cli().await?;
    // `up` con riesgos altos o críticos sin aceptar no se ejecuta (D-086): se
    // recalculan aquí, no se confía en lo que diga la interfaz.
    let risks = engine.compose_up_risks(&cli, &project, &action).await?;
    let missing_risks: Vec<_> =
        dcc_engine::risk::unaccepted(&risks, accepted_risks.as_deref().unwrap_or_default())
            .into_iter()
            .cloned()
            .collect();
    if !missing_risks.is_empty() {
        return Err(EngineError::RisksNotAccepted {
            risks: missing_risks,
        }
        .into());
    }
    let registry = state.streams.clone();
    let manager = mgr.clone();
    let (sid, cancel) = registry.register(&server, None);
    tauri::async_runtime::spawn(async move {
        let ch = on_event.clone();
        let r = engine
            .compose_run(&cli, &project, &action, confirmed, cancel, move |lines| {
                ch.send(ComposeEvent::Lines { lines }).is_ok()
            })
            .await;
        if matches!(&r, Err(e) if e.is_connection_error()) {
            manager.report_connection_error();
        }
        let _ = on_event.send(ComposeEvent::End { closed: closed(r) });
        registry.remove(sid);
    });
    Ok(sid)
}

/// `docker compose logs --follow` en vivo. Se cierra con `stream_close`.
#[tauri::command]
pub async fn compose_logs_open(
    state: State<'_, AppState>,
    server: String,
    project: ProjectRef,
    services: Vec<String>,
    tail: u32,
    timestamps: bool,
    on_event: Channel<ComposeEvent>,
) -> CmdResult<u64> {
    let mgr = state.server(&server)?;
    project.validate()?;
    let engine = mgr.engine().await?;
    let cli = mgr.compose_cli().await?;
    let registry = state.streams.clone();
    let (sid, cancel) = registry.register(&server, None);
    tauri::async_runtime::spawn(async move {
        let ch = on_event.clone();
        let r = engine
            .compose_logs(
                &cli,
                &project,
                &services,
                tail,
                timestamps,
                cancel,
                move |lines| ch.send(ComposeEvent::Lines { lines }).is_ok(),
            )
            .await;
        let _ = on_event.send(ComposeEvent::End { closed: closed(r) });
        registry.remove(sid);
    });
    Ok(sid)
}

// ------------------------------------------------------------------ lista de proyectos

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RegisterResult {
    pub project: Option<ProjectRef>,
    /// Mensaje de Compose si los archivos no son válidos (no se añade nada).
    pub error: Option<String>,
    pub command: String,
}

/// Añade un proyecto a la lista. El nombre lo decide Compose (su `name:` o la
/// carpeta) salvo que se indique otro.
#[tauri::command]
pub async fn compose_register(
    state: State<'_, AppState>,
    server: String,
    files: Vec<String>,
    name: Option<String>,
) -> CmdResult<RegisterResult> {
    let mgr = state.server(&server)?;
    let engine = mgr.engine().await?;
    let cli = mgr.compose_cli().await?;
    let resolved = engine.compose_resolve(&cli, &files).await?;
    let Some(model) = resolved.model else {
        return Ok(RegisterResult {
            project: None,
            error: resolved.error,
            command: resolved.command,
        });
    };
    let name = name
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or(model.name);
    if !valid_project_name(&name) {
        return Err(EngineError::InvalidInput {
            message: format!("nombre de proyecto no válido: {name:?}"),
        }
        .into());
    }
    let entry = SavedProject {
        name: name.clone(),
        files: files.clone(),
    };
    update_registered(state.inner(), &server, |ps| {
        if ps.iter().any(|p| p.name == name) {
            return Err(CommandError::new(
                "compose.duplicate",
                format!("ya hay un proyecto «{name}» en la lista"),
            ));
        }
        ps.push(entry);
        Ok(())
    })?;
    Ok(RegisterResult {
        project: Some(ProjectRef {
            name,
            files,
            working_dir: None,
            env_files: vec![],
        }),
        error: None,
        command: resolved.command,
    })
}

/// Quita un proyecto de la lista de ese servidor. No toca Docker ni los archivos.
#[tauri::command]
pub fn compose_unregister(
    state: State<'_, AppState>,
    server: String,
    name: String,
) -> CmdResult<bool> {
    state.server(&server)?;
    update_registered(state.inner(), &server, |ps| {
        let before = ps.len();
        ps.retain(|p| p.name != name);
        Ok(ps.len() != before)
    })
}

// ------------------------------------------------------------------ archivos

/// Los archivos están en el servidor del proyecto: en el equipo local se leen
/// del disco; en un servidor remoto, del servidor por su sesión SSH (0.10.0).
async fn remote_engine(state: &AppState, server: &str) -> CmdResult<Option<dcc_engine::Engine>> {
    let mgr = state.server(server)?;
    if !mgr.is_remote() {
        return Ok(None);
    }
    Ok(Some(mgr.engine().await?))
}

#[tauri::command]
pub async fn compose_file_read(
    state: State<'_, AppState>,
    server: String,
    path: String,
) -> CmdResult<ComposeFile> {
    if let Some(e) = remote_engine(&state, &server).await? {
        return Ok(dcc_engine::compose::remote_file::read(&e, &path).await?);
    }
    Ok(
        tauri::async_runtime::spawn_blocking(move || cfile::read(&path))
            .await
            .map_err(|e| CommandError::new("internal", e))??,
    )
}

/// Guarda el archivo si no ha cambiado desde que se leyó (`version`).
#[tauri::command]
pub async fn compose_file_save(
    state: State<'_, AppState>,
    server: String,
    path: String,
    content: String,
    version: String,
) -> CmdResult<ComposeFile> {
    if let Some(e) = remote_engine(&state, &server).await? {
        return Ok(dcc_engine::compose::remote_file::save(&e, &path, &content, &version).await?);
    }
    Ok(
        tauri::async_runtime::spawn_blocking(move || cfile::save(&path, &content, &version))
            .await
            .map_err(|e| CommandError::new("internal", e))??,
    )
}

/// Crea `compose.yaml` en una carpeta que aún no tiene archivo Compose.
#[tauri::command]
pub async fn compose_file_create(
    state: State<'_, AppState>,
    server: String,
    dir: String,
    content: String,
) -> CmdResult<ComposeFile> {
    if let Some(e) = remote_engine(&state, &server).await? {
        return Ok(dcc_engine::compose::remote_file::create(&e, &dir, &content).await?);
    }
    Ok(
        tauri::async_runtime::spawn_blocking(move || cfile::create(&dir, &content))
            .await
            .map_err(|e| CommandError::new("internal", e))??,
    )
}

/// Archivo Compose estándar de una carpeta, si lo tiene.
#[tauri::command]
pub async fn compose_find_file(
    state: State<'_, AppState>,
    server: String,
    dir: String,
) -> CmdResult<Option<String>> {
    if !dir.starts_with('/') {
        return Ok(None);
    }
    if let Some(e) = remote_engine(&state, &server).await? {
        return Ok(dcc_engine::compose::remote_file::existing_default(&e, &dir).await?);
    }
    Ok(cfile::existing_default(std::path::Path::new(&dir)).map(|p| p.display().to_string()))
}
