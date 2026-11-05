//! Punto de entrada de la aplicación de escritorio.

mod commands;
mod error;
mod events;
mod state;
mod streams;

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
        .manage(state)
        .on_window_event(|window, event| {
            // Al cerrar la ventana se cierran los flujos abiertos (logs, terminales…),
            // que quedan registrados como cerrados por el usuario.
            if let tauri::WindowEvent::Destroyed = event {
                if let Some(state) = window.try_state::<state::AppState>() {
                    state.streams.close_all();
                }
            }
        })
        .setup(move |app| {
            events::spawn_activity_bridge(app.handle().clone(), activity);
            events::spawn_engine(app.handle().clone(), engine);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
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
            commands::containers::image_defaults,
            commands::containers::networks_brief,
            commands::containers::volumes_brief,
            commands::containers::logs_open,
            commands::containers::stats_open,
            commands::containers::terminal_open,
            commands::containers::terminal_input,
            commands::containers::terminal_resize,
            commands::containers::stream_close,
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
