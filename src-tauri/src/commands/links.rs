// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! E6 (1.6.0): enlaces entre herramientas (D-108).

use std::time::Duration;

use dcc_engine::backup::store::{BackupTarget, LocalTarget};
use tauri::State;

use crate::backups;
use crate::error::CmdResult;
use crate::links::{self, LinkTarget, ToolLinks, VolumesOf};
use crate::state::AppState;

/// Como mucho tantos contenedores por petición (un informe de auditoría).
const MAX_TARGETS: usize = 2000;
/// La lista de contenedores para saber sus volúmenes no debe retrasar la vista.
const LIST_TIMEOUT: Duration = Duration::from_secs(5);

/// Para cada contenedor (por id): si la monitorización lo vigila y qué copia
/// y qué tareas lo protegen. Solo lee; no se registra en el panel.
#[tauri::command]
pub async fn tools_links(
    state: State<'_, AppState>,
    server: String,
    targets: Vec<LinkTarget>,
) -> CmdResult<Vec<ToolLinks>> {
    let mgr = state.server(&server)?;
    let targets: Vec<LinkTarget> = targets.into_iter().take(MAX_TARGETS).collect();
    let status = state.monitor.status(&server);
    let jobs = backups::load_jobs(state.store.path());
    // Carpetas donde puede haber copias de este servidor: la elegida y las de
    // sus tareas.
    let mut dests: Vec<String> = backups::default_destination(state.store.path())
        .into_iter()
        .chain(
            jobs.iter()
                .filter(|j| j.server == server)
                .map(|j| j.destination.clone()),
        )
        .collect();
    dests.sort();
    dests.dedup();
    let srv = server.clone();
    let lists = tokio::task::spawn_blocking(move || {
        dests
            .into_iter()
            .filter_map(|d| {
                let t = LocalTarget::new(std::path::Path::new(&d)).ok()?;
                // Una carpeta que no está (disco externo sin montar) no cuenta.
                t.list(&srv).ok().map(|l| (d, l))
            })
            .collect::<Vec<_>>()
    })
    .await
    .unwrap_or_default();
    let backups = links::complete_backups(lists);
    // Volúmenes de cada contenedor, solo si hay conexión y sin esperar mucho.
    let mut vols = VolumesOf::new();
    if let Ok(engine) = mgr.engine().await {
        if let Ok(Ok(list)) =
            tokio::time::timeout(LIST_TIMEOUT, engine.list_containers_silent()).await
        {
            for c in list {
                let v: Vec<String> = c
                    .mount_points
                    .iter()
                    .filter(|m| m.kind == "volume")
                    .filter_map(|m| m.name.clone())
                    .collect();
                vols.insert(c.id, v);
            }
        }
    }
    Ok(links::compute(
        &server, &targets, &status, &backups, &jobs, &vols,
    ))
}
