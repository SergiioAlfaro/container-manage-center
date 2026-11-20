// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Punto de entrada de la aplicación de escritorio.

mod backups;
mod bulk;
mod bulk_exec;
mod commands;
mod error;
mod events;
mod journal;
mod links;
mod migration;
mod monitor;
mod notify;
mod secrets;
mod servers;
mod state;
mod streams;
mod systemd;
mod uninstall;
mod updates;

use cmc_store::ConfigStore;
use tauri::{Builder, Manager, Runtime};

pub use error::CommandError;

/// Registra plugins, estado, puente de eventos y comandos sobre un `Builder`.
/// Se separa de `run` para poder montar la aplicación en pruebas con el
/// runtime simulado de Tauri.
pub fn configure<R: Runtime>(builder: Builder<R>, store: ConfigStore) -> Builder<R> {
    let state = state::AppState::new(store);
    let activity = state.activity.clone();
    let engine = state.servers.local.clone();
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
                // También se cierran las sesiones SSH de los servidores remotos
                // (0.10.0): no queda ningún proceso `ssh` al salir.
                tauri::WindowEvent::CloseRequested { api, .. }
                    if (state.streams.running() > 0 || state.servers.any_remote_open())
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
                        if let Some(s) = w.try_state::<state::AppState>() {
                            s.servers.close_all().await;
                        }
                        let _ = w.close();
                    });
                }
                tauri::WindowEvent::Destroyed => {
                    state.streams.close_all();
                    state.monitor.stop_all();
                }
                _ => {}
            }
        })
        .setup(move |app| {
            events::spawn_activity_bridge(app.handle().clone(), activity.clone());
            journal::spawn_history_bridge(app.handle().clone(), activity);
            events::spawn_engine(app.handle().clone(), engine);
            let handle = app.handle().clone();
            app.state::<state::AppState>()
                .servers
                .set_spawner(std::sync::Arc::new(move |id, m, cancel| {
                    events::spawn_server_tasks(handle.clone(), id, m, cancel)
                }));
            // Sesiones SSH que dejó una instancia anterior cerrada a la fuerza.
            let paths = app.state::<state::AppState>().servers.ctx.paths.clone();
            tauri::async_runtime::spawn(async move {
                let n = cmc_engine::ssh::session::close_orphans(&paths).await;
                if n > 0 {
                    tracing::info!(n, "sesiones SSH huérfanas cerradas");
                }
            });
            commands::servers::spawn_auto_connect(app.handle().clone());
            commands::app::spawn_startup_check(app.handle().clone());
            // Monitorización de los servidores activados (sobrevive a un reinicio).
            monitor::start_enabled(app.handle());
            // Tareas programadas y contraseñas del nombre anterior (D-113).
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<state::AppState>();
                migration::migrate_units_and_secrets(&state).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::app::updates_check,
            commands::app::updates_last,
            commands::app::open_release_page,
            commands::app::open_terminal_link,
            commands::audit::audit_tools,
            commands::audit::audit_run_open,
            commands::audit::audit_history,
            commands::audit::audit_get,
            commands::audit::audit_export_save,
            commands::bulk::bulk_inventory,
            commands::bulk::bulk_run_open,
            commands::bulk_exec::bulk_exec_preview,
            commands::bulk_exec::bulk_exec_open,
            commands::bulk_exec::bulk_exec_interrupt,
            commands::bulk_exec::exec_templates_save,
            commands::backup::backup_settings,
            commands::monitor::monitor_settings,
            commands::monitor::monitor_settings_set,
            commands::monitor::monitor_set_server,
            commands::monitor::monitor_pause,
            commands::monitor::monitor_status,
            commands::monitor::monitor_live,
            commands::monitor::monitor_minutes,
            commands::monitor::monitor_alerts,
            commands::monitor::monitor_notify,
            commands::links::tools_links,
            commands::backup::backup_pick_folder,
            commands::backup::backup_list,
            commands::backup::backup_verify,
            commands::backup::backup_delete,
            commands::backup::backup_run_open,
            commands::backup::backup_restore_preview,
            commands::backup::backup_restore_open,
            commands::backup::backup_history,
            commands::backup::backup_job_save,
            commands::backup::backup_job_delete,
            commands::backup::backup_timer_preview,
            commands::backup::backup_timer_install,
            commands::backup::backup_timer_set_enabled,
            commands::backup::backup_timer_remove,
            commands::app::system_info,
            commands::components::components_inspect,
            commands::components::components_plan,
            commands::components::components_remove,
            commands::config::config_get,
            commands::config::config_set,
            commands::activity::activity_list,
            commands::activity::activity_clear,
            commands::servers::servers_list,
            commands::servers::server_validate,
            commands::servers::server_save,
            commands::servers::server_delete,
            commands::servers::server_connect,
            commands::servers::server_disconnect,
            commands::servers::server_test,
            commands::servers::server_host_key,
            commands::servers::server_trusted_keys,
            commands::servers::server_trust,
            commands::servers::server_secret_status,
            commands::servers::keyring_status,
            commands::servers::server_forget_secret,
            commands::servers::server_diagnose,
            commands::servers::servers_overview,
            commands::servers::server_shell_open,
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
            tracing_subscriber::EnvFilter::try_from_env("CMC_LOG")
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    refuse_root_with(Notice::Window);
    // Datos del nombre anterior: antes de abrir ningún archivo (D-113).
    migration::migrate_dirs();

    let store = match std::env::var_os("CMC_CONFIG") {
        Some(p) => ConfigStore::at(p),
        None => match ConfigStore::default_location() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Container Manage Center: {e}");
                std::process::exit(1);
            }
        },
    };

    configure(tauri::Builder::default(), store)
        .run(tauri::generate_context!())
        .expect("error al iniciar Container Manage Center");
}

