//! Fase 3: crear, inspeccionar y duplicar contenedores; datos para el asistente;
//! flujos de logs, estadísticas y terminal.

use base64::Engine as _;
use dcc_engine::catalog::{ImageDefaults, NetworkBrief, VolumeBrief};
use dcc_engine::details::ContainerDetails;
use dcc_engine::edit::{EditOutcome, EditPlan, EditRequest, EditableConfig};
use dcc_engine::edit_ops::{ApplyOptions, CommitPreview, CommitRequest, CommitResult};
use dcc_engine::spec::{ContainerSpec, DuplicateSpec, SpecIssue};
use dcc_engine::streams::{LogLine, ShellChoice, StatsSample, StreamEnd, TerminalInput};
use dcc_engine::{ContainerAction, ContainerState, EngineError};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;
use tokio::sync::mpsc;
use ts_rs::TS;

use crate::error::{CmdResult, CommandError};
use crate::state::AppState;

// ------------------------------------------------------------------ asistente

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SpecPreview {
    pub issues: Vec<SpecIssue>,
    /// `docker run --detach …` equivalente (lo que verá el usuario).
    pub run_command: String,
    /// `docker create …` equivalente (la primera operación que se ejecuta).
    pub create_command: String,
    /// Cuerpo JSON de `POST /containers/create`.
    pub body: String,
    /// Valores de variables sensibles sustituidos por `********` en los
    /// comandos y el cuerpo mostrados (a Docker se envían completos).
    pub secrets_hidden: u32,
}

/// Valida y traduce la especificación sin cambiar nada (D-034). Las carpetas
/// de los bind mounts se comprueban donde está Docker: en este equipo o, con
/// un servidor remoto conectado, en el servidor (0.10.0).
#[tauri::command]
pub async fn container_preview(
    state: State<'_, AppState>,
    server: String,
    spec: ContainerSpec,
) -> CmdResult<SpecPreview> {
    let mgr = state.server(&server)?;
    let issues = if mgr.is_remote() {
        match mgr.engine().await {
            Ok(e) => e.spec_issues(&spec).await?,
            // Sin conexión no se puede comprobar el servidor: se valida el resto.
            Err(_) => spec.validate(false),
        }
    } else {
        spec.validate(true)
    };
    let (shown, hidden) = spec.redacted();
    let (_, body) = shown.to_create_body();
    Ok(SpecPreview {
        issues,
        run_command: shown.cli_command(true),
        create_command: shown.cli_command(false),
        body: serde_json::to_string_pretty(&body).unwrap_or_default(),
        secrets_hidden: hidden as u32,
    })
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CreateResult {
    pub id: String,
    pub name: String,
    pub warnings: Vec<String>,
    /// Estado confirmado si se pidió iniciarlo y arrancó.
    pub state: Option<ContainerState>,
    /// El contenedor se creó pero no pudo iniciarse.
    pub start_error: Option<CommandError>,
}

#[tauri::command]
pub async fn container_create(
    state: State<'_, AppState>,
    server: String,
    spec: ContainerSpec,
    start: bool,
) -> CmdResult<CreateResult> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move {
            let created = e.create_container(&spec).await?;
            let display = (!spec.name.trim().is_empty()).then(|| spec.name.trim().to_owned());
            let (st, start_error) = if start {
                match e
                    .container_action(&created.id, display.as_deref(), ContainerAction::Start)
                    .await
                {
                    Ok(s) => (Some(s), None),
                    Err(err) => (None, Some(CommandError::from(err))),
                }
            } else {
                (None, None)
            };
            let name = match (&st, display) {
                (Some(s), _) if !s.name.is_empty() => s.name.clone(),
                (_, Some(n)) => n,
                _ => e
                    .container_state(&created.id, None)
                    .await
                    .map(|s| s.name)
                    .unwrap_or_default(),
            };
            Ok::<_, EngineError>(CreateResult {
                id: created.id,
                name,
                warnings: created.warnings,
                state: st,
                start_error,
            })
        })
        .await?)
}

#[tauri::command]
pub async fn container_inspect(
    state: State<'_, AppState>,
    server: String,
    id: String,
    name: Option<String>,
) -> CmdResult<ContainerDetails> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move { e.inspect_container(&id, name.as_deref()).await })
        .await?)
}

