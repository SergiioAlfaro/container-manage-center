//! Identidad del servidor (requisito 4.2, D-076).
//!
//! - La comprobación normal usa `StrictHostKeyChecking=yes` con el `known_hosts`
//!   de la aplicación y el del usuario: una identidad desconocida o cambiada
//!   nunca se acepta en silencio.
//! - Primera conexión: se obtiene la huella que presenta el servidor por la
//!   misma ruta de red que se usará (también a través de un servidor
//!   intermedio) y se muestra para que el usuario la confirme.
//! - Al confirmar, la clave se captura en un archivo temporal y **solo** se
//!   guarda si su huella coincide con la confirmada; si no coincide (la
//!   identidad cambió entre medias), no se guarda nada.
//! - Solo se modifica el `known_hosts` de la aplicación; `~/.ssh/known_hosts`
//!   se lee pero nunca se toca.

use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;
use ts_rs::TS;

use super::args::{identity_probe_args, HostKeyPolicy};
use super::askpass::{private_dir, random_hex};
use super::problem::{classify_ssh, presented_key, RemoteProblem};
use super::profile::SshProfile;
use super::{HostKeyInfo, SshPaths};

/// Patrón de `known_hosts` del servidor: `host` (puerto 22) o `[host]:puerto`.
pub fn host_pattern(p: &SshProfile) -> String {
    if p.port == 22 {
        p.host.clone()
    } else {
        format!("[{}]:{}", p.host, p.port)
    }
}

/// Lanza `ssh` (o `ssh-keygen`) sin shell, sin entrada y sin heredar askpass.
fn command(program: &Path, args: &[String]) -> Command {
    let mut c = Command::new(program);
    c.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("SSH_ASKPASS")
        .env_remove("SSH_ASKPASS_REQUIRE")
        .env_remove(super::askpass::SOCKET_ENV)
        .env("SHELL", "/bin/sh")
        .kill_on_drop(true);
    c
}

pub(crate) struct Run {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub(crate) async fn run(
    program: &Path,
    args: &[String],
    timeout: Duration,
) -> Result<Run, RemoteProblem> {
    let child = command(program, args).spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            RemoteProblem::SshMissing
        } else {
            RemoteProblem::Other {
                message: e.to_string(),
            }
        }
    })?;
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(o)) => Ok(Run {
            code: o.status.code(),
            stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
        }),
        Ok(Err(e)) => Err(RemoteProblem::Other {
            message: e.to_string(),
        }),
        Err(_) => Err(RemoteProblem::Timeout),
    }
}

fn probe_timeout(p: &SshProfile) -> Duration {
    Duration::from_secs(u64::from(p.options.connect_timeout) + 10)
}

/// La autenticación se alcanzó (luego la identidad ya se comprobó y es buena).
fn reached_authentication(r: &Run) -> bool {
    r.code == Some(0)
        || r.stderr.contains("is known and matches")
        || r.stderr.contains("Permission denied (")
        || r.stderr.contains("Authentications that can continue")
}

/// Comprueba la identidad con las reglas normales. Devuelve la huella
/// presentada si es de confianza, o el problema (desconocida, cambiada o de red).
pub async fn check(paths: &SshPaths, p: &SshProfile) -> Result<Option<HostKeyInfo>, RemoteProblem> {
    let args = identity_probe_args(p, paths, &HostKeyPolicy::Strict);
    let r = run(&paths.ssh, &args, probe_timeout(p)).await?;
    if reached_authentication(&r) {
        return Ok(presented_key(&r.stderr));
    }
    Err(classify_ssh(r.code, &r.stderr, &p.host, p.port))
}

/// Huella que presenta el servidor, sin confiar en ella (para mostrarla).
pub async fn presented(paths: &SshPaths, p: &SshProfile) -> Result<HostKeyInfo, RemoteProblem> {
    let args = identity_probe_args(p, paths, &HostKeyPolicy::StrictEmpty);
    let r = run(&paths.ssh, &args, probe_timeout(p)).await?;
    match presented_key(&r.stderr) {
        Some(k) => Ok(k),
        None => Err(classify_ssh(r.code, &r.stderr, &p.host, p.port)),
    }
}

