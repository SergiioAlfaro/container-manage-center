//! Datos de apoyo para el asistente de creación: valores por defecto de una
//! imagen y listados breves de redes y volúmenes. Todo son peticiones reales
//! registradas.

use bollard::models::ImageConfig;
use bollard::query_parameters::{ListNetworksOptions, ListVolumesOptions};
use dcc_activity::Category;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::client::{ApiOp, Engine};
use crate::error::EngineError;
use crate::spec::valid_image_ref;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImageDefaults {
    pub id: String,
    pub command: Vec<String>,
    pub entrypoint: Vec<String>,
    pub env: Vec<String>,
    /// p. ej. `80/tcp`.
    pub exposed_ports: Vec<String>,
    pub working_dir: String,
    pub user: String,
    pub volumes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NetworkBrief {
    pub name: String,
    pub driver: String,
    pub scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VolumeBrief {
    pub name: String,
    pub driver: String,
}

impl Engine {
    /// `GET /images/{ref}/json`. Devuelve `Ok(None)` nunca: un 404 se propaga
    /// como error de Docker para que el llamador decida.
    pub(crate) async fn inspect_image_config(
        &self,
        reference: &str,
    ) -> Result<Option<ImageConfig>, EngineError> {
        Ok(self.inspect_image_full(reference).await?.config)
    }

    async fn inspect_image_full(
        &self,
        reference: &str,
    ) -> Result<bollard::models::ImageInspect, EngineError> {
        let r = reference.trim();
        if !valid_image_ref(r)
            && !(r.starts_with("sha256:") && r[7..].chars().all(|c| c.is_ascii_hexdigit()))
        {
            return Err(EngineError::InvalidInput {
                message: format!("referencia de imagen no válida: {r:?}"),
            });
        }
        let op = ApiOp {
            category: Category::Image,
            kind: "image.inspect",
            target: Some(r.to_owned()),
            method: "GET",
            path: format!("/images/{r}/json"),
            body: None,
            equivalent: Some(vec!["image".into(), "inspect".into(), r.into()]),
            timeout: None,
        };
        let d = self.docker().clone();
        let name = r.to_owned();
        self.call(op, async move { d.inspect_image(&name).await }, |_| None)
            .await
    }

    pub async fn image_defaults(&self, reference: &str) -> Result<ImageDefaults, EngineError> {
        let i = self.inspect_image_full(reference).await?;
        let c = i.config.unwrap_or_default();
        Ok(ImageDefaults {
            id: i.id.unwrap_or_default(),
            command: c.cmd.unwrap_or_default(),
            entrypoint: c.entrypoint.unwrap_or_default(),
            env: c.env.unwrap_or_default(),
            exposed_ports: {
                let mut p = c.exposed_ports.unwrap_or_default();
                p.sort();
                p
            },
            working_dir: c.working_dir.unwrap_or_default(),
            user: c.user.unwrap_or_default(),
            volumes: c.volumes.unwrap_or_default(),
        })
    }

    pub async fn networks_brief(&self) -> Result<Vec<NetworkBrief>, EngineError> {
        let op = ApiOp::get(Category::Network, "network.list", "/networks")
            .equivalent(["network", "ls"]);
        let d = self.docker().clone();
        let list = self
            .call(
                op,
                async move { d.list_networks(None::<ListNetworksOptions>).await },
                |v| Some(format!("{} redes", v.len())),
            )
            .await?;
        let mut v: Vec<NetworkBrief> = list
            .into_iter()
            .map(|n| NetworkBrief {
                name: n.name.unwrap_or_default(),
                driver: n.driver.unwrap_or_default(),
                scope: n.scope.unwrap_or_default(),
            })
            .collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(v)
    }

    pub async fn volumes_brief(&self) -> Result<Vec<VolumeBrief>, EngineError> {
        let op =
            ApiOp::get(Category::Volume, "volume.list", "/volumes").equivalent(["volume", "ls"]);
        let d = self.docker().clone();
        let r = self
            .call(
                op,
                async move { d.list_volumes(None::<ListVolumesOptions>).await },
                |r| {
                    Some(format!(
                        "{} volúmenes",
                        r.volumes.as_ref().map_or(0, |v| v.len())
                    ))
                },
            )
            .await?;
        let mut v: Vec<VolumeBrief> = r
            .volumes
            .unwrap_or_default()
            .into_iter()
            .map(|v| VolumeBrief {
                name: v.name,
                driver: v.driver,
            })
            .collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(v)
    }
}
