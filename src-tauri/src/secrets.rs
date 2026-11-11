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

use dcc_engine::ssh::{Secret, SecretKind, SecretProvider};
use parking_lot::Mutex;
use serde::Serialize;
use ts_rs::TS;

/// Atributos con los que se guardan en el keyring.
const ATTR_APP: &str = "application";
const APP: &str = "docker-control-center";
const ATTR_SERVER: &str = "dcc-server";
const ATTR_KIND: &str = "dcc-kind";
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
    Unavailable(String),
    #[error("el almacén de secretos no respondió a tiempo")]
    Timeout,
    #[error("el almacén de secretos está bloqueado")]
    Locked,
    #[error("error del almacén de secretos: {0}")]
    Other(String),
}

fn other(e: impl std::fmt::Display) -> KeyringError {
    KeyringError::Other(e.to_string())
}

/// Acceso al keyring del sistema por Secret Service (D-Bus).
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
            (None, None) => return Ok(None),
        };
        let bytes = zeroize::Zeroizing::new(item.get_secret().await.map_err(other)?);
        Ok(std::str::from_utf8(&bytes)
            .ok()
            .map(|s| Secret::new(s.to_owned())))
    }

    /// Borra todos los secretos guardados de un servidor. Devuelve cuántos.
    pub async fn delete(server: &str) -> Result<usize, KeyringError> {
        let ss = service().await?;
        let found = ss.search_items(attrs(server, None)).await.map_err(other)?;
        let mut n = 0;
        for item in found.unlocked.into_iter().chain(found.locked) {
            item.delete().await.map_err(other)?;
            n += 1;
        }
        Ok(n)
    }

    pub async fn has(server: &str, kind: SecretKind) -> Result<bool, KeyringError> {
        let ss = service().await?;
        let found = ss
            .search_items(attrs(server, Some(kind_code(kind))))
            .await
            .map_err(other)?;
        Ok(!found.unlocked.is_empty() || !found.locked.is_empty())
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
            "Docker Control Center — {server_name} ({})",
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

#[cfg(test)]
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

    /// Keyring real (Secret Service). Se ejecuta solo con `DCC_KEYRING_TEST=1`
    /// dentro de una sesión D-Bus con un keyring desbloqueado (ver
    /// `scripts/verify-update-0.10.0.sh`).
    #[tokio::test]
    async fn real_keyring_store_load_delete() {
        if std::env::var_os("DCC_KEYRING_TEST").is_none() {
            eprintln!("omitida: DCC_KEYRING_TEST no definida");
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
        if std::env::var_os("DCC_KEYRING_TEST").is_some() {
            return;
        }
        // Sin bus de sesión ni keyring: nunca se recurre a un archivo.
        std::env::set_var("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent/dcc-bus");
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
