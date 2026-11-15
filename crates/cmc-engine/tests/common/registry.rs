// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Registro de imágenes mínimo (API de distribución v2) para las pruebas de
//! descarga: así el daemon real descarga una imagen sin Internet.
//!
//! Sirve una imagen con manifiesto schema 2 y capas en gzip:
//! - [`unique`]: una imagen nueva, con una capa de contenido aleatorio, que el
//!   daemon no puede tener; así siempre tiene que descargar la capa, sea cual
//!   sea su almacén de imágenes (clásico o containerd).
//! - [`export`]: la imagen de prueba exportada con `GET /images/{name}/get`,
//!   cuyo contenido el daemon ya tiene.
//!
//! Opcionalmente exige autenticación básica, retrasa los blobs de capa (para
//! probar la cancelación), envía las capas a ritmo controlado (para que Docker
//! informe del progreso) y anota los blobs cuya conexión cerró el cliente.
#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

fn sha(b: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(b))
}

#[derive(Clone, Default)]
pub struct Options {
    /// (usuario, contraseña) exigidos con autenticación básica.
    pub auth: Option<(String, String)>,
    /// Retraso antes de enviar cada blob de capa (se puede cambiar después con
    /// [`Registry::set_blob_delay`]).
    pub blob_delay: Option<Duration>,
    /// Pausa entre bloques de 64 KiB al enviar una capa.
    pub chunk_delay: Option<Duration>,
}

const CHUNK: usize = 64 * 1024;

/// (método, ruta, cabecera Authorization).
pub type Requests = Arc<Mutex<Vec<(String, String, Option<String>)>>>;

pub struct Registry {
    pub addr: String,
    /// (método, ruta, cabecera Authorization) de cada petición recibida.
    pub requests: Requests,
    /// Blobs de capa cuya conexión cerró el cliente antes de recibirlos enteros.
    pub aborted: Arc<Mutex<Vec<String>>>,
    blob_delay: Arc<Mutex<Option<Duration>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Registry {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Blobs y manifiesto de una imagen local, listos para servir.
pub struct Served {
    pub manifest: Vec<u8>,
    pub manifest_digest: String,
    pub blobs: HashMap<String, (Vec<u8>, bool)>, // digest → (datos, es capa)
}

impl Served {
    /// Digest y tamaño comprimido de las capas, en orden.
    pub fn layers(&self) -> Vec<(String, usize)> {
        let m: serde_json::Value = serde_json::from_slice(&self.manifest).unwrap();
        m["layers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| {
                (
                    l["digest"].as_str().unwrap().to_owned(),
                    l["size"].as_u64().unwrap() as usize,
                )
            })
            .collect()
    }
}

fn host_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        "arm" => "arm",
        other => other,
    }
}

fn gzip(raw: &[u8]) -> Vec<u8> {
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(raw).unwrap();
    gz.finish().unwrap()
}

fn manifest_for(config: Vec<u8>, layers: Vec<Vec<u8>>) -> Served {
    let mut blobs = HashMap::new();
    let config_digest = sha(&config);
    let mut descs = Vec::new();
    for gz in layers {
        let d = sha(&gz);
        descs.push(serde_json::json!({
            "mediaType": "application/vnd.docker.image.rootfs.diff.tar.gzip",
            "size": gz.len(),
            "digest": d,
        }));
        blobs.insert(d, (gz, true));
    }
    let manifest = serde_json::to_vec(&serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
        "config": {
            "mediaType": "application/vnd.docker.container.image.v1+json",
            "size": config.len(),
            "digest": config_digest,
        },
        "layers": descs,
    }))
    .unwrap();
    blobs.insert(config_digest, (config, false));
    Served {
        manifest_digest: sha(&manifest),
        manifest,
        blobs,
    }
}

