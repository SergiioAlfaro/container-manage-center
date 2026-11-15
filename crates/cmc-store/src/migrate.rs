// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Paso de las carpetas del nombre anterior a las nuevas (1.8.0, D-113).
//!
//! Se ejecuta al arrancar, antes de abrir ningún archivo de datos:
//!
//! | Antes (hasta la 1.7.0) | Ahora |
//! |---|---|
//! | `$XDG_CONFIG_HOME/docker-control-center` | `$XDG_CONFIG_HOME/container-manage-center` |
//! | `$XDG_STATE_HOME/docker-control-center` | `$XDG_STATE_HOME/container-manage-center` |
//! | `$XDG_DATA_HOME/org.dockercontrolcenter.app` (WebKit) | `$XDG_DATA_HOME/<identificador nuevo>` |
//! | `$XDG_CACHE_HOME/org.dockercontrolcenter.app` (WebKit) | `$XDG_CACHE_HOME/<identificador nuevo>` |
//!
//! Cómo, para no perder nada:
//! - Si la carpeta nueva no existe, la antigua se **renombra** (`rename`, en
//!   la misma carpeta padre y por tanto en el mismo sistema de archivos):
//!   es atómico, así que en todo momento existe una de las dos completa y
//!   nunca queda a medias. No se copia ni se borra ningún archivo.
//! - Si `rename` no es posible (la carpeta antigua está en otro sistema de
//!   archivos, por ejemplo por un enlace), se **copia** a una carpeta
//!   temporal, se **comprueba** (mismos archivos, con el mismo tamaño y contenido), se pone
//!   en su sitio y solo entonces se **borra** la antigua.
//! - Si existen las dos, lo que falte en la nueva se mueve desde la antigua;
//!   lo que ya exista en la nueva no se toca, y la antigua se conserva con
//!   esos archivos (se informa: nada se borra sin haberse movido).
//! - Repetirla no hace nada: sin carpeta antigua no hay migración.
//! - Con `CMC_CONFIG` o `CMC_STATE_DIR` (pruebas y usos avanzados) no se migra.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::legacy;

/// Resultado de una migración, para el registro y para avisar al usuario.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    /// Carpetas pasadas al nombre nuevo (antes → ahora).
    pub moved: Vec<(PathBuf, PathBuf)>,
    /// Carpetas antiguas que se conservan porque tenían archivos que ya
    /// existían en la nueva (con la lista de esos archivos).
    pub kept: Vec<(PathBuf, Vec<PathBuf>)>,
    /// Errores: la carpeta antigua sigue intacta.
    pub errors: Vec<(PathBuf, String)>,
}

impl MigrationReport {
    pub fn is_empty(&self) -> bool {
        self.moved.is_empty() && self.kept.is_empty() && self.errors.is_empty()
    }
    fn merge(&mut self, other: MigrationReport) {
        self.moved.extend(other.moved);
        self.kept.extend(other.kept);
        self.errors.extend(other.errors);
    }
}

/// Migra las carpetas por defecto del usuario. `new_identifier` es el
/// identificador de Tauri de esta versión.
pub fn migrate_default_dirs(new_identifier: &str) -> MigrationReport {
    let mut report = MigrationReport::default();
    if std::env::var_os("CMC_CONFIG").is_some() || std::env::var_os("CMC_STATE_DIR").is_some() {
        return report;
    }
    let state = dirs::state_dir().or_else(|| dirs::home_dir().map(|h| h.join(".local/state")));
    let pairs = [
        (dirs::config_dir(), legacy::APP_DIR, crate::APP_DIR),
        (state, legacy::APP_DIR, crate::APP_DIR),
        (dirs::data_dir(), legacy::TAURI_IDENTIFIER, new_identifier),
        (dirs::cache_dir(), legacy::TAURI_IDENTIFIER, new_identifier),
    ];
    for (base, old, new) in pairs {
        if let Some(base) = base {
            report.merge(migrate_dir(&base.join(old), &base.join(new)));
        }
    }
    report
}

/// Pasa `old` a `new` según las reglas del módulo.
pub fn migrate_dir(old: &Path, new: &Path) -> MigrationReport {
    let mut r = MigrationReport::default();
    let old_meta = match fs::symlink_metadata(old) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return r,
        Err(e) => {
            r.errors.push((old.to_owned(), e.to_string()));
            return r;
        }
    };
    if !(old_meta.is_dir() || old_meta.file_type().is_symlink() && old.is_dir()) {
        return r;
    }
    if fs::symlink_metadata(new).is_err() {
        match fs::rename(old, new) {
            Ok(()) => {
                sync_parent(new);
                tracing::info!(from = %old.display(), to = %new.display(), "carpeta migrada al nombre nuevo");
                r.moved.push((old.to_owned(), new.to_owned()));
            }
            Err(_) => match copy_verify_swap(old, new) {
                Ok(()) => r.moved.push((old.to_owned(), new.to_owned())),
                Err(e) => r.errors.push((old.to_owned(), e.to_string())),
            },
        }
        return r;
    }
    // Existen las dos: se mueve lo que falte, sin sobrescribir nada.
    let mut conflicts = Vec::new();
    if let Err(e) = merge_into(old, new, Path::new(""), &mut conflicts) {
        r.errors.push((old.to_owned(), e.to_string()));
        return r;
    }
    if conflicts.is_empty() {
        let _ = remove_empty_tree(old);
        if fs::symlink_metadata(old).is_err() {
            r.moved.push((old.to_owned(), new.to_owned()));
        } else {
            r.kept.push((old.to_owned(), Vec::new()));
        }
    } else {
        r.kept.push((old.to_owned(), conflicts));
    }
    r
}

