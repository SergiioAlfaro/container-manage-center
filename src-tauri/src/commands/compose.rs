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

fn registered(state: &AppState) -> Vec<ProjectRef> {
    state
        .config()
        .compose
        .projects
        .into_iter()
        .map(|p| ProjectRef {
            name: p.name,
            files: p.files,
            working_dir: None,
            env_files: vec![],
        })
        .collect()
}

/// Proyectos con contenedores (por sus etiquetas) y los añadidos por el usuario.
#[tauri::command]
pub async fn compose_projects(
    state: State<'_, AppState>,
    server: String,
) -> CmdResult<Vec<ComposeProject>> {
    let mgr = state.server(&server)?;
    let reg = registered(&state);
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
    Ok(ComposePreview {
        command: action_command(&cli, &project, &action),
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
    on_event: Channel<ComposeEvent>,
) -> CmdResult<u64> {
    let mgr = state.server(&server)?;
    let engine = mgr.engine().await?;
    engine.compose_check(&project, &action, confirmed)?;
    let cli = mgr.compose_cli().await?;
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
    let st = state.inner();
    let (_, _) = st.update_compose(|ps| {
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

/// Quita un proyecto de la lista. No toca Docker ni los archivos.
#[tauri::command]
pub fn compose_unregister(state: State<'_, AppState>, name: String) -> CmdResult<bool> {
    let (removed, _) = state.update_compose(|ps| {
        let before = ps.len();
        ps.retain(|p| p.name != name);
        Ok(ps.len() != before)
    })?;
    Ok(removed)
}

// ------------------------------------------------------------------ archivos

#[tauri::command]
pub async fn compose_file_read(path: String) -> CmdResult<ComposeFile> {
    Ok(
        tauri::async_runtime::spawn_blocking(move || cfile::read(&path))
            .await
            .map_err(|e| CommandError::new("internal", e))??,
    )
}

/// Guarda el archivo si no ha cambiado desde que se leyó (`version`).
#[tauri::command]
pub async fn compose_file_save(
    path: String,
    content: String,
    version: String,
) -> CmdResult<ComposeFile> {
    Ok(
        tauri::async_runtime::spawn_blocking(move || cfile::save(&path, &content, &version))
            .await
            .map_err(|e| CommandError::new("internal", e))??,
    )
}

/// Crea `compose.yaml` en una carpeta que aún no tiene archivo Compose.
#[tauri::command]
pub async fn compose_file_create(dir: String, content: String) -> CmdResult<ComposeFile> {
    Ok(
        tauri::async_runtime::spawn_blocking(move || cfile::create(&dir, &content))
            .await
            .map_err(|e| CommandError::new("internal", e))??,
    )
}

/// Archivo Compose estándar de una carpeta, si lo tiene.
#[tauri::command]
pub fn compose_find_file(dir: String) -> Option<String> {
    if !dir.starts_with('/') {
        return None;
    }
    cfile::existing_default(std::path::Path::new(&dir)).map(|p| p.display().to_string())
}
