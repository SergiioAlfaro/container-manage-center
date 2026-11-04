//! Imágenes: listado con uso por contenedores (Fase 2; el resto llega en la Fase 4).

use std::collections::{BTreeMap, HashMap};

use bollard::query_parameters::ListImagesOptions;
use dcc_activity::Category;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::client::{ApiOp, Engine};
use crate::containers::ContainerSummary;
use crate::error::EngineError;
use crate::resources::path_with_query;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImageSummary {
    pub id: String,
    pub repo_tags: Vec<String>,
    pub repo_digests: Vec<String>,
    #[ts(type = "number")]
    pub created: i64,
    #[ts(type = "number")]
    pub size: i64,
    /// Sin etiqueta (`<none>:<none>`).
    pub dangling: bool,
    pub labels: BTreeMap<String, String>,
    /// Contenedores (de cualquier estado) creados a partir de esta imagen.
    pub used_by: Vec<ImageUser>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImageUser {
    pub id: String,
    pub name: String,
    pub state: String,
}

impl Engine {
    /// Lista imágenes y las cruza con la lista de contenedores para indicar su uso.
    /// Son dos peticiones reales a Docker y ambas quedan registradas.
    pub async fn list_images(
        &self,
        containers: Option<&[ContainerSummary]>,
    ) -> Result<Vec<ImageSummary>, EngineError> {
        let opts = ListImagesOptions::default();
        let op = ApiOp::get(
            Category::Image,
            "image.list",
            path_with_query("/images/json", &opts),
        )
        .equivalent(["image", "ls"]);
        let d = self.docker().clone();
        let images = self
            .call(op, async move { d.list_images(Some(opts)).await }, |v| {
                Some(format!("{} imágenes", v.len()))
            })
            .await?;

        let owned;
        let containers = match containers {
            Some(c) => c,
            None => {
                owned = self.list_containers().await?;
                &owned
            }
        };
        let mut users: HashMap<&str, Vec<ImageUser>> = HashMap::new();
        for c in containers {
            users
                .entry(c.image_id.as_str())
                .or_default()
                .push(ImageUser {
                    id: c.id.clone(),
                    name: c.name.clone(),
                    state: c.state.clone(),
                });
        }
        Ok(images
            .into_iter()
            .map(|i| {
                let tags: Vec<String> = i
                    .repo_tags
                    .into_iter()
                    .filter(|t| t != "<none>:<none>")
                    .collect();
                ImageSummary {
                    used_by: users.remove(i.id.as_str()).unwrap_or_default(),
                    dangling: tags.is_empty(),
                    id: i.id,
                    repo_tags: tags,
                    repo_digests: i
                        .repo_digests
                        .into_iter()
                        .filter(|d| d != "<none>@<none>")
                        .collect(),
                    created: i.created,
                    size: i.size,
                    labels: i.labels.into_iter().collect(),
                }
            })
            .collect())
    }
}
