//! Petición HTTP/1.1 mínima sobre el socket Unix de Docker.
//!
//! - `POST /build`: bollard 0.21 no permite construir con BuildKit sin su
//!   integración gRPC ni enviar varias etiquetas (`t` repetido), así que esta
//!   petición se hace directamente: cuerpo tar en *chunked transfer encoding* y
//!   respuesta leída como una secuencia de objetos JSON.
//! - `POST /networks/create`: el cuerpo de bollard 0.21 no tiene el campo
//!   `CheckDuplicate`, imprescindible con Docker < 25 (D-048).
//! - `GET /system/df?type=volume`: la respuesta cambia de formato según la
//!   versión de Docker y el tipo de bollard 0.21 solo conoce el nuevo (D-051).

use std::io;

use bytes::Bytes;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use crate::client::{Dialer, RawStream};
use crate::error::EngineError;

pub struct Response {
    pub status: u16,
    reader: BufReader<tokio::io::ReadHalf<Box<dyn RawStream>>>,
    chunked: bool,
    remaining: Option<u64>,
    chunk_left: u64,
    done: bool,
    buf: Vec<u8>,
    pub writer: tokio::task::JoinHandle<io::Result<()>>,
}

fn io_err(e: io::Error) -> EngineError {
    EngineError::Io {
        message: e.to_string(),
    }
}

/// Envía la petición y devuelve la respuesta en cuanto llegan sus cabeceras.
/// El cuerpo se sigue enviando en segundo plano mientras se lee la respuesta.
pub async fn post_stream(
    dialer: &Dialer,
    path: &str,
    headers: &[(String, String)],
    body: mpsc::Receiver<Result<Bytes, io::Error>>,
) -> Result<Response, EngineError> {
    post_with(dialer, path, "application/x-tar", headers, body).await
}

/// Envía un cuerpo JSON completo y devuelve el JSON de la respuesta, o el
/// error de Docker con su código HTTP.
pub async fn post_json(
    dialer: &Dialer,
    path: &str,
    json: Vec<u8>,
) -> Result<serde_json::Value, EngineError> {
    let (tx, rx) = mpsc::channel(1);
    tx.send(Ok(Bytes::from(json)))
        .await
        .map_err(|_| EngineError::Other {
            message: "no se pudo preparar la petición".into(),
        })?;
    drop(tx);
    let mut r = post_with(dialer, path, "application/json", &[], rx).await?;
    if !(200..300).contains(&r.status) {
        let message = r.error_message().await;
        return Err(EngineError::Docker {
            status: r.status,
            message,
        });
    }
    Ok(r.next_json().await?.unwrap_or(serde_json::Value::Null))
}

/// `GET` que devuelve la respuesta en cuanto llegan sus cabeceras (el cuerpo
/// se lee después, p. ej. el flujo continuo de `GET /events`, Fase 8).
pub async fn get_stream(dialer: &Dialer, path: &str) -> Result<Response, EngineError> {
    let stream = dialer.connect().await.map_err(io_err)?;
    let (rd, mut wr) = tokio::io::split(stream);
    let head =
        format!("GET {path} HTTP/1.1\r\nHost: docker\r\nUser-Agent: docker-control-center\r\n\r\n");
    let writer = tokio::spawn(async move {
        wr.write_all(head.as_bytes()).await?;
        wr.flush().await?;
        std::future::pending::<()>().await;
        Ok(())
    });
    read_response(rd, writer).await
}

/// `GET` que devuelve el JSON de la respuesta, o el error de Docker.
pub async fn get_json(dialer: &Dialer, path: &str) -> Result<serde_json::Value, EngineError> {
    let mut r = get_stream(dialer, path).await?;
    if !(200..300).contains(&r.status) {
        let message = r.error_message().await;
        return Err(EngineError::Docker {
            status: r.status,
            message,
        });
    }
    Ok(r.next_json().await?.unwrap_or(serde_json::Value::Null))
}

async fn post_with(
    dialer: &Dialer,
    path: &str,
    content_type: &str,
    headers: &[(String, String)],
    mut body: mpsc::Receiver<Result<Bytes, io::Error>>,
) -> Result<Response, EngineError> {
    let stream = dialer.connect().await.map_err(io_err)?;
    let (rd, mut wr) = tokio::io::split(stream);
    let mut head = format!("POST {path} HTTP/1.1\r\nHost: docker\r\nUser-Agent: docker-control-center\r\nContent-Type: {content_type}\r\nTransfer-Encoding: chunked\r\n");
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    let writer = tokio::spawn(async move {
        wr.write_all(head.as_bytes()).await?;
        while let Some(chunk) = body.recv().await {
            let chunk = chunk?;
            if chunk.is_empty() {
                continue;
            }
            wr.write_all(format!("{:x}\r\n", chunk.len()).as_bytes())
                .await?;
            wr.write_all(&chunk).await?;
            wr.write_all(b"\r\n").await?;
        }
        wr.write_all(b"0\r\n\r\n").await?;
        wr.flush().await?;
        // Se mantiene la mitad de escritura abierta: cerrarla podría cortar la respuesta.
        std::future::pending::<()>().await;
        Ok(())
    });
    read_response(rd, writer).await
}

