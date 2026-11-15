// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Perfiles de servidores remotos (0.10.0, D-078) en
//! `$XDG_CONFIG_HOME/container-manage-center/servers.toml`.
//!
//! - Solo datos no sensibles: nunca contraseñas, frases de paso ni claves
//!   privadas (de una clave solo se guarda su ruta). Los secretos van al
//!   keyring del sistema, aparte.
//! - Archivo propio, separado de `config.toml`: la pantalla de preferencias
//!   guarda la configuración completa y no debe poder pisar la lista de
//!   servidores con una copia antigua.
//! - Escritura atómica y permisos 0600, como la configuración.
//! - La validación completa de cada perfil la hace el motor al guardarlo y al
//!   conectar; aquí solo se descartan entradas sin identificador válido.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{write_atomic, StoreError, APP_DIR};

pub const SERVERS_FILE: &str = "servers.toml";
pub const SERVERS_VERSION: u32 = 1;
/// Máximo de servidores registrados.
pub const MAX_SERVERS: usize = 64;

/// Método de autenticación guardado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ServerAuthMethod {
    #[default]
    Agent,
    Key,
    KeyPassphrase,
    Password,
}

/// A qué Docker Engine del servidor se conecta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ServerDockerTarget {
    #[default]
    Default,
    Socket,
    Context,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct ServerConfig {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub description: String,
    pub auth: ServerAuthMethod,
    /// Ruta de la clave privada (métodos `key` y `keyPassphrase`).
    pub key_path: String,
    pub docker_target: ServerDockerTarget,
    /// Socket del servidor (`docker_target = socket`).
    pub docker_socket: String,
    /// Contexto de Docker del servidor (`docker_target = context`).
    pub docker_context: String,
    pub connect_timeout: u32,
    pub proxy_jump: String,
    /// Conectar al abrir la aplicación.
    pub auto_connect: bool,
    /// Guardar la frase de paso o la contraseña en el keyring del sistema.
    pub remember_secret: bool,
    /// Datos de la última conexión correcta (los pone la aplicación, no la interfaz).
    pub last_seen: Option<ServerSeen>,
    /// Proyectos Compose añadidos en este servidor, con rutas **del servidor**
    /// (los cambia la aplicación al añadir o quitar, no la interfaz al guardar).
    pub compose_projects: Vec<crate::SavedProject>,
}

/// Lo último que se supo de un servidor al conectar (para mostrarlo sin conexión).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct ServerSeen {
    /// Fecha RFC 3339.
    pub at: String,
    pub docker_version: String,
    pub api_version: String,
    pub os: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            host: String::new(),
            port: 22,
            user: String::new(),
            description: String::new(),
            auth: ServerAuthMethod::Agent,
            key_path: String::new(),
            docker_target: ServerDockerTarget::Default,
            docker_socket: String::new(),
            docker_context: String::new(),
            connect_timeout: 10,
            proxy_jump: String::new(),
            auto_connect: false,
            remember_secret: false,
            last_seen: None,
            compose_projects: Vec::new(),
        }
    }
}

