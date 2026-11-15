// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
use serde::Serialize;
use tauri::State;
use ts_rs::TS;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub config_path: String,
    pub startup_warning: Option<String>,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        name: "Container Manage Center".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        config_path: state.store.path().display().to_string(),
        startup_warning: state.startup_warning.clone(),
    }
}

// ------------------------------------------------------------ Fase 9

use chrono::{DateTime, Utc};
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::error::{CmdResult, CommandError};
use crate::updates::{UpdateCheck, UpdateSource};

/// Evento con el resultado de la comprobación automática al iniciar.
pub const UPDATE_EVENT: &str = "updates://result";
/// Espera antes de la comprobación automática, para no competir con el arranque.
pub const STARTUP_CHECK_DELAY: std::time::Duration = std::time::Duration::from_secs(5);

/// Resultado de la última comprobación (manual o al iniciar).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateOutcome {
    pub check: Option<UpdateCheck>,
    pub error: Option<String>,
    /// Hecha al iniciar la aplicación (no a petición).
    pub automatic: bool,
    pub at: DateTime<Utc>,
}

fn update_error(e: crate::updates::UpdateError) -> CommandError {
    CommandError::new(e.code(), e)
}

async fn run_check(source: &str) -> Result<UpdateCheck, crate::updates::UpdateError> {
    let src = UpdateSource::parse(source)?.ok_or(crate::updates::UpdateError::NoSource)?;
    crate::updates::fetch(&src, env!("CARGO_PKG_VERSION")).await
}

/// Comprueba si hay una versión nueva. `source` es la fuente escrita en
/// Configuración (aún sin guardar); si falta, se usa la guardada.
#[tauri::command]
pub async fn updates_check(
    state: State<'_, AppState>,
    source: Option<String>,
) -> CmdResult<UpdateCheck> {
    let source = source.unwrap_or_else(|| state.config().updates.source);
    let r = run_check(&source).await;
    *state.last_update.lock() = Some(UpdateOutcome {
        check: r.as_ref().ok().cloned(),
        error: r.as_ref().err().map(|e| e.to_string()),
        automatic: false,
        at: Utc::now(),
    });
    r.map_err(update_error)
}

/// Última comprobación hecha en esta sesión.
#[tauri::command]
pub fn updates_last(state: State<'_, AppState>) -> Option<UpdateOutcome> {
    state.last_update.lock().clone()
}

/// Comprobación al iniciar, si está activada. El resultado se guarda y se envía
/// a la ventana; un fallo (sin red, por ejemplo) no molesta: solo se anota.
pub fn spawn_startup_check<R: Runtime>(app: AppHandle<R>) {
    let cfg = app.state::<AppState>().config().updates;
    if !cfg.check_on_start || cfg.source.is_empty() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(STARTUP_CHECK_DELAY).await;
        let r = run_check(&cfg.source).await;
        if let Err(e) = &r {
            tracing::info!(error = %cmc_activity::redact::redact_text(&e.to_string()), "comprobación de actualizaciones al iniciar");
        }
        let outcome = UpdateOutcome {
            check: r.as_ref().ok().cloned(),
            error: r.err().map(|e| e.to_string()),
            automatic: true,
            at: Utc::now(),
        };
        *app.state::<AppState>().last_update.lock() = Some(outcome.clone());
        let _ = app.emit(UPDATE_EVENT, &outcome);
    });
}

/// Abre en el navegador la página de una versión. Solo `https://` de
/// `github.com` o del servidor de la fuente de actualizaciones (E2,
/// CMC-SEC-16), con `xdg-open` y el enlace como argumento (sin shell).
#[tauri::command]
pub fn open_release_page(state: State<'_, AppState>, url: String) -> CmdResult<()> {
    let last = state
        .last_update
        .lock()
        .as_ref()
        .and_then(|o| o.check.as_ref().map(|c| c.checked_url.clone()));
    let hosts = crate::updates::release_page_hosts(&state.config().updates.source, last.as_deref());
    if !crate::updates::release_page_allowed(&url, &hosts) {
        return Err(CommandError::new(
            "app.open",
            "Solo se abren enlaces https:// de github.com o de la fuente de actualizaciones.",
        ));
    }
    open_in_browser(&url)
}

/// Abre en el navegador un enlace de la salida de una terminal (OSC 8) que el
/// usuario ha confirmado (E2, CMC-SEC-12, D-091). Solo `http://` y `https://`,
/// sin usuario ni contraseña, sin espacios ni caracteres de control.
#[tauri::command]
pub fn open_terminal_link(url: String) -> CmdResult<()> {
    if !terminal_link_allowed(&url) {
        return Err(CommandError::new(
            "app.open",
            "Solo se abren enlaces http:// o https://.",
        ));
    }
    open_in_browser(&url)
}

pub(crate) fn terminal_link_allowed(url: &str) -> bool {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"));
    let Some(rest) = rest else { return false };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    url.len() <= 2048
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
        && !authority.is_empty()
        && !authority.contains('@')
        && !authority.starts_with(':')
}

