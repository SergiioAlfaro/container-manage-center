// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! E3 (1.3.0): auditoría de seguridad (D-093–D-095).
//!
//! Solo lectura. El informe se guarda en el historial (`auditorias.jsonl`,
//! 0600) y se envía a la interfaz con los secretos ocultos. La exportación se
//! escribe en la ruta que el usuario elige en el diálogo nativo (abierto desde
//! Rust: la interfaz no puede indicar una ruta), también con los secretos
//! ocultos y con permisos 0600.

use cmc_engine::fsx::OpenOptionsExt;
use std::io::Write;
use std::path::{Path, PathBuf};

use cmc_activity::redact::redact_text;
use cmc_engine::audit::{
    AuditProgress, AuditReport, AuditRequest, AuditSummary, ImageVulns, VulnTool,
};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::DialogExt;
use ts_rs::TS;

use crate::error::{CmdResult, CommandError};
use crate::state::AppState;

/// Tamaño máximo de un informe exportado.
pub const MAX_EXPORT: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(
    tag = "event",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum AuditEvent {
    Progress {
        progress: AuditProgress,
    },
    End {
        report: Option<Box<AuditReport>>,
        error: Option<CommandError>,
    },
}

fn r(s: &mut String) {
    if let std::borrow::Cow::Owned(x) = redact_text(s) {
        *s = x;
    }
}

/// Oculta los secretos que pudieran llegar en nombres, parámetros o mensajes
/// (D-085), antes de guardar o enviar el informe.
pub fn redact_report(rep: &mut AuditReport) {
    for f in &mut rep.findings {
        r(&mut f.target.name);
        for v in f.params.values_mut() {
            r(v);
        }
    }
    for t in &mut rep.targets {
        r(&mut t.target.name);
        r(&mut t.image);
    }
    for s in &mut rep.skipped {
        if let Some(d) = &mut s.detail {
            r(d);
        }
    }
    for t in &mut rep.vulnerabilities.tools {
        if let Some(p) = &mut t.problem {
            r(p);
        }
    }
    for i in &mut rep.vulnerabilities.images {
        match i {
            ImageVulns::Failed { message, image, .. } => {
                r(message);
                r(image);
            }
            ImageVulns::Done { image, .. } | ImageVulns::NotAnalyzed { image, .. } => r(image),
        }
    }
}

/// Herramientas de vulnerabilidades de este equipo (Trivy, Grype), con su
/// versión y la fecha de su base. No usa la red.
#[tauri::command]
pub async fn audit_tools() -> CmdResult<Vec<VulnTool>> {
    Ok(cmc_engine::audit::vuln::detect_tools().await)
}

/// Ejecuta una auditoría (solo lectura) y la guarda en el historial.
#[tauri::command]
pub async fn audit_run_open(
    state: State<'_, AppState>,
    server: String,
    request: AuditRequest,
    on_event: Channel<AuditEvent>,
) -> CmdResult<u64> {
    let mgr = state.server(&server)?;
    let engine = mgr.engine().await?;
    let compose = if request.include_compose && request.container.is_none() {
        match mgr.compose_cli().await {
            Ok(cli) => {
                let reg = super::compose::registered(&state, &server)?;
                let projects = engine
                    .compose_projects(&reg)
                    .await
                    .map(|ps| ps.into_iter().map(|p| p.reference).collect())
                    .unwrap_or(reg);
                Some((cli, projects))
            }
            Err(_) => None,
        }
    } else {
        None
    };
    let journal = state.journals.audits.clone();
    let registry = state.streams.clone();
    let manager = mgr.clone();
    let (sid, cancel) = registry.register(&server, None);
    tauri::async_runtime::spawn(async move {
        let ch = on_event.clone();
        let r = engine
            .audit(&server, &request, compose, cancel, move |p| {
                let _ = ch.send(AuditEvent::Progress { progress: p });
            })
            .await;
        if matches!(&r, Err(e) if e.is_connection_error()) {
            manager.report_connection_error();
        }
        let end = match r {
            Ok(mut rep) => {
                redact_report(&mut rep);
                if let Some(j) = &journal {
                    if let Err(e) = j.append(rep.clone()) {
                        tracing::warn!(error = %e, "no se pudo guardar la auditoría en el historial");
                    }
                }
                AuditEvent::End {
                    report: Some(Box::new(rep)),
                    error: None,
                }
            }
            Err(e) => AuditEvent::End {
                report: None,
                error: Some(e.into()),
            },
        };
        let _ = on_event.send(end);
        registry.remove(sid);
    });
    Ok(sid)
}

/// Auditorías guardadas de ese servidor, de la más reciente a la más antigua.
#[tauri::command]
pub fn audit_history(state: State<'_, AppState>, server: String) -> CmdResult<Vec<AuditSummary>> {
    state.server(&server)?;
    let Some(j) = &state.journals.audits else {
        return Ok(vec![]);
    };
    Ok(j.entries()
        .iter()
        .rev()
        .filter(|a| a.server == server)
        .map(AuditReport::summary)
        .collect())
}

/// Una auditoría guardada.
#[tauri::command]
pub fn audit_get(state: State<'_, AppState>, server: String, id: String) -> CmdResult<AuditReport> {
    state.server(&server)?;
    state
        .journals
        .audits
        .as_ref()
        .and_then(|j| {
            j.entries()
                .into_iter()
                .rev()
                .find(|a| a.id == id && a.server == server)
        })
        .ok_or_else(|| CommandError::new("audit.notFound", format!("no existe la auditoría {id}")))
}

/// Extensión de cada formato de exportación.
pub fn export_extension(format: &str) -> Option<&'static str> {
    match format {
        "json" => Some("json"),
        "markdown" => Some("md"),
        "html" => Some("html"),
        _ => None,
    }
}

