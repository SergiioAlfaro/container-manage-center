//! Archivos Compose **en un servidor remoto** (0.10.0, D-080).
//!
//! Mismas reglas que [`super::file`] para los archivos del equipo: tamaño
//! máximo, texto UTF-8, no se sobrescribe un archivo que ha cambiado desde que
//! se leyó, el guardado es atómico (temporal en la misma carpeta y `mv`) y
//! conserva los permisos, y la creación no pisa un archivo existente.
//!
//! Todo se hace con órdenes de solo lectura o de escritura acotada (`stat`,
//! `base64`, `test`, `dd`, `chmod`, `mv`, `rm` del temporal), con argumentos
//! entrecomillados uno a uno, por la sesión SSH verificada. Cada orden queda
//! en el panel de actividad, nunca con el contenido del archivo (puede tener
//! secretos). Nunca se supone que un archivo del equipo existe en el servidor.

use base64::Engine as _;
use dcc_activity::Category;
use tokio_util::sync::CancellationToken;

use super::file::{version_of, ComposeFile, MAX_FILE_BYTES};
use super::project::DEFAULT_FILES;
use crate::cli::{run_streaming, CliRequest, RemoteExec, StreamOptions};
use crate::client::Engine;
use crate::error::EngineError;

const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidInput {
        message: message.into(),
    }
}

fn absolute(path: &str) -> Result<(), EngineError> {
    if path.is_empty() || path.contains('\0') || !path.starts_with('/') || path.len() > 4096 {
        return Err(invalid(format!("ruta no válida: {path:?}")));
    }
    Ok(())
}

/// Carpeta y nombre de una ruta absoluta del servidor (sin tocar el equipo).
fn split(path: &str) -> (String, String) {
    match path.rsplit_once('/') {
        Some(("", name)) => ("/".into(), name.into()),
        Some((dir, name)) => (dir.into(), name.into()),
        None => ("/".into(), path.into()),
    }
}

struct Out {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

/// Ejecuta `argv` en el servidor (sin terminal), registrado en actividad sin
/// guardar la salida estándar.
async fn remote(
    engine: &Engine,
    kind: &str,
    target: &str,
    argv: Vec<String>,
    stdin: Option<Vec<u8>>,
    summary: Option<String>,
) -> Result<Out, EngineError> {
    let session = engine
        .ssh_session()
        .cloned()
        .ok_or_else(|| EngineError::Other {
            message: "no es un servidor remoto".into(),
        })?;
    let (program, args) = argv.split_first().expect("orden no vacía");
    let req = CliRequest {
        category: Category::Compose,
        kind: kind.into(),
        target: Some(target.into()),
        program: program.clone(),
        args: args.to_vec(),
        env: vec![],
        cwd: None,
        timeout: TIMEOUT,
        remote: Some(RemoteExec {
            session,
            tty: false,
        }),
    };
    let mut opts = StreamOptions::new(CancellationToken::new());
    opts.stdin = stdin;
    opts.collect_stdout = true;
    opts.record_stdout = false;
    opts.summary = summary;
    let run = run_streaming(engine.activity(), req, opts, |_| true).await?;
    let code = match run.end {
        crate::cli::RunEnd::Exited { code } => code,
        _ => None,
    };
    Ok(Out {
        code,
        stdout: run.stdout,
        stderr: run.stderr_tail,
    })
}

fn sv(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// Tipo y tamaño (`stat -L`), o `None` si no existe.
async fn stat(engine: &Engine, path: &str) -> Result<Option<(String, u64, String)>, EngineError> {
    let mut argv = sv(&[
        "env", "LC_ALL=C", "stat", "-L", "--printf", "%F|%s|%a", "--",
    ]);
    argv.push(path.into());
    let o = remote(engine, "compose.fileStat", path, argv, None, None).await?;
    if o.code != Some(0) {
        if o.stderr.contains("No such file") {
            return Ok(None);
        }
        return Err(EngineError::Other {
            message: format!("{path}: {}", o.stderr.trim()),
        });
    }
    let line = o.stdout.trim_end_matches('\n');
    let mut parts = line.splitn(3, '|');
    let kind = parts.next().unwrap_or("").to_owned();
    let size = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let mode = parts.next().unwrap_or("").to_owned();
    Ok(Some((kind, size, mode)))
}

fn is_file(kind: &str) -> bool {
    kind == "regular file" || kind == "regular empty file"
}

pub async fn read(engine: &Engine, path: &str) -> Result<ComposeFile, EngineError> {
    absolute(path)?;
    let Some((kind, size, _)) = stat(engine, path).await? else {
        return Err(invalid(format!("no existe en el servidor: {path}")));
    };
    if !is_file(&kind) {
        return Err(invalid(format!("{path} no es un archivo")));
    }
    if size as usize > MAX_FILE_BYTES {
        return Err(invalid(format!(
            "{path} ocupa más de {} KiB",
            MAX_FILE_BYTES / 1024
        )));
    }
    // base64: el contenido llega exacto (también saltos de línea y \r).
    let mut argv = sv(&["base64", "--"]);
    argv.push(path.into());
    let o = remote(
        engine,
        "compose.fileRead",
        path,
        argv,
        None,
        Some("Archivo leído (el contenido no se guarda en el registro)".into()),
    )
    .await?;
    if o.code != Some(0) {
        return Err(EngineError::Other {
            message: format!("{path}: {}", o.stderr.trim()),
        });
    }
    let b64: String = o.stdout.split_whitespace().collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| EngineError::InvalidResponse {
            message: format!("contenido no válido: {e}"),
        })?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(invalid(format!("{path} es demasiado grande")));
    }
    let content =
        String::from_utf8(bytes).map_err(|_| invalid(format!("{path} no es texto UTF-8")))?;
    let mut argv = sv(&["test", "-w"]);
    argv.push(path.into());
    let w = remote(engine, "compose.fileWritable", path, argv, None, None).await?;
    Ok(ComposeFile {
        path: path.to_owned(),
        version: version_of(content.as_bytes()),
        writable: w.code == Some(0),
        content,
    })
}

