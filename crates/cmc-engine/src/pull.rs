// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Descarga de imágenes (`docker pull`) con progreso por capa.
//!
//! Una descarga es **una** petición `POST /images/create` y un registro en el
//! panel de actividad que permanece "en curso" hasta que termina. Si el usuario
//! la cancela se cierra la conexión, Docker interrumpe la descarga y el
//! registro queda como cancelado.

use std::collections::BTreeMap;
use std::time::Duration;

use bollard::query_parameters::CreateImageOptions;
use cmc_activity::{Category, Failure, OperationSpec, Outcome, Transport};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::client::Engine;
use crate::error::EngineError;
use crate::reference::ImageRef;
use crate::registry_auth::{self, PullAuth};
use crate::resources::path_with_query;
use crate::streams::{EndReason, StreamEnd};

const FLUSH: Duration = Duration::from_millis(150);

/// Estado de una capa durante la descarga.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LayerProgress {
    pub id: String,
    /// Texto de Docker: "Waiting", "Downloading", "Extracting", "Pull complete"…
    pub status: String,
    #[ts(type = "number | null")]
    pub current: Option<i64>,
    #[ts(type = "number | null")]
    pub total: Option<i64>,
}

/// Lo que se envía a la interfaz en cada actualización.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PullUpdate {
    /// Estado actual de todas las capas conocidas, en orden de aparición.
    pub layers: Vec<LayerProgress>,
    /// Mensajes generales nuevos ("Pulling from…", "Digest: …", "Status: …").
    pub messages: Vec<String>,
}

/// Acumula los mensajes de progreso de Docker.
#[derive(Debug, Default)]
pub struct PullTracker {
    order: Vec<String>,
    layers: BTreeMap<String, LayerProgress>,
    pending: Vec<String>,
    dirty: bool,
    pub digest: Option<String>,
    pub final_status: Option<String>,
}

impl PullTracker {
    pub fn push(
        &mut self,
        id: Option<&str>,
        status: Option<&str>,
        current: Option<i64>,
        total: Option<i64>,
    ) {
        let status = status.unwrap_or_default();
        match id.filter(|i| !i.is_empty()) {
            // Las líneas con ID de capa (12 caracteres hexadecimales) son progreso.
            Some(i) if i.len() == 12 && i.bytes().all(|c| c.is_ascii_hexdigit()) => {
                if !self.layers.contains_key(i) {
                    self.order.push(i.to_owned());
                }
                let e = self
                    .layers
                    .entry(i.to_owned())
                    .or_insert_with(|| LayerProgress {
                        id: i.to_owned(),
                        status: String::new(),
                        current: None,
                        total: None,
                    });
                e.status = status.to_owned();
                // Solo hay bytes mientras descarga o extrae.
                if current.is_some() || total.is_some() {
                    e.current = current;
                    e.total = total.filter(|t| *t > 0);
                } else if matches!(
                    status,
                    "Download complete" | "Pull complete" | "Already exists"
                ) {
                    e.current = e.total;
                }
                self.dirty = true;
            }
            // "1.27: Pulling from library/nginx" llega con la etiqueta como ID.
            Some(i) => self.message(format!("{i}: {status}")),
            None => {
                if let Some(d) = status.strip_prefix("Digest: ") {
                    self.digest = Some(d.trim().to_owned());
                }
                if let Some(st) = status.strip_prefix("Status: ") {
                    self.final_status = Some(st.trim().to_owned());
                }
                self.message(status.to_owned());
            }
        }
    }

    fn message(&mut self, m: String) {
        if !m.trim().is_empty() {
            self.pending.push(m);
            self.dirty = true;
        }
    }

    /// Actualización pendiente para la interfaz, si hubo cambios.
    pub fn take_update(&mut self) -> Option<PullUpdate> {
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        Some(PullUpdate {
            layers: self
                .order
                .iter()
                .filter_map(|i| self.layers.get(i).cloned())
                .collect(),
            messages: std::mem::take(&mut self.pending),
        })
    }

    /// Capas descargadas. Con el almacén de imágenes clásico el estado final es
    /// "Pull complete"; con el de containerd (Docker 29), "Download complete".
    pub fn downloaded(&self) -> usize {
        self.layers
            .values()
            .filter(|l| matches!(l.status.as_str(), "Pull complete" | "Download complete"))
            .count()
    }