/// Nombre de archivo sugerido, sin separadores ni caracteres raros.
fn safe_name(name: &str, ext: &str) -> String {
    let base: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .take(80)
        .collect();
    let base = base.trim_matches(['.', '-']);
    let base = if base.is_empty() { "auditoria" } else { base };
    format!("{base}.{ext}")
}

/// Escribe el informe exportado: secretos ocultos, la extensión del formato,
/// permisos 0600 y escritura atómica (temporal en la misma carpeta).
pub fn write_export(path: &Path, ext: &str, content: &str) -> std::io::Result<PathBuf> {
    let mut path = path.to_path_buf();
    if path.extension().and_then(|e| e.to_str()) != Some(ext) {
        let mut s = path.into_os_string();
        s.push(format!(".{ext}"));
        path = PathBuf::from(s);
    }
    let text = redact_text(content);
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::other("ruta sin carpeta"))?;
    let tmp = dir.join(format!(
        ".{}.cmc-{}.tmp",
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        std::process::id()
    ));
    let write = || -> std::io::Result<()> {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        cmc_engine::fsx::set_mode(&tmp, 0o600)?;
        std::fs::rename(&tmp, &path)
    };
    if let Err(e) = write() {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(path)
}

/// Exporta un informe ya generado por la interfaz (JSON, Markdown o HTML, en
/// el idioma de la interfaz). Pide la ruta con el diálogo nativo; devuelve la
/// ruta guardada o `null` si el usuario cancela.
#[tauri::command]
pub async fn audit_export_save<R: Runtime>(
    app: AppHandle<R>,
    format: String,
    content: String,
    name: String,
) -> CmdResult<Option<String>> {
    let ext = export_extension(&format)
        .ok_or_else(|| CommandError::new("audit.export", format!("formato no válido: {format}")))?;
    if content.len() > MAX_EXPORT {
        return Err(CommandError::new(
            "audit.export",
            "el informe es demasiado grande",
        ));
    }
    let suggested = safe_name(&name, ext);
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_file_name(&suggested)
            .add_filter(ext.to_uppercase(), &[ext])
            .blocking_save_file()
    })
    .await
    .map_err(|e| CommandError::new("audit.export", e.to_string()))?;
    let Some(fp) = picked else { return Ok(None) };
    let path = fp
        .into_path()
        .map_err(|e| CommandError::new("audit.export", e.to_string()))?;
    let saved = write_export(&path, ext, &content)
        .map_err(|e| CommandError::new("audit.export", format!("{}: {e}", path.display())))?;
    Ok(Some(saved.display().to_string()))
}

// Pruebas con permisos, enlaces o sockets de Unix: solo en Linux (D-114).
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[cfg(unix)]
    #[test]
    fn exports_are_redacted_private_and_have_the_right_extension() {
        let d = tempfile::Builder::new()
            .prefix("cmc-test")
            .tempdir()
            .unwrap();
        let p = write_export(
            &d.path().join("informe"),
            "md",
            "# Informe\nMYSQL_PWD=supersecreto\npostgres://app:clave1@db/app\n",
        )
        .unwrap();
        assert_eq!(p, d.path().join("informe.md"));
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(
            !text.contains("supersecreto") && !text.contains("clave1"),
            "{text}"
        );
        assert!(text.contains("MYSQL_PWD=********"));
        assert_eq!(
            std::fs::metadata(&p).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let p2 = write_export(&d.path().join("a.html"), "html", "<p>x</p>").unwrap();
        assert_eq!(p2, d.path().join("a.html"));
        let left: Vec<_> = std::fs::read_dir(d.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name())
            .collect();
        assert_eq!(left.len(), 2, "sin temporales: {left:?}");
        assert_eq!(export_extension("json"), Some("json"));
        assert_eq!(export_extension("exe"), None);
        assert_eq!(safe_name("../../etc/passwd", "md"), "etc-passwd.md");
        assert_eq!(safe_name("", "json"), "auditoria.json");
        assert_eq!(
            safe_name("auditoría local 2026", "html"),
            "auditor-a-local-2026.html"
        );
    }
}
