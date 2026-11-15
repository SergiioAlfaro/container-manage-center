// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Destino de las copias y formato en disco (E4, D-096).
//!
//! ```text
//! <carpeta elegida>/container-manage-center-copias/    0700
//!   <servidor>/                                        0700
//!     20261114T103000Z-1a2b3c/                         0700  copia completa
//!       manifest.json                                  0600  (se escribe el último)
//!       volumes/datos.tar                              0600
//!       databases/db.sql                               0600
//!       containers/web.json                            0600
//!       compose/tienda/compose.yaml                    0600
//!       app/config.toml                                0600
//!     20261114T110000Z-4d5e6f.partial/                       interrumpida
//! ```
//!
//! Una copia se escribe en `<id>.partial` y solo al terminar, con todos los
//! archivos sincronizados y el manifiesto con sus SHA-256 escrito, se renombra
//! a `<id>`. Una copia interrumpida nunca se da por buena ni se restaura.

use crate::fsx::{DirBuilderExt, OpenOptionsExt};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, Datelike, Utc};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use super::model::{
    BackupEntry, BackupState, IntegrityReport, Manifest, ManifestFile, Retention, FORMAT, MANIFEST,
};

/// Carpeta que se crea dentro del destino elegido (1.8.0, D-113). La del
/// nombre anterior se renombra a esta la primera vez que se usa el destino.
pub const ROOT_DIR: &str = "container-manage-center-copias";
pub const PARTIAL_SUFFIX: &str = ".partial";

/// Destino de copias. 1.4.0 solo tiene carpetas locales; la interfaz deja
/// sitio para destinos remotos (los archivos se preparan en local y se suben).
pub trait BackupTarget: Send + Sync {
    /// Descripción para mostrar (la ruta, en local).
    fn describe(&self) -> String;
    /// Empieza una copia nueva.
    fn begin(&self, server: &str, id: &str) -> io::Result<PartialBackup>;
    /// Copias de un servidor, de la más reciente a la más antigua.
    fn list(&self, server: &str) -> io::Result<Vec<BackupEntry>>;
    /// Carpeta local de una copia completa, para leerla o restaurarla.
    fn open(&self, server: &str, id: &str) -> io::Result<PathBuf>;
    /// Elimina una copia (completa o interrumpida).
    fn delete(&self, server: &str, id: &str) -> io::Result<()>;
}

/// Carpeta local elegida por el usuario.
#[derive(Debug, Clone)]
pub struct LocalTarget {
    root: PathBuf,
}

fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.into())
}

/// Un nombre seguro para un componente de ruta (servidor, id).
fn safe_component(s: &str) -> io::Result<&str> {
    let ok = !s.is_empty()
        && s.len() <= 128
        && s != "."
        && s != ".."
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if ok {
        Ok(s)
    } else {
        Err(invalid(format!("nombre no válido: {s:?}")))
    }
}