fn merge_into(old: &Path, new: &Path, rel: &Path, conflicts: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(old.join(rel))? {
        let entry = entry?;
        let rel = rel.join(entry.file_name());
        let (src, dst) = (old.join(&rel), new.join(&rel));
        match fs::symlink_metadata(&dst) {
            Err(_) => fs::rename(&src, &dst)?,
            Ok(m) if m.is_dir() && entry.file_type()?.is_dir() => {
                merge_into(old, new, &rel, conflicts)?
            }
            Ok(_) => conflicts.push(rel),
        }
    }
    Ok(())
}

/// Borra las carpetas vacías que queden (nunca un archivo).
fn remove_empty_tree(p: &Path) -> io::Result<()> {
    for entry in fs::read_dir(p)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            remove_empty_tree(&entry.path())?;
        }
    }
    fs::remove_dir(p)
}

/// Copia `old` a una carpeta temporal junto a `new`, comprueba la copia, la
/// pone en su sitio y solo entonces borra `old`.
fn copy_verify_swap(old: &Path, new: &Path) -> io::Result<()> {
    let tmp = new.with_file_name(format!(
        ".{}.migrando",
        new.file_name().and_then(|n| n.to_str()).unwrap_or("cmc")
    ));
    let _ = fs::remove_dir_all(&tmp);
    let result = (|| {
        copy_tree(old, &tmp)?;
        verify_same(old, &tmp)?;
        fs::rename(&tmp, new)?;
        sync_parent(new);
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&tmp);
        return result;
    }
    // La copia está comprobada y en su sitio: ya se puede borrar la antigua.
    if fs::symlink_metadata(old)?.file_type().is_symlink() {
        fs::remove_file(old)
    } else {
        fs::remove_dir_all(old)
    }
}

fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir(dst)?;
    fs::set_permissions(dst, fs::metadata(src)?.permissions())?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let (s, d) = (entry.path(), dst.join(entry.file_name()));
        let ft = entry.file_type()?;
        if ft.is_dir() {
            copy_tree(&s, &d)?;
        } else if ft.is_symlink() {
            #[cfg(unix)]
            std::os::unix::fs::symlink(fs::read_link(&s)?, &d)?;
        } else if ft.is_file() {
            fs::copy(&s, &d)?;
            fs::File::open(&d)?.sync_all()?;
        }
        // Sockets y otros archivos especiales no son datos: no se copian.
    }
    Ok(())
}

