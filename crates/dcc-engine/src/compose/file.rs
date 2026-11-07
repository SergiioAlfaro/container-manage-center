//! Archivos Compose en el equipo: lectura, guardado y creación (D-053).
//!
//! No son operaciones de Docker (no pasan por el socket ni por el CLI), así que
//! no se registran en el panel de actividad. El guardado es atómico (archivo
//! temporal en la misma carpeta y `rename`), conserva los permisos y se niega
//! a sobrescribir un archivo que ha cambiado desde que se leyó.

use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

use super::project::DEFAULT_FILES;
use crate::error::EngineError;

/// Tamaño máximo que se abre en el editor.
pub const MAX_FILE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ComposeFile {
    pub path: String,
    pub content: String,
    /// Huella del contenido leído (SHA-256): el guardado la comprueba.
    pub version: String,
    /// El usuario de la aplicación puede escribirlo.
    pub writable: bool,
}

fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidInput {
        message: message.into(),
    }
}

fn io(path: &Path, e: std::io::Error) -> EngineError {
    EngineError::Other {
        message: format!("{}: {e}", path.display()),
    }
}

pub fn version_of(content: &[u8]) -> String {
    let d = Sha256::digest(content);
    d.iter().map(|b| format!("{b:02x}")).collect()
}

fn absolute(path: &str) -> Result<PathBuf, EngineError> {
    if path.is_empty() || path.contains('\0') || !path.starts_with('/') || path.len() > 4096 {
        return Err(invalid(format!("ruta no válida: {path:?}")));
    }
    Ok(PathBuf::from(path))
}

fn writable(path: &Path) -> bool {
    std::fs::OpenOptions::new().append(true).open(path).is_ok()
}

pub fn read(path: &str) -> Result<ComposeFile, EngineError> {
    let p = absolute(path)?;
    let meta = std::fs::metadata(&p).map_err(|e| io(&p, e))?;
    if !meta.is_file() {
        return Err(invalid(format!("{path} no es un archivo")));
    }
    if meta.len() as usize > MAX_FILE_BYTES {
        return Err(invalid(format!(
            "{path} ocupa más de {} KiB",
            MAX_FILE_BYTES / 1024
        )));
    }
    let bytes = std::fs::read(&p).map_err(|e| io(&p, e))?;
    let content =
        String::from_utf8(bytes).map_err(|_| invalid(format!("{path} no es texto UTF-8")))?;
    Ok(ComposeFile {
        path: path.to_owned(),
        version: version_of(content.as_bytes()),
        writable: writable(&p),
        content,
    })
}

/// Guarda `content` si el archivo sigue siendo el que se leyó (`expected_version`).
pub fn save(path: &str, content: &str, expected_version: &str) -> Result<ComposeFile, EngineError> {
    let p = absolute(path)?;
    if content.len() > MAX_FILE_BYTES {
        return Err(invalid("el contenido es demasiado grande"));
    }
    let current = read(path)?;
    if current.version != expected_version {
        return Err(EngineError::FileChanged {
            path: path.to_owned(),
        });
    }
    if !current.writable {
        return Err(invalid(format!("no tienes permiso para escribir {path}")));
    }
    let mode = std::fs::metadata(&p)
        .map_err(|e| io(&p, e))?
        .permissions()
        .mode()
        & 0o7777;
    let dir = p.parent().ok_or_else(|| invalid("ruta sin carpeta"))?;
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.dcc-{}.tmp", std::process::id()));
    let write = || -> std::io::Result<()> {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&tmp)?;
        f.write_all(content.as_bytes())?;
        f.sync_all()?;
        // `mode` del open se ve afectado por umask: se fija explícitamente.
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode))?;
        std::fs::rename(&tmp, &p)
    };
    if let Err(e) = write() {
        let _ = std::fs::remove_file(&tmp);
        return Err(io(&p, e));
    }
    read(path)
}

