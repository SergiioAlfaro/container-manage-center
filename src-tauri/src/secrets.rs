// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Secretos de los servidores remotos (0.10.0, D-077).
//!
//! - En memoria, solo durante la sesión, en un `Zeroizing` que se borra al
//!   liberarse. El transporte SSH los lee de aquí ([`SecretProvider`]).
//! - Si el usuario lo pide, también en el keyring del sistema (Secret Service:
//!   GNOME Keyring, KWallet…), cifrado por el propio keyring.
//! - Si no hay keyring, no hay alternativa en texto plano: se avisa y el
//!   secreto se pide en cada conexión.
//! - Nunca se escriben en archivos, argumentos, variables de entorno, registros
//!   ni en el panel de actividad.

use std::collections::HashMap;
use std::time::Duration;

use cmc_engine::ssh::{Secret, SecretKind, SecretProvider};
use parking_lot::Mutex;
use serde::Serialize;
use ts_rs::TS;

/// Atributos con los que se guardan en el keyring.
#[cfg(unix)]
const ATTR_APP: &str = "application";
/// Identificador de la aplicación en el keyring (1.8.0, D-113). Las entradas
/// guardadas con el nombre anterior ([`cmc_store::legacy`]) se pasan a este al
/// usarlas o al arrancar ([`Secrets::migrate_legacy`]).
const APP: &str = "container-manage-center";
#[cfg(unix)]
const ATTR_SERVER: &str = "cmc-server";
#[cfg(unix)]
const ATTR_KIND: &str = "cmc-kind";
/// Abrir el keyring puede mostrar un diálogo del sistema para desbloquearlo.
const KEYRING_TIMEOUT: Duration = Duration::from_secs(60);
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

fn kind_code(k: SecretKind) -> &'static str {
    match k {
        SecretKind::Passphrase => "passphrase",
        SecretKind::Password => "password",
    }
}

