//! Credenciales de registro para descargar imágenes (y las imágenes base de una
//! construcción).
//!
//! La aplicación **no guarda credenciales**. Hay dos orígenes posibles:
//!
//! * La configuración de Docker del usuario (`$DOCKER_CONFIG/config.json` o
//!   `~/.docker/config.json`), que es lo que usa `docker pull`: un *credential
//!   helper* (`credHelpers`/`credsStore`, ejecutado sin shell) o la entrada
//!   `auths` que escribió `docker login`.
//! * Usuario y contraseña escritos para una sola descarga, que solo viven en
//!   memoria mientras dura la petición.
//!
//! Las credenciales viajan a Docker en la cabecera `X-Registry-Auth` y no se
//! escriben nunca en el registro de actividad.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine as _;
use bollard::auth::DockerCredentials;
use dcc_activity::{ActivityLog, Category, Failure, OperationSpec, Outcome, Transport};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use ts_rs::TS;

use crate::reference::DOCKER_HUB_AUTH_KEY;

const HELPER_TIMEOUT: Duration = Duration::from_secs(20);

/// Cómo obtener credenciales para una descarga (lo elige el usuario).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(
    tag = "mode",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum PullAuth {
    /// Como `docker pull`: credenciales de la configuración de Docker, si las hay.
    #[default]
    DockerConfig,
    /// Sin credenciales (descarga anónima).
    Anonymous,
    /// Usuario y contraseña (o token) solo para esta operación; no se guardan.
    Manual { username: String, password: String },
}

impl std::fmt::Debug for PullAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PullAuth({self})")
    }
}

impl std::fmt::Display for PullAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Nunca muestra la contraseña.
        match self {
            Self::DockerConfig => f.write_str("configuración de Docker"),
            Self::Anonymous => f.write_str("anónima"),
            Self::Manual { username, .. } => {
                write!(f, "usuario {username} (solo para esta operación)")
            }
        }
    }
}

/// De dónde salieron las credenciales usadas (sin el secreto).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthSource {
    None,
    ConfigFile(PathBuf),
    Helper(String),
    Manual,
}

