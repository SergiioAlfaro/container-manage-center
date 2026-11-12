//! Edición de contenedores contra Docker (0.9.2): cambios directos
//! (`rename`, `update`), recreación segura con copia de seguridad y vuelta
//! atrás (D-070), restauración de la copia y creación de una imagen a partir
//! de un contenedor (`commit`, D-069).
//!
//! Garantías de la recreación:
//! - el original nunca se elimina: se detiene, se renombra y queda como copia;
//! - no se elimina ningún volumen, carpeta ni imagen;
//! - si el contenedor nuevo no se crea, no se conecta o no se mantiene en
//!   ejecución, se elimina el nuevo (sin volúmenes) y el original vuelve a su
//!   nombre y a su estado.

use std::time::Duration;

use bollard::models::{
    ContainerCreateBody, ContainerInspectResponse, EndpointSettings, ImageConfig,
    NetworkConnectRequest, NetworkDisconnectRequest,
};
use bollard::query_parameters::{
    CommitContainerOptions, CreateContainerOptions, InspectContainerOptions,
    RemoveContainerOptions, RenameContainerOptions,
};
use dcc_activity::{Category, Failure, OperationSpec, Outcome, Transport};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::client::{ApiOp, Engine};
use crate::containers::{ContainerAction, ContainerSummary};
use crate::edit::{
    self, BackupRef, Change, ChangeKind, EditOutcome, EditPlan, EditRequest, EditStep,
    EditableConfig, Risk, REPLACED_INFIX,
};
use crate::error::EngineError;
use crate::resources::{path_with_query, validate_ref};
use crate::spec::{issue, ContainerSpec, Severity, SpecIssue};

/// Tiempo durante el que el contenedor nuevo debe mantenerse en ejecución.
pub const VERIFY_WINDOW: Duration = Duration::from_secs(3);
const VERIFY_STEP: Duration = Duration::from_millis(500);

/// Confirmaciones del usuario al aplicar.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct ApplyOptions {
    /// El usuario ha revisado el resumen y confirma la recreación.
    pub confirm_recreate: bool,
    /// Códigos de los riesgos aceptados expresamente.
    pub accepted_risks: Vec<String>,
    /// Conservar en el contenedor nuevo las direcciones MAC que tiene ahora
    /// (para quien fijó una con `--mac-address`).
    pub keep_mac: bool,
    /// Nombre de la copia de seguridad que mostró la revisión. Se usa si sigue
    /// siendo una copia de este contenedor y está libre: así la copia que se
    /// crea es la que el usuario vio, aunque confirme segundos después.
    pub backup_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct CommitRequest {
    pub repository: String,
    /// Vacía = `latest`.
    pub tag: String,
    pub author: String,
    pub message: String,
    /// Pausar el contenedor mientras se crea la imagen (como `docker commit`).
    pub pause: bool,
    /// El nombre ya existe y el usuario acepta que pase a la imagen nueva.
    pub replace_existing: bool,
}

