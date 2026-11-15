// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Vulnerabilidades de las imágenes con Trivy o Grype (E3, D-094).
//!
//! Las herramientas **se detectan, nunca se instalan** ni se actualizan: la
//! aplicación no descarga su base de datos (`--skip-db-update`,
//! `GRYPE_DB_AUTO_UPDATE=false`). Si no hay herramienta, si su base no está
//! disponible o si el análisis no termina, el resultado lo dice («no
//! analizado», «falló»): nunca se muestra «sin vulnerabilidades» sin un
//! análisis completo, y siempre con la fecha de la base usada.
//!
//! Se ejecutan sin shell, con los argumentos separados, un tiempo máximo y
//! cancelables. Cada análisis queda en el panel de actividad (sin su salida).

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use cmc_activity::{ActivityLog, Category, Failure, OperationSpec, Outcome, Transport};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::model::{ImageVulns, VulnCounts, VulnTool, Vulnerability};

/// Herramientas admitidas, en orden de preferencia.
pub const TOOLS: [&str; 2] = ["trivy", "grype"];
const INFO_TIMEOUT: Duration = Duration::from_secs(30);
pub const SCAN_TIMEOUT: Duration = Duration::from_secs(15 * 60);
/// Vulnerabilidades conservadas por imagen (las más graves).
pub const TOP: usize = 50;
/// Salida máxima que se lee de una herramienta.
const MAX_OUTPUT: usize = 64 * 1024 * 1024;

/// Entorno común: sin actualizaciones ni comprobaciones de versión en red.
fn tool_env(name: &str) -> Vec<(&'static str, String)> {
    match name {
        "grype" => vec![
            ("GRYPE_DB_AUTO_UPDATE", "false".into()),
            ("GRYPE_CHECK_FOR_APP_UPDATE", "false".into()),
        ],
        _ => vec![("TRIVY_NO_PROGRESS", "true".into())],
    }
}

struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