impl AuthSource {
    pub fn describe(&self) -> String {
        match self {
            Self::None => "sin credenciales".into(),
            Self::ConfigFile(p) => format!("credenciales de {}", p.display()),
            Self::Helper(h) => format!("credenciales de docker-credential-{h}"),
            Self::Manual => "credenciales introducidas para esta operación".into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedAuth {
    pub credentials: Option<DockerCredentials>,
    pub source: AuthSource,
    /// Problema no fatal (p. ej. helper no instalado): se descarga sin credenciales.
    pub warning: Option<String>,
}

impl ResolvedAuth {
    fn none() -> Self {
        Self {
            credentials: None,
            source: AuthSource::None,
            warning: None,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    auths: HashMap<String, AuthEntry>,
    #[serde(rename = "credsStore")]
    creds_store: Option<String>,
    #[serde(rename = "credHelpers", default)]
    cred_helpers: HashMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
struct AuthEntry {
    auth: Option<String>,
    username: Option<String>,
    password: Option<String>,
    identitytoken: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HelperReply {
    #[serde(rename = "Username")]
    username: String,
    #[serde(rename = "Secret")]
    secret: String,
}

/// Ruta de la configuración de Docker del usuario.
pub fn docker_config_path() -> Option<PathBuf> {
    match std::env::var_os("DOCKER_CONFIG") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d).join("config.json")),
        _ => dirs::home_dir().map(|h| h.join(".docker").join("config.json")),
    }
}

/// Nombre de host de una clave de `auths` (`https://index.docker.io/v1/` → `index.docker.io`).
fn hostname_of(key: &str) -> String {
    let k = key
        .strip_prefix("https://")
        .or_else(|| key.strip_prefix("http://"))
        .unwrap_or(key);
    k.split('/').next().unwrap_or(k).to_owned()
}

/// Claves equivalentes con las que pueden estar guardadas las credenciales.
fn candidate_keys(auth_key: &str) -> Vec<String> {
    if auth_key == DOCKER_HUB_AUTH_KEY {
        vec![
            DOCKER_HUB_AUTH_KEY.into(),
            "index.docker.io".into(),
            "docker.io".into(),
            "https://index.docker.io/v1".into(),
            "registry-1.docker.io".into(),
        ]
    } else {
        vec![
            auth_key.into(),
            format!("https://{auth_key}"),
            format!("http://{auth_key}"),
            format!("https://{auth_key}/v1/"),
            format!("https://{auth_key}/v2/"),
        ]
    }
}

fn from_entry(e: &AuthEntry, server: &str) -> Option<DockerCredentials> {
    if let Some(tok) = e.identitytoken.as_ref().filter(|t| !t.is_empty()) {
        return Some(DockerCredentials {
            identitytoken: Some(tok.clone()),
            serveraddress: Some(server.to_owned()),
            ..Default::default()
        });
    }
    let (user, pass) = match (&e.username, &e.password, &e.auth) {
        (Some(u), Some(p), _) if !u.is_empty() => (u.clone(), p.clone()),
        (_, _, Some(a)) if !a.is_empty() => {
            let raw = base64::engine::general_purpose::STANDARD
                .decode(a.trim())
                .ok()?;
            let s = String::from_utf8(raw).ok()?;
            let (u, p) = s.split_once(':')?;
            (u.to_owned(), p.to_owned())
        }
        _ => return None,
    };
    Some(DockerCredentials {
        username: Some(user),
        password: Some(pass),
        serveraddress: Some(server.to_owned()),
        ..Default::default()
    })
}

/// Resuelve las credenciales según el modo elegido. Si hay que ejecutar un
/// *credential helper*, la ejecución queda registrada (sin su salida).
pub async fn resolve(log: &ActivityLog, auth_key: &str, mode: &PullAuth) -> ResolvedAuth {
    match mode {
        PullAuth::Anonymous => ResolvedAuth::none(),
        PullAuth::Manual { username, password } => ResolvedAuth {
            credentials: Some(DockerCredentials {
                username: Some(username.clone()),
                password: Some(password.clone()),
                serveraddress: Some(auth_key.to_owned()),
                ..Default::default()
            }),
            source: AuthSource::Manual,
            warning: None,
        },
        PullAuth::DockerConfig => match docker_config_path() {
            Some(p) => resolve_from_config(log, &p, auth_key).await,
            None => ResolvedAuth::none(),
        },
    }
}

pub async fn resolve_from_config(log: &ActivityLog, path: &Path, auth_key: &str) -> ResolvedAuth {
    let cfg: ConfigFile = match std::fs::read_to_string(path) {
        Ok(s) => match serde_json::from_str(&s) {
            Ok(c) => c,
            Err(e) => {
                return ResolvedAuth {
                    warning: Some(format!("no se pudo leer {}: {e}", path.display())),
                    ..ResolvedAuth::none()
                }
            }
        },
        Err(_) => return ResolvedAuth::none(),
    };
    let keys = candidate_keys(auth_key);
    let host = hostname_of(auth_key);
    let helper = keys
        .iter()
        .chain(std::iter::once(&host))
        .find_map(|k| cfg.cred_helpers.get(k))
        .or(cfg.creds_store.as_ref())
        .filter(|h| !h.is_empty());
    if let Some(h) = helper {
        // El servidor que espera el helper es la clave de `docker login`.
        let server = if auth_key == DOCKER_HUB_AUTH_KEY {
            DOCKER_HUB_AUTH_KEY.to_owned()
        } else {
            host.clone()
        };
        return match run_helper(log, h, &server).await {
            Ok(Some(c)) => ResolvedAuth {
                credentials: Some(c),
                source: AuthSource::Helper(h.clone()),
                warning: None,
            },
            Ok(None) => ResolvedAuth::none(),
            Err(w) => ResolvedAuth {
                warning: Some(w),
                ..ResolvedAuth::none()
            },
        };
    }
    for k in &keys {
        if let Some(c) = cfg.auths.get(k).and_then(|e| from_entry(e, k)) {
            return ResolvedAuth {
                credentials: Some(c),
                source: AuthSource::ConfigFile(path.to_owned()),
                warning: None,
            };
        }
    }
    ResolvedAuth::none()
}

/// `docker-credential-<helper> get` con el servidor por la entrada estándar.
/// Se registra la ejecución, pero nunca su salida (contiene el secreto).
async fn run_helper(
    log: &ActivityLog,
    helper: &str,
    server: &str,
) -> Result<Option<DockerCredentials>, String> {
    let name = format!("docker-credential-{helper}");
    if !helper
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
    {
        return Err(format!("nombre de credential helper no válido: {helper}"));
    }
    let Some(program) = crate::sys::find_executable(&name) else {
        return Err(format!(
            "{name} no está instalado; se descarga sin credenciales"
        ));
    };
    let program = program.to_string_lossy().into_owned();
    let handle = log.begin(OperationSpec {
        category: Category::Image,
        kind: "registry.credentials".into(),
        target: Some(server.to_owned()),
        transport: Transport::Cli {
            program: program.clone(),
            args: vec!["get".into()],
        },
        request_body: Some(format!("{server} (entrada estándar)")),
        equivalent_command: None,
    });
    let result = async {
        let mut child = Command::new(&program)
            .arg("get")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("no se pudo ejecutar {name}: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(server.as_bytes())
                .await
                .map_err(|e| e.to_string())?;
        }
        let out = tokio::time::timeout(HELPER_TIMEOUT, child.wait_with_output())
            .await
            .map_err(|_| format!("{name} no respondió en {} s", HELPER_TIMEOUT.as_secs()))?
            .map_err(|e| e.to_string())?;
        Ok::<_, String>(out)
    }
    .await;
    match result {
        Ok(out) if out.status.success() => match serde_json::from_slice::<HelperReply>(&out.stdout)
        {
            Ok(r) => {
                handle.succeed(Outcome {
                    summary: Some(format!("Credenciales encontradas para {server}")),
                    exit_code: out.status.code(),
                    ..Default::default()
                });
                let c = if r.username == "<token>" {
                    DockerCredentials {
                        identitytoken: Some(r.secret),
                        serveraddress: Some(server.to_owned()),
                        ..Default::default()
                    }
                } else {
                    DockerCredentials {
                        username: Some(r.username),
                        password: Some(r.secret),
                        serveraddress: Some(server.to_owned()),
                        ..Default::default()
                    }
                };
                Ok(Some(c))
            }
            Err(e) => {
                let msg = format!("respuesta de {name} no reconocida: {e}");
                handle.fail(Failure {
                    error: msg.clone(),
                    exit_code: out.status.code(),
                    ..Default::default()
                });
                Err(msg)
            }
        },
        Ok(out) => {
            // Los helpers responden "credentials not found…" cuando no hay entrada.
            let err = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            let err = if err.is_empty() {
                String::from_utf8_lossy(&out.stderr).trim().to_owned()
            } else {
                err
            };
            if err.to_lowercase().contains("credentials not found") {
                handle.succeed(Outcome {
                    summary: Some(format!("No hay credenciales guardadas para {server}")),
                    exit_code: out.status.code(),
                    ..Default::default()
                });
                Ok(None)
            } else {
                let msg = format!("{name} falló: {err}");
                handle.fail(Failure {
                    error: msg.clone(),
                    exit_code: out.status.code(),
                    ..Default::default()
                });
                Err(msg)
            }
        }
        Err(e) => {
            handle.fail(Failure {
                error: e.clone(),
                ..Default::default()
            });
            Err(e)
        }
    }
}

/// Valor de la cabecera `X-Registry-Config` de `/build`: mapa registro → credenciales.
pub fn registry_config_header(creds: &HashMap<String, DockerCredentials>) -> String {
    base64::engine::general_purpose::URL_SAFE.encode(serde_json::to_vec(creds).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn write(dir: &Path, name: &str, s: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, s).unwrap();
        p
    }

    #[tokio::test]
    async fn auths_entry_is_decoded_for_hub_and_private_registries() {
        let d = tempfile::tempdir().unwrap();
        let auth = base64::engine::general_purpose::STANDARD.encode("ana:s3creto");
        let p = write(
            d.path(),
            "config.json",
            &format!(
                r#"{{"auths":{{"https://index.docker.io/v1/":{{"auth":"{auth}"}},"reg.example.com:5000":{{"identitytoken":"tok"}}}}}}"#
            ),
        );
        let log = ActivityLog::new(10);
        let r = resolve_from_config(&log, &p, DOCKER_HUB_AUTH_KEY).await;
        let c = r.credentials.unwrap();
        assert_eq!(
            (c.username.as_deref(), c.password.as_deref()),
            (Some("ana"), Some("s3creto"))
        );
        assert_eq!(r.source, AuthSource::ConfigFile(p.clone()));
        let r = resolve_from_config(&log, &p, "reg.example.com:5000").await;
        assert_eq!(r.credentials.unwrap().identitytoken.as_deref(), Some("tok"));
        let r = resolve_from_config(&log, &p, "otro.io").await;
        assert!(r.credentials.is_none());
        // Leer el archivo no es una operación de Docker: no se registra nada.
        assert!(log.snapshot().is_empty());
    }

    #[tokio::test]
    async fn missing_or_broken_config_means_anonymous() {
        let d = tempfile::tempdir().unwrap();
        let log = ActivityLog::new(10);
        let r = resolve_from_config(&log, &d.path().join("no.json"), "x.io").await;
        assert!(r.credentials.is_none() && r.warning.is_none());
        let p = write(d.path(), "config.json", "{roto");
        let r = resolve_from_config(&log, &p, "x.io").await;
        assert!(r.credentials.is_none() && r.warning.is_some());
    }

    /// Helper falso en el PATH: recibe el servidor por stdin y responde como los reales.
    #[tokio::test]
    async fn credential_helper_is_run_without_shell_and_its_output_is_never_recorded() {
        let d = tempfile::tempdir().unwrap();
        let bin = d.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let script = bin.join("docker-credential-dccfake");
        std::fs::write(
            &script,
            "#!/bin/sh\nread srv\nif [ \"$srv\" = \"reg.example.com\" ]; then echo '{\"ServerURL\":\"reg.example.com\",\"Username\":\"ana\",\"Secret\":\"clave-secreta\"}'; else echo 'credentials not found in native keychain'; exit 1; fi\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let old = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var(
            "PATH",
            format!("{}:{}", bin.display(), old.to_string_lossy()),
        );
        let p = write(
            d.path(),
            "config.json",
            r#"{"credHelpers":{"reg.example.com":"dccfake"},"credsStore":"dccfake"}"#,
        );
        let log = ActivityLog::new(10);
        let r = resolve_from_config(&log, &p, "reg.example.com").await;
        assert_eq!(r.source, AuthSource::Helper("dccfake".into()));
        assert_eq!(
            r.credentials.unwrap().password.as_deref(),
            Some("clave-secreta")
        );
        let r = resolve_from_config(&log, &p, "otro.io").await;
        assert!(r.credentials.is_none() && r.warning.is_none(), "{r:?}");
        let recs = log.snapshot();
        assert_eq!(recs.len(), 2);
        for rec in &recs {
            let all = format!("{rec:?}");
            assert!(!all.contains("clave-secreta"), "{all}");
            assert!(
                matches!(&rec.transport, Transport::Cli { args, .. } if args == &vec!["get".to_string()])
            );
        }
        // Helper inexistente: aviso y descarga anónima.
        let p = write(d.path(), "config.json", r#"{"credsStore":"noexiste"}"#);
        let r = resolve_from_config(&log, &p, "reg.example.com").await;
        assert!(r.credentials.is_none());
        assert!(r.warning.unwrap().contains("no está instalado"));
        std::env::set_var("PATH", old);
    }

    #[test]
    fn display_never_shows_the_password() {
        let m = PullAuth::Manual {
            username: "ana".into(),
            password: "pw".into(),
        };
        assert!(!m.to_string().contains("pw"));
        assert!(!format!("{m:?}").contains("pw"));
        assert!(!format!("{:?}", AuthSource::Manual.describe()).contains("pw"));
    }
}
