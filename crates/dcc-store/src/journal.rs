//! Diario persistente de registros JSON, uno por línea (Fase 8, D-058).
//!
//! Lo usan el historial de operaciones y el diario de eventos de Docker. Los
//! archivos se crean con permisos 0600 en una carpeta 0700 del usuario
//! (`$XDG_STATE_HOME/docker-control-center/`). Las líneas que no se pueden leer
//! (archivo dañado o cortado) se descartan y se informa de cuántas. Se conservan
//! las `max` entradas más recientes: el archivo se compacta al abrirlo y cuando
//! crece un 25 % por encima del máximo.

use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JournalStatus {
    pub loaded: usize,
    /// Líneas que no se pudieron leer y se han descartado.
    pub skipped: usize,
}

struct Inner<T> {
    path: PathBuf,
    max: usize,
    entries: VecDeque<T>,
    file: Option<File>,
}

/// Diario de entradas `T` en un archivo JSONL.
pub struct Journal<T> {
    inner: Arc<Mutex<Inner<T>>>,
}

impl<T> Clone for Journal<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

fn ensure_private_dir(dir: &Path) -> std::io::Result<()> {
    if !dir.exists() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
    }
    Ok(())
}

fn write_all<T: Serialize>(path: &Path, entries: &VecDeque<T>) -> std::io::Result<()> {
    let tmp = path.with_extension("jsonl.tmp");
    {
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        for e in entries {
            let mut line = serde_json::to_vec(e).map_err(std::io::Error::other)?;
            line.push(b'\n');
            f.write_all(&line)?;
        }
        f.sync_all()?;
    }
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
    fs::rename(&tmp, path)
}

fn open_append(path: &Path) -> std::io::Result<File> {
    let f = OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o600)
        .open(path)?;
    // Un archivo creado antes con otros permisos queda privado.
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(f)
}

