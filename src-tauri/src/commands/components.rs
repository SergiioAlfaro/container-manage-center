//! Configuración → Desinstalación y componentes (0.9.1, D-065).

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::State;

use crate::error::{CmdResult, CommandError};
use crate::state::AppState;
use crate::uninstall::{self, ComponentsReport, Host, RemovalPlan, RemovalResult};

/// Solo una desinstalación a la vez.
static REMOVING: AtomicBool = AtomicBool::new(false);

/// Analiza lo instalado. Solo consulta y simula (sin privilegios).
#[tauri::command]
pub async fn components_inspect(state: State<'_, AppState>) -> CmdResult<ComponentsReport> {
    uninstall::inspect(&Host::system(), &state.activity, state.store.path()).await
}

/// Comprueba una selección y devuelve la orden exacta y la simulación de APT.
#[tauri::command]
pub async fn components_plan(
    state: State<'_, AppState>,
    packages: Vec<String>,
) -> CmdResult<RemovalPlan> {
    uninstall::plan(
        &Host::system(),
        &state.activity,
        state.store.path(),
        &packages,
    )
    .await
}

/// Desinstala la selección. Exige la confirmación explícita del usuario.
#[tauri::command]
pub async fn components_remove(
    state: State<'_, AppState>,
    packages: Vec<String>,
    confirmed: bool,
) -> CmdResult<RemovalResult> {
    if !confirmed {
        return Err(CommandError::from(dcc_engine::EngineError::NotConfirmed));
    }
    if REMOVING.swap(true, Ordering::SeqCst) {
        return Err(CommandError::new(
            "packages.busy",
            "Ya hay una desinstalación en curso.",
        ));
    }
    let r = uninstall::remove(
        &Host::system(),
        &state.activity,
        state.store.path(),
        &packages,
    )
    .await;
    REMOVING.store(false, Ordering::SeqCst);
    r
}