fn mkdir_private(p: &Path) -> io::Result<()> {
    match fs::DirBuilder::new().mode(0o700).create(p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            let meta = fs::symlink_metadata(p)?;
            if !meta.is_dir() || meta.file_type().is_symlink() {
                return Err(invalid(format!("{} no es una carpeta", p.display())));
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn sync_dir(p: &Path) {
    if let Ok(f) = fs::File::open(p) {
        let _ = f.sync_all();
    }
}

/// Pasa la carpeta de copias del nombre anterior (hasta la 1.7.0) a la nueva,
/// dentro del mismo destino (D-113).
/// - Sin carpeta nueva: un `rename`, atómico en el mismo sistema de archivos;
///   no se copia ni se reescribe ninguna copia.
/// - Con las dos (p. ej. la nueva ya creada por una copia): cada carpeta de
///   servidor y cada copia que falte en la nueva se mueve a ella; lo que ya
///   existe con el mismo nombre no se toca y la carpeta antigua se conserva
///   con ello (no se borra nada que no se haya movido).
/// - Un error se devuelve: nunca se dan las copias antiguas por perdidas en
///   silencio.
pub fn migrate_legacy_root(old: &Path, new: &Path) -> io::Result<()> {
    let old_meta = match fs::symlink_metadata(old) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    if !old_meta.is_dir() {
        // Un enlace o un archivo con ese nombre: no es nuestro, no se toca.
        return Ok(());
    }
    if fs::symlink_metadata(new).is_err() {
        fs::rename(old, new)?;
        if let Some(parent) = new.parent() {
            sync_dir(parent);
        }
        tracing::info!(from = %old.display(), to = %new.display(), "carpeta de copias renombrada");
        return Ok(());
    }
    mkdir_private(new)?;
    for server in fs::read_dir(old)? {
        let server = server?;
        let sname = server.file_name();
        let (src, dst) = (server.path(), new.join(&sname));
        if !server.file_type()?.is_dir() {
            continue;
        }
        if fs::symlink_metadata(&dst).is_err() {
            fs::rename(&src, &dst)?;
            continue;
        }
        for entry in fs::read_dir(&src)? {
            let entry = entry?;
            let target = dst.join(entry.file_name());
            if fs::symlink_metadata(&target).is_err() {
                fs::rename(entry.path(), target)?;
            }
        }
        let _ = fs::remove_dir(&src);
    }
    sync_dir(new);
    // Solo se borra si ha quedado vacía.
    let _ = fs::remove_dir(old);
    Ok(())
}

impl LocalTarget {
    /// `dest`: carpeta absoluta elegida por el usuario. Debe existir.
    pub fn new(dest: &Path) -> io::Result<Self> {
        if !dest.is_absolute() {
            return Err(invalid("la carpeta de destino debe ser una ruta absoluta"));
        }
        let meta = fs::metadata(dest)?;
        if !meta.is_dir() {
            return Err(invalid(format!("{} no es una carpeta", dest.display())));
        }
        let root = dest.join(ROOT_DIR);
        migrate_legacy_root(&dest.join(crate::legacy::BACKUP_ROOT_DIR), &root)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn server_dir(&self, server: &str) -> io::Result<PathBuf> {
        Ok(self.root.join(safe_component(server)?))
    }
}

impl BackupTarget for LocalTarget {
    fn describe(&self) -> String {
        self.root.display().to_string()
    }

    fn begin(&self, server: &str, id: &str) -> io::Result<PartialBackup> {
        mkdir_private(&self.root)?;
        let sdir = self.server_dir(server)?;
        mkdir_private(&sdir)?;
        let id = safe_component(id)?.to_owned();
        let dir = sdir.join(format!("{id}{PARTIAL_SUFFIX}"));
        fs::DirBuilder::new().mode(0o700).create(&dir)?;
        Ok(PartialBackup {
            dir,
            final_dir: sdir.join(&id),
        })
    }

    fn list(&self, server: &str) -> io::Result<Vec<BackupEntry>> {
        let sdir = self.server_dir(server)?;
        let rd = match fs::read_dir(&sdir) {
            Ok(r) => r,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e),
        };
        let mut out = Vec::new();
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if !ft.is_dir() {
                continue;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            let path = e.path();
            if let Some(id) = name.strip_suffix(PARTIAL_SUFFIX) {
                out.push(BackupEntry {
                    id: id.to_owned(),
                    path: path.display().to_string(),
                    state: BackupState::Incomplete,
                    manifest: None,
                    size: dir_size(&path),
                });
                continue;
            }
            if safe_component(&name).is_err() {
                continue;
            }
            match read_manifest(&path) {
                Ok(m) if m.id == name && m.server == server => {
                    let size = m.size();
                    out.push(BackupEntry {
                        id: name,
                        path: path.display().to_string(),
                        state: BackupState::Complete,
                        manifest: Some(m),
                        size,
                    });
                }
                _ => out.push(BackupEntry {
                    id: name,
                    path: path.display().to_string(),
                    state: BackupState::Unreadable,
                    manifest: None,
                    size: dir_size(&path),
                }),
            }
        }
        out.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(out)
    }

    fn open(&self, server: &str, id: &str) -> io::Result<PathBuf> {
        let p = self.server_dir(server)?.join(safe_component(id)?);
        let m = read_manifest(&p)?;
        if m.id != id || m.server != server {
            return Err(invalid("el manifiesto no corresponde a esta copia"));
        }
        Ok(p)
    }

    fn delete(&self, server: &str, id: &str) -> io::Result<()> {
        let sdir = self.server_dir(server)?;
        let id = safe_component(id)?;
        for name in [id.to_owned(), format!("{id}{PARTIAL_SUFFIX}")] {
            let p = sdir.join(&name);
            if let Ok(meta) = fs::symlink_metadata(&p) {
                // Solo carpetas de copias: nunca se sigue un enlace.
                if meta.is_dir() && !meta.file_type().is_symlink() {
                    return fs::remove_dir_all(&p);
                }
            }
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no existe la copia {id}"),
        ))
    }
}

fn dir_size(p: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![p.to_path_buf()];
    while let Some(d) = stack.pop() {
        if let Ok(rd) = fs::read_dir(&d) {
            for e in rd.flatten() {
                match e.file_type() {
                    Ok(t) if t.is_dir() => stack.push(e.path()),
                    Ok(t) if t.is_file() => total += e.metadata().map(|m| m.len()).unwrap_or(0),
                    _ => {}
                }
            }
        }
    }
    total
}

/// Lee y comprueba el manifiesto de una copia.
pub fn read_manifest(dir: &Path) -> io::Result<Manifest> {
    let p = dir.join(MANIFEST);
    let meta = fs::symlink_metadata(&p)?;
    if !meta.is_file() || meta.len() > 16 * 1024 * 1024 {
        return Err(invalid("manifiesto no válido"));
    }
    let text = fs::read_to_string(&p)?;
    let m: Manifest =
        serde_json::from_str(&text).map_err(|e| invalid(format!("manifiesto no válido: {e}")))?;
    if m.format != FORMAT {
        return Err(invalid(format!(
            "formato de copia {} no admitido",
            m.format
        )));
    }
    for f in m.items.iter().flat_map(|i| &i.files) {
        check_relative(&f.path)?;
    }
    Ok(m)
}

/// Ruta relativa segura dentro de la copia: sin `..`, absolutas ni vacías.
pub fn check_relative(rel: &str) -> io::Result<()> {
    let p = Path::new(rel);
    let ok = !rel.is_empty()
        && rel.len() <= 1024
        && !rel.contains('\0')
        && rel != MANIFEST
        && p.components().all(|c| matches!(c, Component::Normal(_)));
    if ok {
        Ok(())
    } else {
        Err(invalid(format!("ruta no válida en la copia: {rel:?}")))
    }
}

/// Nombre de archivo a partir de un nombre de Docker (volúmenes, contenedores,
/// proyectos): letras, números, `.`, `_` y `-`.
pub fn file_stem(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .take(120)
        .collect();
    let s = s.trim_start_matches('.').to_owned();
    if s.is_empty() {
        "elemento".into()
    } else {
        s
    }
}

/// Una copia en curso.
#[derive(Debug)]
pub struct PartialBackup {
    dir: PathBuf,
    final_dir: PathBuf,
}

/// Archivo de la copia en escritura: calcula la suma y el tamaño.
pub struct HashedFile {
    rel: String,
    file: tokio::fs::File,
    hasher: Sha256,
    size: u64,
}

impl HashedFile {
    pub async fn write(&mut self, data: &[u8]) -> io::Result<()> {
        self.hasher.update(data);
        self.size += data.len() as u64;
        self.file.write_all(data).await
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    /// Cierra el archivo (sincronizado) y devuelve su entrada del manifiesto.
    pub async fn finish(mut self) -> io::Result<ManifestFile> {
        self.file.flush().await?;
        self.file.sync_all().await?;
        Ok(ManifestFile {
            path: self.rel,
            sha256: hex(&self.hasher.finalize()),
            size: self.size,
        })
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

impl PartialBackup {
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Crea un archivo nuevo (0600) dentro de la copia. Las carpetas
    /// intermedias se crean con 0700.
    pub async fn create(&mut self, rel: &str) -> io::Result<HashedFile> {
        check_relative(rel)?;
        let path = self.dir.join(rel);
        if let Some(parent) = path.parent() {
            let mut cur = self.dir.clone();
            for c in Path::new(rel)
                .parent()
                .into_iter()
                .flat_map(Path::components)
            {
                cur.push(c);
                mkdir_private(&cur)?;
            }
            debug_assert_eq!(cur, parent);
        }
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .await?;
        Ok(HashedFile {
            rel: rel.to_owned(),
            file,
            hasher: Sha256::new(),
            size: 0,
        })
    }

    /// Escribe un archivo pequeño de una vez.
    pub async fn write_all(&mut self, rel: &str, data: &[u8]) -> io::Result<ManifestFile> {
        let mut f = self.create(rel).await?;
        f.write(data).await?;
        f.finish().await
    }

    /// Termina la copia: escribe el manifiesto (0600) y renombra la carpeta.
    /// Hasta este momento la copia es incompleta.
    pub fn finalize(self, manifest: &Manifest) -> io::Result<PathBuf> {
        let text = serde_json::to_vec_pretty(manifest).map_err(io::Error::other)?;
        let tmp = self.dir.join(".manifest.tmp");
        {
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&tmp)?;
            f.write_all(&text)?;
            f.sync_all()?;
        }
        fs::rename(&tmp, self.dir.join(MANIFEST))?;
        sync_dir(&self.dir);
        if fs::symlink_metadata(&self.final_dir).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "ya existe una copia con ese id",
            ));
        }
        fs::rename(&self.dir, &self.final_dir)?;
        if let Some(parent) = self.final_dir.parent() {
            sync_dir(parent);
        }
        Ok(self.final_dir)
    }
}

fn sha256_file(p: &Path) -> io::Result<String> {
    let mut f = fs::File::open(p)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

/// Comprueba la integridad de una copia: cada archivo del manifiesto existe y
/// su SHA-256 coincide, y no hay archivos de más.
pub fn verify(dir: &Path) -> io::Result<IntegrityReport> {
    let m = read_manifest(dir)?;
    let mut r = IntegrityReport::default();
    let mut expected = BTreeSet::new();
    for f in m.items.iter().flat_map(|i| &i.files) {
        expected.insert(f.path.clone());
        r.checked += 1;
        let p = dir.join(&f.path);
        match fs::symlink_metadata(&p) {
            Ok(meta) if meta.is_file() => {
                if meta.len() != f.size || sha256_file(&p)? != f.sha256 {
                    r.altered.push(f.path.clone());
                }
            }
            _ => r.missing.push(f.path.clone()),
        }
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d)?.flatten() {
            let p = e.path();
            let rel = p
                .strip_prefix(dir)
                .unwrap_or(&p)
                .to_string_lossy()
                .into_owned();
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(p),
                _ if rel == MANIFEST => {}
                _ if !expected.contains(&rel) => r.unexpected.push(rel),
                _ => {}
            }
        }
    }
    r.unexpected.sort();
    r.ok = r.altered.is_empty() && r.missing.is_empty() && r.unexpected.is_empty();
    Ok(r)
}

