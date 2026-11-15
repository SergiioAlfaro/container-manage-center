// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Contenedores: listado y acciones de ciclo de vida (Fase 2, ampliada en la Fase 3).

use std::collections::BTreeMap;

use bollard::query_parameters::{
    InspectContainerOptions, ListContainersOptions, RemoveContainerOptions,
    RestartContainerOptions, StopContainerOptions,
};
use dcc_activity::Category;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::client::{ApiOp, Engine};
use crate::error::EngineError;
use crate::resources::{path_with_query, validate_ref};

/// Segundos de espera antes de forzar la parada (valor por defecto de Docker).
pub const STOP_TIMEOUT_SECS: i32 = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PortMapping {
    pub ip: Option<String>,
    pub private_port: u16,
    pub public_port: Option<u16>,
    pub protocol: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContainerSummary {
    pub id: String,
    /// Nombre principal sin la barra inicial.
    pub name: String,
    pub image: String,
    pub image_id: String,
    pub command: String,
    /// Marca de tiempo Unix (segundos).
    #[ts(type = "number")]
    pub created: i64,
    /// created, running, paused, restarting, exited, removing, dead.
    pub state: String,
    /// Texto de estado de Docker, p. ej. "Up 3 minutes".
    pub status: String,
    pub ports: Vec<PortMapping>,
    pub networks: Vec<String>,
    pub mounts: u32,
    /// Montajes (volúmenes, carpetas del equipo, tmpfs…) tal como los da la lista.
    pub mount_points: Vec<MountPoint>,
    /// Proyecto Compose al que pertenece, si lo hay.
    pub compose_project: Option<String>,
    pub labels: BTreeMap<String, String>,
}

/// Un montaje de un contenedor (`Mounts` de `GET /containers/json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MountPoint {
    /// volume, bind, tmpfs, npipe, cluster, image.
    pub kind: String,
    /// Nombre del volumen (solo `volume`).
    pub name: Option<String>,
    /// Ruta en el equipo (carpeta vinculada o directorio del volumen).
    pub source: String,
    pub destination: String,
    pub rw: bool,
    pub mode: String,
    pub propagation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CreatedContainer {
    pub id: String,
    /// Avisos que devuelve Docker al crear.
    pub warnings: Vec<String>,
}

/// Estado confirmado tras una acción.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContainerState {
    pub id: String,
    pub name: String,
    /// created, running, paused, restarting, exited, removing, dead.
    pub status: String,
    #[ts(type = "number | null")]
    pub exit_code: Option<i64>,
}

/// Acción de ciclo de vida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ContainerAction {
    Start,
    Stop,
    Restart,
    Pause,
    Unpause,
}

impl ContainerAction {
    fn kind(self) -> &'static str {
        match self {
            Self::Start => "container.start",
            Self::Stop => "container.stop",
            Self::Restart => "container.restart",
            Self::Pause => "container.pause",
            Self::Unpause => "container.unpause",
        }
    }
}