/// Imagen nueva de una capa con `size` bytes pseudoaleatorios (no se comprimen),
/// distinta en cada llamada. Lleva la etiqueta `cmc.test=true`.
pub fn unique(size: usize) -> Served {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    let seed = format!(
        "{}-{nanos}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    );
    // xorshift64*: rápido y sin dependencias; suficiente para que gzip no comprima.
    let mut x = u64::from_str_radix(&format!("{:x}", Sha256::digest(seed.as_bytes()))[..16], 16)
        .unwrap()
        | 1;
    let mut data = Vec::with_capacity(size);
    while data.len() < size {
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        data.extend_from_slice(&x.wrapping_mul(0x2545_F491_4F6C_DD1D).to_le_bytes());
    }
    data.truncate(size);
    let mut tar = tar::Builder::new(Vec::new());
    for (path, body) in [("cmc-unique", seed.as_bytes()), ("cmc-data", &data[..])] {
        let mut h = tar::Header::new_gnu();
        h.set_size(body.len() as u64);
        h.set_mode(0o644);
        h.set_mtime(0);
        h.set_cksum();
        tar.append_data(&mut h, path, body).unwrap();
    }
    let raw = tar.into_inner().unwrap();
    let config = serde_json::to_vec(&serde_json::json!({
        "architecture": host_arch(),
        "os": "linux",
        "created": "2026-01-01T00:00:00Z",
        "config": { "Labels": { "cmc.test": "true", "cmc.unique": seed } },
        "rootfs": { "type": "layers", "diff_ids": [sha(&raw)] },
        "history": [{ "created": "2026-01-01T00:00:00Z", "created_by": "cmc-test unique layer" }],
    }))
    .unwrap();
    manifest_for(config, vec![gzip(&raw)])
}

pub async fn export(docker: &bollard::Docker, image: &str) -> Served {
    let mut s = docker.export_image(image);
    let mut tar_bytes = Vec::new();
    while let Some(c) = s.next().await {
        tar_bytes.extend_from_slice(&c.expect("exportar imagen"));
    }
    let mut files: HashMap<String, Vec<u8>> = HashMap::new();
    let mut a = tar::Archive::new(&tar_bytes[..]);
    for e in a.entries().unwrap() {
        let mut e = e.unwrap();
        let p = e.path().unwrap().to_string_lossy().into_owned();
        let mut buf = Vec::new();
        e.read_to_end(&mut buf).unwrap();
        files.insert(p, buf);
    }
    let m: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    let config = files[m[0]["Config"].as_str().unwrap()].clone();
    let layers = m[0]["Layers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| gzip(&files[l.as_str().unwrap()]))
        .collect();
    manifest_for(config, layers)
}

impl Registry {
    pub async fn start(served: Served, opts: Options) -> Self {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap().to_string();
        let requests: Requests = Default::default();
        let reqs = requests.clone();
        let aborted: Arc<Mutex<Vec<String>>> = Default::default();
        let aborted2 = aborted.clone();
        let blob_delay = Arc::new(Mutex::new(opts.blob_delay));
        let blob_delay2 = blob_delay.clone();
        let served = Arc::new(served);
        let task = tokio::spawn(async move {
            loop {
                let Ok((sock, _)) = l.accept().await else {
                    break;
                };
                let served = served.clone();
                let reqs = reqs.clone();
                let opts = opts.clone();
                let aborted = aborted2.clone();
                let blob_delay = blob_delay2.clone();
                tokio::spawn(async move {
                    // Si el cliente intenta TLS (byte 0x16), se responde en HTTP
                    // plano como un servidor Go: así Docker pasa a HTTP, igual
                    // que con un registro local real sin TLS.
                    let mut first = [0u8; 1];
                    if sock.peek(&mut first).await.unwrap_or(0) == 1 && first[0] == 0x16 {
                        let mut s = sock;
                        let _ = s.write_all(b"HTTP/1.0 400 Bad Request\r\n\r\nClient sent an HTTP request to an HTTPS server.\n").await;
                        return;
                    }
                    let (rd, mut wr) = sock.into_split();
                    let mut rd = BufReader::new(rd);
                    loop {
                        let mut line = String::new();
                        if rd.read_line(&mut line).await.unwrap_or(0) == 0 {
                            return;
                        }
                        let mut parts = line.split_whitespace();
                        let (method, path) = (
                            parts.next().unwrap_or("").to_owned(),
                            parts.next().unwrap_or("").to_owned(),
                        );
                        let mut auth = None;
                        loop {
                            let mut h = String::new();
                            if rd.read_line(&mut h).await.unwrap_or(0) == 0 {
                                return;
                            }
                            let h = h.trim_end();
                            if h.is_empty() {
                                break;
                            }
                            if let Some((k, v)) = h.split_once(':') {
                                if k.eq_ignore_ascii_case("authorization") {
                                    auth = Some(v.trim().to_owned());
                                }
                            }
                        }
                        reqs.lock()
                            .unwrap()
                            .push((method.clone(), path.clone(), auth.clone()));
                        let authorized = match &opts.auth {
                            None => true,
                            Some((u, p)) => {
                                let want = format!(
                                    "Basic {}",
                                    base64::engine::general_purpose::STANDARD
                                        .encode(format!("{u}:{p}"))
                                );
                                auth.as_deref() == Some(want.as_str())
                            }
                        };
                        let mut layer_blob: Option<String> = None;
                        let (status, ctype, body, extra): (&str, &str, Vec<u8>, String) =
                            if !authorized {
                                ("401 Unauthorized", "application/json", br#"{"errors":[{"code":"UNAUTHORIZED","message":"authentication required"}]}"#.to_vec(), "WWW-Authenticate: Basic realm=\"cmc-test\"\r\nDocker-Distribution-API-Version: registry/2.0\r\n".into())
                            } else if path == "/v2/" || path == "/v2" {
                                (
                                    "200 OK",
                                    "application/json",
                                    b"{}".to_vec(),
                                    "Docker-Distribution-API-Version: registry/2.0\r\n".into(),
                                )
                            } else if let Some(rest) = path.strip_prefix("/v2/") {
                                if rest.contains("/manifests/") {
                                    let r = rest.rsplit('/').next().unwrap_or("");
                                    if r == "1" || r == served.manifest_digest {
                                        (
                                            "200 OK",
                                            "application/vnd.docker.distribution.manifest.v2+json",
                                            served.manifest.clone(),
                                            format!(
                                                "Docker-Content-Digest: {}\r\n",
                                                served.manifest_digest
                                            ),
                                        )
                                    } else {
                                        ("404 Not Found", "application/json", br#"{"errors":[{"code":"MANIFEST_UNKNOWN","message":"manifest unknown"}]}"#.to_vec(), String::new())
                                    }
                                } else if rest.contains("/blobs/") {
                                    let d = rest.rsplit('/').next().unwrap_or("");
                                    match served.blobs.get(d) {
                                        Some((b, layer)) => {
                                            if *layer && method != "HEAD" {
                                                layer_blob = Some(d.to_owned());
                                                let delay = *blob_delay.lock().unwrap();
                                                if let Some(delay) = delay {
                                                    // Espera vigilando si el cliente cierra la conexión.
                                                    let sleep = tokio::time::sleep(delay);
                                                    tokio::pin!(sleep);
                                                    tokio::select! {
                                                        _ = &mut sleep => {}
                                                        r = rd.fill_buf() => {
                                                            if r.map(|b| b.is_empty()).unwrap_or(true) {
                                                                aborted.lock().unwrap().push(d.to_owned());
                                                                return;
                                                            }
                                                            sleep.await;
                                                        }
                                                    }
                                                }
                                            }
                                            (
                                                "200 OK",
                                                "application/octet-stream",
                                                b.clone(),
                                                format!("Docker-Content-Digest: {d}\r\n"),
                                            )
                                        }
                                        None => (
                                            "404 Not Found",
                                            "application/json",
                                            br#"{"errors":[{"code":"BLOB_UNKNOWN"}]}"#.to_vec(),
                                            String::new(),
                                        ),
                                    }
                                } else {
                                    ("404 Not Found", "text/plain", Vec::new(), String::new())
                                }
                            } else {
                                ("404 Not Found", "text/plain", Vec::new(), String::new())
                            };
                        let head = format!("HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\n{extra}\r\n", body.len());
                        if wr.write_all(head.as_bytes()).await.is_err() {
                            return;
                        }
                        if method == "HEAD" {
                            continue;
                        }
                        let pace = layer_blob.as_ref().and(opts.chunk_delay);
                        for chunk in body.chunks(if pace.is_some() {
                            CHUNK
                        } else {
                            body.len().max(1)
                        }) {
                            if wr.write_all(chunk).await.is_err() || wr.flush().await.is_err() {
                                if let Some(d) = &layer_blob {
                                    aborted.lock().unwrap().push(d.clone());
                                }
                                return;
                            }
                            if let Some(p) = pace {
                                tokio::time::sleep(p).await;
                            }
                        }
                    }
                });
            }
        });
        Self {
            addr,
            requests,
            aborted,
            blob_delay,
            task,
        }
    }

    pub fn set_blob_delay(&self, d: Option<Duration>) {
        *self.blob_delay.lock().unwrap() = d;
    }

    /// Peticiones GET de blobs de capa recibidas.
    pub fn layer_gets(&self, digest: &str) -> usize {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|(m, p, _)| m == "GET" && p.ends_with(&format!("/blobs/{digest}")))
            .count()
    }

    pub fn paths(&self) -> Vec<String> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .map(|(m, p, _)| format!("{m} {p}"))
            .collect()
    }
}