/// Id de una copia: `AAAAMMDDTHHMMSSZ-xxxxxx` (UTC).
pub fn new_id(now: DateTime<Utc>) -> String {
    format!(
        "{}-{}",
        now.format("%Y%m%dT%H%M%SZ"),
        crate::ssh::askpass::random_hex(3)
    )
}

/// Qué copias conserva la política (D-097). `backups`: las copias completas de
/// **una** tarea, con su fecha. Se queda la más reciente de cada uno de los
/// últimos `daily` días con copia, de cada una de las últimas `weekly` semanas
/// ISO y de cada uno de los últimos `monthly` meses. Las que no están en el
/// resultado se pueden borrar; nada más.
pub fn retention_keep(backups: &[(String, DateTime<Utc>)], r: Retention) -> BTreeSet<String> {
    let mut sorted: Vec<&(String, DateTime<Utc>)> = backups.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));
    let mut keep = BTreeSet::new();
    type Key = (i32, u32, u32);
    let mut pick = |n: u32, key: &dyn Fn(&DateTime<Utc>) -> Key| {
        let mut seen: BTreeMap<Key, ()> = BTreeMap::new();
        for (id, t) in &sorted {
            if seen.len() as u32 >= n {
                break;
            }
            let k = key(t);
            if seen.insert(k, ()).is_none() {
                keep.insert(id.clone());
            }
        }
    };
    pick(r.daily, &|t| (t.year(), t.month(), t.day()));
    pick(r.weekly, &|t| {
        let w = t.iso_week();
        (w.year(), w.week(), 0)
    });
    pick(r.monthly, &|t| (t.year(), t.month(), 0));
    keep
}

