// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Detección de Docker Compose (D-032).
//!
//! Se admite Compose **v2 o posterior**, como plugin del CLI (`docker compose`)
//! o como binario independiente (`docker-compose` v2). Compose v1 (Python,
//! `docker-compose 1.x`) no se admite.
//!
//! Solo se ejecuta `version` **sin opciones**: `--short` y `--format` no sirven
//! para detectar, porque si el plugin no está instalado el CLI de Docker
//! interpreta esas opciones como suyas y responde "unknown flag" (salida 125),
//! que es justo el error observado en Debian 13.

use std::path::PathBuf;
use std::time::Duration;

use dcc_activity::{ActivityLog, Category};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::cli::{self, CliOutput, CliRequest};

pub mod file;
pub mod ops;
pub mod project;
pub mod remote_file;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ComposeKind {
    /// `docker compose …`
    Plugin,
    /// `docker-compose …` (binario independiente de Compose v2)
    Standalone,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ComposeStatus {
    /// Compose v2+ disponible.
    Available {
        version: String,
        kind: ComposeKind,
        program: String,
    },
    /// No está instalado: ni plugin en los directorios del CLI ni `docker-compose`.
    NotInstalled {
        cli_path: Option<String>,
        searched: Vec<String>,
    },
    /// Existe el archivo del plugin, pero el CLI no consigue usarlo.
    PluginUnusable {
        plugin_path: String,
        message: String,
    },
    /// Solo hay Compose v1, que no se admite.
    Legacy { version: String, program: String },
    /// Respondió, pero la salida no se reconoce como Compose.
    Unrecognized { program: String, output: String },
    /// Servidor remoto: Compose se ejecuta en el propio servidor (0.10.0, D-080).
    RemotePending,
    /// Error al ejecutar el programa.
    Error { message: String },
}

impl ComposeStatus {
    pub fn remote_pending() -> Self {
        ComposeStatus::RemotePending
    }
}

/// Resultado de interpretar `… version`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionOutput {
    V2(String),
    V1(String),
    /// El CLI de Docker no conoce el subcomando `compose` (plugin no encontrado).
    PluginNotFound,
    Other(String),
}

/// Interpreta la salida de `docker compose version` o `docker-compose version`.
pub fn parse_version(out: &CliOutput) -> VersionOutput {
    let all = format!("{}\n{}", out.stdout, out.stderr);
    if out.success() {
        for line in out.stdout.lines() {
            let l = line.trim();
            // "Docker Compose version v2.29.1", "Docker Compose version 2.26.1+ds1-4"
            if let Some(rest) = l.strip_prefix("Docker Compose version ") {
                let v = rest.trim().trim_start_matches('v');
                if v.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                    return VersionOutput::V2(v.split_whitespace().next().unwrap_or(v).to_owned());
                }
            }
            // "docker-compose version 1.29.2, build unknown"
            if let Some(rest) = l.strip_prefix("docker-compose version ") {
                let v = rest.split([',', ' ']).next().unwrap_or("").to_owned();
                return if v.starts_with('1') {
                    VersionOutput::V1(v)
                } else {
                    VersionOutput::V2(v)
                };
            }
        }
        return VersionOutput::Other(all.trim().to_owned());
    }
    // Mensajes del CLI de Docker cuando no encuentra el plugin (versiones nuevas y antiguas).
    let lower = all.to_lowercase();
    if lower.contains("unknown command: docker compose")
        || lower.contains("'compose' is not a docker command")
        || lower.contains("unknown command \"compose\"")
    {
        return VersionOutput::PluginNotFound;
    }
    VersionOutput::Other(all.trim().to_owned())
}

/// Directorios donde el CLI de Docker busca plugins, en su orden.
pub fn plugin_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(cfg) = std::env::var_os("DOCKER_CONFIG") {
        v.push(PathBuf::from(cfg).join("cli-plugins"));
    } else if let Some(home) = dirs::home_dir() {
        v.push(home.join(".docker/cli-plugins"));
    }
    for d in [
        "/usr/local/lib/docker/cli-plugins",
        "/usr/local/libexec/docker/cli-plugins",
        "/usr/lib/docker/cli-plugins",
        "/usr/libexec/docker/cli-plugins",
    ] {
        v.push(PathBuf::from(d));
    }
    v
}

/// Variables del proceso remoto según a qué Docker del servidor se conecta
/// el perfil (0.10.0): las mismas que usaría el CLI con `--host`/`--context`.
pub fn remote_env(target: &crate::ssh::RemoteDocker) -> Vec<(String, String)> {
    match target {
        crate::ssh::RemoteDocker::Default => vec![],
        crate::ssh::RemoteDocker::Socket { path } => {
            vec![("DOCKER_HOST".into(), format!("unix://{path}"))]
        }
        crate::ssh::RemoteDocker::Context { name } => vec![("DOCKER_CONTEXT".into(), name.clone())],
    }
}

