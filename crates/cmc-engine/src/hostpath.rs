// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Rutas del equipo y rutas de los servidores (1.9.1, auditoría de Windows).
//!
//! Las rutas **del servidor** (servidores remotos por SSH) y las rutas **dentro
//! de los contenedores** son siempre de Linux: empiezan por `/`. Las rutas
//! **del equipo** donde se ejecuta la aplicación dependen del sistema: en
//! Linux, `/…`; en Windows, `C:\…`, `C:/…` o `\\servidor\recurso\…` (lo que
//! devuelve el selector de archivos y lo que Docker Desktop acepta como origen
//! de un bind mount o como archivo Compose).
//!
//! Las funciones `windows_*` son puras y se compilan en todas las plataformas
//! para poder probarlas en Linux; solo se usan en Windows. En Linux, todo se
//! comporta exactamente igual que antes.

/// Ruta absoluta de Linux (`/…`): servidores remotos y contenedores.
pub fn is_unix_absolute(p: &str) -> bool {
    p.starts_with('/')
}

/// `C:\…`, `c:/…` o `\\servidor\recurso\…` (no `\\.\` ni `\\?\`: dispositivos).
pub fn is_windows_absolute(p: &str) -> bool {
    let b = p.as_bytes();
    let drive = b.len() >= 3
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && (b[2] == b'\\' || b[2] == b'/');
    let unc = p.len() > 2
        && (p.starts_with(r"\\") || p.starts_with("//"))
        && !matches!(b.get(2), Some(b'.') | Some(b'?') | Some(b'\\') | Some(b'/'));
    drive || unc
}

/// Ruta absoluta del equipo donde se ejecuta la aplicación.
pub fn is_local_absolute(p: &str) -> bool {
    #[cfg(windows)]
    {
        is_windows_absolute(p)
    }
    #[cfg(not(windows))]
    {
        is_unix_absolute(p)
    }
}

/// Origen de un bind mount en un Docker **local**: en Windows, una ruta del
/// equipo o una ruta de Linux de la máquina virtual de Docker Desktop
/// (`/var/run/docker.sock`, `/mnt/wsl/…`); en Linux, `/…`.
pub fn is_local_bind_source(p: &str) -> bool {
    is_unix_absolute(p) || (cfg!(windows) && is_windows_absolute(p))
}

/// ¿Tiene algún componente `..`? (con `/` y, para rutas de Windows, `\`).
pub fn has_parent_component(p: &str) -> bool {
    p.split(['/', '\\']).any(|c| c == "..")
}

