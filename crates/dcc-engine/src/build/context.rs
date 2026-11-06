//! Contexto de construcción: recorrido de la carpeta aplicando `.dockerignore`
//! y empaquetado en tar **en flujo** (no se carga entero en memoria).
//!
//! Como el CLI: rutas relativas al contexto, propietario 0:0, enlaces simbólicos
//! guardados como enlaces (no se siguen) y sin archivos especiales.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use ts_rs::TS;

use super::ignore::IgnoreMatcher;

const CHUNK: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContextFile {
    pub path: String,
    #[ts(type = "number")]
    pub size: u64,
}

/// Resumen del contexto antes de construir (no habla con Docker).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContextSummary {
    pub files: u64,
    pub dirs: u64,
    pub symlinks: u64,
    #[ts(type = "number")]
    pub bytes: u64,
    /// Entradas excluidas por `.dockerignore`.
    pub excluded: u64,
    /// Sockets, dispositivos y FIFO, que no se envían.
    pub skipped_special: u64,
    pub dockerignore: bool,
    pub ignore_patterns: Vec<String>,
    pub invalid_ignore_lines: Vec<String>,
    /// Los cinco archivos más grandes que se enviarán.
    pub largest: Vec<ContextFile>,
    /// Archivos que no se pueden leer (la construcción fallaría).
    pub unreadable: Vec<String>,
}

pub fn load_ignore(context: &Path, dockerfile: &str) -> (IgnoreMatcher, bool, Vec<String>) {
    let (mut m, invalid, found) = match std::fs::read_to_string(context.join(".dockerignore")) {
        Ok(s) => {
            let (m, bad) = IgnoreMatcher::parse(&s);
            (m, bad, true)
        }
        Err(_) => (IgnoreMatcher::default(), Vec::new(), false),
    };
    if found {
        m.keep(".dockerignore");
        m.keep(dockerfile);
    }
    (m, found, invalid)
}

#[derive(Debug)]
enum Entry {
    Dir(PathBuf, String),
    File(PathBuf, String, u64),
    Symlink(PathBuf, String),
}

/// Recorre el contexto en orden estable (alfabético) y llama a `visit` por cada
/// entrada incluida. Devuelve (excluidas, especiales).
fn walk(
    context: &Path,
    ignore: &IgnoreMatcher,
    visit: &mut dyn FnMut(Entry) -> io::Result<()>,
) -> io::Result<(u64, u64)> {
    let mut excluded = 0;
    let mut special = 0;
    let mut stack: Vec<(PathBuf, String)> = vec![(context.to_owned(), String::new())];
    while let Some((dir, rel_dir)) = stack.pop() {
        let mut entries: Vec<_> = std::fs::read_dir(&dir)?.collect::<Result<_, _>>()?;
        entries.sort_by_key(|e| e.file_name());
        let mut subdirs = Vec::new();
        for e in entries {
            let name = e.file_name().to_string_lossy().into_owned();
            let rel = if rel_dir.is_empty() {
                name
            } else {
                format!("{rel_dir}/{name}")
            };
            let meta = std::fs::symlink_metadata(e.path())?;
            let ft = meta.file_type();
            let is_excluded = ignore.excluded(&rel);
            if ft.is_dir() {
                if is_excluded && !ignore.has_exceptions() {
                    excluded += 1;
                    continue;
                }
                if !is_excluded {
                    visit(Entry::Dir(e.path(), rel.clone()))?;
                } else {
                    excluded += 1;
                }
                subdirs.push((e.path(), rel));
            } else if is_excluded {
                excluded += 1;
            } else if ft.is_symlink() {
                visit(Entry::Symlink(e.path(), rel))?;
            } else if ft.is_file() {
                visit(Entry::File(e.path(), rel, meta.len()))?;
            } else {
                special += 1;
            }
        }
        // Orden alfabético también entre directorios (pila: al revés).
        stack.extend(subdirs.into_iter().rev());
    }
    Ok((excluded, special))
}