async fn run(
    program: &Path,
    args: &[String],
    env: &[(&'static str, String)],
    timeout: Duration,
    cancel: &CancellationToken,
) -> Result<Run, String> {
    let mut cmd = tokio::process::Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let child = cmd
        .spawn()
        .map_err(|e| format!("no se pudo ejecutar {}: {e}", program.display()))?;
    let out = tokio::select! {
        r = tokio::time::timeout(timeout, child.wait_with_output()) => match r {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => return Err(e.to_string()),
            Err(_) => return Err(format!("tiempo agotado ({} s)", timeout.as_secs())),
        },
        _ = cancel.cancelled() => return Err("cancelado".into()),
    };
    if out.stdout.len() > MAX_OUTPUT {
        return Err("la salida es demasiado grande".into());
    }
    Ok(Run {
        code: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

fn sv(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

/// Versión y base de Trivy (`trivy version --format json`).
pub fn parse_trivy_version(json: &str) -> (Option<String>, Option<String>) {
    let v: Value = serde_json::from_str(json).unwrap_or(Value::Null);
    let version = v.get("Version").and_then(Value::as_str).map(str::to_owned);
    let db = v
        .pointer("/VulnerabilityDB/UpdatedAt")
        .and_then(Value::as_str)
        .filter(|s| !s.starts_with("0001-"))
        .map(str::to_owned);
    (version, db)
}

/// Versión de Grype (`grype version -o json`).
pub fn parse_grype_version(json: &str) -> Option<String> {
    let v: Value = serde_json::from_str(json).ok()?;
    v.get("version").and_then(Value::as_str).map(str::to_owned)
}

/// Estado de la base de Grype (`grype db status -o json`): fecha y si es válida.
pub fn parse_grype_db(json: &str) -> (Option<String>, bool) {
    let v: Value = serde_json::from_str(json).unwrap_or(Value::Null);
    let built = v
        .get("built")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && !s.starts_with("0001-"))
        .map(str::to_owned);
    let valid = v
        .get("valid")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| v.get("status").and_then(Value::as_str) == Some("valid"));
    (built.clone(), valid && built.is_some())
}

/// Detecta una herramienta: ruta, versión y estado de su base, sin red.
pub async fn inspect_tool(name: &str, path: &Path) -> VulnTool {
    let cancel = CancellationToken::new();
    let env = tool_env(name);
    let mut tool = VulnTool {
        name: name.to_owned(),
        path: path.display().to_string(),
        version: None,
        db_updated: None,
        db_ready: false,
        problem: None,
    };
    match name {
        "trivy" => match run(
            path,
            &sv(&["version", "--format", "json"]),
            &env,
            INFO_TIMEOUT,
            &cancel,
        )
        .await
        {
            Ok(r) if r.code == Some(0) => {
                let (v, db) = parse_trivy_version(&r.stdout);
                tool.version = v;
                tool.db_ready = db.is_some();
                tool.db_updated = db;
                if !tool.db_ready {
                    tool.problem = Some("audit.vuln.noDatabase".into());
                }
            }
            Ok(r) => tool.problem = Some(format!("audit.vuln.toolError:{}", first_line(&r.stderr))),
            Err(e) => tool.problem = Some(format!("audit.vuln.toolError:{e}")),
        },
        "grype" => {
            match run(
                path,
                &sv(&["version", "-o", "json"]),
                &env,
                INFO_TIMEOUT,
                &cancel,
            )
            .await
            {
                Ok(r) if r.code == Some(0) => tool.version = parse_grype_version(&r.stdout),
                Ok(r) => {
                    tool.problem = Some(format!("audit.vuln.toolError:{}", first_line(&r.stderr)))
                }
                Err(e) => tool.problem = Some(format!("audit.vuln.toolError:{e}")),
            }
            if tool.problem.is_none() {
                match run(
                    path,
                    &sv(&["db", "status", "-o", "json"]),
                    &env,
                    INFO_TIMEOUT,
                    &cancel,
                )
                .await
                {
                    Ok(r) => {
                        let (built, ready) = parse_grype_db(&r.stdout);
                        tool.db_updated = built;
                        tool.db_ready = ready && r.code == Some(0);
                        if !tool.db_ready {
                            tool.problem = Some("audit.vuln.noDatabase".into());
                        }
                    }
                    Err(e) => tool.problem = Some(format!("audit.vuln.toolError:{e}")),
                }
            }
        }
        _ => tool.problem = Some("audit.vuln.unknownTool".into()),
    }
    tool
}

/// Herramientas instaladas en este equipo (en el `PATH`).
pub async fn detect_tools() -> Vec<VulnTool> {
    let mut out = Vec::new();
    for name in TOOLS {
        if let Some(p) = crate::sys::find_executable(name) {
            out.push(inspect_tool(name, &p).await);
        }
    }
    out
}

fn first_line(s: &str) -> String {
    s.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .chars()
        .take(300)
        .collect()
}

fn rank(sev: &str) -> u8 {
    match sev.to_ascii_lowercase().as_str() {
        "critical" => 0,
        "high" => 1,
        "medium" => 2,
        "low" | "negligible" => 3,
        _ => 4,
    }
}

fn finish(mut all: Vec<Vulnerability>) -> (VulnCounts, Vec<Vulnerability>) {
    // Una vulnerabilidad por (id, paquete): Trivy la repite por cada capa o destino.
    all.sort_by(|a, b| {
        rank(&a.severity)
            .cmp(&rank(&b.severity))
            .then(a.id.cmp(&b.id))
            .then(a.package.cmp(&b.package))
    });
    all.dedup_by(|a, b| a.id == b.id && a.package == b.package && a.installed == b.installed);
    let mut counts = VulnCounts::default();
    for v in &all {
        counts.add(&v.severity);
    }
    all.truncate(TOP);
    (counts, all)
}

/// Resultado de `trivy image --format json`.
pub fn parse_trivy(json: &str) -> Result<(VulnCounts, Vec<Vulnerability>), String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("salida no válida: {e}"))?;
    let results = v.get("Results").ok_or("la salida no tiene «Results»")?;
    let mut all = Vec::new();
    for r in results.as_array().map(Vec::as_slice).unwrap_or_default() {
        for x in r
            .get("Vulnerabilities")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            let s = |k: &str| x.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
            all.push(Vulnerability {
                id: s("VulnerabilityID"),
                package: s("PkgName"),
                installed: s("InstalledVersion"),
                fixed: Some(s("FixedVersion")).filter(|f| !f.is_empty()),
                severity: s("Severity").to_ascii_lowercase(),
            });
        }
    }
    Ok(finish(all))
}

/// Resultado de `grype -o json`.
pub fn parse_grype(json: &str) -> Result<(VulnCounts, Vec<Vulnerability>), String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("salida no válida: {e}"))?;
    let matches = v
        .get("matches")
        .and_then(Value::as_array)
        .ok_or("la salida no tiene «matches»")?;
    let mut all = Vec::new();
    for m in matches {
        let vu = m.get("vulnerability").cloned().unwrap_or(Value::Null);
        let art = m.get("artifact").cloned().unwrap_or(Value::Null);
        let s = |o: &Value, k: &str| o.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
        let fixed = vu
            .pointer("/fix/versions")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .filter(|f| !f.is_empty());
        all.push(Vulnerability {
            id: s(&vu, "id"),
            package: s(&art, "name"),
            installed: s(&art, "version"),
            fixed,
            severity: s(&vu, "severity").to_ascii_lowercase(),
        });
    }
    Ok(finish(all))
}