/// Identificador válido: `srv-` y de 8 a 32 dígitos hexadecimales en minúscula.
pub fn valid_id(id: &str) -> bool {
    id.strip_prefix("srv-").is_some_and(|h| {
        (8..=32).contains(&h.len())
            && h.chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
struct ServersFile {
    version: u32,
    #[serde(rename = "server")]
    servers: Vec<ServerConfig>,
}

/// Quita entradas sin identificador válido, repetidas o por encima del máximo.
pub fn normalized(servers: Vec<ServerConfig>) -> Vec<ServerConfig> {
    let mut out: Vec<ServerConfig> = Vec::new();
    for s in servers {
        if valid_id(&s.id) && !out.iter().any(|o| o.id == s.id) && out.len() < MAX_SERVERS {
            out.push(s);
        }
    }
    out
}

/// Resultado de leer la lista.
#[derive(Debug)]
pub struct LoadedServers {
    pub servers: Vec<ServerConfig>,
    pub warning: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ServerStore {
    path: PathBuf,
}

impl ServerStore {
    pub fn default_location() -> Result<Self, StoreError> {
        let dir = dirs::config_dir().ok_or(StoreError::NoConfigDir)?;
        Ok(Self::at(dir.join(APP_DIR).join(SERVERS_FILE)))
    }

    /// Junto a un `config.toml` (mismo directorio).
    pub fn beside(config: &Path) -> Self {
        Self::at(config.with_file_name(SERVERS_FILE))
    }

    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<LoadedServers, StoreError> {
        let raw = match fs::read_to_string(&self.path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(LoadedServers {
                    servers: Vec::new(),
                    warning: None,
                })
            }
            Err(source) => {
                return Err(StoreError::Io {
                    path: self.path.clone(),
                    source,
                })
            }
        };
        match toml::from_str::<ServersFile>(&raw) {
            Ok(f) => Ok(LoadedServers {
                servers: normalized(f.servers),
                warning: None,
            }),
            Err(e) => {
                let backup = self.path.with_extension("toml.corrupt");
                tracing::warn!(error = %e, backup = %backup.display(), "lista de servidores corrupta");
                fs::rename(&self.path, &backup).map_err(|source| StoreError::Io {
                    path: backup.clone(),
                    source,
                })?;
                Ok(LoadedServers {
                    servers: Vec::new(),
                    warning: Some(format!(
                        "La lista de servidores no era válida y se ha guardado una copia en {}.",
                        backup.display()
                    )),
                })
            }
        }
    }

    pub fn save(&self, servers: &[ServerConfig]) -> Result<(), StoreError> {
        let file = ServersFile {
            version: SERVERS_VERSION,
            servers: normalized(servers.to_vec()),
        };
        let body = toml::to_string_pretty(&file)?;
        write_atomic(&self.path, &body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(id: &str) -> ServerConfig {
        ServerConfig {
            id: id.into(),
            name: "Producción A".into(),
            host: "docker-a.example.com".into(),
            user: "deploy".into(),
            auth: ServerAuthMethod::KeyPassphrase,
            key_path: "~/.ssh/id_ed25519".into(),
            remember_secret: true,
            last_seen: Some(ServerSeen {
                at: "2026-11-11T10:00:00Z".into(),
                docker_version: "29.0.1".into(),
                api_version: "1.52".into(),
                os: "Ubuntu 24.04".into(),
            }),
            ..Default::default()
        }
    }

    #[cfg(unix)]
    #[test]
    fn roundtrip_with_private_permissions_and_no_secrets() {
        let dir = tempfile::tempdir().unwrap();
        let store = ServerStore::beside(&dir.path().join("config.toml"));
        assert_eq!(store.path(), dir.path().join("servers.toml"));
        assert!(store.load().unwrap().servers.is_empty());
        let list = vec![
            server("srv-0123456789abcdef"),
            server("srv-fedcba9876543210"),
        ];
        store.save(&list).unwrap();
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(store.path()).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(store.load().unwrap().servers, list);
        let raw = fs::read_to_string(store.path()).unwrap();
        // Ningún campo para secretos: solo la ruta de la clave y si se recuerda en el keyring.
        for word in ["password =", "passphrase =", "secret =", "PRIVATE KEY"] {
            assert!(!raw.contains(word), "{word} en {raw}");
        }
        assert!(raw.contains("[[server]]"));
    }

    #[test]
    fn invalid_duplicate_and_excess_entries_are_dropped() {
        let mut list = vec![
            server("local"),
            server("srv-0123456789abcdef"),
            server("srv-0123456789abcdef"),
            server("srv-ZZZZZZZZ"),
        ];
        for i in 0..70 {
            list.push(server(&format!("srv-{i:08x}")));
        }
        let n = normalized(list);
        assert_eq!(n.len(), MAX_SERVERS);
        assert_eq!(n[0].id, "srv-0123456789abcdef");
    }

    #[test]
    fn a_corrupt_file_is_kept_aside() {
        let dir = tempfile::tempdir().unwrap();
        let store = ServerStore::at(dir.path().join("servers.toml"));
        fs::write(store.path(), "[[server]\nid = ").unwrap();
        let l = store.load().unwrap();
        assert!(l.servers.is_empty() && l.warning.is_some());
        assert!(dir.path().join("servers.toml.corrupt").exists());
    }

    #[test]
    fn missing_fields_take_safe_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let store = ServerStore::at(dir.path().join("servers.toml"));
        fs::write(
            store.path(),
            "[[server]]\nid = \"srv-0123abcd\"\nname = \"B\"\nhost = \"b\"\nuser = \"u\"\n",
        )
        .unwrap();
        let s = &store.load().unwrap().servers[0];
        assert_eq!(
            (
                s.port,
                s.auth,
                s.connect_timeout,
                s.auto_connect,
                s.remember_secret
            ),
            (22, ServerAuthMethod::Agent, 10, false, false)
        );
    }
}