#[tauri::command]
pub async fn container_duplicate_spec(
    state: State<'_, AppState>,
    server: String,
    id: String,
    name: Option<String>,
) -> CmdResult<DuplicateSpec> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move { e.duplicate_spec(&id, name.as_deref()).await })
        .await?)
}

#[tauri::command]
pub async fn image_defaults(
    state: State<'_, AppState>,
    server: String,
    image: String,
) -> CmdResult<ImageDefaults> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move { e.image_defaults(&image).await })
        .await?)
}

#[tauri::command]
pub async fn networks_brief(
    state: State<'_, AppState>,
    server: String,
) -> CmdResult<Vec<NetworkBrief>> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move { e.networks_brief().await })
        .await?)
}

#[tauri::command]
pub async fn volumes_brief(
    state: State<'_, AppState>,
    server: String,
) -> CmdResult<Vec<VolumeBrief>> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move { e.volumes_brief().await })
        .await?)
}

// ------------------------------------------------------------------ flujos

/// Final de un flujo: cómo terminó o por qué falló.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StreamClosed {
    pub end: Option<StreamEnd>,
    pub error: Option<CommandError>,
}

impl From<Result<StreamEnd, EngineError>> for StreamClosed {
    fn from(r: Result<StreamEnd, EngineError>) -> Self {
        match r {
            Ok(end) => Self {
                end: Some(end),
                error: None,
            },
            Err(e) => Self {
                end: None,
                error: Some(e.into()),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(
    tag = "event",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum LogEvent {
    Lines { lines: Vec<LogLine> },
    End { closed: StreamClosed },
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(
    tag = "event",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum StatsEvent {
    Sample { sample: StatsSample },
    End { closed: StreamClosed },
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(
    tag = "event",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum TerminalEvent {
    /// Bytes de salida en base64 (la terminal decodifica UTF-8 por su cuenta).
    Output {
        data: String,
    },
    End {
        closed: StreamClosed,
    },
}

#[tauri::command]
pub async fn logs_open(
    state: State<'_, AppState>,
    server: String,
    id: String,
    name: Option<String>,
    tail: Option<u32>,
    on_event: Channel<LogEvent>,
) -> CmdResult<u64> {
    let mgr = state.server(&server)?;
    let engine = mgr.engine().await?;
    let registry = state.streams.clone();
    let manager = mgr.clone();
    let (sid, cancel) = registry.register(&server, None);
    tauri::async_runtime::spawn(async move {
        let ch = on_event.clone();
        let r = engine
            .stream_logs(&id, name.as_deref(), tail, cancel, move |lines| {
                ch.send(LogEvent::Lines { lines }).is_ok()
            })
            .await;
        if matches!(&r, Err(e) if e.is_connection_error()) {
            manager.report_connection_error();
        }
        let _ = on_event.send(LogEvent::End { closed: r.into() });
        registry.remove(sid);
    });
    Ok(sid)
}

#[tauri::command]
pub async fn stats_open(
    state: State<'_, AppState>,
    server: String,
    id: String,
    name: Option<String>,
    on_event: Channel<StatsEvent>,
) -> CmdResult<u64> {
    let mgr = state.server(&server)?;
    let engine = mgr.engine().await?;
    let registry = state.streams.clone();
    let manager = mgr.clone();
    let (sid, cancel) = registry.register(&server, None);
    tauri::async_runtime::spawn(async move {
        let ch = on_event.clone();
        let r = engine
            .stream_stats(&id, name.as_deref(), cancel, move |sample| {
                ch.send(StatsEvent::Sample { sample }).is_ok()
            })
            .await;
        if matches!(&r, Err(e) if e.is_connection_error()) {
            manager.report_connection_error();
        }
        let _ = on_event.send(StatsEvent::End { closed: r.into() });
        registry.remove(sid);
    });
    Ok(sid)
}

fn clamp_size(cols: u16, rows: u16) -> (u16, u16) {
    (cols.clamp(10, 1000), rows.clamp(3, 500))
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn terminal_open(
    state: State<'_, AppState>,
    server: String,
    id: String,
    name: Option<String>,
    shell: ShellChoice,
    cols: u16,
    rows: u16,
    on_event: Channel<TerminalEvent>,
) -> CmdResult<u64> {
    let mgr = state.server(&server)?;
    let engine = mgr.engine().await?;
    let registry = state.streams.clone();
    let manager = mgr.clone();
    let (tx, rx) = mpsc::channel(256);
    let (sid, cancel) = registry.register(&server, Some(tx));
    tauri::async_runtime::spawn(async move {
        let ch = on_event.clone();
        let b64 = base64::engine::general_purpose::STANDARD;
        let r = engine
            .run_terminal(
                &id,
                name.as_deref(),
                shell.argv(),
                clamp_size(cols, rows),
                cancel,
                rx,
                move |bytes| {
                    ch.send(TerminalEvent::Output {
                        data: b64.encode(&bytes),
                    })
                    .is_ok()
                },
            )
            .await;
        if matches!(&r, Err(e) if e.is_connection_error()) {
            manager.report_connection_error();
        }
        let _ = on_event.send(TerminalEvent::End { closed: r.into() });
        registry.remove(sid);
    });
    Ok(sid)
}

/// Texto tecleado en la terminal (cadena UTF-8 tal como la entrega xterm.js).
#[tauri::command]
pub async fn terminal_input(
    state: State<'_, AppState>,
    stream: u64,
    data: String,
) -> CmdResult<()> {
    let tx = state.streams.input(stream).ok_or_else(|| {
        CommandError::new("stream.closed", "la sesión de terminal ya está cerrada")
    })?;
    tx.send(TerminalInput::Data(data.into_bytes()))
        .await
        .map_err(|_| CommandError::new("stream.closed", "la sesión de terminal ya está cerrada"))
}

#[tauri::command]
pub async fn terminal_resize(
    state: State<'_, AppState>,
    stream: u64,
    cols: u16,
    rows: u16,
) -> CmdResult<()> {
    let (cols, rows) = clamp_size(cols, rows);
    let Some(tx) = state.streams.input(stream) else {
        return Ok(());
    };
    let _ = tx.send(TerminalInput::Resize { cols, rows }).await;
    Ok(())
}

/// Cierra un flujo. Devuelve `false` si ya había terminado.
#[tauri::command]
pub fn stream_close(state: State<'_, AppState>, stream: u64) -> bool {
    state.streams.close(stream)
}

// ------------------------------------------------------------------ 0.9.2

/// Configuración editable de un contenedor (pantalla «Editar configuración»).
#[tauri::command]
pub async fn container_edit_config(
    state: State<'_, AppState>,
    server: String,
    id: String,
) -> CmdResult<EditableConfig> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move { e.edit_config(&id).await })
        .await?)
}

/// Comparación y clasificación de los cambios. No modifica nada.
#[tauri::command]
pub async fn container_edit_plan(
    state: State<'_, AppState>,
    server: String,
    id: String,
    request: EditRequest,
) -> CmdResult<EditPlan> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move { e.edit_plan(&id, &request).await })
        .await?)
}

/// Aplica los cambios: directos o con recreación segura (con confirmación).
#[tauri::command]
pub async fn container_edit_apply(
    state: State<'_, AppState>,
    server: String,
    id: String,
    request: EditRequest,
    options: ApplyOptions,
) -> CmdResult<EditOutcome> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move { e.edit_apply(&id, &request, &options).await })
        .await?)
}

/// Vuelve a poner en servicio la copia que dejó una recreación.
#[tauri::command]
pub async fn container_restore_backup(
    state: State<'_, AppState>,
    server: String,
    id: String,
    start: bool,
    confirmed: bool,
) -> CmdResult<EditOutcome> {
    let mgr = state.server(&server)?;
    if !confirmed {
        return Err(EngineError::NotConfirmed.into());
    }
    Ok(mgr
        .with_engine(|e| async move { e.restore_backup(&id, start, confirmed).await })
        .await?)
}

/// Comprobaciones previas a «Crear imagen a partir de este contenedor».
#[tauri::command]
pub async fn container_commit_preview(
    state: State<'_, AppState>,
    server: String,
    id: String,
    request: CommitRequest,
) -> CmdResult<CommitPreview> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move { e.commit_preview(&id, &request).await })
        .await?)
}

/// `docker commit` mediante la API.
#[tauri::command]
pub async fn container_commit(
    state: State<'_, AppState>,
    server: String,
    id: String,
    request: CommitRequest,
) -> CmdResult<CommitResult> {
    let mgr = state.server(&server)?;
    Ok(mgr
        .with_engine(|e| async move { e.commit_image(&id, &request).await })
        .await?)
}