/// Normalización léxica de una ruta de Windows para comparar: `\` → `/`, sin
/// el prefijo `\\?\` (lo pone `canonicalize`), letra de unidad y resto en
/// minúsculas (Windows no distingue), sin `.` ni barra final. `None` si no es
/// absoluta o tiene `..`. `C:\Users\Ana\` → `c:/users/ana`; `C:\` → `c:`.
pub fn windows_normalize(p: &str) -> Option<String> {
    let p = p.trim();
    if p.contains('\0') {
        return None;
    }
    let p = p
        .strip_prefix(r"\\?\UNC\")
        .map(|r| format!(r"\\{r}"))
        .or_else(|| p.strip_prefix(r"\\?\").map(str::to_owned))
        .unwrap_or_else(|| p.to_owned());
    if !is_windows_absolute(&p) || has_parent_component(&p) {
        return None;
    }
    let unc = p.starts_with(r"\\") || p.starts_with("//");
    let parts: Vec<String> = p
        .split(['/', '\\'])
        .filter(|c| !c.is_empty() && *c != ".")
        .map(|c| c.to_lowercase())
        .collect();
    Some(if unc {
        format!("//{}", parts.join("/"))
    } else {
        parts.join("/")
    })
}

/// Carpetas del sistema de Windows (en cualquier unidad): `Windows`,
/// `Program Files`, `Program Files (x86)`, `ProgramData`, `$Recycle.Bin` y
/// `System Volume Information`.
pub const WINDOWS_SYSTEM_DIRS: [&str; 6] = [
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
    "$recycle.bin",
    "system volume information",
];

/// Componentes de una ruta normalizada con [`windows_normalize`] después de
/// la unidad (`c:`) o del recurso compartido (`//servidor/recurso`).
pub fn windows_components(norm: &str) -> Vec<&str> {
    let skip = if norm.starts_with("//") { 2 } else { 1 };
    norm.split('/')
        .filter(|c| !c.is_empty())
        .skip(skip)
        .collect()
}

/// ¿Es (o está dentro de) una carpeta del sistema de Windows?
pub fn windows_system_dir(norm: &str) -> bool {
    windows_components(norm)
        .first()
        .is_some_and(|c| WINDOWS_SYSTEM_DIRS.contains(c))
}

/// Separa `origen:destino[:opciones]` de un bind mount de `HostConfig.Binds`,
/// admitiendo la letra de unidad de Windows en el origen (`C:\datos:/datos:ro`).
pub fn split_bind(b: &str) -> Option<(&str, &str, Option<&str>)> {
    let bytes = b.as_bytes();
    let skip = if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
    {
        2
    } else {
        0
    };
    let rest = &b[skip..];
    let mut it = rest.splitn(3, ':');
    let first = it.next()?;
    let target = it.next()?;
    let source = &b[..skip + first.len()];
    Some((source, target, it.next()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_absolute() {
        for ok in [
            r"C:\Users\ana",
            "c:/datos",
            r"D:\",
            r"\\nas\copias\cmc",
            "//nas/copias",
        ] {
            assert!(is_windows_absolute(ok), "{ok}");
        }
        for bad in [
            "/home/ana",
            r"datos\x",
            "C:",
            "C:datos",
            r"\\.\pipe\docker_engine",
            r"\\?\C:\x",
            r"\\",
            "",
        ] {
            assert!(!is_windows_absolute(bad), "{bad}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn linux_local_paths_unchanged() {
        assert!(is_local_absolute("/home/ana/compose.yaml"));
        assert!(!is_local_absolute(r"C:\Users\ana\compose.yaml"));
        assert!(is_local_bind_source("/srv/datos"));
        assert!(!is_local_bind_source(r"C:\datos"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_local_paths() {
        assert!(is_local_absolute(r"C:\Users\ana\compose.yaml"));
        assert!(!is_local_absolute("/home/ana/compose.yaml"));
        assert!(is_local_bind_source(r"C:\datos"));
        assert!(is_local_bind_source("/var/run/docker.sock"));
    }

    #[test]
    fn parent_components() {
        assert!(has_parent_component(r"C:\a\..\b"));
        assert!(has_parent_component("/a/../b"));
        assert!(!has_parent_component(r"C:\a\..b\c"));
    }

    #[test]
    fn normalizes_windows_paths() {
        assert_eq!(
            windows_normalize(r"C:\Users\Ana\").as_deref(),
            Some("c:/users/ana")
        );
        assert_eq!(windows_normalize(r"C:\").as_deref(), Some("c:"));
        assert_eq!(
            windows_normalize(r"\\?\C:\Users\.\Ana").as_deref(),
            Some("c:/users/ana")
        );
        assert_eq!(
            windows_normalize(r"\\?\UNC\nas\c").as_deref(),
            Some("//nas/c")
        );
        assert_eq!(
            windows_normalize(r"\\NAS\Copias").as_deref(),
            Some("//nas/copias")
        );
        assert_eq!(windows_normalize(r"C:\a\..\b"), None);
        assert_eq!(windows_normalize("/home/ana"), None);
    }

    #[test]
    fn splits_binds_with_drive_letters() {
        assert_eq!(
            split_bind("/srv:/data:ro"),
            Some(("/srv", "/data", Some("ro")))
        );
        assert_eq!(split_bind("datos:/data"), Some(("datos", "/data", None)));
        assert_eq!(
            split_bind(r"C:\datos:/data"),
            Some((r"C:\datos", "/data", None))
        );
        assert_eq!(
            split_bind("c:/Mis datos:/data:ro,z"),
            Some(("c:/Mis datos", "/data", Some("ro,z")))
        );
        assert_eq!(split_bind("solo"), None);
    }
}
