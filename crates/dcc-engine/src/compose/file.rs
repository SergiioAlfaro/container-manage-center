// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
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

pub(crate) fn not_allowed(path: &str, reason: &str) -> EngineError {
    EngineError::PathNotAllowed {
        path: path.to_owned(),
        reason: reason.to_owned(),
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
    if path.is_empty()
        || path.contains('\0')
        || path.contains('\n')
        || !path.starts_with('/')
        || path.len() > 4096
    {
        return Err(invalid(format!("ruta no válida: {path:?}")));
    }
    Ok(PathBuf::from(path))
}

/// Carpetas del sistema donde nunca hay que leer ni escribir un archivo
/// Compose desde la aplicación (E2, DCC-SEC-06, D-089).
const DENIED_DIRS: [&str; 10] = [
    "/etc", "/root", "/proc", "/sys", "/dev", "/boot", "/run", "/usr", "/var/lib", "/var/run",
];

/// Reglas de las rutas de los archivos Compose (E2, DCC-SEC-06, D-089), para
/// el equipo y para los servidores remotos. Así, aunque algún día se pudiera
/// ejecutar código en la interfaz, estas órdenes no servirían para leer ni
/// sobrescribir otros archivos del usuario (claves SSH, `~/.bashrc`…):
/// * solo archivos YAML (`.yml`, `.yaml`), que es lo que lee Compose;
/// * ninguna carpeta oculta en la ruta (`~/.ssh`, `~/.docker`, `~/.config`,
///   `~/.kube`, `~/.gnupg`…), donde se guardan credenciales y configuración;
/// * ninguna carpeta del sistema (`/etc`, `/root`, `/proc`, `/usr`…);
/// * ruta absoluta y sin `..` (los discos de `/run/media` sí se permiten).
///
/// Si la ruta es un enlace simbólico, se aplican también a su destino real.
pub fn check_compose_path(path: &str) -> Result<(), EngineError> {
    absolute(path)?;
    let norm =
        crate::risk::normalize(path).ok_or_else(|| invalid(format!("ruta no válida: {path:?}")))?;
    let name = norm.rsplit('/').next().unwrap_or("");
    let lower = name.to_ascii_lowercase();
    if !(lower.ends_with(".yml") || lower.ends_with(".yaml")) {
        return Err(not_allowed(path, "solo archivos .yml o .yaml"));
    }
    let dirs: Vec<&str> = norm.split('/').filter(|c| !c.is_empty()).collect();
    if dirs[..dirs.len().saturating_sub(1)]
        .iter()
        .any(|c| c.starts_with('.'))
    {
        return Err(not_allowed(path, "carpeta oculta, como ~/.ssh o ~/.config"));
    }
    // Excepción: los discos extraíbles se montan en `/run/media/<usuario>` en
    // algunas distribuciones (en Debian, en `/media`).
    let removable = norm.starts_with("/run/media/");
    if !removable
        && DENIED_DIRS
            .iter()
            .any(|d| norm == *d || norm.starts_with(&format!("{d}/")))
    {
        return Err(not_allowed(path, "carpeta del sistema"));
    }
    Ok(())
}

/// Ruta real (sigue los enlaces simbólicos) comprobada con las mismas reglas.
/// Si el archivo aún no existe, la ruta escrita.
fn resolved(path: &str) -> Result<PathBuf, EngineError> {
    check_compose_path(path)?;
    match std::fs::canonicalize(path) {
        Ok(real) => {
            let r = real.to_string_lossy().into_owned();
            if r != path {
                check_compose_path(&r).map_err(|_| not_allowed(path, &format!("enlace a {r}")))?;
            }
            Ok(real)
        }
        Err(_) => absolute(path),
    }
}

fn writable(path: &Path) -> bool {
    std::fs::OpenOptions::new().append(true).open(path).is_ok()
}

pub fn read(path: &str) -> Result<ComposeFile, EngineError> {
    let p = resolved(path)?;
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
/// Si `path` es un enlace simbólico, se escribe en su destino real y el enlace
/// se conserva (E2, DCC-SEC-13).
pub fn save(path: &str, content: &str, expected_version: &str) -> Result<ComposeFile, EngineError> {
    let p = resolved(path)?;
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
    // Mismas reglas de rutas que al leer y guardar (D-089), también para la
    // carpeta real si es un enlace.
    let target = |d: &str| format!("{}/{}", d.trim_end_matches('/'), DEFAULT_FILES[0]);
    check_compose_path(&target(dir))?;
    if let Ok(real) = std::fs::canonicalize(&d) {
        check_compose_path(&target(&real.to_string_lossy()))?;
    }
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

    /// `tempfile` crea `.tmpXXXX` (carpeta oculta, rechazada por la política).
    fn tmp() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("dcc-test")
            .tempdir()
            .unwrap()
    }

    #[test]
    fn read_and_save_with_conflict_detection() {
        let d = tmp();
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
        let d = tmp();
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
        let d = tmp();
        let dir = d.path().to_str().unwrap();
        let c = create(dir, "services: {}\n").unwrap();
        assert!(c.path.ends_with("/compose.yaml"));
        assert!(matches!(
            create(dir, "x").unwrap_err(),
            EngineError::AlreadyExists { .. }
        ));
        let d2 = tmp();
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

    #[test]
    fn path_policy_blocks_sensitive_files() {
        for bad in [
            "/etc/passwd",
            "/etc/docker/compose.yaml",
            "/root/compose.yaml",
            "/home/ana/.ssh/id_ed25519",
            "/home/ana/.ssh/config.yaml",
            "/home/ana/.docker/config.json",
            "/home/ana/.config/app/compose.yml",
            "/home/ana/.bashrc",
            "/home/ana/proyecto/notas.txt",
            "/home/ana/proyecto/compose",
            "/home/ana/proyecto/../.ssh/x.yaml",
            "/proc/self/environ",
            "/var/lib/docker/x.yaml",
            "/var/run/docker.sock",
            "/run/user/1000/x.yaml",
            "/usr/share/x.yml",
            "relativo.yaml",
        ] {
            assert!(
                matches!(
                    check_compose_path(bad),
                    Err(EngineError::PathNotAllowed { .. } | EngineError::InvalidInput { .. })
                ),
                "{bad} debería rechazarse"
            );
        }
        for good in [
            "/home/ana/proyecto/compose.yaml",
            "/home/ana/proyecto/docker-compose.yml",
            "/home/ana/proyecto/compose.prod.YAML",
            "/srv/tienda/compose.yaml",
            "/opt/app/docker-compose.override.yml",
            "/media/ana/disco/web/compose.yaml",
            "/run/media/ana/disco/web/compose.yaml",
            "/tmp/prueba/compose.yaml",
        ] {
            assert!(
                check_compose_path(good).is_ok(),
                "{good} debería permitirse"
            );
        }
    }

    #[test]
    fn symlinks_are_checked_by_their_real_target_and_kept_on_save() {
        let d = tmp();
        // Un enlace con nombre YAML que apunta a un archivo no permitido.
        let hidden = d.path().join(".secreto");
        std::fs::create_dir(&hidden).unwrap();
        let key = hidden.join("id.yaml");
        std::fs::write(&key, "clave\n").unwrap();
        let trap = d.path().join("compose.yaml");
        std::os::unix::fs::symlink(&key, &trap).unwrap();
        let trap = trap.to_str().unwrap();
        assert!(matches!(
            read(trap),
            Err(EngineError::PathNotAllowed { .. })
        ));
        assert!(matches!(
            save(trap, "x", &version_of(b"clave\n")),
            Err(EngineError::PathNotAllowed { .. })
        ));
        assert_eq!(std::fs::read_to_string(&key).unwrap(), "clave\n");

        // Un enlace legítimo (compose.yaml -> real/compose.yaml): se guarda
        // en el destino y el enlace sigue siendo un enlace.
        let d2 = tmp();
        let real_dir = d2.path().join("real");
        std::fs::create_dir(&real_dir).unwrap();
        let real = real_dir.join("compose.yaml");
        std::fs::write(&real, "services: {}\n").unwrap();
        let link = d2.path().join("compose.yaml");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let link_s = link.to_str().unwrap();
        let r = read(link_s).unwrap();
        assert_eq!(r.path, link_s);
        save(link_s, "services:\n  a:\n    image: x\n", &r.version).unwrap();
        assert!(std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            std::fs::read_to_string(&real).unwrap(),
            "services:\n  a:\n    image: x\n"
        );
    }

    #[test]
    fn create_checks_the_folder() {
        let d = tmp();
        let hidden = d.path().join(".oculta");
        std::fs::create_dir(&hidden).unwrap();
        assert!(matches!(
            create(hidden.to_str().unwrap(), "services: {}\n"),
            Err(EngineError::PathNotAllowed { .. })
        ));
        // Enlace visible a una carpeta oculta: se comprueba la real.
        let link = d.path().join("visible");
        std::os::unix::fs::symlink(&hidden, &link).unwrap();
        assert!(matches!(
            create(link.to_str().unwrap(), "services: {}\n"),
            Err(EngineError::PathNotAllowed { .. })
        ));
        assert!(!hidden.join("compose.yaml").exists());
        assert!(create("/etc", "x").is_err());
    }
}
