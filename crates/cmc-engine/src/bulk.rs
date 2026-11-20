// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Gestión masiva de contenedores (1.10.0, D-117 a D-120): reglas y ejecución
//! de una operación sobre **un** contenedor.
//!
//! La coordinación (varios contenedores, concurrencia, cancelación, progreso y
//! varios servidores) está en la aplicación (`src-tauri/src/bulk.rs`); aquí solo
//! están las reglas puras y el servicio de Docker que se aplica a cada
//! contenedor, reutilizando las mismas operaciones que el resto de CMC
//! (`run_action`, `container_state`, `remove`, `update`), con el mismo registro
//! de actividad y el mismo bloqueo por contenedor.
//!
//! Lo que Docker **no** permite cambiar en un contenedor existente (por ejemplo
//! sus etiquetas) no se ofrece: exigiría recrearlo, y esta etapa no recrea
//! contenedores automáticamente (D-119).

use std::collections::BTreeMap;

use bollard::models::{
    ContainerInspectResponse, ContainerUpdateBody, RestartPolicy as DockerRestartPolicy,
    RestartPolicyNameEnum,
};
use bollard::query_parameters::{InspectContainerOptions, RemoveContainerOptions};
use cmc_activity::Category;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::client::{ApiOp, Engine};
use crate::containers::{ContainerAction, ContainerState, ContainerSummary, PortMapping};
use crate::error::EngineError;
use crate::resources::path_with_query;

/// Contenedores como máximo en una operación masiva.
pub const MAX_TARGETS: usize = 1000;
/// Reintentos como máximo de la política `on-failure` que se aceptan.
pub const MAX_RETRY_COUNT: u32 = 1000;

/// Operación masiva.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum BulkOperation {
    Start,
    Stop,
    Restart,
    Pause,
    Unpause,
    /// Eliminar. `force` detiene antes los que están en marcha. Nunca borra
    /// volúmenes ni imágenes (D-120).
    Remove {
        force: bool,
    },
    /// Cambiar la política de reinicio (`docker update --restart`), que Docker
    /// permite cambiar sin recrear el contenedor.
    RestartPolicy {
        policy: RestartPolicy,
    },
}

impl BulkOperation {
    /// Identificador estable (registro de actividad y textos).
    pub fn key(&self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::Pause => "pause",
            Self::Unpause => "unpause",
            Self::Remove { .. } => "remove",
            Self::RestartPolicy { .. } => "restartPolicy",
        }
    }

    /// Operación destructiva: exige una confirmación adicional.
    pub fn is_destructive(&self) -> bool {
        matches!(self, Self::Remove { .. })
    }

    fn lifecycle(&self) -> Option<ContainerAction> {
        match self {
            Self::Start => Some(ContainerAction::Start),
            Self::Stop => Some(ContainerAction::Stop),
            Self::Restart => Some(ContainerAction::Restart),
            Self::Pause => Some(ContainerAction::Pause),
            Self::Unpause => Some(ContainerAction::Unpause),
            _ => None,
        }
    }

    /// Valida los parámetros (no habla con Docker).
    pub fn validate(&self) -> Result<(), EngineError> {
        if let Self::RestartPolicy { policy } = self {
            policy.validate()?;
        }
        Ok(())
    }
}

/// Política de reinicio de Docker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RestartPolicy {
    pub name: RestartPolicyName,
    /// Solo para `on-failure`; 0 = sin límite.
    pub maximum_retry_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum RestartPolicyName {
    No,
    Always,
    UnlessStopped,
    OnFailure,
}

impl RestartPolicyName {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::No => "no",
            Self::Always => "always",
            Self::UnlessStopped => "unless-stopped",
            Self::OnFailure => "on-failure",
        }
    }
}