    /// Resumen de una descarga terminada. Si Docker no informó de ninguna capa
    /// (ya tenía todo el contenido, por ejemplo la misma imagen con otro
    /// nombre en el almacén clásico) se dice así, en lugar de "0 capas".
    pub fn finished_summary(&self, shown: &str, auth_note: &str) -> String {
        let head = format!(
            "{shown}: {}{}",
            self.final_status.as_deref().unwrap_or("descarga terminada"),
            self.digest
                .as_deref()
                .map(|d| format!(" ({d})"))
                .unwrap_or_default(),
        );
        if self.layers.is_empty() {
            format!(
                "{head}; Docker ya tenía todo su contenido, no ha descargado capas; {auth_note}"
            )
        } else {
            format!(
                "{head}; {} capas descargadas, {} ya existían; {auth_note}",
                self.downloaded(),
                self.existing()
            )
        }
    }

    pub fn existing(&self) -> usize {
        self.layers
            .values()
            .filter(|l| matches!(l.status.as_str(), "Already exists" | "Exists"))
            .count()
    }
}

/// Opciones y ruta exactas de la petición de descarga.
pub fn pull_request(r: &ImageRef) -> (CreateImageOptions, String) {
    // Como el CLI: nombre completo en fromImage y etiqueta (o digest) en tag.
    let opts = CreateImageOptions {
        from_image: Some(r.name()),
        tag: Some(r.api_tag()),
        ..Default::default()
    };
    let path = path_with_query("/images/create", &opts);
    (opts, path)
}

impl Engine {
    /// Descarga una imagen. `sink` recibe el progreso; si devuelve `false` (la
    /// ventana dejó de escuchar) la descarga se cancela. En un servidor remoto
    /// no se envían las credenciales de este equipo (ver [`Self::pull_image_with`]).
    pub async fn pull_image<F>(
        &self,
        reference: &str,
        auth: &PullAuth,
        cancel: CancellationToken,
        sink: F,
    ) -> Result<StreamEnd, EngineError>
    where
        F: FnMut(PullUpdate) -> bool + Send,
    {
        self.pull_image_with(reference, auth, false, cancel, sink)
            .await
    }