/// Analiza una imagen del Docker de este equipo (`docker_host`, p. ej.
/// `unix:///var/run/docker.sock`) con `tool`, que debe tener la base lista.
pub async fn scan_image(
    log: &ActivityLog,
    tool: &VulnTool,
    image: &str,
    docker_host: &str,
    cancel: &CancellationToken,
) -> ImageVulns {
    if !tool.db_ready {
        return ImageVulns::NotAnalyzed {
            image: image.to_owned(),
            reason: tool
                .problem
                .clone()
                .unwrap_or_else(|| "audit.vuln.noDatabase".into()),
        };
    }
    if image.is_empty()
        || image.starts_with('-')
        || image.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return ImageVulns::NotAnalyzed {
            image: image.to_owned(),
            reason: "audit.vuln.badImage".into(),
        };
    }
    let args: Vec<String> = match tool.name.as_str() {
        "trivy" => sv(&[
            "image",
            "--skip-db-update",
            "--skip-java-db-update",
            "--offline-scan",
            "--scanners",
            "vuln",
            "--format",
            "json",
            "--quiet",
            "--timeout",
            "14m",
            "--docker-host",
            docker_host,
            "--",
            image,
        ]),
        _ => vec![
            "-o".into(),
            "json".into(),
            "-q".into(),
            "--".into(),
            format!("docker:{image}"),
        ],
    };
    let mut env = tool_env(&tool.name);
    env.push(("DOCKER_HOST", docker_host.to_owned()));
    let handle = log.begin(OperationSpec {
        category: Category::Image,
        kind: "audit.vulnScan".into(),
        target: Some(image.to_owned()),
        transport: Transport::Cli {
            program: tool.name.clone(),
            args: args.clone(),
        },
        request_body: None,
        equivalent_command: Some(cmc_activity::quote::render_argv(&tool.name, &args)),
    });
    let r = run(Path::new(&tool.path), &args, &env, SCAN_TIMEOUT, cancel).await;
    let failed = |message: String| ImageVulns::Failed {
        image: image.to_owned(),
        tool: tool.name.clone(),
        message,
    };
    match r {
        Ok(r) if r.code == Some(0) => {
            let parsed = if tool.name == "trivy" {
                parse_trivy(&r.stdout)
            } else {
                parse_grype(&r.stdout)
            };
            match parsed {
                Ok((counts, top)) => {
                    handle.succeed(Outcome {
                        summary: Some(format!("{} vulnerabilidades conocidas", counts.total())),
                        ..Default::default()
                    });
                    ImageVulns::Done {
                        image: image.to_owned(),
                        tool: tool.name.clone(),
                        tool_version: tool.version.clone(),
                        db_updated: tool.db_updated.clone(),
                        counts,
                        top,
                    }
                }
                Err(e) => {
                    handle.fail(Failure {
                        error: e.clone(),
                        stdout: None,
                        stderr: None,
                        http_status: None,
                        exit_code: Some(0),
                    });
                    failed(e)
                }
            }
        }
        Ok(r) => {
            let msg = first_line(&r.stderr);
            handle.fail(Failure {
                error: msg.clone(),
                stdout: None,
                stderr: Some(r.stderr),
                http_status: None,
                exit_code: r.code,
            });
            failed(msg)
        }
        Err(e) => {
            let f = Failure {
                error: e.clone(),
                stdout: None,
                stderr: None,
                http_status: None,
                exit_code: None,
            };
            if cancel.is_cancelled() {
                handle.cancel(f);
            } else {
                handle.fail(f);
            }
            failed(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_and_databases() {
        let t = r#"{"Version":"0.58.1","VulnerabilityDB":{"Version":2,"UpdatedAt":"2026-11-10T06:12:00Z","NextUpdate":"2026-11-11T06:12:00Z"}}"#;
        assert_eq!(
            parse_trivy_version(t),
            (Some("0.58.1".into()), Some("2026-11-10T06:12:00Z".into()))
        );
        assert_eq!(
            parse_trivy_version(r#"{"Version":"0.58.1"}"#),
            (Some("0.58.1".into()), None),
            "sin base"
        );
        assert_eq!(
            parse_trivy_version(
                r#"{"Version":"0.58.1","VulnerabilityDB":{"UpdatedAt":"0001-01-01T00:00:00Z"}}"#
            )
            .1,
            None
        );
        assert_eq!(
            parse_grype_version(r#"{"application":"grype","version":"0.86.1"}"#),
            Some("0.86.1".into())
        );
        assert_eq!(
            parse_grype_db(
                r#"{"schemaVersion":"v6","built":"2026-11-09T04:00:00Z","path":"/x","valid":true}"#
            ),
            (Some("2026-11-09T04:00:00Z".into()), true)
        );
        assert_eq!(
            parse_grype_db(r#"{"valid":false,"error":"no db"}"#),
            (None, false)
        );
        assert_eq!(parse_grype_db("texto que no es JSON"), (None, false));
    }

    #[test]
    fn trivy_and_grype_results() {
        let t = r#"{"Results":[
            {"Target":"debian","Vulnerabilities":[
                {"VulnerabilityID":"CVE-1","PkgName":"openssl","InstalledVersion":"1","FixedVersion":"2","Severity":"CRITICAL"},
                {"VulnerabilityID":"CVE-2","PkgName":"zlib","InstalledVersion":"1","Severity":"LOW"},
                {"VulnerabilityID":"CVE-1","PkgName":"openssl","InstalledVersion":"1","FixedVersion":"2","Severity":"CRITICAL"}]},
            {"Target":"app","Vulnerabilities":null}]}"#;
        let (c, top) = parse_trivy(t).unwrap();
        assert_eq!(
            (c.critical, c.low, c.total()),
            (1, 1, 2),
            "repetidas se cuentan una vez"
        );
        assert_eq!(top[0].id, "CVE-1");
        assert_eq!(top[0].fixed.as_deref(), Some("2"));
        assert_eq!(top[1].fixed, None);
        // Análisis completo sin vulnerabilidades: 0, pero solo con «Results».
        assert_eq!(parse_trivy(r#"{"Results":[]}"#).unwrap().0.total(), 0);
        assert!(parse_trivy(r#"{"SchemaVersion":2}"#).is_err());
        assert!(parse_trivy("error").is_err());

        let g = r#"{"matches":[
            {"vulnerability":{"id":"GHSA-1","severity":"High","fix":{"versions":["1.2","1.3"],"state":"fixed"}},"artifact":{"name":"lib","version":"1.0"}},
            {"vulnerability":{"id":"CVE-9","severity":"Negligible","fix":{"versions":[],"state":"not-fixed"}},"artifact":{"name":"x","version":"0"}},
            {"vulnerability":{"id":"CVE-8","severity":"Unknown"},"artifact":{"name":"y","version":"0"}}]}"#;
        let (c, top) = parse_grype(g).unwrap();
        assert_eq!((c.high, c.low, c.unknown, c.total()), (1, 1, 1, 3));
        assert_eq!(top[0].fixed.as_deref(), Some("1.2, 1.3"));
        assert!(
            parse_grype(r#"{"source":{}}"#).is_err(),
            "sin «matches» no es un análisis completo"
        );
    }

    /// Herramienta falsa (un script) para comprobar la ejecución, el tiempo
    /// máximo, los errores y que nunca se informa de 0 sin análisis completo.
    #[tokio::test]
    async fn scanning_with_fake_tools() {
        let dir = tempfile::Builder::new()
            .prefix("cmc-test")
            .tempdir()
            .unwrap();
        let write = |name: &str, body: &str| {
            let p = dir.path().join(name);
            std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&p, std::os::unix::fs::PermissionsExt::from_mode(0o755))
                .unwrap();
            p
        };
        let ok = write(
            "trivy",
            r#"case "$1" in version) echo '{"Version":"0.58.1","VulnerabilityDB":{"UpdatedAt":"2026-11-10T06:12:00Z"}}';;
               image) [ "$DOCKER_HOST" = "unix:///run/prueba.sock" ] || exit 9
                      echo '{"Results":[{"Vulnerabilities":[{"VulnerabilityID":"CVE-1","PkgName":"p","InstalledVersion":"1","Severity":"HIGH"}]}]}';;
               esac"#,
        );
        let tool = inspect_tool("trivy", &ok).await;
        assert!(tool.db_ready, "{tool:?}");
        assert_eq!(tool.db_updated.as_deref(), Some("2026-11-10T06:12:00Z"));
        let log = ActivityLog::new(100);
        let c = CancellationToken::new();
        match scan_image(&log, &tool, "nginx:1.27", "unix:///run/prueba.sock", &c).await {
            ImageVulns::Done {
                counts, db_updated, ..
            } => {
                assert_eq!(counts.high, 1);
                assert_eq!(db_updated.as_deref(), Some("2026-11-10T06:12:00Z"));
            }
            other => panic!("{other:?}"),
        }
        let rec = log
            .snapshot()
            .into_iter()
            .find(|r| r.kind == "audit.vulnScan")
            .unwrap();
        assert!(rec.stdout.is_none(), "la salida no se guarda en el panel");

        // Sin base: no se analiza.
        let nodb = write("trivy2", r#"echo '{"Version":"0.58.1"}'"#);
        let t2 = inspect_tool("trivy", &nodb).await;
        assert!(!t2.db_ready);
        assert!(matches!(
            scan_image(&log, &t2, "nginx", "unix:///x", &c).await,
            ImageVulns::NotAnalyzed { reason, .. } if reason == "audit.vuln.noDatabase"
        ));
        // Falla a mitad: «falló», nunca 0.
        let broken = write(
            "trivy3",
            r#"case "$1" in version) echo '{"Version":"1","VulnerabilityDB":{"UpdatedAt":"2026-11-10T00:00:00Z"}}';;
               *) echo 'FATAL: base dañada' >&2; exit 1;; esac"#,
        );
        let t3 = inspect_tool("trivy", &broken).await;
        assert!(matches!(
            scan_image(&log, &t3, "nginx", "unix:///x", &c).await,
            ImageVulns::Failed { message, .. } if message.contains("base dañada")
        ));
        // Salida incompleta (sin «Results»): falló.
        let partial = write(
            "trivy4",
            r#"case "$1" in version) echo '{"Version":"1","VulnerabilityDB":{"UpdatedAt":"2026-11-10T00:00:00Z"}}';;
               *) echo '{"SchemaVersion":2}';; esac"#,
        );
        let t4 = inspect_tool("trivy", &partial).await;
        assert!(matches!(
            scan_image(&log, &t4, "nginx", "unix:///x", &c).await,
            ImageVulns::Failed { .. }
        ));
        // Cancelar detiene el análisis.
        let slow = write(
            "trivy5",
            r#"case "$1" in version) echo '{"Version":"1","VulnerabilityDB":{"UpdatedAt":"2026-11-10T00:00:00Z"}}';;
               *) sleep 30;; esac"#,
        );
        let t5 = inspect_tool("trivy", &slow).await;
        let c2 = CancellationToken::new();
        let c3 = c2.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            c3.cancel();
        });
        let start = std::time::Instant::now();
        assert!(matches!(
            scan_image(&log, &t5, "nginx", "unix:///x", &c2).await,
            ImageVulns::Failed { .. }
        ));
        assert!(start.elapsed() < Duration::from_secs(10));
        // Una referencia que parece una opción no se pasa a la herramienta.
        assert!(matches!(
            scan_image(&log, &tool, "--help", "unix:///x", &c).await,
            ImageVulns::NotAnalyzed { reason, .. } if reason == "audit.vuln.badImage"
        ));

        // Grype: versión, base y análisis.
        let grype = write(
            "grype",
            r#"case "$1" in version) echo '{"version":"0.86.1"}';;
               db) echo '{"built":"2026-11-09T04:00:00Z","valid":true}';;
               *) [ "$GRYPE_DB_AUTO_UPDATE" = "false" ] || exit 7
                  echo '{"matches":[]}';; esac"#,
        );
        let g = inspect_tool("grype", &grype).await;
        assert!(g.db_ready, "{g:?}");
        match scan_image(&log, &g, "alpine:3", "unix:///x", &c).await {
            ImageVulns::Done { counts, .. } => assert_eq!(counts.total(), 0),
            other => panic!("{other:?}"),
        }
    }
}