impl RestartPolicy {
    pub fn validate(&self) -> Result<(), EngineError> {
        if self.name != RestartPolicyName::OnFailure && self.maximum_retry_count != 0 {
            return Err(EngineError::InvalidInput {
                message: "el número máximo de reintentos solo se usa con on-failure".into(),
            });
        }
        if self.maximum_retry_count > MAX_RETRY_COUNT {
            return Err(EngineError::InvalidInput {
                message: format!(
                    "el número máximo de reintentos no puede pasar de {MAX_RETRY_COUNT}"
                ),
            });
        }
        Ok(())
    }

    /// Argumento de `docker update --restart`.
    pub fn cli_value(&self) -> String {
        if self.name == RestartPolicyName::OnFailure && self.maximum_retry_count > 0 {
            format!("on-failure:{}", self.maximum_retry_count)
        } else {
            self.name.as_str().to_owned()
        }
    }

    /// Desde la respuesta de Docker. El nombre vacío equivale a `no`.
    pub fn from_docker(p: Option<&DockerRestartPolicy>) -> Self {
        let name = match p.and_then(|p| p.name) {
            Some(RestartPolicyNameEnum::ALWAYS) => RestartPolicyName::Always,
            Some(RestartPolicyNameEnum::UNLESS_STOPPED) => RestartPolicyName::UnlessStopped,
            Some(RestartPolicyNameEnum::ON_FAILURE) => RestartPolicyName::OnFailure,
            _ => RestartPolicyName::No,
        };
        let retries = if name == RestartPolicyName::OnFailure {
            p.and_then(|p| p.maximum_retry_count)
                .unwrap_or(0)
                .clamp(0, i64::from(u32::MAX)) as u32
        } else {
            0
        };
        Self {
            name,
            maximum_retry_count: retries,
        }
    }

    fn to_docker(self) -> DockerRestartPolicy {
        DockerRestartPolicy {
            name: Some(match self.name {
                RestartPolicyName::No => RestartPolicyNameEnum::NO,
                RestartPolicyName::Always => RestartPolicyNameEnum::ALWAYS,
                RestartPolicyName::UnlessStopped => RestartPolicyNameEnum::UNLESS_STOPPED,
                RestartPolicyName::OnFailure => RestartPolicyNameEnum::ON_FAILURE,
            }),
            maximum_retry_count: (self.name == RestartPolicyName::OnFailure)
                .then_some(i64::from(self.maximum_retry_count)),
        }
    }
}

/// Por qué no se aplica una operación a un contenedor (no es un error: se
/// informa y no se envía nada a Docker).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "reason",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum SkipReason {
    /// La operación no es válida en el estado actual (p. ej. iniciar uno en
    /// ejecución o pausar uno detenido).
    State { state: String },
    /// Eliminar uno activo sin `force`.
    NeedsForce { state: String },
    /// Creado con `--rm` (AutoRemove): Docker no admite otra política de reinicio.
    AutoRemove,
    /// Ya tiene la política pedida.
    Unchanged,
    /// El contenedor ya no existe.
    Gone,
}

/// Comprueba si una operación se puede aplicar a un contenedor en el estado
/// `state` que informa Docker. Las mismas reglas que la lista de contenedores
/// (`allowedActions` en la interfaz).
pub fn applicable(op: &BulkOperation, state: &str) -> Result<(), SkipReason> {
    let allowed: &[&str] = match op {
        BulkOperation::Start => &["created", "exited", "dead"],
        BulkOperation::Stop => &["running", "paused", "restarting"],
        BulkOperation::Restart | BulkOperation::Pause => &["running"],
        BulkOperation::Unpause => &["paused"],
        BulkOperation::Remove { force } => {
            if state == "removing" {
                return Err(SkipReason::State {
                    state: state.into(),
                });
            }
            if is_active(state) && !force {
                return Err(SkipReason::NeedsForce {
                    state: state.into(),
                });
            }
            return Ok(());
        }
        // La política se puede cambiar en cualquier estado salvo mientras se elimina.
        BulkOperation::RestartPolicy { .. } => {
            return if state == "removing" {
                Err(SkipReason::State {
                    state: state.into(),
                })
            } else {
                Ok(())
            };
        }
    };
    if allowed.contains(&state) {
        Ok(())
    } else {
        Err(SkipReason::State {
            state: state.into(),
        })
    }
}