/// `container-manage-center --migrate` (1.8.0, D-113). `None` si no se pidió.
pub fn migrate_main() -> Option<i32> {
    migration::cli_main()
}

/// Desinstalación en Windows (1.9.1): `container-manage-center
/// --remove-scheduled-tasks` quita del Programador de tareas las copias
/// programadas **propias** (con la marca de la aplicación) de las tareas
/// configuradas. Lo ejecuta el desinstalador (no al actualizar); los datos y
/// las copias se conservan, como en Linux. `None` si no se pidió.
pub fn uninstall_main() -> Option<i32> {
    if std::env::args().nth(1).as_deref() != Some("--remove-scheduled-tasks") {
        return None;
    }
    #[cfg(not(windows))]
    {
        eprintln!("--remove-scheduled-tasks solo se usa en Windows (el desinstalador lo ejecuta).");
        Some(2)
    }
    #[cfg(windows)]
    {
        let store = match std::env::var_os("CMC_CONFIG") {
            Some(p) => ConfigStore::at(p),
            None => match ConfigStore::default_location() {
                Ok(s) => s,
                Err(_) => return Some(0),
            },
        };
        let Some(sched) = systemd::Systemd::from_env() else {
            return Some(0);
        };
        let log = cmc_activity::ActivityLog::new(64);
        let mut failed = 0;
        tauri::async_runtime::block_on(async {
            for job in backups::load_jobs(store.path()) {
                if let Err(e) = sched.remove(&log, &job.id).await {
                    eprintln!("Container Manage Center: {}: {e}", job.id);
                    failed += 1;
                }
            }
        });
        Some(if failed == 0 { 0 } else { 1 })
    }
}

/// Modo sin ventana de los temporizadores de systemd (E4, D-100):
/// `container-manage-center --run-backup <tarea>`. `None` si no se pidió.
pub fn headless_main() -> Option<i32> {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("--run-backup") {
        return None;
    }
    let Some(id) = args.next() else {
        eprintln!("Uso: container-manage-center --run-backup <tarea>");
        return Some(2);
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("CMC_LOG")
                .unwrap_or_else(|_| "warn".into()),
        )
        .init();
    refuse_root();
    migration::migrate_dirs();
    let store = match std::env::var_os("CMC_CONFIG") {
        Some(p) => ConfigStore::at(p),
        None => match ConfigStore::default_location() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Container Manage Center: {e}");
                return Some(1);
            }
        },
    };
    Some(tauri::async_runtime::block_on(run_backup_job(store, &id)))
}

