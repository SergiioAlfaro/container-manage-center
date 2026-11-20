// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Comandos masivos (1.11.0, D-121 a D-124): vista previa, ejecución con
//! progreso, interrupción de lo activo y comandos guardados.

use std::time::Instant;

use cmc_engine::bulk_exec::BulkCommand;
use cmc_store::{AppConfig, ExecTemplate};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, Runtime, State};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::bulk::BulkTarget;
use crate::bulk_exec::{
    self, BulkExecRequest, ExecCounts, ExecEvent, ExecItemResult, ExecPreview, ExecSummary,
};
use crate::error::{CmdResult, CommandError};
use crate::state::AppState;

/// Argumentos exactos, comando equivalente y riesgo de un comando. No ejecuta
/// nada ni se registra.
#[tauri::command]
pub fn bulk_exec_preview(command: BulkCommand) -> CmdResult<ExecPreview> {
    bulk_exec::preview(&command)
}

/// Ejecuta un comando en varios contenedores. Devuelve el identificador del
/// flujo: `stream_close` cancela lo pendiente y `bulk_exec_interrupt` deja de
/// esperar a lo activo. Los errores de validación y de reserva se devuelven
/// antes de empezar nada.
#[tauri::command]
pub async fn bulk_exec_open<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    request: BulkExecRequest,
    on_event: Channel<ExecEvent>,
) -> CmdResult<u64> {
    bulk_exec::validate(&request, |s| state.server(s).is_ok())?;
    let reservation = state.bulk_jobs.reserve(&request.targets)?;
    let (sid, cancel) = state.streams.register("bulkExec", None);
    let interrupt = CancellationToken::new();
    state.exec_interrupts.insert(sid, interrupt.clone());
    let batch = Uuid::new_v4();
    let handle = bulk_exec::begin_summary(&state.activity, batch, &request);
    let started_at = chrono::Utc::now();
    let started = Instant::now();
    let _ = on_event.send(ExecEvent::Started {
        batch,
        total: request.targets.len() as u32,
        concurrency: request.concurrency,
    });
    let registry = state.streams.clone();
    let interrupts = state.exec_interrupts.clone();
    let command = request.command.clone();
    tauri::async_runtime::spawn(async move {
        let _reservation = reservation;
        let (ch_run, ch_item) = (on_event.clone(), on_event.clone());
        let app2 = app.clone();
        let cmd2 = command.clone();
        let int2 = interrupt.clone();
        let results = crate::bulk::run_pool(
            request.targets,
            request.concurrency,
            cancel,
            move |index, target: BulkTarget| {
                let app = app2.clone();
                let cmd = cmd2.clone();
                let interrupt = int2.clone();
                async move {
                    // Interrumpir también impide lanzar lo que aún no empezó.
                    if interrupt.is_cancelled() {
                        return bulk_exec::cancelled(index, target);
                    }
                    let state = app.state::<AppState>();
                    let mgr = match state.server(&target.server) {
                        Ok(m) => m,
                        Err(e) => return bulk_exec::not_sent(index, target, e),
                    };
                    let engine = match mgr.engine().await {
                        Ok(e) => e,
                        Err(e) => return bulk_exec::not_sent(index, target, e.into()),
                    };
                    let tagged = engine.with_activity(engine.activity().with_batch(batch));
                    let t0 = Instant::now();
                    let r = tagged
                        .bulk_exec(&cmd, &target.id, &target.name, interrupt)
                        .await;
                    if let Err(e) = &r {
                        if e.error.is_connection_error() {
                            mgr.report_connection_error();
                        }
                    }
                    bulk_exec::item_result(index, target, t0, r)
                }
            },
            move |index| {
                let _ = ch_run.send(ExecEvent::Running { index });
            },
            move |r: &ExecItemResult| {
                let _ = ch_item.send(ExecEvent::Item {
                    result: Box::new(r.clone()),
                });
            },
        )
        .await;
        let counts = ExecCounts::of(&results);
        bulk_exec::finish_summary(handle, &counts);
        let summary = ExecSummary {
            batch,
            command,
            total: results.len() as u32,
            counts,
            started_at,
            finished_at: chrono::Utc::now(),
            duration_ms: started.elapsed().as_millis() as u64,
            results,
        };
        let _ = on_event.send(ExecEvent::Finished {
            summary: Box::new(summary),
        });
        interrupts.remove(sid);
        registry.remove(sid);
    });
    Ok(sid)
}

/// Deja de esperar a las ejecuciones activas de un comando masivo y no lanza
/// las pendientes. Docker Exec no permite detener un proceso ya lanzado: puede
/// seguir dentro del contenedor (cada resultado lo indica). Nunca se mata ni se
/// reinicia el contenedor.
#[tauri::command]
pub fn bulk_exec_interrupt(state: State<'_, AppState>, sid: u64) -> CmdResult<bool> {
    let found = state.exec_interrupts.interrupt(sid);
    if found {
        // Lo pendiente tampoco empieza.
        state.streams.close(sid);
    }
    Ok(found)
}

/// Sustituye la lista de comandos guardados. Se rechazan los que parecen
/// contener secretos. Devuelve la configuración guardada.
#[tauri::command]
pub async fn exec_templates_save<R: Runtime>(
    app: AppHandle<R>,
    templates: Vec<ExecTemplate>,
) -> CmdResult<AppConfig> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<AppState>().replace_exec_templates(templates)
    })
    .await
    .map_err(|e| CommandError::new("internal", e))?
}
