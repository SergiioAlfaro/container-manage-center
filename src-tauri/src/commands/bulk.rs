// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Gestión masiva (1.10.0, D-117 a D-120): inventario de varios servidores y
//! ejecución de operaciones masivas con progreso.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use cmc_engine::bulk::BulkContainer;
use cmc_engine::{ConnectionState, EngineManager};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, Runtime, State};
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::bulk::{self, BulkEnvironment, BulkEvent, BulkInventory, BulkRequest, BulkSummary};
use crate::error::CmdResult;
use crate::state::AppState;

/// Consultas de `inspect` a la vez por servidor al llenar la tabla.
const DETAILS_CONCURRENCY: usize = 8;

/// Contenedores de todos los servidores **conectados** (el equipo local y los
/// remotos). Los servidores sin conexión aparecen sin contenedores: no se
/// conecta a ninguno por su cuenta (puede pedir secretos).
///
/// Las lecturas no se registran en la actividad (se repiten con la
/// actualización automática, como la monitorización, D-102). Con
/// `refresh = true` se vuelven a pedir los datos de `inspect` de todos.
#[tauri::command]
pub async fn bulk_inventory(state: State<'_, AppState>, refresh: bool) -> CmdResult<BulkInventory> {
    let views = state.servers.list();
    let mut set = tokio::task::JoinSet::new();
    for (pos, v) in views.into_iter().enumerate() {
        let mgr = state.server(&v.id)?;
        let connected = matches!(v.state, ConnectionState::Connected { .. });
        let cache = state.bulk_details.clone();
        set.spawn(async move {
            let mut env = BulkEnvironment {
                server: v.id.clone(),
                remote: v.remote,
                name: v.name,
                address: v.address,
                connection: bulk::connection_label(&v.state).into(),
                containers: Vec::new(),
                error: None,
            };
            if connected {
                match list_server(&mgr, &v.id, &cache, refresh).await {
                    Ok(list) => env.containers = list,
                    Err(e) => env.error = Some(e),
                }
            }
            (pos, env)
        });
    }
    let mut envs = Vec::new();
    while let Some(r) = set.join_next().await {
        match r {
            Ok(x) => envs.push(x),
            Err(e) => tracing::error!("inventario de la gestión masiva: {e}"),
        }
    }
    envs.sort_by_key(|(pos, _)| *pos);
    Ok(BulkInventory {
        fetched_at: chrono::Utc::now(),
        environments: envs.into_iter().map(|(_, e)| e).collect(),
    })
}

async fn list_server(
    mgr: &EngineManager,
    server: &str,
    cache: &Arc<bulk::DetailsCache>,
    refresh: bool,
) -> CmdResult<Vec<BulkContainer>> {
    let engine = mgr.engine().await?;
    let list = match engine.list_containers_silent().await {
        Ok(l) => l,
        Err(e) => {
            if e.is_connection_error() {
                mgr.report_connection_error();
            }
            return Err(e.into());
        }
    };
    let ids: HashSet<&str> = list.iter().map(|c| c.id.as_str()).collect();
    cache.retain(server, &ids);
    let missing: Vec<String> = list
        .iter()
        .filter(|c| refresh || cache.get(server, &c.id).is_none())
        .map(|c| c.id.clone())
        .collect();
    let sem = Arc::new(Semaphore::new(DETAILS_CONCURRENCY));
    let mut set = tokio::task::JoinSet::new();
    for id in missing {
        let (engine, sem) = (engine.clone(), sem.clone());
        set.spawn(async move {
            let _p = sem.acquire_owned().await;
            let d = engine.bulk_details_silent(&id).await;
            (id, d)
        });
    }
    while let Some(Ok((id, d))) = set.join_next().await {
        match d {
            Ok(d) => cache.set(server, &id, d),
            // Puede haberse eliminado entre el listado y la consulta: sin datos.
            Err(_) => cache.remove(server, &id),
        }
    }
    Ok(list
        .into_iter()
        .map(|c| {
            let d = cache.get(server, &c.id);
            BulkContainer::from_summary(c, d)
        })
        .collect())
}

/// Ejecuta una operación masiva. Devuelve el identificador del flujo: se
/// cancela lo pendiente con `stream_close`. Los errores de validación y de
/// reserva se devuelven antes de empezar nada.
#[tauri::command]
pub async fn bulk_run_open<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    request: BulkRequest,
    on_event: Channel<BulkEvent>,
) -> CmdResult<u64> {
    bulk::validate(&request, |s| state.server(s).is_ok())?;
    let reservation = state.bulk_jobs.reserve(&request.targets)?;
    let concurrency = state.config().bulk.concurrency;
    let (sid, cancel) = state.streams.register("bulk", None);
    let batch = Uuid::new_v4();
    let handle = bulk::begin_summary(&state.activity, batch, &request, concurrency);
    let started_at = chrono::Utc::now();
    let started = Instant::now();
    let _ = on_event.send(BulkEvent::Started {
        batch,
        total: request.targets.len() as u32,
        concurrency,
    });
    let registry = state.streams.clone();
    let op = request.operation.clone();
    tauri::async_runtime::spawn(async move {
        let _reservation = reservation;
        let ch = on_event.clone();
        let app2 = app.clone();
        let op2 = op.clone();
        let results = bulk::run_items(
            request.targets,
            concurrency,
            cancel,
            move |index, target| {
                let app = app2.clone();
                let op = op2.clone();
                async move {
                    let state = app.state::<AppState>();
                    let mgr = match state.server(&target.server) {
                        Ok(m) => m,
                        Err(e) => return bulk::not_sent(index, target, e),
                    };
                    let engine = match mgr.engine().await {
                        Ok(e) => e,
                        Err(e) => return bulk::not_sent(index, target, e.into()),
                    };
                    let tagged = engine.with_activity(engine.activity().with_batch(batch));
                    let t0 = Instant::now();
                    let r = tagged.bulk_apply(&op, &target.id, &target.name).await;
                    if let Err(e) = &r {
                        if e.error.is_connection_error() {
                            mgr.report_connection_error();
                        }
                    }
                    let result = bulk::item_result(index, target, t0, r);
                    // La tabla muestra enseguida la política nueva o la eliminación.
                    let t = &result.target;
                    match (&result.details, &result.state, result.status) {
                        (Some(d), _, _) => state.bulk_details.set(&t.server, &t.id, *d),
                        (None, None, bulk::BulkItemStatus::Succeeded) => {
                            state.bulk_details.remove(&t.server, &t.id)
                        }
                        _ => {}
                    }
                    result
                }
            },
            move |ev| {
                let _ = ch.send(ev);
            },
        )
        .await;
        let counts = bulk::BulkCounts::of(&results);
        bulk::finish_summary(handle, &counts);
        let summary = BulkSummary {
            batch,
            operation: op,
            total: results.len() as u32,
            counts,
            started_at,
            finished_at: chrono::Utc::now(),
            duration_ms: started.elapsed().as_millis() as u64,
            results,
        };
        let _ = on_event.send(BulkEvent::Finished {
            summary: Box::new(summary),
        });
        registry.remove(sid);
    });
    Ok(sid)
}