fn state_str(v: Option<impl Serialize>) -> String {
    v.and_then(|s| serde_json::to_value(s).ok())
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

impl Engine {
    pub async fn list_containers(&self) -> Result<Vec<ContainerSummary>, EngineError> {
        let opts = ListContainersOptions {
            all: true,
            ..Default::default()
        };
        let op = ApiOp::get(
            Category::Container,
            "container.list",
            path_with_query("/containers/json", &opts),
        )
        .equivalent(["ps", "--all"]);
        let d = self.docker().clone();
        let list = self
            .call(
                op,
                async move { d.list_containers(Some(opts)).await },
                |v| Some(format!("{} contenedores", v.len())),
            )
            .await?;
        Ok(summaries(list))
    }

    /// Lista de contenedores sin registrarla (muestreo de la monitorización,
    /// D-102, y enlaces entre herramientas, D-108): son lecturas repetidas que
    /// llenarían el panel de actividad.
    pub async fn list_containers_silent(&self) -> Result<Vec<ContainerSummary>, EngineError> {
        let opts = ListContainersOptions {
            all: true,
            ..Default::default()
        };
        let d = self.docker().clone();
        let list = self
            .call_silent(Some(std::time::Duration::from_secs(15)), async move {
                d.list_containers(Some(opts)).await
            })
            .await?;
        Ok(summaries(list))
    }
}

fn summaries(list: Vec<bollard::models::ContainerSummary>) -> Vec<ContainerSummary> {
    {
        list.into_iter()
            .map(|c| {
                let labels: BTreeMap<String, String> =
                    c.labels.unwrap_or_default().into_iter().collect();
                ContainerSummary {
                    id: c.id.unwrap_or_default(),
                    name: c
                        .names
                        .unwrap_or_default()
                        .first()
                        .map(|n| n.trim_start_matches('/').to_owned())
                        .unwrap_or_default(),
                    image: c.image.unwrap_or_default(),
                    image_id: c.image_id.unwrap_or_default(),
                    command: c.command.unwrap_or_default(),
                    created: c.created.unwrap_or(0),
                    state: state_str(c.state),
                    status: c.status.unwrap_or_default(),
                    ports: c
                        .ports
                        .unwrap_or_default()
                        .into_iter()
                        .map(|p| PortMapping {
                            ip: p.ip,
                            private_port: p.private_port,
                            public_port: p.public_port,
                            protocol: state_str(p.typ),
                        })
                        .collect(),
                    networks: c
                        .network_settings
                        .and_then(|n| n.networks)
                        .map(|n| n.into_keys().collect())
                        .unwrap_or_default(),
                    mounts: c.mounts.as_ref().map(|m| m.len() as u32).unwrap_or(0),
                    mount_points: c
                        .mounts
                        .unwrap_or_default()
                        .into_iter()
                        .map(|m| MountPoint {
                            kind: state_str(m.typ),
                            name: m.name.filter(|n| !n.is_empty()),
                            source: m.source.unwrap_or_default(),
                            destination: m.destination.unwrap_or_default(),
                            rw: m.rw.unwrap_or(true),
                            mode: m.mode.unwrap_or_default(),
                            propagation: m.propagation.unwrap_or_default(),
                        })
                        .collect(),
                    compose_project: labels.get("com.docker.compose.project").cloned(),
                    labels,
                }
            })
            .collect()
    }
}

impl Engine {
    /// Ejecuta una acción de ciclo de vida. `display` es el nombre para el registro.
    /// Ejecuta una acción de ciclo de vida y devuelve el estado **confirmado**.
    ///
    /// Tras la acción se consulta `GET /containers/{id}/json` (inspect). Motivo
    /// (D-031): el daemon responde a `POST /stop` en cuanto el contenedor se
    /// detiene, pero actualiza la réplica que sirve `GET /containers/json`
    /// un poco después (`SetStopped` antes de `CheckpointTo` en
    /// `handleContainerExit`). Inspect toma el bloqueo del contenedor, así que
    /// devuelve el estado real y, cuando responde, el listado ya es coherente.
    pub async fn container_action(
        &self,
        id: &str,
        display: Option<&str>,
        action: ContainerAction,
    ) -> Result<ContainerState, EngineError> {
        validate_ref("contenedor", id)?;
        let _lock = self.locks().acquire(&format!("container:{id}"))?;
        self.run_action(id, display, action).await?;
        self.container_state(id, display).await
    }

    pub(crate) async fn run_action(
        &self,
        id: &str,
        display: Option<&str>,
        action: ContainerAction,
    ) -> Result<(), EngineError> {
        let target = display.unwrap_or(id).to_owned();
        let d = self.docker().clone();
        let id_s = id.to_owned();
        let t = STOP_TIMEOUT_SECS.to_string();
        let post = |path: String, eq: Vec<String>| ApiOp {
            category: Category::Container,
            kind: action.kind(),
            target: Some(target.clone()),
            method: "POST",
            path,
            body: None,
            equivalent: Some(eq),
            timeout: None,
        };
        let ok = |s: &'static str| move |_: &()| Some(s.to_owned());
        match action {
            ContainerAction::Start => {
                let op = post(
                    format!("/containers/{id}/start"),
                    vec!["start".into(), id.into()],
                );
                self.call(
                    op,
                    async move { d.start_container(&id_s, None).await },
                    ok("Contenedor iniciado"),
                )
                .await
            }
            ContainerAction::Stop => {
                let opts = StopContainerOptions {
                    t: Some(STOP_TIMEOUT_SECS),
                    signal: None,
                };
                let op = post(
                    path_with_query(&format!("/containers/{id}/stop"), &opts),
                    vec!["stop".into(), "--time".into(), t, id.into()],
                );
                self.call(
                    op,
                    async move { d.stop_container(&id_s, Some(opts)).await },
                    ok("Contenedor detenido"),
                )
                .await
            }
            ContainerAction::Restart => {
                let opts = RestartContainerOptions {
                    t: Some(STOP_TIMEOUT_SECS),
                    signal: None,
                };
                let op = post(
                    path_with_query(&format!("/containers/{id}/restart"), &opts),
                    vec!["restart".into(), "--time".into(), t, id.into()],
                );
                self.call(
                    op,
                    async move { d.restart_container(&id_s, Some(opts)).await },
                    ok("Contenedor reiniciado"),
                )
                .await
            }
            ContainerAction::Pause => {
                let op = post(
                    format!("/containers/{id}/pause"),
                    vec!["pause".into(), id.into()],
                );
                self.call(
                    op,
                    async move { d.pause_container(&id_s).await },
                    ok("Contenedor pausado"),
                )
                .await
            }
            ContainerAction::Unpause => {
                let op = post(
                    format!("/containers/{id}/unpause"),
                    vec!["unpause".into(), id.into()],
                );
                self.call(
                    op,
                    async move { d.unpause_container(&id_s).await },
                    ok("Contenedor reanudado"),
                )
                .await
            }
        }
    }

    /// Estado actual de un contenedor según inspect (fuente consistente).
    pub async fn container_state(
        &self,
        id: &str,
        display: Option<&str>,
    ) -> Result<ContainerState, EngineError> {
        validate_ref("contenedor", id)?;
        let opts = InspectContainerOptions { size: false };
        let op = ApiOp {
            category: Category::Container,
            kind: "container.state",
            target: Some(display.unwrap_or(id).to_owned()),
            method: "GET",
            path: path_with_query(&format!("/containers/{id}/json"), &opts),
            body: None,
            equivalent: Some(vec![
                "container".into(),
                "inspect".into(),
                "--format".into(),
                "{{.State.Status}}".into(),
                id.into(),
            ]),
            timeout: None,
        };
        let d = self.docker().clone();
        let id_s = id.to_owned();
        let r = self
            .call(
                op,
                async move { d.inspect_container(&id_s, Some(opts)).await },
                |r| {
                    r.state
                        .as_ref()
                        .map(|s| format!("Estado: {}", state_str(s.status.as_ref())))
                },
            )
            .await?;
        let st = r.state.unwrap_or_default();
        Ok(ContainerState {
            id: r.id.unwrap_or_else(|| id.to_owned()),
            name: r
                .name
                .unwrap_or_default()
                .trim_start_matches('/')
                .to_owned(),
            status: state_str(st.status.as_ref()),
            exit_code: st.exit_code,
        })
    }

    /// `GET /containers/{id}/json` completo para la pantalla de detalle.
    pub async fn inspect_raw(
        &self,
        id: &str,
        display: Option<&str>,
    ) -> Result<bollard::models::ContainerInspectResponse, EngineError> {
        validate_ref("contenedor", id)?;
        let opts = InspectContainerOptions { size: false };
        let op = ApiOp {
            category: Category::Container,
            kind: "container.inspect",
            target: Some(display.unwrap_or(id).to_owned()),
            method: "GET",
            path: path_with_query(&format!("/containers/{id}/json"), &opts),
            body: None,
            equivalent: Some(vec!["container".into(), "inspect".into(), id.into()]),
            timeout: None,
        };
        let d = self.docker().clone();
        let id_s = id.to_owned();
        self.call(
            op,
            async move { d.inspect_container(&id_s, Some(opts)).await },
            |r| {
                r.state
                    .as_ref()
                    .map(|s| format!("Estado: {}", state_str(s.status.as_ref())))
            },
        )
        .await
    }

    pub async fn inspect_container(
        &self,
        id: &str,
        display: Option<&str>,
    ) -> Result<crate::details::ContainerDetails, EngineError> {
        Ok(crate::details::details_from(
            &self.inspect_raw(id, display).await?,
        ))
    }

    /// Crea un contenedor a partir de la especificación del asistente (D-034).
    /// Se valida de nuevo en el backend, comprobando el disco para los bind mounts.
    pub async fn create_container(
        &self,
        spec: &crate::spec::ContainerSpec,
    ) -> Result<CreatedContainer, EngineError> {
        // Las carpetas se comprueban donde está Docker (0.10.0).
        let issues = self.spec_issues(spec).await?;
        if crate::spec::ContainerSpec::has_errors(&issues) {
            return Err(EngineError::InvalidSpec { issues });
        }
        // Los riesgos altos o críticos exigen aceptación explícita; se
        // recalculan aquí para que la interfaz no pueda saltarse el aviso (D-086).
        let risks = spec.risks(self.ssh_session().is_none());
        let missing: Vec<_> = crate::risk::unaccepted(&risks, &spec.accepted_risks)
            .into_iter()
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Err(EngineError::RisksNotAccepted { risks: missing });
        }
        let (name, body) = spec.to_create_body();
        // Lo que queda en el registro de actividad no incluye los secretos.
        let (shown, _) = spec.redacted();
        let shown_body = shown.to_create_body().1;
        let opts = bollard::query_parameters::CreateContainerOptions {
            name: name.clone(),
            platform: String::new(),
        };
        let op = ApiOp {
            category: Category::Container,
            kind: "container.create",
            target: Some(name.clone().unwrap_or_else(|| spec.image.trim().to_owned())),
            method: "POST",
            path: path_with_query("/containers/create", &opts),
            body: Some(serde_json::to_string_pretty(&shown_body).unwrap_or_default()),
            equivalent: Some(shown.cli_args(false)),
            timeout: None,
        };
        let d = self.docker().clone();
        let r = self
            .call(
                op,
                async move { d.create_container(Some(opts), body).await },
                |r| {
                    Some(format!(
                        "Contenedor creado: {}",
                        r.id.chars().take(12).collect::<String>()
                    ))
                },
            )
            .await?;
        Ok(CreatedContainer {
            id: r.id,
            warnings: r.warnings,
        })
    }

    /// Lee la configuración de un contenedor para duplicarlo: inspect del
    /// contenedor y de su imagen (para descartar los valores heredados).
    pub async fn duplicate_spec(
        &self,
        id: &str,
        display: Option<&str>,
    ) -> Result<crate::spec::DuplicateSpec, EngineError> {
        let c = self.inspect_raw(id, display).await?;
        let image_id = c.image.clone().unwrap_or_default();
        let image = match self.inspect_image_config(&image_id).await {
            Ok(cfg) => cfg,
            Err(EngineError::Docker { status: 404, .. }) => None,
            Err(e) => return Err(e),
        };
        Ok(crate::spec::from_inspect(&c, image.as_ref()))
    }

    /// Elimina un contenedor. `force` detiene antes uno en ejecución; `volumes`
    /// borra además sus volúmenes anónimos.
    pub async fn remove_container(
        &self,
        id: &str,
        display: Option<&str>,
        force: bool,
        volumes: bool,
    ) -> Result<(), EngineError> {
        validate_ref("contenedor", id)?;
        let _lock = self.locks().acquire(&format!("container:{id}"))?;
        let opts = RemoveContainerOptions {
            v: volumes,
            force,
            link: false,
        };
        let mut eq = vec!["rm".to_owned()];
        if force {
            eq.push("--force".into());
        }
        if volumes {
            eq.push("--volumes".into());
        }
        eq.push(id.into());
        let op = ApiOp {
            category: Category::Container,
            kind: "container.remove",
            target: Some(display.unwrap_or(id).to_owned()),
            method: "DELETE",
            path: path_with_query(&format!("/containers/{id}"), &opts),
            body: None,
            equivalent: Some(eq),
            timeout: None,
        };
        let d = self.docker().clone();
        let id_s = id.to_owned();
        self.call(
            op,
            async move { d.remove_container(&id_s, Some(opts)).await },
            |_| Some("Contenedor eliminado".into()),
        )
        .await
    }
}
