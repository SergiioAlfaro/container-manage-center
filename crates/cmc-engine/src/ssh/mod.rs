// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Docker remoto por SSH (0.10.0, D-075 a D-079).
//!
//! Transporte: el cliente OpenSSH del sistema ejecuta `docker system dial-stdio`
//! en el servidor (el mismo mecanismo que `docker -H ssh://…`). Una conexión
//! maestra por servidor (`ControlMaster`) y un canal multiplexado por cada
//! conexión HTTP de bollard, sin repetir la autenticación.
//!
//! Garantías:
//! - `ssh` se lanza siempre con argumentos separados, sin shell local.
//! - La orden remota es una constante o argumentos validados y entrecomillados.
//! - `StrictHostKeyChecking=yes`: nunca se acepta una identidad desconocida o
//!   cambiada sin una confirmación explícita con su huella (`hostkey`).
//! - Los secretos se entregan por `SSH_ASKPASS` a través de un socket efímero;
//!   nunca en argumentos, variables de entorno ni archivos (`askpass`).

pub mod args;
pub mod askpass;
pub mod connect;
pub mod hostkey;
pub mod problem;
pub mod profile;
pub mod session;
pub mod shell;

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub use problem::RemoteProblem;
pub use profile::{RemoteDocker, SecretKind, SshAuth, SshOptions, SshProfile};
pub use session::SshSession;

/// Secreto en memoria que se borra al liberarse.
pub type Secret = zeroize::Zeroizing<String>;

/// Quien sabe dar la frase de paso o la contraseña de un servidor (keyring,
/// memoria de la sesión…). Lo implementa la aplicación.
pub trait SecretProvider: Send + Sync {
    fn secret(&self, server_id: &str, kind: SecretKind) -> Option<Secret>;
}

/// Sin secretos (pruebas y perfiles que no los necesitan).
pub struct NoSecrets;

impl SecretProvider for NoSecrets {
    fn secret(&self, _: &str, _: SecretKind) -> Option<Secret> {
        None
    }
}

/// Rutas y programas que necesita el transporte SSH.
#[derive(Debug, Clone)]
pub struct SshPaths {
    /// Programa `ssh` (normalmente `/usr/bin/ssh`).
    pub ssh: PathBuf,
    /// `ssh-keygen`, para leer huellas de `known_hosts`.
    pub ssh_keygen: PathBuf,
    /// Directorio privado (0700) para los sockets de control y de askpass.
    pub runtime_dir: PathBuf,
    /// `known_hosts` propio de la aplicación (0600).
    pub app_known_hosts: PathBuf,
    /// `~/.ssh/known_hosts` del usuario (solo se lee, nunca se modifica).
    pub user_known_hosts: PathBuf,
    /// Programa que actúa como `SSH_ASKPASS` (la propia aplicación).
    pub askpass: PathBuf,
}

impl SshPaths {
    /// Rutas por defecto del usuario actual. `None` si no hay cliente `ssh`.
    pub fn detect(config_dir: PathBuf, askpass: PathBuf) -> Option<Self> {
        let ssh = crate::sys::find_executable("ssh")?;
        let ssh_keygen = crate::sys::find_executable("ssh-keygen")
            .unwrap_or_else(|| "/usr/bin/ssh-keygen".into());
        let runtime_dir = runtime_dir();
        let home = dirs::home_dir().unwrap_or_else(|| "/".into());
        Some(Self {
            ssh,
            ssh_keygen,
            runtime_dir,
            app_known_hosts: config_dir.join("known_hosts"),
            user_known_hosts: home.join(".ssh").join("known_hosts"),
            askpass,
        })
    }
}

/// `$XDG_RUNTIME_DIR/container-manage-center/ssh` o, si no existe,
/// `/tmp/container-manage-center-<uid>/ssh`. Rutas cortas: un socket Unix no
/// admite más de ~104 caracteres.
pub fn runtime_dir() -> PathBuf {
    runtime_dir_named(RUNTIME_DIR)
}

/// Carpeta de los sockets de control (1.8.0, D-113).
pub const RUNTIME_DIR: &str = "container-manage-center";

/// Quita la carpeta de sockets del nombre anterior si ha quedado vacía (solo
/// tiene archivos temporales de sesiones ya cerradas; nada que migrar).
pub fn remove_legacy_runtime_dir() {
    let old = runtime_dir_named(crate::legacy::RUNTIME_DIR);
    let _ = std::fs::remove_dir(&old);
    if let Some(parent) = old.parent() {
        let _ = std::fs::remove_dir(parent);
    }
}

#[cfg(windows)]
fn runtime_dir_named(name: &str) -> PathBuf {
    std::env::temp_dir().join(name).join("ssh")
}

#[cfg(unix)]
fn runtime_dir_named(name: &str) -> PathBuf {
    let uid = unsafe { libc::getuid() };
    match std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from) {
        Some(d) if d.is_absolute() && d.is_dir() => d.join(name).join("ssh"),
        _ => PathBuf::from(format!("/tmp/{name}-{uid}")).join("ssh"),
    }
}

/// Contexto compartido por todas las conexiones SSH de la aplicación.
#[derive(Clone)]
pub struct SshContext {
    pub paths: SshPaths,
    pub secrets: Arc<dyn SecretProvider>,
}

impl std::fmt::Debug for SshContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SshContext")
            .field("paths", &self.paths)
            .finish_non_exhaustive()
    }
}

/// Huella de una clave de host (`SHA256:…`) y su tipo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HostKeyInfo {
    /// `ssh-ed25519`, `ecdsa-sha2-nistp256`, `ssh-rsa`…
    pub key_type: String,
    /// `SHA256:` seguido de la huella en base64.
    pub fingerprint: String,
}

/// Paso de la prueba de conexión (requisito 3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CheckId {
    /// Resolución DNS o validez de la dirección.
    Resolve,
    /// El puerto responde como servidor SSH.
    Reach,
    /// Identidad del servidor (clave de host conocida y sin cambios).
    Identity,
    /// Autenticación SSH.
    Auth,
    /// El CLI `docker` existe en el servidor.
    Docker,
    /// El usuario puede usar el socket de Docker y el daemon responde.
    Permission,
    /// Versión de la API compatible.
    Api,
}

impl CheckId {
    pub const ALL: [CheckId; 7] = [
        CheckId::Resolve,
        CheckId::Reach,
        CheckId::Identity,
        CheckId::Auth,
        CheckId::Docker,
        CheckId::Permission,
        CheckId::Api,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CheckStatus {
    Ok,
    Failed,
    /// No se ha llegado a comprobar (falló un paso anterior).
    NotRun,
    /// No aplica (p. ej. DNS y alcance con un servidor intermedio).
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CheckStep {
    pub id: CheckId,
    pub status: CheckStatus,
    /// Dato concreto y no sensible (dirección resuelta, versión, huella…).
    pub detail: Option<String>,
}

/// Lista de pasos con todos en «sin comprobar».
pub fn pending_steps() -> Vec<CheckStep> {
    CheckId::ALL
        .iter()
        .map(|&id| CheckStep {
            id,
            status: CheckStatus::NotRun,
            detail: None,
        })
        .collect()
}

pub(crate) fn set_step(
    steps: &mut [CheckStep],
    id: CheckId,
    status: CheckStatus,
    detail: Option<String>,
) {
    if let Some(s) = steps.iter_mut().find(|s| s.id == id) {
        s.status = status;
        s.detail = detail;
    }
}