/// Clave de confianza guardada para el servidor y en qué archivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TrustedKey {
    pub key: HostKeyInfo,
    /// En el `known_hosts` de la aplicación (si no, en el del usuario).
    pub in_app_file: bool,
}

/// Huellas guardadas para el servidor (aplicación primero, luego usuario).
pub async fn trusted(paths: &SshPaths, p: &SshProfile) -> Vec<TrustedKey> {
    let mut out = Vec::new();
    for (file, in_app) in [
        (&paths.app_known_hosts, true),
        (&paths.user_known_hosts, false),
    ] {
        if !file.exists() {
            continue;
        }
        let args = vec![
            "-l".to_owned(),
            "-F".into(),
            host_pattern(p),
            "-f".into(),
            file.display().to_string(),
        ];
        if let Ok(r) = run(&paths.ssh_keygen, &args, Duration::from_secs(10)).await {
            out.extend(
                parse_fingerprints(&r.stdout)
                    .into_iter()
                    .map(|key| TrustedKey {
                        key,
                        in_app_file: in_app,
                    }),
            );
        }
    }
    out
}

/// Salida de `ssh-keygen -l`: `256 SHA256:… [host]:port (ED25519)` o
/// `[host]:port ED25519 SHA256:…` (con `-F`).
pub fn parse_fingerprints(out: &str) -> Vec<HostKeyInfo> {
    let mut v = Vec::new();
    for line in out.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let Some(fp) = words.iter().find(|w| w.starts_with("SHA256:")) else {
            continue;
        };
        let ty = words
            .iter()
            .find_map(|w| w.strip_prefix('(').and_then(|r| r.strip_suffix(')')))
            .or_else(|| {
                words
                    .iter()
                    .find(|w| matches!(**w, "ED25519" | "RSA" | "ECDSA" | "DSA"))
                    .copied()
            })
            .unwrap_or("");
        let key_type = match ty {
            "ED25519" => "ssh-ed25519",
            "RSA" => "ssh-rsa",
            "ECDSA" => "ecdsa-sha2",
            "DSA" => "ssh-dss",
            other => other,
        };
        v.push(HostKeyInfo {
            key_type: key_type.to_owned(),
            fingerprint: (*fp).to_owned(),
        });
    }
    v
}

/// Mismas huellas aunque el tipo se escriba distinto (`ecdsa-sha2-nistp256` / `ecdsa-sha2`).
fn same_key(a: &HostKeyInfo, b: &HostKeyInfo) -> bool {
    a.fingerprint == b.fingerprint
}

/// Resultado de confiar en una identidad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustError {
    /// La huella capturada no es la confirmada: no se guarda nada.
    Mismatch {
        presented: Vec<HostKeyInfo>,
    },
    /// La clave anterior está en `~/.ssh/known_hosts`: la aplicación no lo modifica.
    InUserFile {
        file: String,
        pattern: String,
    },
    Problem(RemoteProblem),
    Io(String),
}