/// Escribe `content` en un archivo nuevo (`dd conv=excl`: falla si existe).
async fn write_new(
    engine: &Engine,
    path: &str,
    content: &str,
    kind: &str,
) -> Result<(), EngineError> {
    let argv = vec![
        "dd".into(),
        format!("of={path}"),
        "conv=excl,fsync".into(),
        "status=none".into(),
    ];
    let o = remote(
        engine,
        kind,
        path,
        argv,
        Some(content.as_bytes().to_vec()),
        Some("Archivo escrito".into()),
    )
    .await?;
    if o.code != Some(0) {
        if o.stderr.contains("File exists") {
            return Err(EngineError::AlreadyExists { path: path.into() });
        }
        return Err(EngineError::Other {
            message: format!("{path}: {}", o.stderr.trim()),
        });
    }
    Ok(())
}

/// Guarda si el archivo sigue siendo el que se leyó (`expected_version`).
pub async fn save(
    engine: &Engine,
    path: &str,
    content: &str,
    expected_version: &str,
) -> Result<ComposeFile, EngineError> {
    absolute(path)?;
    if content.len() > MAX_FILE_BYTES {
        return Err(invalid("el contenido es demasiado grande"));
    }
    let current = read(engine, path).await?;
    if current.version != expected_version {
        return Err(EngineError::FileChanged {
            path: path.to_owned(),
        });
    }
    if !current.writable {
        return Err(invalid(format!("no tienes permiso para escribir {path}")));
    }
    let Some((_, _, mode)) = stat(engine, path).await? else {
        return Err(EngineError::FileChanged {
            path: path.to_owned(),
        });
    };
    let (dir, name) = split(path);
    let tmp = format!(
        "{}/.{name}.dcc-{}.tmp",
        dir.trim_end_matches('/'),
        crate::ssh::askpass::random_hex(4)
    );
    let result = async {
        write_new(engine, &tmp, content, "compose.fileSave").await?;
        if mode.chars().all(|c| c.is_ascii_digit()) && !mode.is_empty() {
            let o = remote(
                engine,
                "compose.fileSave",
                path,
                vec!["chmod".into(), mode.clone(), "--".into(), tmp.clone()],
                None,
                None,
            )
            .await?;
            if o.code != Some(0) {
                return Err(EngineError::Other {
                    message: o.stderr.trim().to_owned(),
                });
            }
        }
        let o = remote(
            engine,
            "compose.fileSave",
            path,
            vec![
                "mv".into(),
                "-f".into(),
                "--".into(),
                tmp.clone(),
                path.into(),
            ],
            None,
            Some(format!("Guardado {path}")),
        )
        .await?;
        if o.code != Some(0) {
            return Err(EngineError::Other {
                message: o.stderr.trim().to_owned(),
            });
        }
        Ok(())
    }
    .await;
    if let Err(e) = result {
        let _ = remote(
            engine,
            "compose.fileSave",
            path,
            vec!["rm".into(), "-f".into(), "--".into(), tmp],
            None,
            None,
        )
        .await;
        return Err(e);
    }
    read(engine, path).await
}

/// Archivo Compose estándar que ya existe en la carpeta del servidor.
pub async fn existing_default(engine: &Engine, dir: &str) -> Result<Option<String>, EngineError> {
    absolute(dir)?;
    for n in DEFAULT_FILES {
        let p = format!("{}/{n}", dir.trim_end_matches('/'));
        if let Some((kind, _, _)) = stat(engine, &p).await? {
            if is_file(&kind) {
                return Ok(Some(p));
            }
        }
    }
    Ok(None)
}

/// Crea `compose.yaml` en una carpeta del servidor sin archivo Compose.
pub async fn create(engine: &Engine, dir: &str, content: &str) -> Result<ComposeFile, EngineError> {
    absolute(dir)?;
    if content.len() > MAX_FILE_BYTES {
        return Err(invalid("el contenido es demasiado grande"));
    }
    match stat(engine, dir).await? {
        Some((kind, _, _)) if kind == "directory" => {}
        Some(_) => return Err(invalid(format!("{dir} no es una carpeta"))),
        None => return Err(invalid(format!("no existe en el servidor: {dir}"))),
    }
    if let Some(p) = existing_default(engine, dir).await? {
        return Err(EngineError::AlreadyExists { path: p });
    }
    let path = format!("{}/compose.yaml", dir.trim_end_matches('/'));
    write_new(engine, &path, content, "compose.fileCreate").await?;
    read(engine, &path).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_split_without_touching_the_local_filesystem() {
        assert_eq!(
            split("/srv/app/compose.yaml"),
            ("/srv/app".into(), "compose.yaml".into())
        );
        assert_eq!(split("/compose.yaml"), ("/".into(), "compose.yaml".into()));
        assert!(absolute("relativo/compose.yaml").is_err());
        assert!(absolute("/x\0y").is_err());
        assert!(absolute("/srv/a b/compose.yaml").is_ok());
    }
}