/// Montaje cuyo contenido no entra en la imagen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExcludedMount {
    /// volume, bind, tmpfs…
    pub kind: String,
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommitPreview {
    pub container_name: String,
    pub status: String,
    /// Nombre completo de la imagen (`repositorio:etiqueta`), si es válido.
    pub reference: Option<String>,
    pub issues: Vec<SpecIssue>,
    /// ID de la imagen que ya tiene ese nombre.
    pub existing_image_id: Option<String>,
    pub excluded_mounts: Vec<ExcludedMount>,
    /// Orden `docker commit` equivalente.
    pub equivalent: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommitResult {
    pub image_id: String,
    pub reference: String,
    /// Imagen que tenía antes ese nombre (queda sin etiqueta, no se elimina).
    pub replaced_image_id: Option<String>,
}

const MAX_AUTHOR: usize = 200;
const MAX_MESSAGE: usize = 500;

fn short(id: &str) -> String {
    id.trim_start_matches("sha256:").chars().take(12).collect()
}

fn step(out: &mut EditOutcome, code: &str, r: &Result<(), EngineError>) {
    out.steps.push(EditStep {
        code: code.into(),
        ok: r.is_ok(),
        detail: r.as_ref().err().map(|e| e.to_string()),
    });
}

struct Context {
    c: ContainerInspectResponse,
    image_cfg: Option<ImageConfig>,
    list: Vec<ContainerSummary>,
    /// Contenedores que comparten la red de este (`--network container:<id>`).
    dependents: Vec<String>,
}

fn backups_of(list: &[ContainerSummary], name: &str) -> Vec<BackupRef> {
    let mut v: Vec<BackupRef> = list
        .iter()
        .filter(|c| edit::backup_origin(&c.name) == Some(name))
        .map(|c| BackupRef {
            id: c.id.clone(),
            name: c.name.clone(),
            original_name: name.to_owned(),
            state: c.state.clone(),
            created: c.created,
        })
        .collect();
    v.sort_by(|a, b| b.name.cmp(&a.name));
    v
}

impl Engine {
    async fn inspect_sized(
        &self,
        id: &str,
        size: bool,
    ) -> Result<ContainerInspectResponse, EngineError> {
        validate_ref("contenedor", id)?;
        let opts = InspectContainerOptions { size };
        let mut eq = vec!["container".to_owned(), "inspect".to_owned()];
        if size {
            eq.push("--size".into());
        }
        eq.push(id.into());
        let op = ApiOp {
            category: Category::Container,
            kind: "container.inspect",
            target: Some(id.to_owned()),
            method: "GET",
            path: path_with_query(&format!("/containers/{id}/json"), &opts),
            body: None,
            equivalent: Some(eq),
            timeout: None,
        };
        let d = self.docker().clone();
        let id_s = id.to_owned();
        self.call(
            op,
            async move { d.inspect_container(&id_s, Some(opts)).await },
            |r| r.name.clone(),
        )
        .await
    }

    async fn edit_context(&self, id: &str, size: bool) -> Result<Context, EngineError> {
        let c = self.inspect_sized(id, size).await?;
        let image_id = c.image.clone().unwrap_or_default();
        let image_cfg = match self.inspect_image_config(&image_id).await {
            Ok(cfg) => cfg,
            Err(EngineError::Docker { status: 404, .. }) => None,
            Err(e) => return Err(e),
        };
        let list = self.list_containers().await?;
        let dependents = self.dependents_of(&c).await;
        Ok(Context {
            c,
            image_cfg,
            list,
            dependents,
        })
    }

    /// Contenedores que comparten la red de este: al recrearlo dejarían de
    /// apuntar a él. Si la consulta falla no se da por bueno: se avisa igual.
    async fn dependents_of(&self, c: &ContainerInspectResponse) -> Vec<String> {
        let id = c.id.clone().unwrap_or_default();
        let d = self.docker().clone();
        let all = self
            .call_silent(
                Some(Duration::from_secs(15)),
                d.list_containers(Some(bollard::query_parameters::ListContainersOptions {
                    all: true,
                    ..Default::default()
                })),
            )
            .await;
        match all {
            Ok(all) => all
                .into_iter()
                .filter(|o| {
                    o.host_config
                        .as_ref()
                        .and_then(|h| h.network_mode.as_deref())
                        .is_some_and(|m| m == format!("container:{id}"))
                })
                .filter_map(|o| o.names.and_then(|n| n.first().cloned()))
                .map(|n| n.trim_start_matches('/').to_owned())
                .collect(),
            Err(_) => vec!["?".into()],
        }
    }

    /// La MAC por red al crear existe desde la API 1.44 (Docker 25).
    async fn endpoint_mac_supported(&self) -> bool {
        let d = self.docker().clone();
        let v = self
            .call_silent(
                Some(Duration::from_secs(10)),
                async move { d.version().await },
            )
            .await
            .ok()
            .and_then(|v| v.api_version)
            .unwrap_or_default();
        let mut parts = v.split('.').map(|x| x.parse::<u32>().unwrap_or(0));
        let (major, minor) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
        (major, minor) >= (1, 44)
    }

    /// Configuración editable de un contenedor (pantalla «Editar configuración»).
    pub async fn edit_config(&self, id: &str) -> Result<EditableConfig, EngineError> {
        let ctx = self.edit_context(id, true).await?;
        let mut cfg = edit::editable_config(&ctx.c, ctx.image_cfg.as_ref());
        let info = self.info().await?;
        cfg.host_cpus = info.ncpu.unwrap_or(0).max(0) as u32;
        cfg.host_memory = info.mem_total.unwrap_or(0).max(0) as u64;
        cfg.backups = backups_of(&ctx.list, &cfg.current.name);
        Ok(cfg)
    }

    /// Imagen con la que crear el contenedor nuevo: la referencia original si
    /// sigue apuntando a la misma imagen; si no, el ID (con aviso).
    async fn resolve_image(
        &self,
        c: &ContainerInspectResponse,
    ) -> Result<(Option<String>, Option<Risk>), EngineError> {
        let image_id = c.image.clone().unwrap_or_default();
        let reference = c
            .config
            .as_ref()
            .and_then(|x| x.image.clone())
            .unwrap_or_default();
        let lookup = |r: String| async move {
            match self.inspect_image_full(&r).await {
                Ok(i) => Ok(i.id),
                Err(EngineError::Docker { status: 404, .. })
                | Err(EngineError::InvalidInput { .. }) => Ok(None),
                Err(e) => Err(e),
            }
        };
        if !reference.is_empty() && lookup(reference.clone()).await?.as_deref() == Some(&image_id) {
            return Ok((Some(reference), None));
        }
        if !image_id.is_empty() && lookup(image_id.clone()).await?.is_some() {
            let r = (reference != image_id && !reference.is_empty())
                .then(|| edit::risk("imageTagMoved", &[("image", reference.clone())]));
            return Ok((Some(image_id), r));
        }
        Ok((None, None))
    }

    async fn build_plan(
        &self,
        ctx: &Context,
        req: &EditRequest,
        size_known: bool,
    ) -> Result<(EditPlan, EditRequest, EditRequest), EngineError> {
        let c = &ctx.c;
        let id = c.id.clone().unwrap_or_default();
        let current = edit::current_request(c, ctx.image_cfg.as_ref());
        let desired = edit::normalize(req);
        let (changes, kept) = edit::diff(&current, &desired);
        let host = c.host_config.clone().unwrap_or_default();
        let labels = c
            .config
            .as_ref()
            .and_then(|x| x.labels.clone())
            .unwrap_or_default();
        let auto_remove = host.auto_remove.unwrap_or(false);
        let status = edit::status_of(c);
        // Carpetas: en este equipo o, con un servidor remoto, en el servidor.
        let remote = self.ssh_session().is_some();
        let mut issues = edit::validate(&current, &desired, &changes, auto_remove, !remote);
        if remote && changes.iter().any(|x| x.field == "mounts") {
            let new_binds: Vec<(usize, String)> = desired
                .mounts
                .iter()
                .enumerate()
                .filter(|(_, m)| {
                    m.kind == crate::spec::MountKind::Bind
                        && m.source.trim().starts_with('/')
                        && !current.mounts.contains(m)
                })
                .map(|(i, m)| (i, m.source.trim().to_owned()))
                .collect();
            issues.extend(self.missing_bind_issues(&new_binds).await?);
        }
        let changed = |f: &str| changes.iter().any(|x| x.field == f);

        if changed("name")
            && ctx
                .list
                .iter()
                .any(|o| o.id != id && o.name == desired.name)
        {
            issues.push(issue(Severity::Error, "name", "name.inUse", &[]));
        }
        let needs_recreate = changes.iter().any(|x| x.kind == ChangeKind::Recreate);
        let mut blockers: Vec<Risk> = Vec::new();
        let mut risks: Vec<Risk> = Vec::new();
        let mut notices: Vec<Risk> = Vec::new();
        let mut image_ref = None;
        let mut create_body = None;
        let mut backup = None;
        let mut equivalent: Vec<String> = Vec::new();
        let update = edit::update_body(&host, &current, &desired);

        if needs_recreate {
            if changed("ports") {
                issues.extend(edit::port_conflicts(&desired.ports, &ctx.list, &id));
            }
            if !matches!(status.as_str(), "running" | "exited" | "created") {
                blockers.push(edit::risk("state", &[("status", status.clone())]));
            }
            if auto_remove {
                blockers.push(edit::risk("autoRemove", &[]));
            }
            if let Some(p) = labels.get("com.docker.compose.project") {
                blockers.push(edit::risk("composeManaged", &[("project", p.clone())]));
            }
            if labels.contains_key("com.docker.swarm.service.id") {
                blockers.push(edit::risk("swarmManaged", &[]));
            }
            let (img, moved) = self.resolve_image(c).await?;
            match &img {
                None => blockers.push(edit::risk(
                    "imageMissing",
                    &[(
                        "image",
                        c.config
                            .as_ref()
                            .and_then(|x| x.image.clone())
                            .unwrap_or_default(),
                    )],
                )),
                Some(_) => risks.extend(moved),
            }
            if let Some(size) = c.size_rw.filter(|s| *s > 0) {
                risks.push(edit::risk("writableLayer", &[("bytes", size.to_string())]));
            } else if !size_known || c.size_rw.is_none() {
                risks.push(edit::risk("writableLayerUnknown", &[]));
            }
            let dependents = ctx.dependents.clone();
            if !dependents.is_empty() {
                risks.push(edit::risk(
                    "dependents",
                    &[("containers", dependents.join(", "))],
                ));
            }
            let detached: Vec<String> = current
                .mounts
                .iter()
                .filter(|m| {
                    !desired
                        .mounts
                        .iter()
                        .any(|d| d.target == m.target && d.source == m.source)
                })
                .map(|m| format!("{} ({})", m.target, m.source))
                .collect();
            if !detached.is_empty() {
                risks.push(edit::risk(
                    "mountDetached",
                    &[("mounts", detached.join(", "))],
                ));
            }
            let has_endpoints = c
                .network_settings
                .as_ref()
                .and_then(|n| n.networks.as_ref())
                .is_some_and(|n| !n.is_empty())
                && !matches!(host.network_mode.as_deref(), Some("host") | Some("none"))
                && !host
                    .network_mode
                    .as_deref()
                    .is_some_and(|m| m.starts_with("container:"));
            if has_endpoints && !self.endpoint_mac_supported().await {
                // Antes de la API 1.44 la MAC fija va en `Config.MacAddress`,
                // que la aplicación no puede leer ni enviar: no se conserva.
                notices.push(edit::risk("macLegacy", &[]));
            } else if has_endpoints && status == "running" {
                // El usuario decide si conserva las direcciones MAC actuales.
                notices.push(edit::risk("macChoice", &[]));
            } else if has_endpoints && status == "exited" {
                // Docker ya no muestra la MAC de un contenedor detenido.
                notices.push(edit::risk("macUnknown", &[]));
            }
            let taken: Vec<String> = ctx.list.iter().map(|o| o.name.clone()).collect();
            // La copia lleva el nombre que tendrá el contenedor nuevo: así aparece
            // entre sus copias y restaurarla lo sustituye, aunque la misma
            // recreación le cambie el nombre (lo detectó la prueba en la aplicación real).
            let bname = edit::backup_name(
                &desired.name,
                edit::BACKUP_INFIX,
                chrono::Utc::now(),
                &taken,
            );
            if let Some(img) = &img {
                match edit::recreate_body(c, img, &current, &desired, &edit::macs_to_keep(c, false))
                {
                    Ok((body, _)) => {
                        create_body = serde_json::to_string_pretty(&edit::redacted_body(&body)).ok()
                    }
                    Err(e) => blockers.push(edit::risk("configUnreadable", &[("detail", e)])),
                }
            }
            if status == "running" {
                equivalent.push(self.docker_cli(&[
                    "stop".into(),
                    "--time".into(),
                    crate::containers::STOP_TIMEOUT_SECS.to_string(),
                    current.name.clone(),
                ]));
            }
            equivalent.push(self.docker_cli(&[
                "rename".into(),
                current.name.clone(),
                bname.clone(),
            ]));
            equivalent.push(format!("POST /containers/create?name={}", desired.name));
            if status == "running" {
                equivalent.push(self.docker_cli(&["start".into(), desired.name.clone()]));
            }
            backup = Some(bname);
            image_ref = img;
        } else {
            if let Some((_, args)) = &update {
                let mut a = vec!["update".to_owned()];
                a.extend(args.iter().cloned());
                a.push(current.name.clone());
                equivalent.push(self.docker_cli(&a));
            }
            if changed("name") {
                equivalent.push(self.docker_cli(&[
                    "rename".into(),
                    current.name.clone(),
                    desired.name.clone(),
                ]));
            }
        }

        let plan = EditPlan {
            container_id: id,
            name: current.name.clone(),
            will_stop: needs_recreate && status == "running",
            status,
            changes,
            kept,
            needs_recreate,
            issues,
            blockers,
            risks,
            notices,
            backup_name: backup,
            image_ref,
            equivalent,
            create_body,
        };
        Ok((plan, current, desired))
    }

    /// Compara lo deseado con lo actual y dice qué hace falta para aplicarlo
    /// (2.4.1–2.4.3). No modifica nada.
    pub async fn edit_plan(&self, id: &str, req: &EditRequest) -> Result<EditPlan, EngineError> {
        let ctx = self.edit_context(id, true).await?;
        Ok(self.build_plan(&ctx, req, true).await?.0)
    }

    /// Aplica los cambios. Los directos se envían a Docker sobre el contenedor
    /// existente; si alguno exige recreación, se hace la recreación segura
    /// (solo con `confirm_recreate` y todos los riesgos aceptados).
    pub async fn edit_apply(
        &self,
        id: &str,
        req: &EditRequest,
        opts: &ApplyOptions,
    ) -> Result<EditOutcome, EngineError> {
        validate_ref("contenedor", id)?;
        let _lock = self.locks().acquire(&format!("container:{id}"))?;
        let ctx = self.edit_context(id, true).await?;
        let (mut plan, current, desired) = self.build_plan(&ctx, req, true).await?;
        if plan.changes.is_empty() {
            return Err(EngineError::InvalidInput {
                message: "no hay cambios que aplicar".into(),
            });
        }
        if let (Some(shown), Some(_)) = (&opts.backup_name, &plan.backup_name) {
            let taken = ctx.list.iter().any(|o| o.name == *shown);
            if edit::backup_origin(shown) == Some(desired.name.as_str()) && !taken {
                plan.backup_name = Some(shown.clone());
            }
        }
        if plan.has_errors() {
            return Err(EngineError::InvalidSpec {
                issues: plan.issues,
            });
        }
        if !plan.needs_recreate {
            return Ok(self.apply_direct(&ctx.c, &plan, &current, &desired).await);
        }
        if !plan.blockers.is_empty() {
            return Err(EngineError::InvalidInput {
                message: format!(
                    "la recreación no se puede hacer con seguridad ({})",
                    plan.blockers
                        .iter()
                        .map(|x| x.code.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            });
        }
        if opts.keep_mac && plan.notices.iter().any(|n| n.code == "macLegacy") {
            return Err(EngineError::InvalidInput {
                message: "esta versión de Docker no permite conservar la dirección MAC".into(),
            });
        }
        if !opts.confirm_recreate
            || plan
                .risks
                .iter()
                .any(|r| !opts.accepted_risks.contains(&r.code))
        {
            return Err(EngineError::NotConfirmed);
        }
        Ok(self
            .recreate(&ctx.c, &plan, &current, &desired, opts.keep_mac)
            .await)
    }

    // ---------------------------------------------------- operaciones básicas

    async fn op_rename(&self, id: &str, shown: &str, new_name: &str) -> Result<(), EngineError> {
        let opts = RenameContainerOptions {
            name: new_name.to_owned(),
        };
        let op = ApiOp {
            category: Category::Container,
            kind: "container.rename",
            target: Some(shown.to_owned()),
            method: "POST",
            path: path_with_query(&format!("/containers/{id}/rename"), &opts),
            body: None,
            equivalent: Some(vec!["rename".into(), shown.into(), new_name.into()]),
            timeout: None,
        };
        let d = self.docker().clone();
        let id_s = id.to_owned();
        let summary = format!("{shown} → {new_name}");
        self.call(
            op,
            async move { d.rename_container(&id_s, opts).await },
            move |_| Some(summary),
        )
        .await
    }

    async fn op_update(
        &self,
        id: &str,
        shown: &str,
        body: bollard::models::ContainerUpdateBody,
        args: &[String],
    ) -> Result<(), EngineError> {
        let mut eq = vec!["update".to_owned()];
        eq.extend(args.iter().cloned());
        eq.push(shown.to_owned());
        let op = ApiOp {
            category: Category::Container,
            kind: "container.update",
            target: Some(shown.to_owned()),
            method: "POST",
            path: format!("/containers/{id}/update"),
            body: Some(serde_json::to_string_pretty(&body).unwrap_or_default()),
            equivalent: Some(eq),
            timeout: None,
        };
        let d = self.docker().clone();
        let id_s = id.to_owned();
        let summary = format!("Actualizado: {}", args.join(" "));
        self.call(
            op,
            async move { d.update_container(&id_s, body).await },
            move |_| Some(summary),
        )
        .await
    }

    async fn op_create_raw(
        &self,
        name: &str,
        body: ContainerCreateBody,
    ) -> Result<String, EngineError> {
        let opts = CreateContainerOptions {
            name: Some(name.to_owned()),
            platform: String::new(),
        };
        let op = ApiOp {
            category: Category::Container,
            kind: "container.create",
            target: Some(name.to_owned()),
            method: "POST",
            path: path_with_query("/containers/create", &opts),
            // Sin valores sensibles en el registro.
            body: Some(
                serde_json::to_string_pretty(&edit::redacted_body(&body)).unwrap_or_default(),
            ),
            // No hay una orden `docker create` corta que reproduzca fielmente
            // la configuración completa: no se inventa una.
            equivalent: None,
            timeout: None,
        };
        let d = self.docker().clone();
        let r = self
            .call(
                op,
                async move { d.create_container(Some(opts), body).await },
                |r| Some(format!("Contenedor creado: {}", short(&r.id))),
            )
            .await?;
        Ok(r.id)
    }

    /// Elimina el contenedor **nuevo** que no llegó a funcionar. Nunca con
    /// `v=true`: los volúmenes no se tocan.
    async fn op_remove_new(&self, id: &str, shown: &str) -> Result<(), EngineError> {
        let opts = RemoveContainerOptions {
            v: false,
            force: true,
            link: false,
        };
        let op = ApiOp {
            category: Category::Container,
            kind: "container.remove",
            target: Some(shown.to_owned()),
            method: "DELETE",
            path: path_with_query(&format!("/containers/{id}"), &opts),
            body: None,
            equivalent: Some(vec!["rm".into(), "--force".into(), id.into()]),
            timeout: None,
        };
        let d = self.docker().clone();
        let id_s = id.to_owned();
        self.call(
            op,
            async move { d.remove_container(&id_s, Some(opts)).await },
            |_| Some("Contenedor nuevo eliminado (no llegó a funcionar)".into()),
        )
        .await
    }

    async fn op_connect(
        &self,
        network: &str,
        id: &str,
        shown: &str,
        endpoint: EndpointSettings,
    ) -> Result<(), EngineError> {
        let body = NetworkConnectRequest {
            container: id.to_owned(),
            endpoint_config: Some(endpoint),
        };
        let op = ApiOp {
            category: Category::Network,
            kind: "network.connect",
            target: Some(format!("{shown} → {network}")),
            method: "POST",
            path: format!("/networks/{network}/connect"),
            body: Some(serde_json::to_string_pretty(&body).unwrap_or_default()),
            equivalent: Some(vec![
                "network".into(),
                "connect".into(),
                network.into(),
                shown.into(),
            ]),
            timeout: None,
        };
        let d = self.docker().clone();
        let key = network.to_owned();
        let summary = format!("{shown} conectado a {network}");
        self.call(
            op,
            async move { d.connect_network(&key, body).await },
            move |_| Some(summary),
        )
        .await
    }

    async fn op_disconnect(&self, network: &str, id: &str, shown: &str) -> Result<(), EngineError> {
        let body = NetworkDisconnectRequest {
            container: id.to_owned(),
            force: None,
        };
        let op = ApiOp {
            category: Category::Network,
            kind: "network.disconnect",
            target: Some(format!("{shown} → {network}")),
            method: "POST",
            path: format!("/networks/{network}/disconnect"),
            body: Some(serde_json::to_string_pretty(&body).unwrap_or_default()),
            equivalent: Some(vec![
                "network".into(),
                "disconnect".into(),
                network.into(),
                shown.into(),
            ]),
            timeout: None,
        };
        let d = self.docker().clone();
        let key = network.to_owned();
        let summary = format!("{shown} desconectado de {network}");
        self.call(
            op,
            async move { d.disconnect_network(&key, body).await },
            move |_| Some(summary),
        )
        .await
    }

    // ---------------------------------------------------- cambios directos

    async fn apply_direct(
        &self,
        c: &ContainerInspectResponse,
        plan: &EditPlan,
        current: &EditRequest,
        desired: &EditRequest,
    ) -> EditOutcome {
        let id = plan.container_id.clone();
        let host = c.host_config.clone().unwrap_or_default();
        let mut out = EditOutcome {
            container_id: id.clone(),
            name: current.name.clone(),
            ..Default::default()
        };
        if let Some((body, args)) = edit::update_body(&host, current, desired) {
            let r = self.op_update(&id, &current.name, body, &args).await;
            step(&mut out, "update", &r);
            if let Err(e) = r {
                out.error = Some(e.to_string());
                return out;
            }
        }
        if current.name != desired.name {
            let r = self.op_rename(&id, &current.name, &desired.name).await;
            step(&mut out, "rename", &r);
            match r {
                Ok(()) => out.name = desired.name.clone(),
                Err(e) => {
                    out.error = Some(e.to_string());
                    return out;
                }
            }
        }
        // Verificación: lo que dice Docker después debe coincidir con lo pedido.
        let verified: Result<(), EngineError> = match self.inspect_sized(&id, false).await {
            Ok(after) => {
                let now = edit::current_request(&after, None);
                let (rest, _) = edit::diff(
                    &EditRequest {
                        env: desired.env.clone(),
                        ports: desired.ports.clone(),
                        mounts: desired.mounts.clone(),
                        description: desired.description.clone(),
                        ..now
                    },
                    desired,
                );
                if rest.is_empty() {
                    Ok(())
                } else {
                    Err(EngineError::Other {
                        message: format!(
                            "Docker no refleja todos los cambios: {}",
                            rest.iter()
                                .map(|x: &Change| x.field.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    })
                }
            }
            Err(e) => Err(e),
        };
        step(&mut out, "verify", &verified);
        if let Err(e) = verified {
            out.error = Some(e.to_string());
        }
        out.started = edit::status_of(c) == "running";
        out
    }

    // ---------------------------------------------------- recreación segura

    async fn recreate(
        &self,
        c: &ContainerInspectResponse,
        plan: &EditPlan,
        current: &EditRequest,
        desired: &EditRequest,
        keep_mac: bool,
    ) -> EditOutcome {
        let id = plan.container_id.clone();
        let was_running = plan.status == "running";
        let backup = plan.backup_name.clone().unwrap_or_default();
        let image = plan.image_ref.clone().unwrap_or_default();
        let mut out = EditOutcome {
            container_id: id.clone(),
            name: current.name.clone(),
            recreated: false,
            ..Default::default()
        };
        // Registro de conjunto de la operación; cada paso real queda aparte.
        let changed: Vec<&str> = plan.changes.iter().map(|x| x.field.as_str()).collect();
        let handle = self.activity().begin(OperationSpec {
            category: Category::Container,
            kind: "container.recreate".into(),
            target: Some(current.name.clone()),
            transport: Transport::App {
                action: "Recreación segura de un contenedor".into(),
            },
            request_body: serde_json::to_string_pretty(&serde_json::json!({
                "container": current.name,
                "changes": plan.changes,
                "backup": backup,
                "image": image,
            }))
            .ok(),
            equivalent_command: None,
        });
        let finish = |out: EditOutcome| {
            match &out.error {
                None => handle.succeed(Outcome {
                    summary: Some(format!(
                        "Recreado con cambios en {}; el original queda como {}",
                        changed.join(", "),
                        out.backup_name.clone().unwrap_or_default()
                    )),
                    ..Default::default()
                }),
                Some(e) => handle.fail(Failure {
                    error: if out.rollback_failed {
                        format!("{e}. No se pudo volver del todo al estado anterior")
                    } else if out.rolled_back {
                        format!("{e}. Se ha vuelto al contenedor original")
                    } else {
                        e.clone()
                    },
                    ..Default::default()
                }),
            }
            out
        };

        let macs = edit::macs_to_keep(c, keep_mac);
        // 1. Detener el original.
        if was_running {
            let r = self
                .run_action(&id, Some(&current.name), ContainerAction::Stop)
                .await;
            step(&mut out, "stop", &r);
            if let Err(e) = r {
                out.error = Some(e.to_string());
                out.started = true;
                return finish(out);
            }
        }
        // Con el original detenido, sus redes muestran solo lo configurado.
        let stopped = match self.inspect_sized(&id, false).await {
            Ok(s) if !matches!(edit::status_of(&s).as_str(), "exited" | "created") => {
                Err(EngineError::Other {
                    message: format!(
                        "el contenedor no quedó detenido (estado: {})",
                        edit::status_of(&s)
                    ),
                })
            }
            other => other,
        };
        let built = stopped.and_then(|s| {
            edit::recreate_body(&s, &image, current, desired, &macs)
                .map_err(|message| EngineError::Other { message })
        });
        let (body, nets) = match built {
            Ok(x) => x,
            Err(e) => {
                step(&mut out, "prepare", &Err(e.clone()));
                out.error = Some(e.to_string());
                self.rollback(&mut out, &id, &backup, current, None, false, was_running)
                    .await;
                return finish(out);
            }
        };

        // 2. Apartar el original con el nombre de la copia.
        let r = self.op_rename(&id, &current.name, &backup).await;
        step(&mut out, "backup", &r);
        if let Err(e) = r {
            out.error = Some(e.to_string());
            self.rollback(&mut out, &id, &backup, current, None, false, was_running)
                .await;
            return finish(out);
        }

        // 3. Crear el nuevo.
        let created = self.op_create_raw(&desired.name, body).await;
        step(
            &mut out,
            "create",
            &created.as_ref().map(|_| ()).map_err(|e| e.clone()),
        );
        let new_id = match created {
            Ok(i) => i,
            Err(e) => {
                out.error = Some(e.to_string());
                self.rollback(&mut out, &id, &backup, current, None, true, was_running)
                    .await;
                return finish(out);
            }
        };

        // 4. Redes adicionales.
        let mut failed: Option<EngineError> = None;
        for (net, ep) in nets.connect {
            let r = self.op_connect(&net, &new_id, &desired.name, ep).await;
            step(&mut out, "connect", &r);
            if let Err(e) = r {
                failed = Some(e);
                break;
            }
        }
        if failed.is_none() {
            for net in nets.disconnect {
                let r = self.op_disconnect(&net, &new_id, &desired.name).await;
                step(&mut out, "disconnect", &r);
                if let Err(e) = r {
                    failed = Some(e);
                    break;
                }
            }
        }

        // 5. Iniciar y comprobar que se mantiene en ejecución.
        if failed.is_none() && was_running {
            let r = self
                .run_action(&new_id, Some(&desired.name), ContainerAction::Start)
                .await;
            step(&mut out, "start", &r);
            match r {
                Err(e) => failed = Some(e),
                Ok(()) => {
                    let v = match self.verify_running(&new_id).await {
                        Ok(()) => self.verify_macs(&new_id, &macs).await,
                        e => e,
                    };
                    step(&mut out, "verify", &v);
                    failed = v.err();
                }
            }
        } else if failed.is_none() {
            // Detenido: basta con que Docker lo haya creado con la configuración.
            let v = self.inspect_sized(&new_id, false).await.map(drop);
            step(&mut out, "verify", &v);
            failed = v.err();
        }

        if let Some(e) = failed {
            out.error = Some(e.to_string());
            self.rollback(
                &mut out,
                &id,
                &backup,
                current,
                Some(&new_id),
                true,
                was_running,
            )
            .await;
            return finish(out);
        }

        out.container_id = new_id;
        out.name = desired.name.clone();
        out.recreated = true;
        out.started = was_running;
        out.backup_id = Some(id);
        out.backup_name = Some(backup);
        finish(out)
    }

    /// El contenedor debe seguir en ejecución, sin reinicios, durante la ventana.
    async fn verify_running(&self, id: &str) -> Result<(), EngineError> {
        let d = self.docker().clone();
        let deadline = tokio::time::Instant::now() + VERIFY_WINDOW;
        loop {
            let c = self
                .call_silent(
                    Some(Duration::from_secs(10)),
                    d.inspect_container(id, None::<InspectContainerOptions>),
                )
                .await?;
            let st = c.state.clone().unwrap_or_default();
            let status = edit::status_of(&c);
            if status != "running" || c.restart_count.unwrap_or(0) > 0 {
                let detail = st.error.filter(|e| !e.is_empty()).unwrap_or_default();
                return Err(EngineError::Other {
                    message: format!(
                        "el contenedor nuevo no se mantiene en ejecución (estado: {status}, código de salida: {}{})",
                        st.exit_code.unwrap_or(0),
                        if detail.is_empty() {
                            String::new()
                        } else {
                            format!(", {detail}")
                        }
                    ),
                });
            }
            if tokio::time::Instant::now() >= deadline {
                // Queda registrado el estado comprobado.
                return self.container_state(id, None).await.map(drop);
            }
            tokio::time::sleep(VERIFY_STEP).await;
        }
    }

    /// Si se pidió conservar las direcciones MAC, Docker debe haberlas aplicado.
    async fn verify_macs(
        &self,
        id: &str,
        macs: &std::collections::HashMap<String, String>,
    ) -> Result<(), EngineError> {
        if macs.is_empty() {
            return Ok(());
        }
        let d = self.docker().clone();
        let c = self
            .call_silent(
                Some(Duration::from_secs(10)),
                d.inspect_container(id, None::<InspectContainerOptions>),
            )
            .await?;
        let nets = c
            .network_settings
            .and_then(|n| n.networks)
            .unwrap_or_default();
        for (net, mac) in macs {
            let got = nets
                .get(net)
                .and_then(|e| e.mac_address.clone())
                .unwrap_or_default();
            if !got.eq_ignore_ascii_case(mac) {
                return Err(EngineError::Other {
                    message: format!(
                        "Docker no aplicó la dirección MAC {mac} en la red {net} (tiene {got})"
                    ),
                });
            }
        }
        Ok(())
    }

    /// Vuelve al contenedor original: elimina el nuevo (sin volúmenes),
    /// devuelve su nombre al original y lo inicia si estaba en ejecución.
    #[allow(clippy::too_many_arguments)]
    async fn rollback(
        &self,
        out: &mut EditOutcome,
        original: &str,
        backup: &str,
        current: &EditRequest,
        new_id: Option<&str>,
        renamed: bool,
        was_running: bool,
    ) {
        let mut ok = true;
        if let Some(n) = new_id {
            let r = self.op_remove_new(n, "contenedor nuevo").await;
            step(out, "rollbackRemove", &r);
            ok &= r.is_ok();
        }
        if renamed && ok {
            let r = self.op_rename(original, backup, &current.name).await;
            step(out, "rollbackRename", &r);
            ok &= r.is_ok();
        }
        if was_running {
            let r = self
                .run_action(original, Some(&current.name), ContainerAction::Start)
                .await;
            step(out, "rollbackStart", &r);
            ok &= r.is_ok();
            out.started = r.is_ok();
        }
        out.rolled_back = ok;
        out.rollback_failed = !ok;
        out.container_id = original.to_owned();
        out.name = current.name.clone();
    }

    // ---------------------------------------------------- copia de seguridad

    /// Vuelve a poner en servicio la copia que dejó una recreación: el
    /// contenedor que ocupa ahora el nombre se detiene y se aparta (no se
    /// elimina) y la copia recupera su nombre original.
    pub async fn restore_backup(
        &self,
        backup_id: &str,
        start: bool,
        confirmed: bool,
    ) -> Result<EditOutcome, EngineError> {
        if !confirmed {
            return Err(EngineError::NotConfirmed);
        }
        validate_ref("contenedor", backup_id)?;
        let _lock = self.locks().acquire(&format!("container:{backup_id}"))?;
        let b = self.inspect_sized(backup_id, false).await?;
        let bid = b.id.clone().unwrap_or_default();
        let bname = b
            .name
            .clone()
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_owned();
        let Some(orig) = edit::backup_origin(&bname).map(str::to_owned) else {
            return Err(EngineError::InvalidInput {
                message: format!("{bname} no es una copia de seguridad de una recreación"),
            });
        };
        if !matches!(edit::status_of(&b).as_str(), "exited" | "created") {
            return Err(EngineError::InvalidInput {
                message: "la copia debe estar detenida".into(),
            });
        }
        let list = self.list_containers().await?;
        let holder = list.iter().find(|c| c.name == orig).cloned();
        let _lock2 = match &holder {
            Some(h) => Some(self.locks().acquire(&format!("container:{}", h.id))?),
            None => None,
        };
        let mut out = EditOutcome {
            container_id: bid.clone(),
            name: bname.clone(),
            ..Default::default()
        };
        let handle = self.activity().begin(OperationSpec {
            category: Category::Container,
            kind: "container.restoreBackup".into(),
            target: Some(orig.clone()),
            transport: Transport::App {
                action: "Restauración de la copia de un contenedor".into(),
            },
            request_body: None,
            equivalent_command: None,
        });
        let mut set_aside: Option<(String, String, bool)> = None;
        let result: Result<(), EngineError> = async {
            if let Some(h) = &holder {
                let running = h.state == "running";
                if !matches!(h.state.as_str(), "running" | "exited" | "created") {
                    return Err(EngineError::InvalidInput {
                        message: format!(
                            "{orig} está en un estado que no permite apartarlo ({})",
                            h.state
                        ),
                    });
                }
                if running {
                    let r = self
                        .run_action(&h.id, Some(&orig), ContainerAction::Stop)
                        .await;
                    step(&mut out, "stop", &r);
                    r?;
                }
                let taken: Vec<String> = list.iter().map(|c| c.name.clone()).collect();
                let aside = edit::backup_name(&orig, REPLACED_INFIX, chrono::Utc::now(), &taken);
                let r = self.op_rename(&h.id, &orig, &aside).await;
                step(&mut out, "backup", &r);
                if let Err(e) = r {
                    if running {
                        let s = self
                            .run_action(&h.id, Some(&orig), ContainerAction::Start)
                            .await;
                        step(&mut out, "rollbackStart", &s);
                        out.rollback_failed = s.is_err();
                        out.rolled_back = s.is_ok();
                    } else {
                        out.rolled_back = true;
                    }
                    return Err(e);
                }
                set_aside = Some((h.id.clone(), aside, running));
            }
            let r = self.op_rename(&bid, &bname, &orig).await;
            step(&mut out, "rename", &r);
            let r = match r {
                Ok(()) if start => {
                    let s = self
                        .run_action(&bid, Some(&orig), ContainerAction::Start)
                        .await;
                    step(&mut out, "start", &s);
                    match s {
                        Ok(()) => {
                            let v = self.verify_running(&bid).await;
                            step(&mut out, "verify", &v);
                            v
                        }
                        Err(e) => Err(e),
                    }
                }
                other => other,
            };
            if let Err(e) = r {
                // Deshacer: la copia vuelve a su nombre y el apartado al suyo.
                let mut ok = true;
                let now = self.inspect_sized(&bid, false).await.ok();
                if now.as_ref().map(edit::status_of).as_deref() == Some("running") {
                    let s = self
                        .run_action(&bid, Some(&orig), ContainerAction::Stop)
                        .await;
                    step(&mut out, "rollbackStop", &s);
                    ok &= s.is_ok();
                }
                let named_orig = now
                    .and_then(|n| n.name)
                    .is_some_and(|n| n.trim_start_matches('/') == orig);
                if named_orig {
                    let s = self.op_rename(&bid, &orig, &bname).await;
                    step(&mut out, "rollbackRename", &s);
                    ok &= s.is_ok();
                }
                if let Some((hid, aside, running)) = &set_aside {
                    if ok {
                        let s = self.op_rename(hid, aside, &orig).await;
                        step(&mut out, "rollbackRename", &s);
                        ok &= s.is_ok();
                        if ok && *running {
                            let s = self
                                .run_action(hid, Some(&orig), ContainerAction::Start)
                                .await;
                            step(&mut out, "rollbackStart", &s);
                            ok &= s.is_ok();
                        }
                    }
                }
                out.rolled_back = ok;
                out.rollback_failed = !ok;
                return Err(e);
            }
            Ok(())
        }
        .await;
        match result {
            Ok(()) => {
                out.name = orig.clone();
                out.started = start;
                if let Some((hid, aside, _)) = set_aside {
                    out.backup_id = Some(hid);
                    out.backup_name = Some(aside);
                }
                handle.succeed(Outcome {
                    summary: Some(match &out.backup_name {
                        Some(a) => {
                            format!("{bname} restaurado como {orig}; el anterior queda como {a}")
                        }
                        None => format!("{bname} restaurado como {orig}"),
                    }),
                    ..Default::default()
                });
            }
            Err(e) => {
                out.error = Some(e.to_string());
                handle.fail(Failure {
                    error: e.to_string(),
                    ..Default::default()
                });
            }
        }
        Ok(out)
    }

    // ---------------------------------------------------- imagen desde contenedor

    fn commit_check(req: &CommitRequest) -> (Option<crate::reference::ImageRef>, Vec<SpecIssue>) {
        let mut issues = Vec::new();
        let repo = req.repository.trim();
        let tag = if req.tag.trim().is_empty() {
            "latest"
        } else {
            req.tag.trim()
        };
        let mut reference = None;
        if repo.is_empty() {
            issues.push(issue(
                Severity::Error,
                "repository",
                "commit.repositoryRequired",
                &[],
            ));
        } else if !crate::reference::valid_tag(tag) {
            issues.push(issue(Severity::Error, "tag", "commit.tagInvalid", &[]));
        } else {
            match crate::reference::validate_tag_target(repo, tag) {
                Ok(r) => reference = Some(r),
                Err(crate::reference::RefError::InvalidTag) => {
                    issues.push(issue(Severity::Error, "tag", "commit.tagInvalid", &[]))
                }
                Err(_) => issues.push(issue(
                    Severity::Error,
                    "repository",
                    "commit.repositoryInvalid",
                    &[],
                )),
            }
        }
        for (field, value, max) in [
            ("author", &req.author, MAX_AUTHOR),
            ("message", &req.message, MAX_MESSAGE),
        ] {
            if value.chars().count() > max {
                issues.push(issue(
                    Severity::Error,
                    field,
                    "commit.tooLong",
                    &[("max", max.to_string())],
                ));
            }
            if value.chars().any(char::is_control) {
                issues.push(issue(Severity::Error, field, "commit.controlChars", &[]));
            }
        }
        (reference, issues)
    }

    fn commit_args(req: &CommitRequest, container: &str, reference: &str) -> Vec<String> {
        let mut a = vec!["commit".to_owned()];
        if !req.author.trim().is_empty() {
            a.extend(["--author".into(), req.author.trim().to_owned()]);
        }
        if !req.message.trim().is_empty() {
            a.extend(["--message".into(), req.message.trim().to_owned()]);
        }
        if !req.pause {
            a.push("--pause=false".into());
        }
        a.push(container.to_owned());
        a.push(reference.to_owned());
        a
    }

    async fn image_id_of(&self, reference: &str) -> Result<Option<String>, EngineError> {
        let d = self.docker().clone();
        let r = reference.to_owned();
        match self
            .call_silent(Some(Duration::from_secs(15)), async move {
                d.inspect_image(&r).await
            })
            .await
        {
            Ok(i) => Ok(i.id),
            Err(EngineError::Docker { status: 404, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Comprueba el formulario de «Crear imagen a partir de este contenedor»:
    /// nombre completo, si ya existe y qué montajes quedan fuera. No crea nada.
    pub async fn commit_preview(
        &self,
        id: &str,
        req: &CommitRequest,
    ) -> Result<CommitPreview, EngineError> {
        let c = self.inspect_sized(id, false).await?;
        let name = c
            .name
            .clone()
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_owned();
        let (reference, issues) = Self::commit_check(req);
        let familiar = reference.as_ref().map(|r| r.familiar());
        let existing = match &familiar {
            Some(f) => self.image_id_of(f).await?,
            None => None,
        };
        let excluded_mounts = c
            .mounts
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|m| ExcludedMount {
                kind: m.typ.unwrap_or_default(),
                source: m
                    .name
                    .filter(|n| !n.is_empty())
                    .or(m.source)
                    .unwrap_or_default(),
                target: m.destination.unwrap_or_default(),
            })
            .collect();
        let equivalent = self.docker_cli(&Self::commit_args(
            req,
            &name,
            familiar.as_deref().unwrap_or("REPOSITORIO:ETIQUETA"),
        ));
        Ok(CommitPreview {
            container_name: name,
            status: edit::status_of(&c),
            reference: familiar,
            issues,
            existing_image_id: existing,
            excluded_mounts,
            equivalent,
        })
    }

    /// `docker commit`: crea una imagen con el sistema de archivos del
    /// contenedor. No incluye volúmenes ni carpetas montadas, y no modifica el
    /// contenedor (solo lo pausa mientras dura si `pause`).
    pub async fn commit_image(
        &self,
        id: &str,
        req: &CommitRequest,
    ) -> Result<CommitResult, EngineError> {
        validate_ref("contenedor", id)?;
        let (reference, issues) = Self::commit_check(req);
        if ContainerSpec::has_errors(&issues) {
            return Err(EngineError::InvalidSpec { issues });
        }
        let reference = reference.expect("sin errores hay referencia");
        let familiar = reference.familiar();
        let _lock = self.locks().acquire(&format!("container:{id}"))?;
        // El contenedor debe existir (y se usa su nombre en el registro).
        let c = self.inspect_sized(id, false).await?;
        let name = c
            .name
            .clone()
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_owned();
        let existing = self.image_id_of(&familiar).await?;
        if existing.is_some() && !req.replace_existing {
            return Err(EngineError::AlreadyExists {
                path: familiar.clone(),
            });
        }
        let tag = reference.api_tag();
        let opts = CommitContainerOptions {
            container: Some(id.to_owned()),
            // El repositorio tal como lo escribió el usuario, como hace el CLI.
            repo: Some(req.repository.trim().to_owned()),
            tag: Some(tag),
            comment: Some(req.message.trim().to_owned()).filter(|s| !s.is_empty()),
            author: Some(req.author.trim().to_owned()).filter(|s| !s.is_empty()),
            pause: req.pause,
            changes: None,
        };
        let op = ApiOp {
            category: Category::Image,
            kind: "container.commit",
            target: Some(format!("{name} → {familiar}")),
            method: "POST",
            path: path_with_query("/commit", &opts),
            body: None,
            equivalent: Some(Self::commit_args(req, &name, &familiar)),
            timeout: None,
        };
        let d = self.docker().clone();
        let fam = familiar.clone();
        let r = self
            .call(
                op,
                async move {
                    d.commit_container(opts, bollard::models::ContainerConfig::default())
                        .await
                },
                move |r| Some(format!("Imagen {fam} creada: {}", short(&r.id))),
            )
            .await?;
        Ok(CommitResult {
            image_id: r.id.clone(),
            reference: familiar,
            replaced_image_id: existing.filter(|e| *e != r.id),
        })
    }
}