/// Lee la línea de estado y las cabeceras.
async fn read_response(
    rd: tokio::io::ReadHalf<Box<dyn RawStream>>,
    writer: tokio::task::JoinHandle<io::Result<()>>,
) -> Result<Response, EngineError> {
    let mut reader = BufReader::new(rd);
    let mut line = String::new();
    reader.read_line(&mut line).await.map_err(io_err)?;
    let status: u16 = line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| EngineError::InvalidResponse {
            message: format!("respuesta HTTP no válida: {line:?}"),
        })?;
    let mut chunked = false;
    let mut remaining = None;
    loop {
        line.clear();
        if reader.read_line(&mut line).await.map_err(io_err)? == 0 {
            break;
        }
        let l = line.trim_end();
        if l.is_empty() {
            break;
        }
        if let Some((k, v)) = l.split_once(':') {
            let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
            if k == "transfer-encoding" && v.eq_ignore_ascii_case("chunked") {
                chunked = true;
            } else if k == "content-length" {
                remaining = v.parse().ok();
            }
        }
    }
    Ok(Response {
        status,
        reader,
        chunked,
        remaining,
        chunk_left: 0,
        done: false,
        buf: Vec::new(),
        writer,
    })
}

impl Response {
    /// Siguiente trozo del cuerpo (ya sin la codificación chunked); `None` al final.
    async fn read_body(&mut self) -> Result<Option<Vec<u8>>, EngineError> {
        if self.done {
            return Ok(None);
        }
        if self.chunked {
            if self.chunk_left == 0 {
                let mut line = String::new();
                if self.reader.read_line(&mut line).await.map_err(io_err)? == 0 {
                    self.done = true;
                    return Ok(None);
                }
                if line.trim().is_empty() {
                    // CRLF tras el trozo anterior.
                    line.clear();
                    if self.reader.read_line(&mut line).await.map_err(io_err)? == 0 {
                        self.done = true;
                        return Ok(None);
                    }
                }
                let size = u64::from_str_radix(line.trim().split(';').next().unwrap_or(""), 16)
                    .map_err(|_| EngineError::InvalidResponse {
                        message: format!("trozo HTTP no válido: {line:?}"),
                    })?;
                if size == 0 {
                    self.done = true;
                    return Ok(None);
                }
                self.chunk_left = size;
            }
            let want = self.chunk_left.min(64 * 1024) as usize;
            let mut buf = vec![0u8; want];
            let n = self.reader.read(&mut buf).await.map_err(io_err)?;
            if n == 0 {
                self.done = true;
                return Ok(None);
            }
            self.chunk_left -= n as u64;
            buf.truncate(n);
            Ok(Some(buf))
        } else {
            let want = self.remaining.map_or(64 * 1024, |r| r.min(64 * 1024)) as usize;
            if want == 0 {
                self.done = true;
                return Ok(None);
            }
            let mut buf = vec![0u8; want];
            let n = self.reader.read(&mut buf).await.map_err(io_err)?;
            if n == 0 {
                self.done = true;
                return Ok(None);
            }
            if let Some(r) = self.remaining.as_mut() {
                *r -= n as u64;
            }
            buf.truncate(n);
            Ok(Some(buf))
        }
    }

    /// Siguiente objeto JSON de la respuesta; `None` al terminar.
    pub async fn next_json(&mut self) -> Result<Option<serde_json::Value>, EngineError> {
        loop {
            // Salta espacios y saltos de línea entre objetos.
            let start = self
                .buf
                .iter()
                .position(|b| !b.is_ascii_whitespace())
                .unwrap_or(self.buf.len());
            self.buf.drain(..start);
            if !self.buf.is_empty() {
                let mut it = serde_json::Deserializer::from_slice(&self.buf)
                    .into_iter::<serde_json::Value>();
                match it.next() {
                    Some(Ok(v)) => {
                        let used = it.byte_offset();
                        self.buf.drain(..used);
                        return Ok(Some(v));
                    }
                    Some(Err(e)) if e.is_eof() => {}
                    Some(Err(e)) => {
                        return Err(EngineError::InvalidResponse {
                            message: format!("respuesta de Docker no válida: {e}"),
                        });
                    }
                    None => {}
                }
            }
            match self.read_body().await? {
                Some(b) => self.buf.extend_from_slice(&b),
                None => {
                    if self.buf.iter().all(|b| b.is_ascii_whitespace()) {
                        return Ok(None);
                    }
                    return Err(EngineError::InvalidResponse {
                        message: format!(
                            "respuesta de Docker cortada: {}",
                            String::from_utf8_lossy(&self.buf)
                        ),
                    });
                }
            }
        }
    }

