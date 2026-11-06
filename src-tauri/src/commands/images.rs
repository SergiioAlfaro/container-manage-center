//! Fase 4: imágenes. Inspección, capas, etiquetado, eliminación, limpieza,
//! búsqueda, descarga (flujo) y construcción (flujo).

use std::path::PathBuf;

use dcc_engine::build::{BuildEnd, BuildSpec, BuildUpdate, ContextSummary};
use dcc_engine::images::{ImageDetails, ImageLayer, PruneResult, RemoveResult, SearchResult};
use dcc_engine::pull::PullUpdate;
use dcc_engine::reference::ImageRef;
use dcc_engine::registry_auth::PullAuth;
use dcc_engine::spec::{Severity, SpecIssue};
use dcc_engine::EngineError;
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;
use ts_rs::TS;

use super::containers::StreamClosed;
use crate::error::{CmdResult, CommandError};
use crate::state::AppState;

#[tauri::command]
pub async fn image_details(
    state: State<'_, AppState>,
    reference: String,
) -> CmdResult<ImageDetails> {
    Ok(state
        .engine
        .with_engine(|e| async move { e.image_details(&reference).await })
        .await?)
}

#[tauri::command]
pub async fn image_history(
    state: State<'_, AppState>,
    reference: String,
) -> CmdResult<Vec<ImageLayer>> {
    Ok(state
        .engine
        .with_engine(|e| async move { e.image_history(&reference).await })
        .await?)
}

#[tauri::command]
pub async fn image_tag(
    state: State<'_, AppState>,
    source: String,
    repo: String,
    tag: String,
) -> CmdResult<String> {
    Ok(state
        .engine
        .with_engine(|e| async move { e.tag_image(&source, &repo, &tag).await })
        .await?)
}

#[tauri::command]
pub async fn image_remove(
    state: State<'_, AppState>,
    reference: String,
    force: bool,
    confirmed: bool,
) -> CmdResult<RemoveResult> {
    if !confirmed {
        return Err(EngineError::NotConfirmed.into());
    }
    Ok(state
        .engine
        .with_engine(|e| async move { e.remove_image(&reference, force, true).await })
        .await?)
}

#[tauri::command]
pub async fn images_prune(
    state: State<'_, AppState>,
    all: bool,
    confirmed: bool,
) -> CmdResult<PruneResult> {
    if !confirmed {
        return Err(EngineError::NotConfirmed.into());
    }
    Ok(state
        .engine
        .with_engine(|e| async move { e.prune_images(all, true).await })
        .await?)
}

#[tauri::command]
pub async fn images_search(
    state: State<'_, AppState>,
    term: String,
    limit: u32,
    official_only: bool,
) -> CmdResult<Vec<SearchResult>> {
    Ok(state
        .engine
        .with_engine(|e| async move { e.search_images(&term, limit, official_only).await })
        .await?)
}

/// Interpreta una referencia como lo hace Docker (sin hablar con Docker).
#[tauri::command]
pub fn image_ref_parse(reference: String) -> CmdResult<ImageRef> {
    ImageRef::parse(&reference).map_err(|e| {
        let code = serde_json::to_value(e)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        CommandError::new(format!("ref.{code}"), e)
    })
}

// ------------------------------------------------------------------ descarga

#[derive(Debug, Clone, Serialize, TS)]
#[serde(
    tag = "event",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum PullEvent {
    Progress { update: PullUpdate },
    End { closed: StreamClosed },
}

#[tauri::command]
pub async fn image_pull_open(
    state: State<'_, AppState>,
    reference: String,
    auth: PullAuth,
    on_event: Channel<PullEvent>,
) -> CmdResult<u64> {
    // Errores de formato antes de abrir nada.
    image_ref_parse(reference.clone())?;
    let engine = state.engine.engine().await?;
    let registry = state.streams.clone();
    let manager = state.engine.clone();
    let (sid, cancel) = registry.register(None);
    tauri::async_runtime::spawn(async move {
        let ch = on_event.clone();
        let r = engine
            .pull_image(&reference, &auth, cancel, move |update| {
                ch.send(PullEvent::Progress { update }).is_ok()
            })
            .await;
        // La contraseña escrita para esta descarga deja de existir aquí.
        drop(auth);
        if matches!(&r, Err(e) if e.is_connection_error()) {
            manager.report_connection_error();
        }
        let _ = on_event.send(PullEvent::End { closed: r.into() });
        registry.remove(sid);
    });
    Ok(sid)
}

// ------------------------------------------------------------------ construcción

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BuildPreview {
    pub issues: Vec<SpecIssue>,
    /// `docker build …` equivalente (secretos ocultos).
    pub command: String,
    /// Petición `POST /build?…` (secretos ocultos).
    pub request: String,
    pub secrets_hidden: u32,
    /// Resumen del contexto si la carpeta y el Dockerfile son válidos.
    pub context: Option<ContextSummary>,
    pub context_error: Option<String>,
}

/// Valida la construcción y resume el contexto. Solo lee la carpeta: no habla con Docker.
#[tauri::command]
pub async fn build_preview(spec: BuildSpec) -> CmdResult<BuildPreview> {
    tauri::async_runtime::spawn_blocking(move || {
        let issues = spec.validate(true);
        let (shown, hidden) = spec.redacted();
        let mut command = dcc_activity::quote::render_argv("docker", &shown.cli_args());
        if spec.builder == dcc_engine::build::Builder::Classic {
            command = format!("DOCKER_BUILDKIT=0 {command}");
        }
        let ctx_ok = !issues.iter().any(|i| {
            i.severity == Severity::Error && (i.field == "context" || i.field == "dockerfile")
        });
        let (context, context_error) = if ctx_ok && !spec.context.is_empty() {
            match dcc_engine::build::context::scan(&PathBuf::from(&spec.context), &spec.dockerfile)
            {
                Ok(s) => (Some(s), None),
                Err(e) => (None, Some(e.to_string())),
            }
        } else {
            (None, None)
        };
        BuildPreview {
            issues,
            command,
            request: format!("POST {}", shown.path()),
            secrets_hidden: hidden as u32,
            context,
            context_error,
        }
    })
    .await
    .map_err(|e| CommandError::new("engine.other", e))
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BuildClosed {
    pub result: Option<BuildEnd>,
    pub error: Option<CommandError>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(
    tag = "event",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum BuildEvent {
    Progress { update: BuildUpdate },
    End { closed: BuildClosed },
}

#[tauri::command]
pub async fn build_open(
    state: State<'_, AppState>,
    spec: BuildSpec,
    on_event: Channel<BuildEvent>,
) -> CmdResult<u64> {
    let issues = spec.validate(true);
    if issues.iter().any(|i| i.severity == Severity::Error) {
        return Err(EngineError::InvalidSpec { issues }.into());
    }
    let engine = state.engine.engine().await?;
    let registry = state.streams.clone();
    let manager = state.engine.clone();
    let (sid, cancel) = registry.register(None);
    tauri::async_runtime::spawn(async move {
        let ch = on_event.clone();
        let r = engine
            .build_image(&spec, cancel, move |update| {
                ch.send(BuildEvent::Progress { update }).is_ok()
            })
            .await;
        if matches!(&r, Err(e) if e.is_connection_error()) {
            manager.report_connection_error();
        }
        let closed = match r {
            Ok(result) => BuildClosed {
                result: Some(result),
                error: None,
            },
            Err(e) => BuildClosed {
                result: None,
                error: Some(e.into()),
            },
        };
        let _ = on_event.send(BuildEvent::End { closed });
        registry.remove(sid);
    });
    Ok(sid)
}