/// Detecta Compose **en el servidor** por su sesión SSH (0.10.0, D-080):
/// `docker compose version` y, si no, `docker-compose version`. Nada se
/// instala ni se cambia; cada ejecución queda registrada.
pub async fn detect_remote(
    log: &ActivityLog,
    session: std::sync::Arc<crate::ssh::SshSession>,
) -> ComposeStatus {
    let env = remote_env(&session.profile().docker);
    let req = |program: &str, args: &[&str]| {
        let mut r = request(program, args, &env);
        r.timeout = Duration::from_secs(30);
        r.remote = Some(cli::RemoteExec {
            session: session.clone(),
            tty: false,
        });
        r
    };
    match cli::run(log, req("docker", &["compose", "version"])).await {
        Ok(out) => match parse_version(&out) {
            VersionOutput::V2(version) => {
                return ComposeStatus::Available {
                    version,
                    kind: ComposeKind::Plugin,
                    program: "docker".into(),
                }
            }
            VersionOutput::V1(version) => {
                return ComposeStatus::Legacy {
                    version,
                    program: "docker".into(),
                }
            }
            VersionOutput::Other(output) if out.success() => {
                return ComposeStatus::Unrecognized {
                    program: "docker".into(),
                    output,
                }
            }
            _ => {}
        },
        Err(e) => {
            return ComposeStatus::Error {
                message: e.to_string(),
            }
        }
    }
    match cli::run(log, req("docker-compose", &["version"])).await {
        // 127: el shell del servidor no encuentra el programa.
        Ok(out) if out.exit_code == Some(127) => ComposeStatus::NotInstalled {
            cli_path: None,
            searched: vec!["docker compose".into(), "docker-compose".into()],
        },
        Ok(out) => match parse_version(&out) {
            VersionOutput::V2(version) => ComposeStatus::Available {
                version,
                kind: ComposeKind::Standalone,
                program: "docker-compose".into(),
            },
            VersionOutput::V1(version) => ComposeStatus::Legacy {
                version,
                program: "docker-compose".into(),
            },
            _ => ComposeStatus::Unrecognized {
                program: "docker-compose".into(),
                output: format!("{}{}", out.stdout, out.stderr).trim().to_owned(),
            },
        },
        Err(e) => ComposeStatus::Error {
            message: e.to_string(),
        },
    }
}

fn request(program: &str, args: &[&str], env: &[(String, String)]) -> CliRequest {
    CliRequest {
        category: Category::Compose,
        kind: "compose.version".into(),
        target: None,
        program: program.to_owned(),
        args: args.iter().map(|s| s.to_string()).collect(),
        env: env.to_vec(),
        cwd: None,
        timeout: Duration::from_secs(15),
        remote: None,
    }
}