/// Copias que la política permite eliminar para la tarea `job` (D-097): solo
/// copias **completas** creadas por **esa** tarea. Las manuales, las de otras
/// tareas, las incompletas y las ilegibles nunca se incluyen.
pub fn retention_victims(entries: &[BackupEntry], job: &str, r: Retention) -> Vec<String> {
    let mine: Vec<(String, DateTime<Utc>)> = entries
        .iter()
        .filter(|e| e.state == BackupState::Complete)
        .filter_map(|e| {
            let m = e.manifest.as_ref()?;
            if m.job.as_deref() != Some(job) {
                return None;
            }
            let t = DateTime::parse_from_rfc3339(&m.created_at)
                .ok()?
                .with_timezone(&Utc);
            Some((e.id.clone(), t))
        })
        .collect();
    let keep = retention_keep(&mine, r);
    let mut out: Vec<String> = mine
        .into_iter()
        .map(|(id, _)| id)
        .filter(|id| !keep.contains(id))
        .collect();
    out.sort();
    out
}

pub fn check_retention(r: &Retention) -> Result<(), String> {
    if r.daily + r.weekly + r.monthly == 0 {
        return Err("la retención debe conservar al menos una copia".into());
    }
    if r.daily > 366 || r.weekly > 520 || r.monthly > 240 {
        return Err("valores de retención demasiado altos".into());
    }
    Ok(())
}

