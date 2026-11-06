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

// ------------------------------------------------------------------ Fase 4

/// Imagen inspeccionada: lo que muestra el detalle de la interfaz.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImageDetails {
    pub id: String,
    pub repo_tags: Vec<String>,
    pub repo_digests: Vec<String>,
    pub created: Option<String>,
    #[ts(type = "number")]
    pub size: i64,
    pub architecture: String,
    pub os: String,
    pub variant: Option<String>,
    pub author: String,
    pub comment: String,
    pub command: Vec<String>,
    pub entrypoint: Vec<String>,
    pub env: Vec<crate::spec::KeyValue>,
    pub exposed_ports: Vec<String>,
    pub volumes: Vec<String>,
    pub working_dir: String,
    pub user: String,
    pub labels: BTreeMap<String, String>,
    pub stop_signal: Option<String>,
    pub healthcheck: Option<Vec<String>>,
    /// Número de capas del sistema de archivos.
    pub layers: usize,
    /// JSON completo devuelto por Docker.
    pub raw: String,
}

/// Una entrada de `docker image history`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImageLayer {
    /// ID de la imagen intermedia o `None` si Docker la marca como `<missing>`.
    pub id: Option<String>,
    #[ts(type = "number")]
    pub created: i64,
    pub created_by: String,
    #[ts(type = "number")]
    pub size: i64,
    pub comment: String,
    pub tags: Vec<String>,
    /// Instrucción sin contenido en el sistema de archivos (ENV, CMD, LABEL…).
    pub empty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RemoveResult {
    pub untagged: Vec<String>,
    pub deleted: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PruneResult {
    pub untagged: Vec<String>,
    pub deleted: Vec<String>,
    #[ts(type = "number")]
    pub space_reclaimed: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchResult {
    pub name: String,
    pub description: String,
    #[ts(type = "number")]
    pub stars: i64,
    pub official: bool,
}

/// Referencia a una imagen local: nombre (`repo:tag`, `repo@digest`) o ID.
pub fn validate_image_target(r: &str) -> Result<(), EngineError> {
    let ok = crate::spec::valid_image_ref(r)
        || (r.starts_with("sha256:")
            && r.len() > 7
            && r[7..].chars().all(|c| c.is_ascii_hexdigit()));
    if ok {
        Ok(())
    } else {
        Err(EngineError::InvalidInput {
            message: format!("referencia de imagen no válida: {r:?}"),
        })
    }
}

fn human(b: i64) -> String {
    let b = b.max(0) as f64;
    if b >= (1u64 << 30) as f64 {
        format!("{:.2} GiB", b / (1u64 << 30) as f64)
    } else if b >= (1u64 << 20) as f64 {
        format!("{:.1} MiB", b / (1u64 << 20) as f64)
    } else if b >= 1024.0 {
        format!("{:.1} KiB", b / 1024.0)
    } else {
        format!("{b} B")
    }
}

impl Engine {
    pub async fn image_details(&self, reference: &str) -> Result<ImageDetails, EngineError> {
        let i = self.inspect_image_full(reference).await?;
        let raw = serde_json::to_string_pretty(&i).unwrap_or_default();
        let c = i.config.clone().unwrap_or_default();
        let mut ports = c.exposed_ports.clone().unwrap_or_default();
        ports.sort();
        let mut volumes = c.volumes.clone().unwrap_or_default();
        volumes.sort();
        Ok(ImageDetails {
            id: i.id.clone().unwrap_or_default(),
            repo_tags: i.repo_tags.clone().unwrap_or_default(),
            repo_digests: i.repo_digests.clone().unwrap_or_default(),
            created: i.created.clone(),
            size: i.size.unwrap_or(0),
            architecture: i.architecture.clone().unwrap_or_default(),
            os: i.os.clone().unwrap_or_default(),
            variant: i.variant.clone().filter(|v| !v.is_empty()),
            author: i.author.clone().unwrap_or_default(),
            comment: i.comment.clone().unwrap_or_default(),
            command: c.cmd.clone().unwrap_or_default(),
            entrypoint: c.entrypoint.clone().unwrap_or_default(),
            env: c
                .env
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(|e| match e.split_once('=') {
                    Some((k, v)) => crate::spec::KeyValue::new(k, v),
                    None => crate::spec::KeyValue::new(e.clone(), ""),
                })
                .collect(),
            exposed_ports: ports,
            volumes,
            working_dir: c.working_dir.clone().unwrap_or_default(),
            user: c.user.clone().unwrap_or_default(),
            labels: c.labels.clone().unwrap_or_default().into_iter().collect(),
            stop_signal: c.stop_signal.clone().filter(|s| !s.is_empty()),
            healthcheck: c.healthcheck.as_ref().and_then(|h| h.test.clone()),
            layers: i
                .root_fs
                .as_ref()
                .and_then(|r| r.layers.as_ref())
                .map_or(0, |l| l.len()),
            raw,
        })
    }

    /// Capas de la imagen (`docker image history`).
    pub async fn image_history(&self, reference: &str) -> Result<Vec<ImageLayer>, EngineError> {
        let r = reference.trim();
        validate_image_target(r)?;
        let op = ApiOp {
            category: Category::Image,
            kind: "image.history",
            target: Some(r.to_owned()),
            method: "GET",
            path: format!("/images/{r}/history"),
            body: None,
            equivalent: Some(vec![
                "image".into(),
                "history".into(),
                "--no-trunc".into(),
                r.into(),
            ]),
            timeout: None,
        };
        let d = self.docker().clone();
        let name = r.to_owned();
        let items = self
            .call(op, async move { d.image_history(&name).await }, |v| {
                Some(format!("{} capas", v.len()))
            })
            .await?;
        Ok(items
            .into_iter()
            .map(|h| ImageLayer {
                id: (h.id != "<missing>" && !h.id.is_empty()).then_some(h.id),
                created: h.created,
                // Docker antepone "/bin/sh -c #(nop) " a las instrucciones sin efecto.
                empty: h.size == 0 && h.created_by.contains("#(nop)"),
                created_by: h.created_by,
                size: h.size,
                comment: h.comment,
                tags: h
                    .tags
                    .into_iter()
                    .filter(|t| t != "<none>:<none>")
                    .collect(),
            })
            .collect())
    }

    /// `docker tag SRC REPO:TAG`.
    pub async fn tag_image(
        &self,
        source: &str,
        repo: &str,
        tag: &str,
    ) -> Result<String, EngineError> {
        let s = source.trim();
        validate_image_target(s)?;
        let target =
            crate::reference::validate_tag_target(repo.trim(), tag.trim()).map_err(|e| {
                EngineError::InvalidInput {
                    message: format!("destino no válido: {e}"),
                }
            })?;
        // Se envía el repositorio tal como lo escribió el usuario (forma corta),
        // como hace el CLI.
        let opts = bollard::query_parameters::TagImageOptions {
            repo: Some(repo.trim().to_owned()),
            tag: Some(tag.trim().to_owned()),
        };
        let full = format!("{}:{}", repo.trim(), tag.trim());
        let op = ApiOp {
            category: Category::Image,
            kind: "image.tag",
            target: Some(s.to_owned()),
            method: "POST",
            path: path_with_query(&format!("/images/{s}/tag"), &opts),
            body: None,
            equivalent: Some(vec!["tag".into(), s.into(), full.clone()]),
            timeout: None,
        };
        let d = self.docker().clone();
        let name = s.to_owned();
        let summary = format!("Etiquetada como {}", target.familiar());
        self.call(
            op,
            async move { d.tag_image(&name, Some(opts)).await },
            move |_| Some(summary),
        )
        .await?;
        Ok(full)
    }

    /// `docker image rm [--force] REF`. Con un nombre solo quita esa etiqueta si
    /// la imagen tiene otras; con el ID elimina la imagen (requiere `force` si
    /// tiene varias etiquetas o la usan contenedores detenidos).
    pub async fn remove_image(
        &self,
        reference: &str,
        force: bool,
        confirmed: bool,
    ) -> Result<RemoveResult, EngineError> {
        if !confirmed {
            return Err(EngineError::NotConfirmed);
        }
        let r = reference.trim();
        validate_image_target(r)?;
        let _guard = self.locks().acquire(&format!("image:{r}"))?;
        let opts = bollard::query_parameters::RemoveImageOptions {
            force,
            noprune: false,
            platforms: None,
        };
        let mut eq = vec!["image".to_owned(), "rm".to_owned()];
        if force {
            eq.push("--force".into());
        }
        eq.push(r.into());
        let op = ApiOp {
            category: Category::Image,
            kind: "image.remove",
            target: Some(r.to_owned()),
            method: "DELETE",
            path: path_with_query(&format!("/images/{r}"), &opts),
            body: None,
            equivalent: Some(eq),
            timeout: None,
        };
        let d = self.docker().clone();
        let name = r.to_owned();
        let items = self
            .call(
                op,
                async move { d.remove_image(&name, Some(opts), None).await },
                |v| {
                    let u = v.iter().filter(|i| i.untagged.is_some()).count();
                    let del = v.iter().filter(|i| i.deleted.is_some()).count();
                    Some(format!(
                        "{u} etiquetas quitadas, {del} capas o imágenes eliminadas"
                    ))
                },
            )
            .await?;
        Ok(RemoveResult {
            untagged: items.iter().filter_map(|i| i.untagged.clone()).collect(),
            deleted: items.iter().filter_map(|i| i.deleted.clone()).collect(),
        })
    }

    /// `docker image prune --force [--all]`: sin `all`, solo las imágenes sin
    /// etiqueta que no usa ningún contenedor; con `all`, todas las que no usa
    /// ningún contenedor.
    pub async fn prune_images(
        &self,
        all: bool,
        confirmed: bool,
    ) -> Result<PruneResult, EngineError> {
        self.prune_images_filtered(all, None, confirmed).await
    }

    /// Como [`Engine::prune_images`], limitado opcionalmente a una etiqueta
    /// (`clave=valor`); las pruebas lo usan para no tocar imágenes ajenas.
    pub async fn prune_images_filtered(
        &self,
        all: bool,
        label: Option<&str>,
        confirmed: bool,
    ) -> Result<PruneResult, EngineError> {
        if !confirmed {
            return Err(EngineError::NotConfirmed);
        }
        let mut filters = HashMap::new();
        filters.insert("dangling".to_owned(), vec![(!all).to_string()]);
        let mut eq = vec!["image".to_owned(), "prune".to_owned(), "--force".to_owned()];
        if all {
            eq.push("--all".into());
        }
        if let Some(l) = label {
            if l.is_empty() || l.chars().any(|c| c.is_whitespace() || c.is_control()) {
                return Err(EngineError::InvalidInput {
                    message: format!("filtro de etiqueta no válido: {l:?}"),
                });
            }
            filters.insert("label".to_owned(), vec![l.to_owned()]);
            eq.push("--filter".into());
            eq.push(format!("label={l}"));
        }
        let opts = bollard::query_parameters::PruneImagesOptions {
            filters: Some(filters),
        };
        let op = ApiOp {
            category: Category::Image,
            kind: "image.prune",
            target: None,
            method: "POST",
            path: path_with_query("/images/prune", &opts),
            body: None,
            equivalent: Some(eq),
            timeout: Some(std::time::Duration::from_secs(600)),
        };
        let d = self.docker().clone();
        let r = self
            .call(op, async move { d.prune_images(Some(opts)).await }, |r| {
                Some(format!(
                    "{} elementos eliminados; espacio recuperado: {}",
                    r.images_deleted.as_ref().map_or(0, |v| v.len()),
                    human(r.space_reclaimed.unwrap_or(0))
                ))
            })
            .await?;
        let items = r.images_deleted.unwrap_or_default();
        Ok(PruneResult {
            untagged: items.iter().filter_map(|i| i.untagged.clone()).collect(),
            deleted: items.iter().filter_map(|i| i.deleted.clone()).collect(),
            space_reclaimed: r.space_reclaimed.unwrap_or(0),
        })
    }

    /// `docker search`: busca en Docker Hub a través del daemon (necesita que el
    /// daemon tenga acceso a Internet).
    pub async fn search_images(
        &self,
        term: &str,
        limit: u32,
        official_only: bool,
    ) -> Result<Vec<SearchResult>, EngineError> {
        let t = term.trim();
        if t.is_empty() || t.len() > 255 || t.chars().any(|c| c.is_control()) {
            return Err(EngineError::InvalidInput {
                message: "término de búsqueda no válido".into(),
            });
        }
        let limit = limit.clamp(1, 100);
        let filters = official_only.then(|| {
            let mut f = HashMap::new();
            f.insert("is-official".to_owned(), vec!["true".to_owned()]);
            f
        });
        let opts = bollard::query_parameters::SearchImagesOptions {
            term: t.to_owned(),
            limit: Some(limit as i32),
            filters,
        };
        let mut eq = vec!["search".to_owned(), "--limit".to_owned(), limit.to_string()];
        if official_only {
            eq.push("--filter".into());
            eq.push("is-official=true".into());
        }
        eq.push(t.into());
        let op = ApiOp {
            category: Category::Image,
            kind: "image.search",
            target: Some(t.to_owned()),
            method: "GET",
            path: path_with_query("/images/search", &opts),
            body: None,
            equivalent: Some(eq),
            timeout: Some(std::time::Duration::from_secs(60)),
        };
        let d = self.docker().clone();
        let items = self
            .call(op, async move { d.search_images(opts).await }, |v| {
                Some(format!("{} resultados", v.len()))
            })
            .await?;
        Ok(items
            .into_iter()
            .map(|i| SearchResult {
                name: i.name.unwrap_or_default(),
                description: i.description.unwrap_or_default(),
                stars: i.star_count.unwrap_or(0),
                official: i.is_official.unwrap_or(false),
            })
            .collect())
    }
}
