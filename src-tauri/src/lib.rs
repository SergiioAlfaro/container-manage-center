//! Punto de entrada de la aplicación de escritorio.

mod commands;
mod error;
mod events;
mod journal;
mod state;
mod streams;
mod uninstall;
mod updates;

use dcc_store::ConfigStore;
use tauri::{Builder, Manager, Runtime};

pub use error::CommandError;

/// Registra plugins, estado, puente de eventos y comandos sobre un `Builder`.
/// Se separa de `run` para poder montar la aplicación en pruebas con el
/// runtime simulado de Tauri.
pub fn configure<R: Runtime>(builder: Builder<R>, store: ConfigStore) -> Builder<R> {
    let state = state::AppState::new(store);
    let activity = state.activity.clone();
    let engine = state.engine.clone();
    builder
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .on_window_event(|window, event| {
            let Some(state) = window.try_state::<state::AppState>() else {
                return;
            };
            match event {
                // Al cerrar la ventana se cierran los flujos abiertos (logs,
                // terminales…) y se espera a que terminen, para que las shells
                // salgan y no quede ningún proceso dentro de los contenedores
                // (0.9.1, D-063). Quedan registrados como cerrados por el usuario.
                tauri::WindowEvent::CloseRequested { api, .. }
                    if state.streams.running() > 0
                        && !state
                            .closing
                            .swap(true, std::sync::atomic::Ordering::SeqCst) =>
                {
                    api.prevent_close();
                    let streams = state.streams.clone();
                    let w = window.clone();
                    tauri::async_runtime::spawn(async move {
                        let left = streams
                            .close_all_and_wait(std::time::Duration::from_secs(8))
                            .await;
                        if left > 0 {
                            tracing::warn!(left, "flujos sin terminar al cerrar la ventana");
                        }
                        let _ = w.close();
                    });
                }
                tauri::WindowEvent::Destroyed => state.streams.close_all(),
                _ => {}
            }
        })
        .setup(move |app| {
            events::spawn_activity_bridge(app.handle().clone(), activity.clone());
            journal::spawn_history_bridge(app.handle().clone(), activity);
            journal::spawn_events_watcher(app.handle().clone(), engine.clone());
            events::spawn_engine(app.handle().clone(), engine);
            commands::app::spawn_startup_check(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::app::updates_check,
            commands::app::updates_last,
            commands::app::open_release_page,
            commands::app::system_info,
            commands::components::components_inspect,
            commands::components::components_plan,
            commands::components::components_remove,
            commands::config::config_get,
            commands::config::config_set,
            commands::activity::activity_list,
            commands::activity::activity_clear,
            commands::engine::engine_state,
            commands::engine::engine_reconnect,
            commands::engine::engine_test,
            commands::engine::engine_overview,
            commands::engine::compose_status,
            commands::resources::containers_list,
            commands::resources::container_action,
            commands::resources::container_remove,
            commands::resources::images_list,
            commands::containers::container_preview,
            commands::containers::container_create,
            commands::containers::container_inspect,
            commands::containers::container_duplicate_spec,
            commands::containers::container_edit_config,
            commands::containers::container_edit_plan,
            commands::containers::container_edit_apply,
            commands::containers::container_restore_backup,
            commands::containers::container_commit_preview,
            commands::containers::container_commit,
            commands::containers::image_defaults,
            commands::containers::networks_brief,
            commands::containers::volumes_brief,
            commands::containers::logs_open,
            commands::containers::stats_open,
            commands::containers::terminal_open,
            commands::containers::terminal_input,
            commands::containers::terminal_resize,
            commands::containers::stream_close,
            commands::images::image_details,
            commands::images::image_history,
            commands::images::image_tag,
            commands::images::image_remove,
            commands::images::images_prune,
            commands::images::images_search,
            commands::images::image_ref_parse,
            commands::images::image_pull_open,
            commands::images::build_preview,
            commands::images::build_open,
            commands::networks::networks_list,
            commands::networks::network_details,
            commands::networks::network_preview,
            commands::networks::network_create,
            commands::networks::network_remove,
            commands::networks::networks_prune,
            commands::networks::network_connect,
            commands::networks::network_disconnect,
            commands::volumes::volumes_list,
            commands::volumes::volume_details,
            commands::volumes::volume_sizes,
            commands::volumes::volume_preview,
            commands::volumes::volume_create,
            commands::volumes::volume_remove,
            commands::volumes::volumes_prune,
            commands::volumes::binds_list,
            commands::compose::compose_projects,
            commands::compose::compose_config,
            commands::compose::compose_preview,
            commands::compose::compose_run_open,
            commands::compose::compose_logs_open,
            commands::compose::compose_register,
            commands::compose::compose_unregister,
            commands::compose::compose_file_read,
            commands::compose::compose_file_save,
            commands::compose::compose_file_create,
            commands::compose::compose_find_file,
            commands::events::events_recent,
            commands::events::events_journal,
            commands::events::events_query,
            commands::events::events_journal_clear,
            commands::events::history_list,
            commands::events::history_clear,
        ])
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("DCC_LOG")
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    refuse_root();

    let store = match std::env::var_os("DCC_CONFIG") {
        Some(p) => ConfigStore::at(p),
        None => match ConfigStore::default_location() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Docker Control Center: {e}");
                std::process::exit(1);
            }
        },
    };

    configure(tauri::Builder::default(), store)
        .run(tauri::generate_context!())
        .expect("error al iniciar Docker Control Center");
}

/// La aplicación no debe ejecutarse como root (requisito de seguridad §12).
/// Se puede forzar con `DCC_ALLOW_ROOT=1` solo para entornos de prueba aislados.
fn refuse_root() {
    #[cfg(unix)]
    {
        // SAFETY: geteuid no tiene precondiciones y siempre tiene éxito.
        let euid = unsafe { geteuid() };
        if euid == 0 && std::env::var_os("DCC_ALLOW_ROOT").is_none() {
            eprintln!(
                "Docker Control Center no debe ejecutarse como root.\n\
                 Ejecútalo con tu usuario normal. Para acceder a Docker, consulta INSTALL.md."
            );
            std::process::exit(1);
        }
    }
}

#[cfg(unix)]
extern "C" {
    fn geteuid() -> u32;
}

#[cfg(test)]
mod ipc_tests;