/// Abre `url` (ya validada) con `xdg-open`, como argumento y sin shell.
pub(crate) fn open_in_browser(url: &str) -> CmdResult<()> {
    let program = cmc_engine::sys::find_executable("xdg-open").ok_or_else(|| {
        CommandError::new(
            "app.open",
            "No se encuentra xdg-open (paquete xdg-utils): copia el enlace y ábrelo en el navegador.",
        )
    })?;
    std::process::Command::new(program)
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| {
            CommandError::new("app.open", format!("No se pudo abrir el navegador: {e}"))
        })?;
    Ok(())
}

/// Información del equipo y de la aplicación (Configuración → Sistema).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemInfo {
    pub app_version: String,
    /// Paquete .deb instalado (versión), si lo hay.
    pub package_version: Option<String>,
    /// Ruta del ejecutable en uso.
    pub executable: Option<String>,
    /// `PRETTY_NAME` de /etc/os-release.
    pub os: Option<String>,
    pub kernel: Option<String>,
    pub arch: String,
    pub glibc: Option<String>,
    pub webkit_gtk: Option<String>,
    /// Escritorio y tipo de sesión (X11, Wayland).
    pub desktop: Option<String>,
    pub session_type: Option<String>,
    pub config_path: String,
    pub state_dir: Option<String>,
}

fn os_release() -> Option<String> {
    let s = std::fs::read_to_string("/etc/os-release")
        .or_else(|_| std::fs::read_to_string("/usr/lib/os-release"))
        .ok()?;
    s.lines().find_map(|l| {
        l.strip_prefix("PRETTY_NAME=")
            .map(|v| v.trim_matches('"').to_owned())
    })
}

/// Versión del paquete instalado según la base de datos de dpkg (solo lectura).
pub fn dpkg_version(status: &str, package: &str) -> Option<String> {
    status.split("\n\n").find_map(|stanza| {
        let mut name = None;
        let mut installed = false;
        let mut version = None;
        for l in stanza.lines() {
            if let Some(v) = l.strip_prefix("Package: ") {
                name = Some(v.trim());
            } else if let Some(v) = l.strip_prefix("Status: ") {
                installed = v.trim().ends_with(" installed");
            } else if let Some(v) = l.strip_prefix("Version: ") {
                version = Some(v.trim().to_owned());
            }
        }
        (name == Some(package) && installed)
            .then_some(version)
            .flatten()
    })
}

#[cfg(windows)]
fn glibc_version() -> Option<String> {
    None
}

#[cfg(unix)]
fn glibc_version() -> Option<String> {
    // SAFETY: gnu_get_libc_version devuelve una cadena estática de glibc.
    let p = unsafe { libc::gnu_get_libc_version() };
    if p.is_null() {
        return None;
    }
    Some(
        unsafe { std::ffi::CStr::from_ptr(p) }
            .to_string_lossy()
            .into_owned(),
    )
}

#[tauri::command]
pub fn system_info(state: State<'_, AppState>) -> SystemInfo {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    SystemInfo {
        app_version: env!("CARGO_PKG_VERSION").into(),
        package_version: std::fs::read_to_string("/var/lib/dpkg/status")
            .ok()
            .and_then(|s| dpkg_version(&s, crate::uninstall::OWN_PACKAGE)),
        executable: std::env::current_exe()
            .ok()
            .map(|p| p.display().to_string()),
        os: os_release(),
        kernel: std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .ok()
            .map(|s| s.trim().to_owned()),
        arch: std::env::consts::ARCH.into(),
        glibc: glibc_version(),
        webkit_gtk: tauri::webview_version().ok(),
        desktop: env("XDG_CURRENT_DESKTOP"),
        session_type: env("XDG_SESSION_TYPE"),
        config_path: state.store.path().display().to_string(),
        state_dir: state.journals.dir.as_ref().map(|d| d.display().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpkg_status_is_read_only_for_installed_packages() {
        let status = "Package: otro\nStatus: install ok installed\nVersion: 1\n\n\
Package: container-manage-center\nStatus: install ok installed\nPriority: optional\nVersion: 0.9.0\n\n\
Package: viejo\nStatus: deinstall ok config-files\nVersion: 2\n";
        assert_eq!(
            dpkg_version(status, "container-manage-center").as_deref(),
            Some("0.9.0")
        );
        assert_eq!(dpkg_version(status, "viejo"), None, "desinstalado");
        assert_eq!(dpkg_version(status, "nada"), None);
    }

    #[test]
    fn only_https_links_are_opened() {
        for bad in [
            "http://ejemplo.es",
            "file:///etc/passwd",
            "https://a b",
            "javascript:x",
            "https://x\n--flag",
        ] {
            let hosts = crate::updates::release_page_hosts("org/repo", None);
            assert!(!crate::updates::release_page_allowed(bad, &hosts), "{bad}");
        }
    }

    #[test]
    fn terminal_links_are_http_only() {
        for good in [
            "https://example.com/a?b=c#d",
            "http://localhost:8080/",
            "http://127.0.0.1:3000",
        ] {
            assert!(terminal_link_allowed(good), "{good}");
        }
        for bad in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "ssh://host",
            "https://user:pw@example.com/",
            "https://",
            "https://a b",
            "https://x\n--flag",
            "--help",
            "HTTPS://example.com",
        ] {
            assert!(!terminal_link_allowed(bad), "{bad}");
        }
        assert!(!terminal_link_allowed(&format!(
            "https://e.com/{}",
            "a".repeat(2050)
        )));
    }
}