/// En marcha, pausado o reiniciándose.
pub fn is_active(state: &str) -> bool {
    matches!(state, "running" | "paused" | "restarting")
}

/// ID completo de contenedor (64 caracteres hexadecimales en minúsculas). Una
/// operación masiva no acepta nombres ni prefijos: así no puede afectar por
/// error a otro contenedor con un nombre o un prefijo parecido.
pub fn is_full_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Salud según el texto de estado de Docker (`Up 3 minutes (unhealthy)`).
pub fn health_of(status: &str) -> Option<String> {
    ["unhealthy", "health: starting", "healthy"]
        .into_iter()
        .find(|h| status.contains(&format!("({h})")))
        .map(|h| {
            if h == "health: starting" {
                "starting".to_owned()
            } else {
                h.to_owned()
            }
        })
}

/// Código de salida según el texto de estado de Docker (`Exited (137) 2 hours ago`).
pub fn exit_code_of(status: &str) -> Option<i64> {
    let rest = status.strip_prefix("Exited (")?;
    rest.split(')').next()?.trim().parse().ok()
}

/// Contenedor con un problema según lo que informa Docker: eliminado a medias
/// (`dead`), reiniciándose (`restarting`), con la comprobación de salud fallida
/// (`unhealthy`) o detenido con un código de salida distinto de 0. No cuentan
/// 143 (SIGTERM) ni 137 (SIGKILL), que es como termina un contenedor detenido
/// con `docker stop`; por eso un contenedor terminado por falta de memoria
/// (también 137) no se distingue desde la lista.
pub fn has_error(state: &str, status: &str) -> bool {
    match state {
        "dead" | "restarting" => true,
        "exited" => exit_code_of(status).is_some_and(|c| !matches!(c, 0 | 137 | 143)),
        _ => health_of(status).as_deref() == Some("unhealthy"),
    }
}

/// Datos de un contenedor que la lista de Docker no incluye y hacen falta para
/// la gestión masiva (de `inspect`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BulkDetails {
    pub restart_policy: RestartPolicy,
    /// Creado con `--rm`.
    pub auto_remove: bool,
}

impl BulkDetails {
    pub fn from_inspect(r: &ContainerInspectResponse) -> Self {
        let hc = r.host_config.as_ref();
        Self {
            restart_policy: RestartPolicy::from_docker(hc.and_then(|h| h.restart_policy.as_ref())),
            auto_remove: hc.and_then(|h| h.auto_remove).unwrap_or(false),
        }
    }
}

/// Un contenedor en la tabla de la gestión masiva.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BulkContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    /// created, running, paused, restarting, exited, removing, dead.
    pub state: String,
    /// Texto de estado de Docker.
    pub status: String,
    #[ts(type = "number")]
    pub created: i64,
    pub ports: Vec<PortMapping>,
    pub labels: BTreeMap<String, String>,
    pub compose_project: Option<String>,
    #[ts(type = "number | null")]
    pub exit_code: Option<i64>,
    /// healthy, unhealthy, starting.
    pub health: Option<String>,
    /// Ver [`has_error`].
    pub error: bool,
    /// `None` si aún no se ha podido consultar.
    pub details: Option<BulkDetails>,
}

impl BulkContainer {
    pub fn from_summary(c: ContainerSummary, details: Option<BulkDetails>) -> Self {
        Self {
            exit_code: exit_code_of(&c.status),
            health: health_of(&c.status),
            error: has_error(&c.state, &c.status),
            id: c.id,
            name: c.name,
            image: c.image,
            state: c.state,
            status: c.status,
            created: c.created,
            ports: c.ports,
            labels: c.labels,
            compose_project: c.compose_project,
            details,
        }
    }
}