/// Estado del almacén seguro del sistema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum KeyringStatus {
    Available,
    /// No hay servicio de secretos (sesión sin GNOME Keyring/KWallet…):
    /// los secretos se piden en cada conexión.
    Unavailable {
        reason: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum KeyringError {
    #[error("no hay un almacén seguro de secretos disponible: {0}")]
    #[cfg_attr(windows, allow(dead_code))]
    Unavailable(String),
    #[error("el almacén de secretos no respondió a tiempo")]
    Timeout,
    #[error("el almacén de secretos está bloqueado")]
    #[cfg_attr(windows, allow(dead_code))]
    Locked,
    #[error("error del almacén de secretos: {0}")]
    Other(String),
}

fn other(e: impl std::fmt::Display) -> KeyringError {
    KeyringError::Other(e.to_string())
}

/// Windows (1.9.0, D-114): Administrador de credenciales de Windows
/// (credenciales genéricas del usuario, cifradas por Windows con su sesión).
/// Nombre: `container-manage-center/<servidor>/<tipo>`.
#[cfg(windows)]
mod keyring {
    use super::*;
    use windows_sys::Win32::Security::Credentials::{
        CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
        CRED_TYPE_GENERIC,
    };

    const KINDS: [SecretKind; 2] = [SecretKind::Passphrase, SecretKind::Password];

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn target(server: &str, kind: SecretKind) -> Vec<u16> {
        wide(&format!("{APP}/{server}/{}", kind_code(kind)))
    }

    pub async fn probe() -> Result<(), KeyringError> {
        Ok(())
    }

    pub async fn store(
        server: &str,
        label: &str,
        kind: SecretKind,
        secret: &str,
    ) -> Result<(), KeyringError> {
        let mut name = target(server, kind);
        let mut comment = wide(label);
        let mut user = wide(server);
        let blob = zeroize::Zeroizing::new(secret.as_bytes().to_vec());
        let cred = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: name.as_mut_ptr(),
            Comment: comment.as_mut_ptr(),
            CredentialBlobSize: blob.len() as u32,
            CredentialBlob: blob.as_ptr() as *mut u8,
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: user.as_mut_ptr(),
            ..Default::default()
        };
        // SAFETY: todos los punteros apuntan a búferes vivos hasta el final de la llamada.
        if unsafe { CredWriteW(&cred, 0) } == 0 {
            return Err(other(std::io::Error::last_os_error()));
        }
        Ok(())
    }

    pub async fn load(server: &str, kind: SecretKind) -> Result<Option<Secret>, KeyringError> {
        let name = target(server, kind);
        let mut p: *mut CREDENTIALW = std::ptr::null_mut();
        // SAFETY: `name` termina en 0; `p` recibe una estructura que se libera con CredFree.
        if unsafe { CredReadW(name.as_ptr(), CRED_TYPE_GENERIC, 0, &mut p) } == 0 {
            let e = std::io::Error::last_os_error();
            // ERROR_NOT_FOUND
            return if e.raw_os_error() == Some(1168) {
                Ok(None)
            } else {
                Err(other(e))
            };
        }
        // SAFETY: CredReadW ha devuelto una credencial válida.
        let bytes = unsafe {
            let c = &*p;
            zeroize::Zeroizing::new(
                std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize)
                    .to_vec(),
            )
        };
        // SAFETY: liberar lo que reservó CredReadW.
        unsafe { CredFree(p as *const _) };
        Ok(std::str::from_utf8(&bytes)
            .ok()
            .map(|s| Secret::new(s.to_owned())))
    }

    pub async fn delete(server: &str) -> Result<usize, KeyringError> {
        let mut n = 0;
        for k in KINDS {
            let name = target(server, k);
            // SAFETY: `name` termina en 0.
            if unsafe { CredDeleteW(name.as_ptr(), CRED_TYPE_GENERIC, 0) } != 0 {
                n += 1;
            }
        }
        Ok(n)
    }

    pub async fn has(server: &str, kind: SecretKind) -> Result<bool, KeyringError> {
        Ok(load(server, kind).await?.is_some())
    }

    /// En Windows no hubo versiones con el nombre anterior.
    pub async fn migrate_unlocked() -> Result<usize, KeyringError> {
        Ok(0)
    }
}

/// Acceso al keyring del sistema por Secret Service (D-Bus).
#[cfg(unix)]
mod keyring {
    use super::*;
    use secret_service::{EncryptionType, SecretService};

