// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Resolución de los sockets candidatos, en el mismo orden de prioridad que el
//! CLI de Docker: `DOCKER_HOST`, contexto activo, rootless y socket del sistema.
//!
//! Todo es puro salvo `HostEnv::from_process`, para poder probarlo sin tocar el
//! entorno real.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::model::{ConnectionMode, Endpoint, EndpointSource};

pub const SYSTEM_SOCKET: &str = "/var/run/docker.sock";

/// Tuberías con nombre de Docker Desktop en Windows (1.9.0, D-114), en orden:
/// la general (lleva al motor activo) y la del motor Linux.
#[cfg(windows)]
pub const WINDOWS_PIPES: [&str; 2] = [
    r"\\.\pipe\docker_engine",
    r"\\.\pipe\dockerDesktopLinuxEngine",
];

/// Entorno relevante para localizar Docker.
#[derive(Debug, Clone, Default)]
pub struct HostEnv {
    pub docker_host: Option<String>,
    pub docker_context: Option<String>,
    /// Directorio de configuración del CLI (`$DOCKER_CONFIG` o `~/.docker`).
    pub docker_config_dir: Option<PathBuf>,
    pub xdg_runtime_dir: Option<PathBuf>,
    pub uid: u32,
}

impl HostEnv {
    pub fn from_process() -> Self {
        let nonempty = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
        Self {
            docker_host: nonempty("DOCKER_HOST"),
            docker_context: nonempty("DOCKER_CONTEXT"),
            docker_config_dir: nonempty("DOCKER_CONFIG")
                .map(PathBuf::from)
                .or_else(|| dirs::home_dir().map(|h| h.join(".docker"))),
            xdg_runtime_dir: nonempty("XDG_RUNTIME_DIR").map(PathBuf::from),
            uid: crate::sys::uid(),
        }
    }
}

/// Resultado de interpretar un host de Docker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedHost {
    Unix(String),
    Unsupported(String),
    Invalid(String),
}

/// Interpreta `unix:///ruta`, una ruta absoluta, o detecta transportes no soportados.
pub fn parse_host(host: &str) -> ParsedHost {
    let host = host.trim();
    if host.contains('\0') {
        return ParsedHost::Invalid("contiene un carácter nulo".into());
    }
    // Windows: `npipe:////./pipe/docker_engine` → `\\.\pipe\docker_engine`.
    #[cfg(windows)]
    if let Some(rest) = host.strip_prefix("npipe://") {
        let p = rest.replace('/', "\\");
        return if p.starts_with(r"\\.\pipe\") && p.len() > 9 && !p.contains("..") {
            ParsedHost::Unix(p)
        } else {
            ParsedHost::Invalid("no es una tubería con nombre local".into())
        };
    }
    let path = if let Some(rest) = host.strip_prefix("unix://") {
        rest
    } else if host.contains("://") {
        return ParsedHost::Unsupported(host.to_owned());
    } else {
        host
    };
    if path.is_empty() {
        return ParsedHost::Invalid("la ruta está vacía".into());
    }
    if !path.starts_with('/') {
        return ParsedHost::Invalid("la ruta debe ser absoluta".into());
    }
    if path.split('/').any(|c| c == "..") {
        return ParsedHost::Invalid("la ruta no puede contener «..»".into());
    }
    ParsedHost::Unix(path.to_owned())
}