/// Permisos de un archivo o carpeta (para mostrarlos y probarlos).
pub fn mode_of(p: &Path) -> io::Result<u32> {
    Ok(crate::fsx::mode_of(&fs::metadata(p)?, 0o700) & 0o777)
}

#[cfg(test)]
mod tests {

    #[test]
    fn the_backup_folder_from_before_1_8_0_moves_to_the_new_name() {
        let d = tempfile::tempdir().unwrap();
        let old = d.path().join(crate::legacy::BACKUP_ROOT_DIR);
        std::fs::create_dir_all(old.join("local/20261110T030000Z-aaaaaa")).unwrap();
        std::fs::write(
            old.join("local/20261110T030000Z-aaaaaa/manifest.json"),
            "{}",
        )
        .unwrap();
        // Al abrir el destino se renombra entera.
        let t = LocalTarget::new(d.path()).unwrap();
        assert!(!old.exists());
        assert!(t
            .root()
            .join("local/20261110T030000Z-aaaaaa/manifest.json")
            .is_file());
        assert_eq!(t.root(), d.path().join(ROOT_DIR));

        // Con las dos: se mueve lo que falta y no se sobrescribe nada.
        std::fs::create_dir_all(old.join("local/20261111T030000Z-bbbbbb")).unwrap();
        std::fs::create_dir_all(old.join("local/20261110T030000Z-aaaaaa")).unwrap();
        std::fs::write(old.join("local/20261110T030000Z-aaaaaa/otro"), "x").unwrap();
        std::fs::create_dir_all(old.join("srv-2/20261112T030000Z-cccccc")).unwrap();
        migrate_legacy_root(&old, t.root()).unwrap();
        assert!(t.root().join("local/20261111T030000Z-bbbbbb").is_dir());
        assert!(t.root().join("srv-2/20261112T030000Z-cccccc").is_dir());
        assert!(!t.root().join("local/20261110T030000Z-aaaaaa/otro").exists());
        // Lo que coincidía se queda en la antigua, que por eso se conserva.
        assert!(old.join("local/20261110T030000Z-aaaaaa/otro").exists());
        // Sin carpeta antigua no hace nada.
        let e = tempfile::tempdir().unwrap();
        migrate_legacy_root(&e.path().join("no"), &e.path().join("nuevo")).unwrap();
        assert!(!e.path().join("nuevo").exists());
    }