/// Resultado de aplicar la operación a un contenedor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemDone {
    /// Docker la ha ejecutado y el estado posterior está confirmado.
    Applied {
        /// Estado confirmado (`None` si se ha eliminado).
        state: Option<ContainerState>,
        details: Option<BulkDetails>,
    },
    /// No se ha enviado nada a Docker.
    Skipped { reason: SkipReason, state: String },
}

/// En qué momento falló (para saber si el contenedor pudo cambiar).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BulkStage {
    /// Al comprobar el estado: no se ha enviado ninguna orden a Docker.
    Check,
    /// Al ejecutar la orden.
    Execute,
    /// Docker aceptó la orden, pero no se pudo confirmar el estado posterior.
    Confirm,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemError {
    pub stage: BulkStage,
    pub error: EngineError,
}

impl ItemError {
    fn at(stage: BulkStage) -> impl FnOnce(EngineError) -> Self {
        move |error| Self { stage, error }
    }

    /// No se sabe si el contenedor cambió: la orden se envió y Docker no
    /// respondió con un código (corte, tiempo agotado o error del cliente
    /// HTTP), o la aceptó pero no se pudo confirmar el resultado. Solo una
    /// respuesta de Docker con su código, o un fallo antes de enviar nada,
    /// permite decir que no cambió. Nunca se reintenta (podría duplicarla).
    pub fn outcome_unknown(&self) -> bool {
        match self.stage {
            BulkStage::Check => false,
            BulkStage::Execute => !matches!(
                self.error,
                EngineError::Docker { .. }
                    | EngineError::InvalidInput { .. }
                    | EngineError::Busy { .. }
                    | EngineError::NotConnected
            ),
            BulkStage::Confirm => true,
        }
    }
}

impl Engine {
    /// Datos de `inspect` sin registrarlos (se piden en lote al actualizar la
    /// tabla y llenarían el panel de actividad, como la monitorización, D-102).
    pub async fn bulk_details_silent(&self, id: &str) -> Result<BulkDetails, EngineError> {
        let r = self.inspect_silent(id).await?;
        Ok(BulkDetails::from_inspect(&r))
    }

    async fn inspect_silent(&self, id: &str) -> Result<ContainerInspectResponse, EngineError> {
        crate::resources::validate_ref("contenedor", id)?;
        let d = self.docker().clone();
        let id_s = id.to_owned();
        self.call_silent(Some(std::time::Duration::from_secs(15)), async move {
            d.inspect_container(&id_s, Some(InspectContainerOptions { size: false }))
                .await
        })
        .await
    }