/// Guarda la identidad del servidor en el `known_hosts` de la aplicación,
/// solo si la huella que presenta ahora es exactamente `expected`.
/// `replace`: sustituye una clave anterior de la aplicación (identidad cambiada).
pub async fn trust(
    paths: &SshPaths,
    p: &SshProfile,
    expected: &HostKeyInfo,
    replace: bool,
) -> Result<HostKeyInfo, TrustError> {
    let pattern = host_pattern(p);
    // Una clave distinta en el archivo del usuario no se puede sustituir desde aquí.
    if replace {
        let user = trusted(paths, p).await;
        if user
            .iter()
            .any(|t| !t.in_app_file && !same_key(&t.key, expected))
        {
            return Err(TrustError::InUserFile {
                file: paths.user_known_hosts.display().to_string(),
                pattern,
            });
        }
    }
    private_dir(&paths.runtime_dir).map_err(|e| TrustError::Io(e.to_string()))?;
    let capture = paths.runtime_dir.join(format!("capture-{}", random_hex(8)));
    let _cleanup = Cleanup(capture.clone());
    let args = identity_probe_args(
        p,
        paths,
        &HostKeyPolicy::CaptureInto(capture.display().to_string()),
    );
    let r = run(&paths.ssh, &args, probe_timeout(p))
        .await
        .map_err(TrustError::Problem)?;
    let captured = std::fs::read_to_string(&capture).unwrap_or_default();
    if captured.trim().is_empty() {
        return Err(TrustError::Problem(classify_ssh(
            r.code, &r.stderr, &p.host, p.port,
        )));
    }
    let listed = run(
        &paths.ssh_keygen,
        &["-l".to_owned(), "-f".into(), capture.display().to_string()],
        Duration::from_secs(10),
    )
    .await
    .map_err(TrustError::Problem)?;
    let presented = parse_fingerprints(&listed.stdout);
    let Some(found) = presented.iter().find(|k| same_key(k, expected)).cloned() else {
        return Err(TrustError::Mismatch { presented });
    };
    if replace {
        forget(paths, p).await.map_err(TrustError::Io)?;
    }
    append(&paths.app_known_hosts, &captured).map_err(|e| TrustError::Io(e.to_string()))?;
    Ok(found)
}

/// Quita del `known_hosts` de la aplicación las claves del servidor.
pub async fn forget(paths: &SshPaths, p: &SshProfile) -> Result<(), String> {
    let file = &paths.app_known_hosts;
    if !file.exists() {
        return Ok(());
    }
    let args = vec![
        "-R".to_owned(),
        host_pattern(p),
        "-f".into(),
        file.display().to_string(),
    ];
    let r = run(&paths.ssh_keygen, &args, Duration::from_secs(10))
        .await
        .map_err(|e| format!("{e:?}"))?;
    // ssh-keygen deja una copia `.old`: no se conserva.
    let _ = std::fs::remove_file(format!("{}.old", file.display()));
    let _ = std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o600));
    if r.code == Some(0) || r.stderr.contains("not found") {
        Ok(())
    } else {
        Err(r.stderr.lines().next().unwrap_or("").to_owned())
    }
}

/// Añade líneas al `known_hosts` de la aplicación (0600, directorio existente).
fn append(file: &Path, lines: &str) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(file)?;
    std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o600))?;
    for line in lines
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        writeln!(f, "{line}")?;
    }
    f.sync_all()
}

struct Cleanup(std::path::PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        let _ = std::fs::remove_file(format!("{}.old", self.0.display()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_and_fingerprint_parsing() {
        let mut p = crate::ssh::session::tests::profile_for("127.0.0.1", 22, "a");
        assert_eq!(host_pattern(&p), "127.0.0.1");
        p.port = 2201;
        assert_eq!(host_pattern(&p), "[127.0.0.1]:2201");
        let l = "256 SHA256:iHgHJYlX3wOd7kwqzTZJpiOy7ALikJI0cSlcmKEHVjo [127.0.0.1]:2201 (ED25519)\n3072 SHA256:abc host (RSA)\n";
        assert_eq!(
            parse_fingerprints(l),
            vec![
                HostKeyInfo {
                    key_type: "ssh-ed25519".into(),
                    fingerprint: "SHA256:iHgHJYlX3wOd7kwqzTZJpiOy7ALikJI0cSlcmKEHVjo".into()
                },
                HostKeyInfo {
                    key_type: "ssh-rsa".into(),
                    fingerprint: "SHA256:abc".into()
                },
            ]
        );
        let f = "# Host [127.0.0.1]:2201 found: line 1 \n[127.0.0.1]:2201 ED25519 SHA256:xyz\n";
        assert_eq!(
            parse_fingerprints(f),
            vec![HostKeyInfo {
                key_type: "ssh-ed25519".into(),
                fingerprint: "SHA256:xyz".into()
            }]
        );
    }
}