/// Ejecuta una tarea de copia sin ventana. Código de salida: 0 bien, 1 error,
/// 75 si ya hay otra ejecución de la misma tarea.
pub async fn run_backup_job(store: ConfigStore, id: &str) -> i32 {
    let state = state::AppState::new(store);
    let job = match backups::find_job(state.store.path(), id) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("Container Manage Center: {}", e.message);
            return 1;
        }
    };
    let _lock = match job_lock(&state, id) {
        Ok(l) => l,
        Err(code) => return code,
    };
    let mgr = match state.server(&job.server) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Container Manage Center: {}", e.message);
            return 1;
        }
    };
    let _ = mgr.connect().await;
    let r = backups::run_backup(
        &state,
        &mgr,
        &job.server,
        &job.destination,
        &job.request,
        Some(&job),
        "timer",
        tokio_util::sync::CancellationToken::new(),
        |_| {},
    )
    .await;
    // Lo hecho queda en el historial de operaciones, como desde la ventana.
    if state.config().activity.keep_history {
        if let Some(j) = &state.journals.history {
            for rec in state.activity.snapshot().into_iter().rev() {
                if let Some(e) = cmc_activity::HistoryEntry::from_record(&rec) {
                    let _ = j.append(e);
                }
            }
        }
    }
    state.servers.close_all().await;
    match r {
        Ok(e) => {
            println!("Copia {} completa en {}", e.id, e.path);
            0
        }
        Err(e) => {
            eprintln!(
                "Container Manage Center: la copia «{}» ha fallado: {} ({})",
                job.name, e.message, e.code
            );
            1
        }
    }
}

/// Evita dos ejecuciones simultáneas de la misma tarea (`flock`).
fn job_lock(state: &state::AppState, id: &str) -> Result<std::fs::File, i32> {
    use cmc_engine::fsx::OpenOptionsExt;
    let dir = state
        .journals
        .dir
        .clone()
        .unwrap_or_else(std::env::temp_dir);
    let _ = std::fs::create_dir_all(&dir);
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).truncate(false).write(true).mode(0o600);
    // Windows (1.9.0, D-114): sin compartir; otra ejecución no puede abrirlo
    // mientras esta lo tenga abierto (ERROR_SHARING_VIOLATION).
    #[cfg(windows)]
    std::os::windows::fs::OpenOptionsExt::share_mode(&mut opts, 0);
    let f = match opts.open(dir.join(format!("copia-{id}.lock"))) {
        Ok(f) => f,
        #[cfg(windows)]
        Err(e) if e.raw_os_error() == Some(32) => {
            eprintln!("Container Manage Center: ya hay una copia de «{id}» en curso");
            return Err(75);
        }
        Err(e) => {
            eprintln!("Container Manage Center: {e}");
            return Err(1);
        }
    };
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        // SAFETY: descriptor válido mientras viva `f`.
        if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            eprintln!("Container Manage Center: ya hay una copia de «{id}» en curso");
            return Err(75);
        }
    }
    Ok(f)
}

/// La aplicación no debe ejecutarse como root (requisito de seguridad §12) ni,
/// en Windows, con privilegios de administrador elevados (W-18, 1.9.1).
/// `CMC_ALLOW_ROOT=1` lo permite solo en las compilaciones de desarrollo y
/// prueba (entornos aislados). En la compilación de publicación se ignora
/// (E2, CMC-SEC-11, D-091). Sin ventana (copias programadas, `--migrate`) solo
/// se escribe el motivo en la salida de errores.
fn refuse_root() {
    refuse_root_with(Notice::Stderr);
}

/// Cómo se avisa de que no puede arrancar.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Notice {
    /// Solo la salida de errores.
    Stderr,
    /// Además, un cuadro de mensaje en Windows: la compilación de publicación
    /// no tiene consola y el usuario no vería nada.
    Window,
}