/// Nombre del contexto activo (`DOCKER_CONTEXT` o `currentContext` de config.json).
fn current_context(env: &HostEnv) -> Option<String> {
    if let Some(c) = &env.docker_context {
        return Some(c.clone());
    }
    let dir = env.docker_config_dir.as_ref()?;
    let raw = std::fs::read_to_string(dir.join("config.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    v.get("currentContext")?
        .as_str()
        .map(str::to_owned)
        .filter(|s| !s.is_empty())
}

/// Host de un contexto, leyendo `contexts/meta/<sha256(nombre)>/meta.json`.
pub fn context_host(config_dir: &Path, name: &str) -> Option<String> {
    let hash = Sha256::digest(name.as_bytes());
    let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    let raw = std::fs::read_to_string(
        config_dir
            .join("contexts")
            .join("meta")
            .join(hex)
            .join("meta.json"),
    )
    .ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    v.pointer("/Endpoints/docker/Host")?
        .as_str()
        .map(str::to_owned)
}

/// Candidatos ordenados por prioridad, sin duplicados.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resolution {
    pub candidates: Vec<Endpoint>,
    /// Host explícito que no se puede usar (tcp, ssh…), para informar al usuario.
    pub unsupported: Option<String>,
    /// Ruta configurada inválida (modo personalizado).
    pub invalid: Option<(String, String)>,
}

pub fn resolve(mode: &ConnectionMode, env: &HostEnv) -> Resolution {
    let mut r = Resolution::default();
    let push = |r: &mut Resolution, path: String, source: EndpointSource| {
        if !r
            .candidates
            .iter()
            .any(|c| same_socket(&c.socket_path, &path))
        {
            r.candidates.push(Endpoint {
                socket_path: path,
                source,
            });
        }
    };

    if let ConnectionMode::Custom { socket_path } = mode {
        match parse_host(socket_path) {
            ParsedHost::Unix(p) => push(&mut r, p, EndpointSource::Custom),
            ParsedHost::Unsupported(h) => r.unsupported = Some(h),
            ParsedHost::Invalid(why) => r.invalid = Some((socket_path.clone(), why)),
        }
        return r;
    }

    if let Some(h) = &env.docker_host {
        match parse_host(h) {
            ParsedHost::Unix(p) => push(&mut r, p, EndpointSource::DockerHost),
            ParsedHost::Unsupported(h) | ParsedHost::Invalid(h) => r.unsupported = Some(h),
        }
    }

    if let (Some(name), Some(dir)) = (current_context(env), env.docker_config_dir.as_ref()) {
        if name != "default" {
            if let Some(host) = context_host(dir, &name) {
                match parse_host(&host) {
                    ParsedHost::Unix(p) => push(&mut r, p, EndpointSource::Context { name }),
                    ParsedHost::Unsupported(h) | ParsedHost::Invalid(h) => {
                        r.unsupported.get_or_insert(h);
                    }
                }
            }
        }
    }

    #[cfg(windows)]
    {
        for pipe in WINDOWS_PIPES {
            push(&mut r, pipe.to_owned(), EndpointSource::System);
        }
        #[allow(clippy::needless_return)]
        return r;
    }
    #[cfg(unix)]
    let runtime = env
        .xdg_runtime_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", env.uid)));
    #[cfg(unix)]
    if env.uid != 0 {
        push(
            &mut r,
            runtime.join("docker.sock").display().to_string(),
            EndpointSource::Rootless,
        );
    }
    #[cfg(unix)]
    push(&mut r, SYSTEM_SOCKET.to_owned(), EndpointSource::System);
    #[cfg(unix)]
    r
}

/// `/var/run` suele ser un enlace a `/run`; ambas rutas son el mismo socket.
fn same_socket(a: &str, b: &str) -> bool {
    let norm = |p: &str| {
        p.strip_prefix("/var")
            .filter(|r| r.starts_with("/run/"))
            .map(str::to_owned)
            .unwrap_or_else(|| p.to_owned())
    };
    norm(a) == norm(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> HostEnv {
        HostEnv {
            uid: 1000,
            xdg_runtime_dir: Some("/run/user/1000".into()),
            ..Default::default()
        }
    }

    fn paths(r: &Resolution) -> Vec<(&str, &EndpointSource)> {
        r.candidates
            .iter()
            .map(|c| (c.socket_path.as_str(), &c.source))
            .collect()
    }

    #[test]
    fn parse_host_variants() {
        assert_eq!(
            parse_host("unix:///run/docker.sock"),
            ParsedHost::Unix("/run/docker.sock".into())
        );
        assert_eq!(
            parse_host(" /tmp/x.sock "),
            ParsedHost::Unix("/tmp/x.sock".into())
        );
        assert!(matches!(
            parse_host("tcp://10.0.0.1:2375"),
            ParsedHost::Unsupported(_)
        ));
        assert!(matches!(
            parse_host("ssh://user@host"),
            ParsedHost::Unsupported(_)
        ));
        assert!(matches!(
            parse_host("relative.sock"),
            ParsedHost::Invalid(_)
        ));
        assert!(matches!(parse_host("unix://"), ParsedHost::Invalid(_)));
        assert!(matches!(parse_host("/a/../b.sock"), ParsedHost::Invalid(_)));
        assert!(matches!(parse_host("/a\0b"), ParsedHost::Invalid(_)));
    }

    #[test]
    fn default_order_is_rootless_then_system() {
        let r = resolve(&ConnectionMode::Auto, &env());
        assert_eq!(
            paths(&r),
            vec![
                ("/run/user/1000/docker.sock", &EndpointSource::Rootless),
                (SYSTEM_SOCKET, &EndpointSource::System)
            ]
        );
    }

    #[test]
    fn root_user_has_no_rootless_candidate() {
        let r = resolve(
            &ConnectionMode::Auto,
            &HostEnv {
                uid: 0,
                ..Default::default()
            },
        );
        assert_eq!(paths(&r), vec![(SYSTEM_SOCKET, &EndpointSource::System)]);
    }

    #[test]
    fn missing_xdg_runtime_dir_falls_back_to_run_user() {
        let r = resolve(
            &ConnectionMode::Auto,
            &HostEnv {
                uid: 1234,
                ..Default::default()
            },
        );
        assert_eq!(r.candidates[0].socket_path, "/run/user/1234/docker.sock");
    }

    #[test]
    fn docker_host_goes_first() {
        let e = HostEnv {
            docker_host: Some("unix:///srv/d.sock".into()),
            ..env()
        };
        let r = resolve(&ConnectionMode::Auto, &e);
        assert_eq!(r.candidates[0].socket_path, "/srv/d.sock");
        assert_eq!(r.candidates[0].source, EndpointSource::DockerHost);
        assert_eq!(r.candidates.len(), 3);
    }

    #[test]
    fn tcp_docker_host_is_reported_but_others_still_tried() {
        let e = HostEnv {
            docker_host: Some("tcp://1.2.3.4:2376".into()),
            ..env()
        };
        let r = resolve(&ConnectionMode::Auto, &e);
        assert_eq!(r.unsupported.as_deref(), Some("tcp://1.2.3.4:2376"));
        assert_eq!(r.candidates.len(), 2);
    }

    #[test]
    fn duplicates_are_removed_including_var_run_alias() {
        let e = HostEnv {
            docker_host: Some("unix:///run/docker.sock".into()),
            ..env()
        };
        let r = resolve(&ConnectionMode::Auto, &e);
        assert_eq!(
            r.candidates
                .iter()
                .filter(|c| c.socket_path.ends_with("/run/docker.sock"))
                .count(),
            1
        );
    }

    #[test]
    fn context_from_config_json_is_used() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.json"),
            r#"{"currentContext":"rootless"}"#,
        )
        .unwrap();
        let hash: String = Sha256::digest(b"rootless")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let meta = dir.path().join("contexts/meta").join(hash);
        std::fs::create_dir_all(&meta).unwrap();
        std::fs::write(
            meta.join("meta.json"),
            r#"{"Name":"rootless","Endpoints":{"docker":{"Host":"unix:///run/user/1000/docker.sock"}}}"#,
        )
        .unwrap();
        let e = HostEnv {
            docker_config_dir: Some(dir.path().into()),
            ..env()
        };
        let r = resolve(&ConnectionMode::Auto, &e);
        assert_eq!(
            r.candidates[0].source,
            EndpointSource::Context {
                name: "rootless".into()
            }
        );
        // El rootless por defecto coincide con el del contexto: no se duplica.
        assert_eq!(r.candidates.len(), 2);
    }

    #[test]
    fn docker_context_env_overrides_config_and_default_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("config.json"),
            r#"{"currentContext":"remoto"}"#,
        )
        .unwrap();
        let e = HostEnv {
            docker_config_dir: Some(dir.path().into()),
            docker_context: Some("default".into()),
            ..env()
        };
        let r = resolve(&ConnectionMode::Auto, &e);
        assert!(r
            .candidates
            .iter()
            .all(|c| !matches!(c.source, EndpointSource::Context { .. })));
    }

    #[test]
    fn custom_mode_uses_only_the_given_path() {
        let r = resolve(
            &ConnectionMode::Custom {
                socket_path: "unix:///opt/d.sock".into(),
            },
            &env(),
        );
        assert_eq!(paths(&r), vec![("/opt/d.sock", &EndpointSource::Custom)]);
        let r = resolve(
            &ConnectionMode::Custom {
                socket_path: "d.sock".into(),
            },
            &env(),
        );
        assert!(r.candidates.is_empty());
        assert!(r.invalid.is_some());
    }
}