pub fn scan(context: &Path, dockerfile: &str) -> io::Result<ContextSummary> {
    let (ignore, found, invalid) = load_ignore(context, dockerfile);
    let mut s = ContextSummary {
        files: 0,
        dirs: 0,
        symlinks: 0,
        bytes: 0,
        excluded: 0,
        skipped_special: 0,
        dockerignore: found,
        ignore_patterns: ignore.patterns(),
        invalid_ignore_lines: invalid,
        largest: Vec::new(),
        unreadable: Vec::new(),
    };
    let mut all: Vec<ContextFile> = Vec::new();
    let (excluded, special) = walk(context, &ignore, &mut |e| {
        match e {
            Entry::Dir(..) => s.dirs += 1,
            Entry::Symlink(..) => s.symlinks += 1,
            Entry::File(p, rel, size) => {
                s.files += 1;
                s.bytes += size;
                if std::fs::File::open(&p).is_err() && s.unreadable.len() < 20 {
                    s.unreadable.push(rel.clone());
                }
                all.push(ContextFile { path: rel, size });
            }
        }
        Ok(())
    })?;
    s.excluded = excluded;
    s.skipped_special = special;
    all.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
    all.truncate(5);
    s.largest = all;
    Ok(s)
}

/// `Write` que envía bloques de 64 KiB por un canal (para el cuerpo de `/build`).
pub struct ChannelWriter {
    tx: mpsc::Sender<Result<Bytes, io::Error>>,
    buf: Vec<u8>,
    pub sent: u64,
}

impl ChannelWriter {
    pub fn new(tx: mpsc::Sender<Result<Bytes, io::Error>>) -> Self {
        Self {
            tx,
            buf: Vec::with_capacity(CHUNK),
            sent: 0,
        }
    }
    fn send(&mut self) -> io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let chunk = Bytes::from(std::mem::replace(&mut self.buf, Vec::with_capacity(CHUNK)));
        self.sent += chunk.len() as u64;
        self.tx
            .blocking_send(Ok(chunk))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "la petición se cerró"))
    }
}

impl Write for ChannelWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(data);
        if self.buf.len() >= CHUNK {
            self.send()?;
        }
        Ok(data.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.send()
    }
}

/// Escribe el contexto en tar sobre `out`. Devuelve los bytes del tar.
pub fn write_tar<W: Write>(context: &Path, dockerfile: &str, out: W) -> io::Result<W> {
    let (ignore, _, _) = load_ignore(context, dockerfile);
    let mut tar = tar::Builder::new(out);
    tar.mode(tar::HeaderMode::Deterministic);
    tar.follow_symlinks(false);
    walk(context, &ignore, &mut |e| {
        match e {
            Entry::Dir(p, rel) => {
                let meta = std::fs::metadata(&p)?;
                let mut h = tar::Header::new_gnu();
                h.set_metadata_in_mode(&meta, tar::HeaderMode::Deterministic);
                h.set_entry_type(tar::EntryType::Directory);
                h.set_size(0);
                tar.append_data(&mut h, format!("{rel}/"), io::empty())?;
            }
            Entry::File(p, rel, _) => {
                let f = std::fs::File::open(&p)
                    .map_err(|e| io::Error::new(e.kind(), format!("{rel}: {e}")))?;
                let meta = f.metadata()?;
                let mut h = tar::Header::new_gnu();
                h.set_metadata_in_mode(&meta, tar::HeaderMode::Deterministic);
                tar.append_data(&mut h, &rel, f)?;
            }
            Entry::Symlink(p, rel) => {
                let target = std::fs::read_link(&p)?;
                let meta = std::fs::symlink_metadata(&p)?;
                let mut h = tar::Header::new_gnu();
                h.set_metadata_in_mode(&meta, tar::HeaderMode::Deterministic);
                h.set_entry_type(tar::EntryType::Symlink);
                h.set_size(0);
                tar.append_link(&mut h, &rel, target)?;
            }
        }
        Ok(())
    })?;
    tar.into_inner()
}