fn refuse_root_with(notice: Notice) {
    #[cfg(unix)]
    {
        let _ = notice;
        // SAFETY: geteuid no tiene precondiciones y siempre tiene éxito.
        let euid = unsafe { geteuid() };
        let override_set = std::env::var_os("CMC_ALLOW_ROOT").is_some();
        if !may_run(euid == 0, override_set, cfg!(debug_assertions)) {
            eprintln!(
                "Container Manage Center no debe ejecutarse como root.\n\
                 Ejecútalo con tu usuario normal. Para acceder a Docker, consulta INSTALL.md."
            );
            std::process::exit(1);
        }
    }
    #[cfg(windows)]
    {
        // Si no se puede saber, se deja arrancar: es una medida de prudencia,
        // no un control de acceso (Windows ya decide qué puede hacer el proceso).
        let elevated = match win_elevation::is_elevated() {
            Ok(e) => e,
            Err(code) => {
                tracing::warn!("no se pudo comprobar si el proceso está elevado (error {code})");
                false
            }
        };
        let override_set = std::env::var_os("CMC_ALLOW_ROOT").is_some();
        if !may_run(elevated, override_set, cfg!(debug_assertions)) {
            const MSG: &str = "Container Manage Center no debe ejecutarse como administrador.\n\
                 Ábrelo con tu usuario normal, sin «Ejecutar como administrador». \
                 Docker Desktop no lo necesita.";
            eprintln!("{MSG}");
            if notice == Notice::Window {
                win_elevation::message_box("Container Manage Center", MSG);
            }
            std::process::exit(1);
        }
    }
}

/// ¿Puede arrancar? Con privilegios (root en Linux, proceso elevado en
/// Windows), solo en una compilación de desarrollo y con `CMC_ALLOW_ROOT`.
fn may_run(privileged: bool, override_set: bool, dev_build: bool) -> bool {
    !privileged || (override_set && dev_build)
}

/// Comprobación de elevación en Windows (W-18).
#[cfg(windows)]
mod win_elevation {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    /// `true` si el proceso se ejecuta con el testigo elevado (UAC «Ejecutar
    /// como administrador», o una cuenta de administrador con UAC desactivado).
    /// `Err` con el código de Windows si no se puede consultar.
    pub fn is_elevated() -> Result<bool, u32> {
        let mut token: HANDLE = std::ptr::null_mut();
        // SAFETY: GetCurrentProcess devuelve un seudo-identificador que no hay
        // que cerrar; `token` es una salida válida.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            // SAFETY: sin precondiciones.
            return Err(unsafe { GetLastError() });
        }
        let mut info = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut len = 0u32;
        // SAFETY: `info` tiene el tamaño que se indica y `token` es válido.
        let ok = unsafe {
            GetTokenInformation(
                token,
                TokenElevation,
                (&mut info as *mut TOKEN_ELEVATION).cast(),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut len,
            )
        };
        // SAFETY: sin precondiciones.
        let err = unsafe { GetLastError() };
        // SAFETY: `token` lo abrió OpenProcessToken y no se usa después.
        unsafe { CloseHandle(token) };
        if ok == 0 {
            return Err(err);
        }
        Ok(info.TokenIsElevated != 0)
    }

    pub fn message_box(title: &str, text: &str) {
        let w = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
        let (t, m) = (w(title), w(text));
        // SAFETY: cadenas UTF-16 terminadas en cero que viven durante la llamada.
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                m.as_ptr(),
                t.as_ptr(),
                MB_OK | MB_ICONERROR,
            )
        };
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn elevation_can_be_queried() {
            // El valor depende de cómo se ejecuten las pruebas; lo que importa
            // es que la consulta funciona.
            assert!(super::is_elevated().is_ok());
        }
    }
}

#[cfg(test)]
mod root_tests {
    use super::may_run;

    #[test]
    fn root_is_refused_in_release_builds_even_with_the_override() {
        // Sin privilegios: siempre. Con privilegios (root o proceso elevado de
        // Windows): solo en desarrollo y con CMC_ALLOW_ROOT.
        assert!(may_run(false, false, false));
        assert!(may_run(false, true, false));
        assert!(!may_run(true, false, false));
        assert!(
            !may_run(true, true, false),
            "publicación: CMC_ALLOW_ROOT se ignora"
        );
        assert!(!may_run(true, false, true));
        assert!(may_run(true, true, true), "desarrollo y pruebas aisladas");
    }
}

#[cfg(unix)]
extern "C" {
    fn geteuid() -> u32;
}

#[cfg(test)]
mod ipc_tests;