fn digest(p: &Path) -> io::Result<(u64, u64)> {
    // FNV-1a de 64 bits sobre el contenido completo, más el tamaño: basta para
    // detectar una copia incompleta o distinta (no es una firma de seguridad).
    let mut f = fs::File::open(p)?;
    let mut buf = [0u8; 64 * 1024];
    let (mut h, mut n) = (0xcbf2_9ce4_8422_2325u64, 0u64);
    loop {
        let k = f.read(&mut buf)?;
        if k == 0 {
            return Ok((n, h));
        }
        n += k as u64;
        for b in &buf[..k] {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

fn verify_same(a: &Path, b: &Path) -> io::Result<()> {
    for entry in fs::read_dir(a)? {
        let entry = entry?;
        let (x, y) = (entry.path(), b.join(entry.file_name()));
        let ft = entry.file_type()?;
        if ft.is_dir() {
            verify_same(&x, &y)?;
        } else if ft.is_file() && digest(&x)? != digest(&y)? {
            return Err(io::Error::other(format!(
                "la copia de {} no coincide con el original",
                x.display()
            )));
        }
    }
    Ok(())
}

fn sync_parent(p: &Path) {
    if let Some(parent) = p.parent() {
        if let Ok(f) = fs::File::open(parent) {
            let _ = f.sync_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn write(p: &Path, s: &str) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, s).unwrap();
    }

    #[test]
    fn without_the_old_folder_nothing_happens() {
        let d = tempfile::tempdir().unwrap();
        let r = migrate_dir(&d.path().join("viejo"), &d.path().join("nuevo"));
        assert!(r.is_empty());
        assert!(!d.path().join("nuevo").exists());
    }

    #[test]
    fn the_old_folder_is_renamed_with_all_its_files_and_permissions() {
        let d = tempfile::tempdir().unwrap();
        let (old, new) = (
            d.path().join("docker-control-center"),
            d.path().join("container-manage-center"),
        );
        write(&old.join("config.toml"), "theme = \"dark\"\n");
        write(&old.join("servers.toml"), "[[servers]]\nid = \"srv\"\n");
        write(&old.join("monitor/minutos.jsonl"), "{}\n");
        fs::set_permissions(old.join("servers.toml"), fs::Permissions::from_mode(0o600)).unwrap();
        let r = migrate_dir(&old, &new);
        assert_eq!(r.moved, vec![(old.clone(), new.clone())]);
        assert!(r.errors.is_empty() && r.kept.is_empty());
        assert!(!old.exists());
        assert_eq!(
            fs::read_to_string(new.join("servers.toml")).unwrap(),
            "[[servers]]\nid = \"srv\"\n"
        );
        assert_eq!(
            fs::read_to_string(new.join("monitor/minutos.jsonl")).unwrap(),
            "{}\n"
        );
        let mode = fs::metadata(new.join("servers.toml"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        // Repetirla no hace nada.
        assert!(migrate_dir(&old, &new).is_empty());
    }

    #[test]
    fn with_both_folders_only_missing_files_move_and_nothing_is_overwritten() {
        let d = tempfile::tempdir().unwrap();
        let (old, new) = (d.path().join("viejo"), d.path().join("nuevo"));
        write(&old.join("config.toml"), "antiguo");
        write(&old.join("servers.toml"), "servidores");
        write(&old.join("copias/a.json"), "a");
        write(&new.join("config.toml"), "nuevo");
        write(&new.join("copias/b.json"), "b");
        let r = migrate_dir(&old, &new);
        assert_eq!(
            r.kept,
            vec![(old.clone(), vec![PathBuf::from("config.toml")])]
        );
        assert_eq!(
            fs::read_to_string(new.join("config.toml")).unwrap(),
            "nuevo"
        );
        assert_eq!(
            fs::read_to_string(new.join("servers.toml")).unwrap(),
            "servidores"
        );
        assert_eq!(fs::read_to_string(new.join("copias/a.json")).unwrap(), "a");
        assert_eq!(fs::read_to_string(new.join("copias/b.json")).unwrap(), "b");
        // Lo que no se pudo mover sigue en la antigua.
        assert_eq!(
            fs::read_to_string(old.join("config.toml")).unwrap(),
            "antiguo"
        );
        assert!(!old.join("servers.toml").exists());
    }

    #[test]
    fn with_both_folders_and_no_conflicts_the_old_one_disappears() {
        let d = tempfile::tempdir().unwrap();
        let (old, new) = (d.path().join("viejo"), d.path().join("nuevo"));
        write(&old.join("estado/actividad.jsonl"), "x");
        fs::create_dir_all(&new).unwrap();
        let r = migrate_dir(&old, &new);
        assert_eq!(r.moved, vec![(old.clone(), new.clone())]);
        assert!(!old.exists());
        assert_eq!(
            fs::read_to_string(new.join("estado/actividad.jsonl")).unwrap(),
            "x"
        );
    }

    #[test]
    fn copy_verify_swap_keeps_the_original_until_the_copy_is_checked() {
        let d = tempfile::tempdir().unwrap();
        let (old, new) = (d.path().join("viejo"), d.path().join("nuevo"));
        write(&old.join("a/b.txt"), "datos");
        std::os::unix::fs::symlink("b.txt", old.join("a/enlace")).unwrap();
        copy_verify_swap(&old, &new).unwrap();
        assert!(!old.exists());
        assert_eq!(fs::read_to_string(new.join("a/b.txt")).unwrap(), "datos");
        assert_eq!(
            fs::read_link(new.join("a/enlace")).unwrap(),
            PathBuf::from("b.txt")
        );
        assert!(!d.path().join(".nuevo.migrando").exists());
    }

    #[test]
    fn a_failed_copy_leaves_the_original_untouched() {
        let d = tempfile::tempdir().unwrap();
        let old = d.path().join("viejo");
        write(&old.join("a.txt"), "datos");
        // El destino está dentro de una ruta que no existe: la copia falla.
        let new = d.path().join("no/existe/nuevo");
        assert!(copy_verify_swap(&old, &new).is_err());
        assert_eq!(fs::read_to_string(old.join("a.txt")).unwrap(), "datos");
    }

    #[test]
    fn a_file_with_the_old_name_is_not_ours_and_is_left_alone() {
        let d = tempfile::tempdir().unwrap();
        let old = d.path().join("docker-control-center");
        fs::write(&old, "otra cosa").unwrap();
        assert!(migrate_dir(&old, &d.path().join("nuevo")).is_empty());
        assert!(old.is_file());
    }
}