/// Archivo Compose estándar que ya existe en la carpeta, si lo hay.
pub fn existing_default(dir: &Path) -> Option<PathBuf> {
    DEFAULT_FILES
        .iter()
        .map(|n| dir.join(n))
        .find(|p| p.exists())
}

/// Crea `compose.yaml` en `dir`. Se niega si la carpeta ya tiene un archivo Compose.
pub fn create(dir: &str, content: &str) -> Result<ComposeFile, EngineError> {
    let d = absolute(dir)?;
    if !d.is_dir() {
        return Err(invalid(format!("{dir} no es una carpeta")));
    }
    if content.len() > MAX_FILE_BYTES {
        return Err(invalid("el contenido es demasiado grande"));
    }
    if let Some(e) = existing_default(&d) {
        return Err(EngineError::AlreadyExists {
            path: e.display().to_string(),
        });
    }
    let p = d.join(DEFAULT_FILES[0]);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .open(&p)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                EngineError::AlreadyExists {
                    path: p.display().to_string(),
                }
            } else {
                io(&p, e)
            }
        })?;
    f.write_all(content.as_bytes()).map_err(|e| io(&p, e))?;
    f.sync_all().map_err(|e| io(&p, e))?;
    read(&p.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_and_save_with_conflict_detection() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("compose.yaml");
        std::fs::write(&f, "services: {}\n").unwrap();
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o640)).unwrap();
        let path = f.to_str().unwrap();
        let r = read(path).unwrap();
        assert_eq!(r.content, "services: {}\n");
        assert!(r.writable);
        let saved = save(path, "services:\n  web:\n    image: nginx\n", &r.version).unwrap();
        assert_ne!(saved.version, r.version);
        assert_eq!(
            std::fs::read_to_string(&f).unwrap(),
            "services:\n  web:\n    image: nginx\n"
        );
        assert_eq!(
            std::fs::metadata(&f).unwrap().permissions().mode() & 0o777,
            0o640,
            "conserva los permisos"
        );
        // Otro programa lo cambia: guardar con la versión antigua no sobrescribe.
        std::fs::write(&f, "cambiado fuera\n").unwrap();
        assert_eq!(
            save(path, "mío\n", &saved.version).unwrap_err(),
            EngineError::FileChanged { path: path.into() }
        );
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "cambiado fuera\n");
        let leftovers: Vec<_> = std::fs::read_dir(d.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name())
            .collect();
        assert_eq!(leftovers.len(), 1, "no quedan temporales: {leftovers:?}");
    }

    #[test]
    fn read_rejects_bad_input() {
        let d = tempfile::tempdir().unwrap();
        assert!(read("relativo.yaml").is_err());
        assert!(read(d.path().to_str().unwrap()).is_err(), "una carpeta");
        let big = d.path().join("big.yaml");
        std::fs::write(&big, vec![b'a'; MAX_FILE_BYTES + 1]).unwrap();
        assert!(read(big.to_str().unwrap()).is_err());
        let bin = d.path().join("bin.yaml");
        std::fs::write(&bin, [0xff, 0xfe, 0x00]).unwrap();
        assert!(read(bin.to_str().unwrap()).is_err());
    }

    #[test]
    fn create_refuses_to_overwrite_any_compose_file() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().to_str().unwrap();
        let c = create(dir, "services: {}\n").unwrap();
        assert!(c.path.ends_with("/compose.yaml"));
        assert!(matches!(
            create(dir, "x").unwrap_err(),
            EngineError::AlreadyExists { .. }
        ));
        let d2 = tempfile::tempdir().unwrap();
        std::fs::write(d2.path().join("docker-compose.yml"), "viejo").unwrap();
        assert!(matches!(
            create(d2.path().to_str().unwrap(), "x").unwrap_err(),
            EngineError::AlreadyExists { .. }
        ));
        assert_eq!(
            std::fs::read_to_string(d2.path().join("docker-compose.yml")).unwrap(),
            "viejo"
        );
    }
}