    /// Aplica `op` a un contenedor (identificado por su ID completo):
    ///
    /// 1. toma el bloqueo del contenedor (otra operación en curso sobre él →
    ///    `Busy`, nunca se duplica);
    /// 2. consulta su estado real y comprueba que la operación es válida;
    /// 3. la ejecuta con la misma petición registrada que el resto de CMC;
    /// 4. confirma el estado posterior con `inspect` (D-031).
    ///
    /// Nunca reintenta. `display` es el nombre para el registro.
    pub async fn bulk_apply(
        &self,
        op: &BulkOperation,
        id: &str,
        display: &str,
    ) -> Result<ItemDone, ItemError> {
        let check = ItemError::at(BulkStage::Check);
        if !is_full_id(id) {
            return Err(check(EngineError::InvalidInput {
                message: "la gestión masiva necesita el ID completo del contenedor".into(),
            }));
        }
        op.validate().map_err(ItemError::at(BulkStage::Check))?;
        let _lock = self
            .locks()
            .acquire(&format!("container:{id}"))
            .map_err(ItemError::at(BulkStage::Check))?;

        let current = match self.inspect_silent(id).await {
            Ok(r) => r,
            Err(EngineError::Docker { status: 404, .. }) => {
                return Ok(ItemDone::Skipped {
                    reason: SkipReason::Gone,
                    state: String::new(),
                })
            }
            Err(e) => return Err(check(e)),
        };
        // Docker resuelve también nombres y prefijos: se exige que el
        // contenedor encontrado sea exactamente el pedido.
        if current.id.as_deref() != Some(id) {
            return Err(check(EngineError::InvalidResponse {
                message: "Docker devolvió otro contenedor para ese ID".into(),
            }));
        }
        let state = current
            .state
            .as_ref()
            .and_then(|s| s.status)
            .and_then(|s| serde_json::to_value(s).ok())
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        if let Err(reason) = applicable(op, &state) {
            return Ok(ItemDone::Skipped { reason, state });
        }
        let details = BulkDetails::from_inspect(&current);

        match op {
            BulkOperation::Remove { force } => {
                self.remove_unlocked(id, display, *force)
                    .await
                    .map_err(ItemError::at(BulkStage::Execute))?;
                // La eliminación se confirma comprobando que ya no existe.
                match self.inspect_silent(id).await {
                    Err(EngineError::Docker { status: 404, .. }) => Ok(ItemDone::Applied {
                        state: None,
                        details: None,
                    }),
                    Ok(_) => Err(ItemError::at(BulkStage::Confirm)(EngineError::Other {
                        message:
                            "Docker aceptó la eliminación, pero el contenedor sigue existiendo"
                                .into(),
                    })),
                    Err(e) => Err(ItemError::at(BulkStage::Confirm)(e)),
                }
            }
            BulkOperation::RestartPolicy { policy } => {
                if details.auto_remove && policy.name != RestartPolicyName::No {
                    return Ok(ItemDone::Skipped {
                        reason: SkipReason::AutoRemove,
                        state,
                    });
                }
                if details.restart_policy == *policy {
                    return Ok(ItemDone::Skipped {
                        reason: SkipReason::Unchanged,
                        state,
                    });
                }
                let body = ContainerUpdateBody {
                    restart_policy: Some(policy.to_docker()),
                    ..Default::default()
                };
                self.op_update(id, display, body, &["--restart".into(), policy.cli_value()])
                    .await
                    .map_err(ItemError::at(BulkStage::Execute))?;
                let after = self
                    .bulk_details_silent(id)
                    .await
                    .map_err(ItemError::at(BulkStage::Confirm))?;
                if after.restart_policy != *policy {
                    return Err(ItemError::at(BulkStage::Confirm)(EngineError::Other {
                        message: format!(
                            "Docker aceptó el cambio, pero la política es {}",
                            after.restart_policy.cli_value()
                        ),
                    }));
                }
                let st = self
                    .container_state(id, Some(display))
                    .await
                    .map_err(ItemError::at(BulkStage::Confirm))?;
                Ok(ItemDone::Applied {
                    state: Some(st),
                    details: Some(after),
                })
            }
            lifecycle => {
                let action = lifecycle.lifecycle().expect("operación de ciclo de vida");
                self.run_action(id, Some(display), action)
                    .await
                    .map_err(ItemError::at(BulkStage::Execute))?;
                let st = self
                    .container_state(id, Some(display))
                    .await
                    .map_err(ItemError::at(BulkStage::Confirm))?;
                Ok(ItemDone::Applied {
                    state: Some(st),
                    details: Some(details),
                })
            }
        }
    }

    /// `DELETE /containers/{id}` sin tomar el bloqueo (lo tiene quien llama).
    /// Nunca borra volúmenes (`v=false`).
    async fn remove_unlocked(
        &self,
        id: &str,
        display: &str,
        force: bool,
    ) -> Result<(), EngineError> {
        let opts = RemoveContainerOptions {
            v: false,
            force,
            link: false,
        };
        let mut eq = vec!["rm".to_owned()];
        if force {
            eq.push("--force".into());
        }
        eq.push(id.into());
        let op = ApiOp {
            category: Category::Container,
            kind: "container.remove",
            target: Some(display.to_owned()),
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

#[cfg(test)]
#[path = "bulk_tests.rs"]
mod tests;