    async fn service() -> Result<SecretService<'static>, KeyringError> {
        SecretService::connect(EncryptionType::Dh)
            .await
            .map_err(|e| KeyringError::Unavailable(e.to_string()))
    }

    fn attrs<'a>(server: &'a str, kind: Option<&'a str>) -> HashMap<&'a str, &'a str> {
        let mut a = HashMap::from([(ATTR_APP, APP), (ATTR_SERVER, server)]);
        if let Some(k) = kind {
            a.insert(ATTR_KIND, k);
        }
        a
    }

    fn legacy_attrs<'a>(server: &'a str, kind: Option<&'a str>) -> HashMap<&'a str, &'a str> {
        use cmc_store::legacy;
        let mut a = HashMap::from([
            (ATTR_APP, legacy::KEYRING_APP),
            (legacy::KEYRING_ATTR_SERVER, server),
        ]);
        if let Some(k) = kind {
            a.insert(legacy::KEYRING_ATTR_KIND, k);
        }
        a
    }

    /// Etiqueta visible de una entrada pasada al nombre nuevo.
    pub(super) fn renamed_label(old: &str) -> String {
        match old.strip_prefix(cmc_store::legacy::KEYRING_LABEL_PREFIX) {
            Some(rest) => format!("Container Manage Center{rest}"),
            None => old.to_owned(),
        }
    }

    /// Pasa una entrada del nombre anterior a los atributos nuevos: la crea
    /// con el mismo secreto y la misma etiqueta (con el nombre nuevo) y solo
    /// después borra la antigua.
    async fn move_legacy_item(
        ss: &SecretService<'_>,
        item: &secret_service::Item<'_>,
        server: &str,
        kind: &str,
    ) -> Result<zeroize::Zeroizing<Vec<u8>>, KeyringError> {
        let secret = zeroize::Zeroizing::new(item.get_secret().await.map_err(other)?);
        let label = renamed_label(&item.get_label().await.map_err(other)?);
        let col = ss.get_default_collection().await.map_err(other)?;
        if col.is_locked().await.map_err(other)? {
            col.unlock().await.map_err(|_| KeyringError::Locked)?;
        }
        col.create_item(
            &label,
            attrs(server, Some(kind)),
            &secret,
            true,
            "text/plain",
        )
        .await
        .map_err(other)?;
        item.delete().await.map_err(other)?;
        Ok(secret)
    }

    /// Pasa al nombre nuevo todas las entradas antiguas desbloqueadas (las
    /// bloqueadas se pasan cuando se usan, para no pedir desbloquear el
    /// keyring al arrancar). Devuelve cuántas se pasaron.
    pub async fn migrate_unlocked() -> Result<usize, KeyringError> {
        use cmc_store::legacy;
        let ss = service().await?;
        let found = ss
            .search_items(HashMap::from([(ATTR_APP, legacy::KEYRING_APP)]))
            .await
            .map_err(other)?;
        let mut n = 0;
        for item in found.unlocked {
            let a = item.get_attributes().await.map_err(other)?;
            let (Some(server), Some(kind)) = (
                a.get(legacy::KEYRING_ATTR_SERVER),
                a.get(legacy::KEYRING_ATTR_KIND),
            ) else {
                continue;
            };
            move_legacy_item(&ss, &item, server, kind).await?;
            n += 1;
        }
        Ok(n)
    }

    pub async fn probe() -> Result<(), KeyringError> {
        let ss = service().await?;
        ss.get_default_collection()
            .await
            .map_err(|e| KeyringError::Unavailable(e.to_string()))?;
        Ok(())
    }

    pub async fn store(
        server: &str,
        label: &str,
        kind: SecretKind,
        secret: &str,
    ) -> Result<(), KeyringError> {
        let ss = service().await?;
        let col = ss.get_default_collection().await.map_err(other)?;
        if col.is_locked().await.map_err(other)? {
            col.unlock().await.map_err(|_| KeyringError::Locked)?;
        }
        col.create_item(
            label,
            attrs(server, Some(kind_code(kind))),
            secret.as_bytes(),
            true,
            "text/plain",
        )
        .await
        .map_err(other)?;
        Ok(())
    }

    pub async fn load(server: &str, kind: SecretKind) -> Result<Option<Secret>, KeyringError> {
        let ss = service().await?;
        let found = ss
            .search_items(attrs(server, Some(kind_code(kind))))
            .await
            .map_err(other)?;
        let item = match (
            found.unlocked.into_iter().next(),
            found.locked.into_iter().next(),
        ) {
            (Some(i), _) => i,
            (None, Some(i)) => {
                i.unlock().await.map_err(|_| KeyringError::Locked)?;
                i
            }
            (None, None) => {
                // Guardada con el nombre anterior (hasta la 1.7.0): se pasa al
                // nuevo y se usa.
                let old = ss
                    .search_items(legacy_attrs(server, Some(kind_code(kind))))
                    .await
                    .map_err(other)?;
                let item = match (
                    old.unlocked.into_iter().next(),
                    old.locked.into_iter().next(),
                ) {
                    (Some(i), _) => i,
                    (None, Some(i)) => {
                        i.unlock().await.map_err(|_| KeyringError::Locked)?;
                        i
                    }
                    (None, None) => return Ok(None),
                };
                let bytes = move_legacy_item(&ss, &item, server, kind_code(kind)).await?;
                return Ok(std::str::from_utf8(&bytes)
                    .ok()
                    .map(|s| Secret::new(s.to_owned())));
            }
        };
        let bytes = zeroize::Zeroizing::new(item.get_secret().await.map_err(other)?);
        Ok(std::str::from_utf8(&bytes)
            .ok()
            .map(|s| Secret::new(s.to_owned())))
    }

    /// Borra todos los secretos guardados de un servidor. Devuelve cuántos.
    pub async fn delete(server: &str) -> Result<usize, KeyringError> {
        let ss = service().await?;
        let mut n = 0;
        for a in [attrs(server, None), legacy_attrs(server, None)] {
            let found = ss.search_items(a).await.map_err(other)?;
            for item in found.unlocked.into_iter().chain(found.locked) {
                item.delete().await.map_err(other)?;
                n += 1;
            }
        }
        Ok(n)
    }

    pub async fn has(server: &str, kind: SecretKind) -> Result<bool, KeyringError> {
        let ss = service().await?;
        for a in [
            attrs(server, Some(kind_code(kind))),
            legacy_attrs(server, Some(kind_code(kind))),
        ] {
            let found = ss.search_items(a).await.map_err(other)?;
            if !found.unlocked.is_empty() || !found.locked.is_empty() {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

async fn timed<T>(
    limit: Duration,
    f: impl std::future::Future<Output = Result<T, KeyringError>>,
) -> Result<T, KeyringError> {
    tokio::time::timeout(limit, f)
        .await
        .map_err(|_| KeyringError::Timeout)?
}

/// Secretos de la sesión y acceso al keyring.
#[derive(Default)]
pub struct Secrets {
    cache: Mutex<HashMap<(String, SecretKind), Secret>>,
}

impl SecretProvider for Secrets {
    fn secret(&self, server_id: &str, kind: SecretKind) -> Option<Secret> {
        self.cache
            .lock()
            .get(&(server_id.to_owned(), kind))
            .cloned()
    }
}

impl Secrets {
    /// Guarda el secreto para esta sesión (solo en memoria).
    pub fn remember_for_session(&self, server_id: &str, kind: SecretKind, secret: Secret) {
        self.cache
            .lock()
            .insert((server_id.to_owned(), kind), secret);
    }

    /// Olvida los secretos de un servidor en memoria.
    pub fn forget_session(&self, server_id: &str) {
        self.cache.lock().retain(|(id, _), _| id != server_id);
    }

    pub fn in_session(&self, server_id: &str, kind: SecretKind) -> bool {
        self.cache
            .lock()
            .contains_key(&(server_id.to_owned(), kind))
    }

    /// Pasa al nombre nuevo las contraseñas guardadas por una versión anterior
    /// (1.8.0, D-113). Sin keyring o si falla, no pasa nada: se hará al usar
    /// cada una.
    pub async fn migrate_legacy(&self) -> Result<usize, KeyringError> {
        timed(KEYRING_TIMEOUT, keyring::migrate_unlocked()).await
    }

    pub async fn keyring_status(&self) -> KeyringStatus {
        match timed(PROBE_TIMEOUT, keyring::probe()).await {
            Ok(()) => KeyringStatus::Available,
            Err(e) => KeyringStatus::Unavailable {
                reason: e.to_string(),
            },
        }
    }

    /// Guarda el secreto en el keyring del sistema.
    pub async fn store(
        &self,
        server_id: &str,
        server_name: &str,
        kind: SecretKind,
        secret: &Secret,
    ) -> Result<(), KeyringError> {
        let label = format!(
            "Container Manage Center — {server_name} ({})",
            match kind {
                SecretKind::Passphrase => "frase de paso de la clave SSH",
                SecretKind::Password => "contraseña SSH",
            }
        );
        timed(
            KEYRING_TIMEOUT,
            keyring::store(server_id, &label, kind, secret),
        )
        .await
    }

    /// Carga en memoria el secreto guardado en el keyring, si lo hay y no está
    /// ya en la sesión. Devuelve si quedó disponible.
    pub async fn prefetch(&self, server_id: &str, kind: SecretKind) -> Result<bool, KeyringError> {
        if self.in_session(server_id, kind) {
            return Ok(true);
        }
        match timed(KEYRING_TIMEOUT, keyring::load(server_id, kind)).await? {
            Some(s) => {
                self.remember_for_session(server_id, kind, s);
                Ok(true)
            }
            None => Ok(false),
        }
    }

    pub async fn stored(&self, server_id: &str, kind: SecretKind) -> Result<bool, KeyringError> {
        timed(PROBE_TIMEOUT, keyring::has(server_id, kind)).await
    }

    /// Borra los secretos del servidor: de la sesión y del keyring.
    pub async fn delete(&self, server_id: &str) -> Result<usize, KeyringError> {
        self.forget_session(server_id);
        timed(KEYRING_TIMEOUT, keyring::delete(server_id)).await
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn session_cache_is_per_server_and_kind() {
        let s = Secrets::default();
        s.remember_for_session(
            "srv-aaaaaaaa",
            SecretKind::Passphrase,
            Secret::new("uno".into()),
        );
        assert_eq!(
            s.secret("srv-aaaaaaaa", SecretKind::Passphrase)
                .as_deref()
                .map(String::as_str),
            Some("uno")
        );
        assert!(s.secret("srv-aaaaaaaa", SecretKind::Password).is_none());
        assert!(s.secret("srv-bbbbbbbb", SecretKind::Passphrase).is_none());
        s.forget_session("srv-aaaaaaaa");
        assert!(s.secret("srv-aaaaaaaa", SecretKind::Passphrase).is_none());
    }

    /// Keyring real (Secret Service). Se ejecuta solo con `CMC_KEYRING_TEST=1`
    /// dentro de una sesión D-Bus con un keyring desbloqueado (ver
    /// `scripts/verify-evolution-1.9.0.sh`).
    #[tokio::test]
    async fn real_keyring_store_load_delete() {
        if std::env::var_os("CMC_KEYRING_TEST").is_none() {
            eprintln!("omitida: CMC_KEYRING_TEST no definida");
            return;
        }
        let id = format!("srv-{:016x}", std::process::id() as u64 * 7919);
        let s = Secrets::default();
        assert_eq!(s.keyring_status().await, KeyringStatus::Available);
        assert!(!s.stored(&id, SecretKind::Password).await.unwrap());
        s.store(
            &id,
            "Prueba",
            SecretKind::Password,
            &Secret::new("Contraseña ñ 1".into()),
        )
        .await
        .unwrap();
        // Sustituir no duplica.
        s.store(
            &id,
            "Prueba",
            SecretKind::Password,
            &Secret::new("Contraseña ñ 2".into()),
        )
        .await
        .unwrap();
        assert!(s.stored(&id, SecretKind::Password).await.unwrap());
        assert!(!s.stored(&id, SecretKind::Passphrase).await.unwrap());
        assert!(s.prefetch(&id, SecretKind::Password).await.unwrap());
        assert_eq!(
            s.secret(&id, SecretKind::Password)
                .as_deref()
                .map(String::as_str),
            Some("Contraseña ñ 2")
        );
        assert_eq!(s.delete(&id).await.unwrap(), 1);
        assert!(s.secret(&id, SecretKind::Password).is_none());
        assert!(!s.stored(&id, SecretKind::Password).await.unwrap());
        assert!(!s.prefetch(&id, SecretKind::Password).await.unwrap());
    }

    #[tokio::test]
    async fn without_a_secret_service_it_is_reported_unavailable() {
        if std::env::var_os("CMC_KEYRING_TEST").is_some() {
            return;
        }
        // Sin bus de sesión ni keyring: nunca se recurre a un archivo.
        std::env::set_var("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent/cmc-bus");
        let s = Secrets::default();
        assert!(matches!(
            s.keyring_status().await,
            KeyringStatus::Unavailable { .. }
        ));
        assert!(s
            .store(
                "srv-aaaaaaaa",
                "x",
                SecretKind::Password,
                &Secret::new("x".into())
            )
            .await
            .is_err());
    }
}