    /// Cuerpo completo (para respuestas de error), limitado a 1 MiB.
    pub async fn error_message(&mut self) -> String {
        let mut all = Vec::new();
        while let Ok(Some(b)) = self.read_body().await {
            all.extend_from_slice(&b);
            if all.len() > 1 << 20 {
                break;
            }
        }
        serde_json::from_slice::<serde_json::Value>(&all)
            .ok()
            .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(str::to_owned))
            .unwrap_or_else(|| String::from_utf8_lossy(&all).trim().to_owned())
    }
}

impl Drop for Response {
    fn drop(&mut self) {
        // Cerrar la conexión es lo que hace que Docker cancele la construcción.
        self.writer.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::UnixListener;

    async fn server(
        reply: &'static [u8],
    ) -> (tempfile::TempDir, String, tokio::task::JoinHandle<Vec<u8>>) {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("s.sock");
        let l = UnixListener::bind(&p).unwrap();
        let task = tokio::spawn(async move {
            let (mut s, _) = l.accept().await.unwrap();
            let mut got = Vec::new();
            let mut buf = [0u8; 4096];
            // Lee hasta el final del cuerpo chunked.
            while !got.ends_with(b"0\r\n\r\n") {
                let n = s.read(&mut buf).await.unwrap();
                if n == 0 {
                    break;
                }
                got.extend_from_slice(&buf[..n]);
            }
            s.write_all(reply).await.unwrap();
            s.shutdown().await.unwrap();
            got
        });
        (d, p.to_string_lossy().into_owned(), task)
    }

    #[tokio::test]
    async fn sends_chunked_body_and_parses_chunked_json_stream() {
        let reply = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n1d\r\n{\"stream\":\"Step 1/1 : FROM x\"\r\n11\r\n}\r\n{\"aux\":{\"ID\":1\r\n4\r\n}}\r\n\r\n0\r\n\r\n";
        let (_d, sock, task) = server(reply).await;
        let (tx, rx) = mpsc::channel(4);
        tx.send(Ok(Bytes::from_static(b"hola"))).await.unwrap();
        tx.send(Ok(Bytes::from_static(b" tar"))).await.unwrap();
        drop(tx);
        let mut r = post_stream(
            &crate::client::Dialer::Unix(sock.clone()),
            "/build?t=a&t=b",
            &[("X-Registry-Config".into(), "e30=".into())],
            rx,
        )
        .await
        .unwrap();
        assert_eq!(r.status, 200);
        let a = r.next_json().await.unwrap().unwrap();
        assert_eq!(a["stream"], "Step 1/1 : FROM x");
        let b = r.next_json().await.unwrap().unwrap();
        assert_eq!(b["aux"]["ID"], 1);
        assert!(r.next_json().await.unwrap().is_none());
        let got = String::from_utf8(task.await.unwrap()).unwrap();
        assert!(got.starts_with("POST /build?t=a&t=b HTTP/1.1\r\n"), "{got}");
        assert!(got.contains("Transfer-Encoding: chunked\r\n"));
        assert!(got.contains("X-Registry-Config: e30=\r\n"));
        assert!(
            got.ends_with("4\r\nhola\r\n4\r\n tar\r\n0\r\n\r\n"),
            "{got:?}"
        );
    }

    #[tokio::test]
    async fn error_responses_expose_dockers_message() {
        let reply = b"HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: 38\r\n\r\n{\"message\":\"Cannot locate Dockerfile\"}";
        let (_d, sock, _task) = server(reply).await;
        let (tx, rx) = mpsc::channel(1);
        drop(tx);
        let mut r = post_stream(
            &crate::client::Dialer::Unix(sock.clone()),
            "/build",
            &[],
            rx,
        )
        .await
        .unwrap();
        assert_eq!(r.status, 400);
        assert_eq!(r.error_message().await, "Cannot locate Dockerfile");
    }
}
