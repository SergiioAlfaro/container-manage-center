// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Paso de los datos del nombre anterior (*Docker Control Center*, hasta la
//! 1.7.0) al nuevo (1.8.0, D-113).
//!
//! 1. **Carpetas** (configuración, estado, datos y caché de WebKit): al
//!    arrancar, antes de abrir ningún archivo ([`cmc_store::migrate`]).
//! 2. **Tareas programadas**: unidades de systemd `dcc-backup-*` →
//!    `cmc-backup-*`, conservando si estaban activadas ([`crate::systemd`]).
//! 3. **Contraseñas** del keyring: las desbloqueadas al arrancar; las demás,
//!    al usarlas ([`crate::secrets`]).
//! 4. **Carpeta de copias** de los destinos conocidos (el de la configuración
//!    y los de las tareas) al arrancar, y de cualquier otro al usarlo
//!    ([`cmc_engine::backup::store::migrate_legacy_root`]). Un destino que no
//!    está montado (un disco externo) se pasa cuando se vuelva a usar.
//!
//! Cada paso es repetible: sin datos antiguos no hace nada. Si algo falla, lo
//! antiguo se conserva y se avisa al arrancar; nunca se borra nada sin
//! haberlo pasado antes al nombre nuevo.
//!
//! `container-manage-center --migrate` hace los pasos 1, 2 y 4 sin abrir la
//! ventana (lo usa `install.sh` después de instalar, con el usuario normal).

use std::sync::OnceLock;

/// Identificador de Tauri de esta versión (igual que `tauri.conf.json`).
pub const IDENTIFIER: &str = "org.containermanagecenter.app";

static WARNING: OnceLock<Option<String>> = OnceLock::new();

/// Aviso para mostrar al arrancar si la migración de carpetas no fue completa.
pub fn startup_warning() -> Option<String> {
    WARNING.get().cloned().flatten()
}

fn describe(report: &cmc_store::migrate::MigrationReport) -> Option<String> {
    let mut parts = Vec::new();
    for (p, e) in &report.errors {
        parts.push(format!(
            "No se pudo pasar {} al nombre nuevo ({e}); se conserva sin cambios.",
            p.display()
        ));
    }
    for (p, files) in &report.kept {
        if !files.is_empty() {
            parts.push(format!(
                "En {} quedan {} archivo(s) que ya existían con el nombre nuevo: se conservan sin cambios.",
                p.display(),
                files.len()
            ));
        }
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// Paso 1. Se llama una vez, al principio de cada modo de arranque.
pub fn migrate_dirs() {
    let report = cmc_store::migrate::migrate_default_dirs(IDENTIFIER);
    cmc_engine::ssh::remove_legacy_runtime_dir();
    for (from, to) in &report.moved {
        tracing::info!(from = %from.display(), to = %to.display(), "datos pasados al nombre nuevo");
    }
    let warning = describe(&report);
    if let Some(w) = &warning {
        tracing::warn!("{w}");
    }
    let _ = WARNING.set(warning);
}

/// Paso 4 para los destinos conocidos. Devuelve los problemas.
fn migrate_backup_roots(config: &std::path::Path) -> Vec<String> {
    use cmc_engine::backup::store::{migrate_legacy_root, ROOT_DIR};
    let mut dests: std::collections::BTreeSet<String> = crate::backups::load_jobs(config)
        .into_iter()
        .map(|j| j.destination)
        .collect();
    dests.extend(crate::backups::default_destination(config));
    let mut problems = Vec::new();
    for d in dests {
        let d = std::path::PathBuf::from(d);
        if !d.is_absolute() || !d.is_dir() {
            continue;
        }
        if let Err(e) = migrate_legacy_root(
            &d.join(cmc_engine::legacy::BACKUP_ROOT_DIR),
            &d.join(ROOT_DIR),
        ) {
            problems.push(format!("copias de {}: {e}", d.display()));
        }
    }
    problems
}

/// Pasos 2, 3 y 4, en segundo plano tras abrir la ventana. Lo que hacen queda
/// en el panel de actividad.
pub async fn migrate_units_and_secrets(state: &crate::state::AppState) {
    for p in migrate_backup_roots(state.store.path()) {
        tracing::warn!("carpeta de copias sin pasar al nombre nuevo: {p}");
    }
    if let Some(sd) = crate::systemd::Systemd::from_env() {
        let jobs = crate::backups::load_jobs(state.store.path());
        if let Ok(exe) = std::env::current_exe() {
            for p in sd.migrate_legacy(&state.activity, &jobs, &exe).await {
                tracing::warn!("tarea programada sin pasar al nombre nuevo: {p}");
            }
        }
    }
    match state.servers.secrets.migrate_legacy().await {
        Ok(0) => {}
        Ok(n) => tracing::info!(n, "contraseñas del keyring pasadas al nombre nuevo"),
        Err(e) => tracing::info!(error = %e, "contraseñas del keyring: se pasarán al usarlas"),
    }
}

/// `container-manage-center --migrate`: pasos 1, 2 y 4, sin ventana. `None` si
/// no se pidió. Código de salida 0 si no queda nada pendiente.
pub fn cli_main() -> Option<i32> {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("--migrate") {
        return None;
    }
    crate::refuse_root();
    migrate_dirs();
    let mut code = 0;
    if let Some(w) = startup_warning() {
        eprintln!("Container Manage Center: {w}");
        code = 1;
    }
    let Ok(store) = cmc_store::ConfigStore::default_location() else {
        return Some(code);
    };
    for p in migrate_backup_roots(store.path()) {
        eprintln!("Container Manage Center: carpeta de copias sin pasar al nombre nuevo: {p}");
        code = 1;
    }
    if let (Some(sd), Ok(exe)) = (crate::systemd::Systemd::from_env(), std::env::current_exe()) {
        let jobs = crate::backups::load_jobs(store.path());
        let log = cmc_activity::ActivityLog::new(1000);
        let problems = tauri::async_runtime::block_on(sd.migrate_legacy(&log, &jobs, &exe));
        for p in &problems {
            eprintln!("Container Manage Center: tarea programada sin pasar al nombre nuevo: {p}");
        }
        if !problems.is_empty() {
            code = 1;
        }
    }
    Some(code)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_identifier_is_the_one_in_tauri_conf() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["identifier"], super::IDENTIFIER);
    }
}
