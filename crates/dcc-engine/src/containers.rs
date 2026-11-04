//! Contenedores: listado y acciones de ciclo de vida (Fase 2, ampliada en la Fase 3).

use std::collections::BTreeMap;

use bollard::query_parameters::{
    ListContainersOptions, RemoveContainerOptions, RestartContainerOptions, StopContainerOptions,
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
    /// Proyecto Compose al que pertenece, si lo hay.
    pub compose_project: Option<String>,
    pub labels: BTreeMap<String, String>,
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
        Ok(list
            .into_iter()
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
                    mounts: c.mounts.map(|m| m.len() as u32).unwrap_or(0),
                    compose_project: labels.get("com.docker.compose.project").cloned(),
                    labels,
                }
            })
            .collect())
    }

    /// Ejecuta una acción de ciclo de vida. `display` es el nombre para el registro.
    pub async fn container_action(
        &self,
        id: &str,
        display: Option<&str>,
        action: ContainerAction,
    ) -> Result<(), EngineError> {
        validate_ref("contenedor", id)?;
        let _lock = self.locks().acquire(&format!("container:{id}"))?;
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