impl<T: Serialize + DeserializeOwned + Clone> Journal<T> {
    /// Abre (o crea) el diario y carga sus entradas.
    pub fn open(path: impl Into<PathBuf>, max: usize) -> std::io::Result<(Self, JournalStatus)> {
        let path = path.into();
        let max = max.max(1);
        if let Some(dir) = path.parent() {
            ensure_private_dir(dir)?;
        }
        let mut status = JournalStatus::default();
        let mut entries = VecDeque::new();
        match fs::symlink_metadata(&path) {
            Ok(m) if !m.file_type().is_file() => {
                return Err(std::io::Error::other(format!(
                    "{} no es un archivo normal",
                    path.display()
                )));
            }
            Ok(_) => {
                let f = File::open(&path)?;
                for line in BufReader::new(f).split(b'\n') {
                    let line = line?;
                    if line.iter().all(u8::is_ascii_whitespace) {
                        continue;
                    }
                    match serde_json::from_slice::<T>(&line) {
                        Ok(e) => {
                            entries.push_back(e);
                            if entries.len() > max {
                                entries.pop_front();
                            }
                            status.loaded += 1;
                        }
                        Err(_) => status.skipped += 1,
                    }
                }
                status.loaded = entries.len();
                if status.skipped > 0 || status.loaded >= max {
                    write_all(&path, &entries)?;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        let file = Some(open_append(&path)?);
        Ok((
            Self {
                inner: Arc::new(Mutex::new(Inner {
                    path,
                    max,
                    entries,
                    file,
                })),
            },
            status,
        ))
    }

    /// Añade una entrada y la escribe en el disco.
    pub fn append(&self, e: T) -> std::io::Result<()> {
        let mut g = self.inner.lock();
        let mut line = serde_json::to_vec(&e).map_err(std::io::Error::other)?;
        line.push(b'\n');
        g.entries.push_back(e);
        let max = g.max;
        if g.entries.len() > max + max / 4 {
            while g.entries.len() > max {
                g.entries.pop_front();
            }
            let path = g.path.clone();
            write_all(&path, &g.entries)?;
            g.file = Some(open_append(&path)?);
            return Ok(());
        }
        match g.file.as_mut() {
            Some(f) => {
                f.write_all(&line)?;
                f.flush()
            }
            None => Ok(()),
        }
    }

    /// Entradas en orden de llegada (las más antiguas primero), hasta `max`.
    pub fn entries(&self) -> Vec<T> {
        let g = self.inner.lock();
        let skip = g.entries.len().saturating_sub(g.max);
        g.entries.iter().skip(skip).cloned().collect()
    }

    pub fn len(&self) -> usize {
        let g = self.inner.lock();
        g.entries.len().min(g.max)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Borra todas las entradas, también del disco.
    pub fn clear(&self) -> std::io::Result<()> {
        let mut g = self.inner.lock();
        g.entries.clear();
        let path = g.path.clone();
        write_all(&path, &g.entries)?;
        g.file = Some(open_append(&path)?);
        Ok(())
    }

    pub fn path(&self) -> PathBuf {
        self.inner.lock().path.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct E {
        n: u32,
        texto: String,
    }

    fn e(n: u32) -> E {
        E {
            n,
            texto: format!("evento {n}"),
        }
    }

    #[test]
    fn entries_survive_a_restart_with_private_permissions() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("estado/docker-control-center/eventos.jsonl");
        let (j, st) = Journal::<E>::open(&path, 100).unwrap();
        assert_eq!(st, JournalStatus::default());
        for n in 0..5 {
            j.append(e(n)).unwrap();
        }
        drop(j);
        let (j, st) = Journal::<E>::open(&path, 100).unwrap();
        assert_eq!(st.loaded, 5);
        assert_eq!(j.entries(), (0..5).map(e).collect::<Vec<_>>());
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }

    #[test]
    fn corrupt_lines_are_skipped_and_reported() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("h.jsonl");
        fs::write(
            &path,
            "{\"n\":1,\"texto\":\"a\"}\nbasura\n\n{\"n\":2,\"texto\":\"b\"}\n{\"n\":3,\"tex",
        )
        .unwrap();
        let (j, st) = Journal::<E>::open(&path, 100).unwrap();
        assert_eq!(
            st,
            JournalStatus {
                loaded: 2,
                skipped: 2
            }
        );
        assert_eq!(j.entries().iter().map(|x| x.n).collect::<Vec<_>>(), [1, 2]);
        // El archivo se ha reescrito limpio y se puede seguir añadiendo.
        j.append(e(4)).unwrap();
        let (j2, st) = Journal::<E>::open(&path, 100).unwrap();
        assert_eq!((st.loaded, st.skipped), (3, 0));
        assert_eq!(
            j2.entries().iter().map(|x| x.n).collect::<Vec<_>>(),
            [1, 2, 4]
        );
    }

    #[test]
    fn only_the_most_recent_entries_are_kept() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("h.jsonl");
        let (j, _) = Journal::<E>::open(&path, 10).unwrap();
        for n in 0..40 {
            j.append(e(n)).unwrap();
        }
        assert_eq!(
            j.entries().iter().map(|x| x.n).collect::<Vec<_>>(),
            (30..40).collect::<Vec<_>>()
        );
        let lines = fs::read_to_string(&path).unwrap().lines().count();
        assert!(lines <= 12, "el archivo se compacta: {lines} líneas");
        drop(j);
        let (j, _) = Journal::<E>::open(&path, 5).unwrap();
        assert_eq!(
            j.entries().iter().map(|x| x.n).collect::<Vec<_>>(),
            (35..40).collect::<Vec<_>>()
        );
        assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 5);
    }

    #[test]
    fn clear_empties_memory_and_disk() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("h.jsonl");
        let (j, _) = Journal::<E>::open(&path, 10).unwrap();
        j.append(e(1)).unwrap();
        j.clear().unwrap();
        assert!(j.is_empty());
        assert_eq!(fs::read_to_string(&path).unwrap(), "");
        j.append(e(2)).unwrap();
        let (j, _) = Journal::<E>::open(&path, 10).unwrap();
        assert_eq!(j.entries(), [e(2)]);
    }

    #[test]
    fn refuses_a_symlink_or_directory() {
        let d = tempfile::tempdir().unwrap();
        let target = d.path().join("otro");
        fs::write(&target, "").unwrap();
        let link = d.path().join("h.jsonl");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(Journal::<E>::open(&link, 10).is_err());
        let dir = d.path().join("carpeta.jsonl");
        fs::create_dir(&dir).unwrap();
        assert!(Journal::<E>::open(&dir, 10).is_err());
    }
}