/// Detecta Compose ejecutando el CLI real. Cada ejecución queda registrada.
pub async fn detect(
    log: &ActivityLog,
    docker_cli: Option<String>,
    standalone: Option<String>,
    env: &[(String, String)],
) -> ComposeStatus {
    let dirs = plugin_dirs();
    let plugin_file = dirs
        .iter()
        .map(|d| d.join("docker-compose"))
        .find(|p| p.is_file());

    if let Some(cli) = &docker_cli {
        match cli::run(log, request(cli, &["compose", "version"], env)).await {
            Ok(out) => match parse_version(&out) {
                VersionOutput::V2(version) => {
                    return ComposeStatus::Available {
                        version,
                        kind: ComposeKind::Plugin,
                        program: cli.clone(),
                    }
                }
                VersionOutput::V1(version) => {
                    return ComposeStatus::Legacy {
                        version,
                        program: cli.clone(),
                    }
                }
                VersionOutput::PluginNotFound => {
                    if let Some(p) = &plugin_file {
                        return ComposeStatus::PluginUnusable {
                            plugin_path: p.display().to_string(),
                            message: out.stderr.trim().to_owned(),
                        };
                    }
                }
                VersionOutput::Other(output) => {
                    if out.success() {
                        return ComposeStatus::Unrecognized {
                            program: cli.clone(),
                            output,
                        };
                    }
                    if plugin_file.is_none() && standalone.is_none() {
                        return ComposeStatus::Error { message: output };
                    }
                }
            },
            Err(e) => {
                return ComposeStatus::Error {
                    message: e.to_string(),
                }
            }
        }
    }

    if let Some(sa) = &standalone {
        match cli::run(log, request(sa, &["version"], env)).await {
            Ok(out) => match parse_version(&out) {
                VersionOutput::V2(version) => {
                    return ComposeStatus::Available {
                        version,
                        kind: ComposeKind::Standalone,
                        program: sa.clone(),
                    }
                }
                VersionOutput::V1(version) => {
                    return ComposeStatus::Legacy {
                        version,
                        program: sa.clone(),
                    }
                }
                VersionOutput::PluginNotFound | VersionOutput::Other(_) => {
                    return ComposeStatus::Unrecognized {
                        program: sa.clone(),
                        output: format!("{}{}", out.stdout, out.stderr).trim().to_owned(),
                    };
                }
            },
            Err(e) => {
                return ComposeStatus::Error {
                    message: e.to_string(),
                }
            }
        }
    }

    ComposeStatus::NotInstalled {
        cli_path: docker_cli,
        searched: dirs
            .iter()
            .map(|d| d.join("docker-compose").display().to_string())
            .chain(std::iter::once("docker-compose (PATH)".to_owned()))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(code: i32, stdout: &str, stderr: &str) -> CliOutput {
        CliOutput {
            exit_code: Some(code),
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    // Salidas reales observadas (Docker CLI 29.9.2 con y sin plugin, Compose 5.6.1).
    #[test]
    fn plugin_present() {
        assert_eq!(
            parse_version(&out(0, "Docker Compose version v5.6.1\n", "")),
            VersionOutput::V2("5.6.1".into())
        );
    }

    #[test]
    fn debian_style_version() {
        assert_eq!(
            parse_version(&out(0, "Docker Compose version 2.26.1+ds1-4\n", "")),
            VersionOutput::V2("2.26.1+ds1-4".into())
        );
    }

    #[test]
    fn plugin_missing_new_cli() {
        let o = out(
            1,
            "",
            "docker: unknown command: docker compose\n\nRun 'docker --help' for more information\n",
        );
        assert_eq!(parse_version(&o), VersionOutput::PluginNotFound);
    }

    #[test]
    fn plugin_missing_old_cli() {
        let o = out(
            1,
            "",
            "docker: 'compose' is not a docker command.\nSee 'docker --help'\n",
        );
        assert_eq!(parse_version(&o), VersionOutput::PluginNotFound);
    }

    #[test]
    fn unknown_flag_is_not_misread_as_a_version() {
        // Lo que devolvía la versión 0.2.0 al usar --format sin plugin.
        let o = out(
            125,
            "",
            "unknown flag: --format\n\nUsage:  docker [OPTIONS] COMMAND [ARG...]\n",
        );
        assert!(matches!(parse_version(&o), VersionOutput::Other(_)));
    }

    #[test]
    fn compose_v1_is_legacy() {
        assert_eq!(
            parse_version(&out(
                0,
                "docker-compose version 1.29.2, build unknown\n",
                ""
            )),
            VersionOutput::V1("1.29.2".into())
        );
    }

    #[test]
    fn garbage_is_other() {
        assert!(matches!(
            parse_version(&out(0, "hola\n", "")),
            VersionOutput::Other(_)
        ));
    }

    #[tokio::test]
    async fn nothing_installed_reports_searched_locations_without_running_anything() {
        let log = ActivityLog::new(10);
        let s = detect(&log, None, None, &[]).await;
        match s {
            ComposeStatus::NotInstalled { cli_path, searched } => {
                assert!(cli_path.is_none());
                assert!(searched
                    .iter()
                    .any(|p| p.ends_with("/usr/libexec/docker/cli-plugins/docker-compose")));
            }
            other => panic!("{other:?}"),
        }
        assert!(log.is_empty());
    }

    #[tokio::test]
    async fn fake_cli_without_plugin_and_v2_standalone() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let docker = dir.path().join("docker");
        std::fs::write(
            &docker,
            "#!/bin/sh\necho \"docker: unknown command: docker compose\" >&2\nexit 1\n",
        )
        .unwrap();
        let sa = dir.path().join("docker-compose");
        std::fs::write(&sa, "#!/bin/sh\necho 'Docker Compose version v2.26.1'\n").unwrap();
        for p in [&docker, &sa] {
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let log = ActivityLog::new(10);
        let s = detect(
            &log,
            Some(docker.display().to_string()),
            Some(sa.display().to_string()),
            &[],
        )
        .await;
        // Si el sistema tiene el plugin instalado de verdad, gana el diagnóstico del plugin.
        if plugin_dirs()
            .iter()
            .any(|d| d.join("docker-compose").is_file())
        {
            assert!(matches!(s, ComposeStatus::PluginUnusable { .. }), "{s:?}");
        } else {
            assert_eq!(
                s,
                ComposeStatus::Available {
                    version: "2.26.1".into(),
                    kind: ComposeKind::Standalone,
                    program: sa.display().to_string()
                }
            );
            assert_eq!(log.len(), 2, "ambas ejecuciones quedan registradas");
        }
    }
}