    /// Como [`Self::pull_image`]; `share_local_credentials` permite enviar a un
    /// servidor remoto las credenciales de la configuración de Docker de este
    /// equipo (E2, CMC-SEC-07, D-090).
    pub async fn pull_image_with<F>(
        &self,
        reference: &str,
        auth: &PullAuth,
        share_local_credentials: bool,
        cancel: CancellationToken,
        mut sink: F,
    ) -> Result<StreamEnd, EngineError>
    where
        F: FnMut(PullUpdate) -> bool + Send,
    {
        let auth = registry_auth::effective_auth(
            self.ssh_session().is_some(),
            share_local_credentials,
            auth,
        );
        let r = ImageRef::parse(reference).map_err(|e| EngineError::InvalidInput {
            message: format!("referencia de imagen no válida: {e}"),
        })?;
        if let PullAuth::Manual { username, password } = auth {
            if username.trim().is_empty() || password.is_empty() {
                return Err(EngineError::InvalidInput {
                    message: "faltan el usuario o la contraseña".into(),
                });
            }
        }
        let resolved = registry_auth::resolve(self.activity(), &r.auth_key(), auth).await;
        let (opts, path) = pull_request(&r);
        let shown = r.familiar();
        let handle = self.activity().begin(OperationSpec {
            category: Category::Image,
            kind: "image.pull".into(),
            target: Some(shown.clone()),
            transport: Transport::Api {
                method: "POST".into(),
                path,
                endpoint: self.endpoint().socket_path.clone(),
            },
            request_body: None,
            equivalent_command: Some(self.docker_cli(&["pull".into(), shown.clone()])),
        });
        let mut stream = self
            .docker()
            .create_image(Some(opts), None, resolved.credentials.clone());
        let mut tracker = PullTracker::default();
        if let Some(w) = &resolved.warning {
            tracker.message(format!("Aviso: {w}"));
        }
        let mut tick = tokio::time::interval(FLUSH);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        let result: Result<EndReason, EngineError> = loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => break Ok(EndReason::Closed),
                item = stream.next() => match item {
                    Some(Ok(info)) => {
                        if let Some(err) = info.error_detail.and_then(|d| d.message) {
                            break Err(EngineError::Docker { status: 500, message: err });
                        }
                        let (cur, tot) = info.progress_detail.map_or((None, None), |p| (p.current, p.total));
                        tracker.push(info.id.as_deref(), info.status.as_deref(), cur, tot);
                    }
                    Some(Err(e)) => break Err(EngineError::from(e)),
                    None => break Ok(EndReason::Finished),
                },
                _ = tick.tick() => {
                    if let Some(u) = tracker.take_update() {
                        if !sink(u) {
                            break Ok(EndReason::Closed);
                        }
                    }
                }
            }
        };
        drop(stream);
        if let Some(u) = tracker.take_update() {
            let _ = sink(u);
        }
        let auth_note = resolved.source.describe();
        match result {
            Ok(EndReason::Finished) => {
                let summary = tracker.finished_summary(&shown, &auth_note);
                handle.succeed(Outcome {
                    summary: Some(summary.clone()),
                    ..Default::default()
                });
                Ok(StreamEnd {
                    reason: EndReason::Finished,
                    summary,
                    exit_code: None,
                    container: None,
                })
            }
            Ok(EndReason::Closed) => {
                let summary = format!(
                    "Descarga de {shown} cancelada por el usuario ({} capas completadas)",
                    tracker.downloaded() + tracker.existing()
                );
                handle.fail(Failure {
                    error: summary.clone(),
                    ..Default::default()
                });
                Ok(StreamEnd {
                    reason: EndReason::Closed,
                    summary,
                    exit_code: None,
                    container: None,
                })
            }
            Err(e) => {
                handle.fail(Failure {
                    error: format!("{e} ({auth_note})"),
                    http_status: e.http_status(),
                    ..Default::default()
                });
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracker_follows_docker_progress_messages() {
        let mut t = PullTracker::default();
        t.push(Some("1.27"), Some("Pulling from library/nginx"), None, None);
        t.push(Some("a1b2c3d4e5f6"), Some("Pulling fs layer"), None, None);
        t.push(Some("0f0f0f0f0f0f"), Some("Already exists"), None, None);
        t.push(
            Some("a1b2c3d4e5f6"),
            Some("Downloading"),
            Some(512),
            Some(2048),
        );
        let u = t.take_update().unwrap();
        assert_eq!(u.messages, vec!["1.27: Pulling from library/nginx"]);
        assert_eq!(u.layers.len(), 2);
        assert_eq!(u.layers[0].id, "a1b2c3d4e5f6", "orden de aparición");
        assert_eq!(
            (u.layers[0].current, u.layers[0].total),
            (Some(512), Some(2048))
        );
        assert!(t.take_update().is_none(), "sin cambios no se envía nada");
        t.push(Some("a1b2c3d4e5f6"), Some("Download complete"), None, None);
        t.push(
            Some("a1b2c3d4e5f6"),
            Some("Extracting"),
            Some(100),
            Some(4096),
        );
        t.push(Some("a1b2c3d4e5f6"), Some("Pull complete"), None, None);
        t.push(None, Some("Digest: sha256:abc"), None, None);
        t.push(
            None,
            Some("Status: Downloaded newer image for nginx:1.27"),
            None,
            None,
        );
        let u = t.take_update().unwrap();
        assert_eq!(u.layers[0].status, "Pull complete");
        assert_eq!(u.layers[0].current, Some(4096));
        assert_eq!(t.digest.as_deref(), Some("sha256:abc"));
        assert_eq!(
            t.final_status.as_deref(),
            Some("Downloaded newer image for nginx:1.27")
        );
        assert_eq!((t.downloaded(), t.existing()), (1, 1));
        assert_eq!(
            t.finished_summary("nginx:1.27", "sin credenciales"),
            "nginx:1.27: Downloaded newer image for nginx:1.27 (sha256:abc); 1 capas descargadas, 1 ya existían; sin credenciales"
        );
    }

    /// Almacén clásico con la imagen ya presente con otro nombre: Docker solo
    /// envía estos tres mensajes (comprobado con Docker 29 y overlay2).
    #[test]
    fn summary_without_layers_says_nothing_was_downloaded() {
        let mut t = PullTracker::default();
        t.push(Some("1"), Some("Pulling from cmc-test/pulled"), None, None);
        t.push(None, Some("Digest: sha256:a974"), None, None);
        t.push(
            None,
            Some("Status: Downloaded newer image for 127.0.0.1:5000/cmc-test/pulled:1"),
            None,
            None,
        );
        let u = t.take_update().unwrap();
        assert!(u.layers.is_empty());
        assert_eq!(u.messages.len(), 3);
        assert_eq!(
            t.finished_summary("127.0.0.1:5000/cmc-test/pulled:1", "sin credenciales"),
            "127.0.0.1:5000/cmc-test/pulled:1: Downloaded newer image for 127.0.0.1:5000/cmc-test/pulled:1 (sha256:a974); Docker ya tenía todo su contenido, no ha descargado capas; sin credenciales"
        );
    }

    #[test]
    fn request_matches_the_cli() {
        let (_, path) = pull_request(&ImageRef::parse("nginx").unwrap());
        assert_eq!(
            path,
            "/images/create?fromImage=docker.io%2Flibrary%2Fnginx&tag=latest&platform="
        );
        let (_, path) = pull_request(&ImageRef::parse("127.0.0.1:5000/app:v1").unwrap());
        assert_eq!(
            path,
            "/images/create?fromImage=127.0.0.1%3A5000%2Fapp&tag=v1&platform="
        );
    }
}