    use super::*;
    use crate::backup::model::{BackupItem, Consistency, ItemKind};

    fn tmp() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("cmc-test")
            .tempdir()
            .unwrap()
    }

    fn manifest(id: &str, files: Vec<ManifestFile>) -> Manifest {
        Manifest {
            format: FORMAT,
            id: id.into(),
            server: "local".into(),
            created_at: "2026-11-14T10:00:00Z".into(),
            finished_at: "2026-11-14T10:00:01Z".into(),
            app_version: "1.4.0".into(),
            job: None,
            items: vec![BackupItem {
                kind: ItemKind::Volume,
                name: "datos".into(),
                files,
                consistency: Consistency::Idle,
                details: BTreeMap::new(),
                warnings: vec![],
            }],
        }
    }

    #[tokio::test]
    async fn complete_backup_is_private_hashed_and_listed() {
        let d = tmp();
        let t = LocalTarget::new(d.path()).unwrap();
        let mut p = t.begin("local", "20261114T100000Z-aaaaaa").unwrap();
        let mut f = p.create("volumes/datos.tar").await.unwrap();
        f.write(b"hola ").await.unwrap();
        f.write(b"mundo").await.unwrap();
        let entry = f.finish().await.unwrap();
        assert_eq!(entry.size, 10);
        assert_eq!(entry.sha256, hex(&Sha256::digest(b"hola mundo")));
        // Mientras no se termina, es incompleta.
        let l = t.list("local").unwrap();
        assert_eq!(l[0].state, BackupState::Incomplete);
        assert!(t.open("local", "20261114T100000Z-aaaaaa").is_err());
        let dir = p
            .finalize(&manifest("20261114T100000Z-aaaaaa", vec![entry]))
            .unwrap();
        // Permisos: carpetas 0700, archivos 0600.
        assert_eq!(mode_of(t.root()).unwrap(), 0o700);
        assert_eq!(mode_of(&t.root().join("local")).unwrap(), 0o700);
        assert_eq!(mode_of(&dir).unwrap(), 0o700);
        assert_eq!(mode_of(&dir.join("volumes")).unwrap(), 0o700);
        assert_eq!(mode_of(&dir.join("volumes/datos.tar")).unwrap(), 0o600);
        assert_eq!(mode_of(&dir.join(MANIFEST)).unwrap(), 0o600);
        let l = t.list("local").unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].state, BackupState::Complete);
        assert_eq!(l[0].size, 10);
        assert!(verify(&dir).unwrap().ok);
        assert_eq!(t.open("local", "20261114T100000Z-aaaaaa").unwrap(), dir);
        // Otro servidor no la ve.
        assert!(t.list("srv-0123abcd").unwrap().is_empty());
    }

    #[tokio::test]
    async fn altered_missing_and_extra_files_are_detected() {
        let d = tmp();
        let t = LocalTarget::new(d.path()).unwrap();
        let mut p = t.begin("local", "b1").unwrap();
        let a = p.write_all("volumes/a.tar", b"aaaa").await.unwrap();
        let b = p.write_all("volumes/b.tar", b"bbbb").await.unwrap();
        let dir = p.finalize(&manifest("b1", vec![a, b])).unwrap();
        // Un byte cambiado (mismo tamaño): la suma lo detecta.
        fs::write(dir.join("volumes/a.tar"), b"aaab").unwrap();
        fs::remove_file(dir.join("volumes/b.tar")).unwrap();
        fs::write(dir.join("volumes/c.tar"), b"x").unwrap();
        let r = verify(&dir).unwrap();
        assert!(!r.ok);
        assert_eq!(r.altered, vec!["volumes/a.tar"]);
        assert_eq!(r.missing, vec!["volumes/b.tar"]);
        assert_eq!(r.unexpected, vec!["volumes/c.tar"]);
        assert_eq!(r.checked, 2);
    }

    #[tokio::test]
    async fn interrupted_backups_are_never_taken_as_good_and_paths_are_safe() {
        let d = tmp();
        let t = LocalTarget::new(d.path()).unwrap();
        {
            let mut p = t.begin("local", "corte").unwrap();
            let mut f = p.create("volumes/a.tar").await.unwrap();
            f.write(b"medio archivo").await.unwrap();
            // Se interrumpe: no se llama a finalize.
        }
        let l = t.list("local").unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].state, BackupState::Incomplete);
        assert!(l[0].manifest.is_none());
        assert!(t.open("local", "corte").is_err());
        // Se puede eliminar.
        t.delete("local", "corte").unwrap();
        assert!(t.list("local").unwrap().is_empty());
        // Rutas y nombres.
        let mut p = t.begin("local", "rutas").unwrap();
        for bad in ["../fuera", "/abs", "a/../../b", "", "manifest.json"] {
            assert!(p.create(bad).await.is_err(), "{bad}");
        }
        assert!(t.begin("../x", "id").is_err());
        assert!(t.begin("local", "a/b").is_err());
        assert!(LocalTarget::new(Path::new("relativa")).is_err());
        assert_eq!(file_stem("mi volumen/ñ"), "mi_volumen__");
        assert_eq!(file_stem("..."), "elemento");
        // Un manifiesto con rutas peligrosas no se acepta.
        let mut m = manifest(
            "x",
            vec![ManifestFile {
                path: "../../etc/passwd".into(),
                sha256: String::new(),
                size: 0,
            }],
        );
        let dir = d.path().join("m");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join(MANIFEST), serde_json::to_vec(&m).unwrap()).unwrap();
        assert!(read_manifest(&dir).is_err());
        m.items.clear();
        m.format = 99;
        fs::write(dir.join(MANIFEST), serde_json::to_vec(&m).unwrap()).unwrap();
        assert!(read_manifest(&dir).is_err(), "formato desconocido");
    }

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    /// Retención exacta: 7 diarias, 4 semanales, 3 mensuales sobre 120 días
    /// de copias (dos al día).
    #[test]
    fn retention_is_exact() {
        let start = at("2026-07-01T03:00:00Z");
        let mut all = Vec::new();
        for day in 0..120 {
            for h in [0, 12] {
                let t = start + chrono::Duration::days(day) + chrono::Duration::hours(h);
                all.push((t.format("%Y%m%dT%H%M%SZ").to_string(), t));
            }
        }
        let keep = retention_keep(&all, Retention::default());
        let newest = |pred: &dyn Fn(&DateTime<Utc>) -> bool| -> String {
            all.iter()
                .filter(|(_, t)| pred(t))
                .max_by_key(|(_, t)| *t)
                .unwrap()
                .0
                .clone()
        };
        let last = all.iter().map(|x| x.1).max().unwrap();
        let mut expected = BTreeSet::new();
        // 7 días: la última copia (12:00) de cada uno de los 7 últimos días.
        for d in 0..7 {
            let day = (last - chrono::Duration::days(d)).date_naive();
            expected.insert(newest(&|t| t.date_naive() == day));
        }
        // 4 semanas ISO y 3 meses: la más reciente de cada una.
        let mut weeks: Vec<(i32, u32)> = all
            .iter()
            .map(|(_, t)| (t.iso_week().year(), t.iso_week().week()))
            .collect();
        weeks.sort();
        weeks.dedup();
        for w in weeks.iter().rev().take(4) {
            expected.insert(newest(&|t| {
                (t.iso_week().year(), t.iso_week().week()) == *w
            }));
        }
        let mut months: Vec<(i32, u32)> = all.iter().map(|(_, t)| (t.year(), t.month())).collect();
        months.sort();
        months.dedup();
        for m in months.iter().rev().take(3) {
            expected.insert(newest(&|t| (t.year(), t.month()) == *m));
        }
        assert_eq!(keep, expected);
        // Las solapadas se cuentan una vez: el total es ≤ 7 + 4 + 3.
        assert!(keep.len() <= 14 && keep.len() >= 7, "{}", keep.len());
        // Pocas copias en días distintos: se conservan todas; dos del mismo
        // día cuentan como una (se queda la más reciente).
        let few = vec![all[0].clone(), all[2].clone(), all[4].clone()];
        assert_eq!(retention_keep(&few, Retention::default()).len(), 3);
        let same_day = &all[..2];
        assert_eq!(
            retention_keep(same_day, Retention::default()),
            BTreeSet::from([all[1].0.clone()])
        );
        // Solo diarias: exactamente 2 días → 2 copias.
        let r = Retention {
            daily: 2,
            weekly: 0,
            monthly: 0,
        };
        assert_eq!(retention_keep(&all, r).len(), 2);
        assert!(check_retention(&Retention {
            daily: 0,
            weekly: 0,
            monthly: 0
        })
        .is_err());
        // Solo se borran copias completas de la misma tarea.
        let entry = |id: &str, job: Option<&str>, t: &str, state: BackupState| BackupEntry {
            id: id.into(),
            path: String::new(),
            state,
            manifest: (state == BackupState::Complete).then(|| {
                let mut m = manifest(id, vec![]);
                m.job = job.map(str::to_owned);
                m.created_at = t.into();
                m
            }),
            size: 0,
        };
        let entries = vec![
            entry(
                "j1",
                Some("diaria"),
                "2026-11-10T03:00:00Z",
                BackupState::Complete,
            ),
            entry(
                "j2",
                Some("diaria"),
                "2026-11-11T03:00:00Z",
                BackupState::Complete,
            ),
            entry(
                "j3",
                Some("diaria"),
                "2026-11-12T03:00:00Z",
                BackupState::Complete,
            ),
            entry("m1", None, "2026-01-01T03:00:00Z", BackupState::Complete),
            entry(
                "o1",
                Some("otra"),
                "2026-01-01T03:00:00Z",
                BackupState::Complete,
            ),
            entry("p1", None, "", BackupState::Incomplete),
            entry("u1", None, "", BackupState::Unreadable),
        ];
        let one = Retention {
            daily: 1,
            weekly: 0,
            monthly: 0,
        };
        assert_eq!(retention_victims(&entries, "diaria", one), vec!["j1", "j2"]);
        assert_eq!(
            retention_victims(&entries, "diaria", Retention::default()),
            Vec::<String>::new()
        );
        assert!(retention_victims(&entries, "nada", one).is_empty());
        assert!(check_retention(&Retention::default()).is_ok());
    }
}