/// Lanza el empaquetado en un hilo y devuelve el receptor de bloques y una
/// tarea que informa de los bytes enviados (o del error al leer el contexto).
pub fn stream_tar(
    context: PathBuf,
    dockerfile: String,
) -> (
    mpsc::Receiver<Result<Bytes, io::Error>>,
    tokio::task::JoinHandle<io::Result<u64>>,
) {
    let (tx, rx) = mpsc::channel(8);
    let task = tokio::task::spawn_blocking(move || {
        let err_tx = tx.clone();
        let res = write_tar(&context, &dockerfile, ChannelWriter::new(tx)).and_then(|mut w| {
            w.flush()?;
            Ok(w.sent)
        });
        if let Err(e) = &res {
            let _ = err_tx.blocking_send(Err(io::Error::new(e.kind(), e.to_string())));
        }
        res
    });
    (rx, task)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn ctx() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        std::fs::write(p.join("Dockerfile"), "FROM scratch\n").unwrap();
        std::fs::write(
            p.join(".dockerignore"),
            "*.log\nsecretos\nDockerfile\n!secretos/publico.txt\n",
        )
        .unwrap();
        std::fs::write(p.join("app.txt"), "hola").unwrap();
        std::fs::write(p.join("debug.log"), "x".repeat(100)).unwrap();
        std::fs::create_dir_all(p.join("secretos")).unwrap();
        std::fs::write(p.join("secretos/clave.pem"), "privada").unwrap();
        std::fs::write(p.join("secretos/publico.txt"), "pub").unwrap();
        std::fs::create_dir_all(p.join("src/sub")).unwrap();
        std::fs::write(p.join("src/sub/main.c"), "int main(){}").unwrap();
        symlink("app.txt", p.join("enlace")).unwrap();
        d
    }

    fn names(tar_bytes: &[u8]) -> Vec<(String, tar::EntryType, u64)> {
        let mut a = tar::Archive::new(tar_bytes);
        a.entries()
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                let h = e.header();
                assert_eq!(
                    (h.uid().unwrap(), h.gid().unwrap()),
                    (0, 0),
                    "propietario 0:0 como el CLI"
                );
                (
                    e.path().unwrap().to_string_lossy().into_owned(),
                    h.entry_type(),
                    h.size().unwrap(),
                )
            })
            .collect()
    }

    #[test]
    fn tar_applies_dockerignore_keeps_dockerfile_and_symlinks() {
        let d = ctx();
        let tar = write_tar(d.path(), "Dockerfile", Vec::new()).unwrap();
        let n: Vec<String> = names(&tar).into_iter().map(|(p, _, _)| p).collect();
        assert_eq!(
            n,
            // Las entradas de directorio se escriben al verlas; su contenido, después.
            vec![
                ".dockerignore",
                "Dockerfile",
                "app.txt",
                "enlace",
                "src/",
                "secretos/publico.txt",
                "src/sub/",
                "src/sub/main.c"
            ]
        );
        let link = names(&tar)
            .into_iter()
            .find(|(p, _, _)| p == "enlace")
            .unwrap();
        assert_eq!(link.1, tar::EntryType::Symlink);
    }

    #[test]
    fn scan_reports_what_will_be_sent() {
        let d = ctx();
        let s = scan(d.path(), "Dockerfile").unwrap();
        assert_eq!(s.files, 5, "{s:?}");
        assert_eq!(s.dirs, 2);
        assert_eq!(s.symlinks, 1);
        assert_eq!(s.excluded, 3, "debug.log, secretos/ y secretos/clave.pem");
        assert!(s.dockerignore);
        assert_eq!(s.largest[0].path, ".dockerignore");
        assert!(s.unreadable.is_empty());
    }

    #[test]
    fn without_dockerignore_everything_is_sent() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("Dockerfile"), "FROM scratch").unwrap();
        std::fs::write(d.path().join("x.log"), "1").unwrap();
        let s = scan(d.path(), "Dockerfile").unwrap();
        assert_eq!((s.files, s.excluded, s.dockerignore), (2, 0, false));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn streaming_produces_the_same_tar() {
        let d = ctx();
        let (mut rx, task) = stream_tar(d.path().to_owned(), "Dockerfile".into());
        let mut all = Vec::new();
        while let Some(c) = rx.recv().await {
            all.extend_from_slice(&c.unwrap());
        }
        let sent = task.await.unwrap().unwrap();
        assert_eq!(sent as usize, all.len());
        assert_eq!(all, write_tar(d.path(), "Dockerfile", Vec::new()).unwrap());
    }
}
