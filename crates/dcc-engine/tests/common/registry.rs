//! Registro de imágenes mínimo (API de distribución v2) para las pruebas de
//! descarga: así el daemon real descarga una imagen sin Internet.
//!
//! Sirve la imagen de prueba exportada con `GET /images/{name}/get`, con la capa
//! comprimida en gzip y un manifiesto schema 2. Opcionalmente exige
//! autenticación básica y puede retrasar los blobs para probar la cancelación.
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
    /// Retraso antes de enviar cada blob de capa.
    pub blob_delay: Option<Duration>,
}

/// (método, ruta, cabecera Authorization).
pub type Requests = Arc<Mutex<Vec<(String, String, Option<String>)>>>;

pub struct Registry {
    pub addr: String,
    /// (método, ruta, cabecera Authorization) de cada petición recibida.
    pub requests: Requests,
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
    let mut blobs = HashMap::new();
    let config_digest = sha(&config);
    let mut layers = Vec::new();
    for l in m[0]["Layers"].as_array().unwrap() {
        let raw = &files[l.as_str().unwrap()];
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gz.write_all(raw).unwrap();
        let gz = gz.finish().unwrap();
        let d = sha(&gz);
        layers.push(serde_json::json!({
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
        "layers": layers,
    }))
    .unwrap();
    blobs.insert(config_digest, (config, false));
    Served {
        manifest_digest: sha(&manifest),
        manifest,
        blobs,
    }
}

impl Registry {
    pub async fn start(served: Served, opts: Options) -> Self {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap().to_string();
        let requests: Requests = Default::default();
        let reqs = requests.clone();
        let served = Arc::new(served);
        let task = tokio::spawn(async move {
            loop {
                let Ok((sock, _)) = l.accept().await else {
                    break;
                };
                let served = served.clone();
                let reqs = reqs.clone();
                let opts = opts.clone();
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
                        let (status, ctype, body, extra): (&str, &str, Vec<u8>, String) =
                            if !authorized {
                                ("401 Unauthorized", "application/json", br#"{"errors":[{"code":"UNAUTHORIZED","message":"authentication required"}]}"#.to_vec(), "WWW-Authenticate: Basic realm=\"dcc-test\"\r\nDocker-Distribution-API-Version: registry/2.0\r\n".into())
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
                                            if *layer {
                                                if let Some(delay) = opts.blob_delay {
                                                    tokio::time::sleep(delay).await;
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
                        if method != "HEAD" && wr.write_all(&body).await.is_err() {
                            return;
                        }
                    }
                });
            }
        });
        Self {
            addr,
            requests,
            task,
        }
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
