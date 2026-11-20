// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Gestión masiva (1.10.0, D-117 a D-120): coordinación de una operación sobre
//! varios contenedores, que pueden estar en distintos servidores conectados.
//!
//! - Cada contenedor se identifica por **(servidor, ID completo)**: los ID de
//!   servidores distintos nunca se mezclan.
//! - Concurrencia limitada (`BulkSettings::concurrency`), cancelación de lo que
//!   aún no ha empezado y un resultado independiente por contenedor: no hay
//!   transacción global, porque Docker no la ofrece.
//! - Sin reintentos: un contenedor cuyo resultado no se conoce (corte durante
//!   la orden) se informa como «desconocido», nunca se vuelve a enviar.
//! - Un contenedor no puede estar en dos operaciones masivas a la vez
//!   ([`BulkJobs`]); además, cada orden toma el bloqueo del contenedor del motor.
//! - Todo queda en el registro de actividad: un resumen (`container.bulk`) y
//!   cada petición real, con el mismo identificador de lote.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, Utc};
use cmc_activity::{ActivityLog, Category, OperationSpec, Outcome, Transport};
use cmc_engine::bulk::{
    BulkContainer, BulkDetails, BulkOperation, BulkStage, ItemDone, SkipReason, MAX_TARGETS,
};
use cmc_engine::{ConnectionState, EngineError};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;
use uuid::Uuid;

use crate::error::{CmdResult, CommandError};

/// Un contenedor de una operación masiva.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BulkTarget {
    /// Servidor (`local` o el identificador de un perfil remoto).
    pub server: String,
    /// ID completo del contenedor en ese servidor.
    pub id: String,
    /// Nombre (para el registro y los resultados).
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BulkRequest {
    pub operation: BulkOperation,
    pub targets: Vec<BulkTarget>,
    /// Confirmación del diálogo (obligatoria).
    pub confirmed: bool,
    /// Eliminar exige además escribir el número de contenedores (segunda
    /// confirmación, comprobada también aquí).
    pub confirm_count: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BulkItemStatus {
    /// Docker la ejecutó y el estado posterior está confirmado.
    Succeeded,
    /// Falló sin cambiar el contenedor (Docker la rechazó o no se pudo enviar).
    Failed,
    /// No se aplicaba (estado incompatible, sin cambios…): no se envió nada.
    Skipped,
    /// Cancelada antes de empezar.
    Cancelled,
    /// Se envió, pero no se sabe si se aplicó: hay que comprobarlo.
    Unknown,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BulkItemResult {
    /// Posición en la petición.
    pub index: u32,
    pub target: BulkTarget,
    pub status: BulkItemStatus,
    /// Estado confirmado después (`None` si se eliminó o no se conoce).
    pub state: Option<String>,
    pub skip: Option<SkipReason>,
    pub error: Option<CommandError>,
    /// Momento del fallo.
    pub stage: Option<BulkStage>,
    pub details: Option<BulkDetails>,
    #[ts(type = "number")]
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BulkCounts {
    pub succeeded: u32,
    pub failed: u32,
    pub skipped: u32,
    pub cancelled: u32,
    pub unknown: u32,
}

impl BulkCounts {
    pub fn of(results: &[BulkItemResult]) -> Self {
        let mut c = Self::default();
        for r in results {
            match r.status {
                BulkItemStatus::Succeeded => c.succeeded += 1,
                BulkItemStatus::Failed => c.failed += 1,
                BulkItemStatus::Skipped => c.skipped += 1,
                BulkItemStatus::Cancelled => c.cancelled += 1,
                BulkItemStatus::Unknown => c.unknown += 1,
            }
        }
        c
    }

    /// Todo lo que se intentó salió bien (lo omitido no es un fallo).
    pub fn clean(&self) -> bool {
        self.failed == 0 && self.unknown == 0 && self.cancelled == 0
    }

    pub fn text(&self) -> String {
        let mut parts = vec![format!("{} correctas", self.succeeded)];
        for (n, w) in [
            (self.failed, "fallidas"),
            (self.unknown, "con resultado desconocido"),
            (self.skipped, "omitidas"),
            (self.cancelled, "canceladas"),
        ] {
            if n > 0 {
                parts.push(format!("{n} {w}"));
            }
        }
        parts.join(", ")
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BulkSummary {
    /// Identificador de la operación masiva (también en el registro).
    pub batch: Uuid,
    pub operation: BulkOperation,
    pub total: u32,
    pub counts: BulkCounts,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    #[ts(type = "number")]
    pub duration_ms: u64,
    pub results: Vec<BulkItemResult>,
}

/// Progreso que recibe la interfaz.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum BulkEvent {
    Started {
        batch: Uuid,
        total: u32,
        concurrency: u8,
    },
    /// Empieza el contenedor `index`.
    Running {
        index: u32,
    },
    Item {
        result: Box<BulkItemResult>,
    },
    Finished {
        summary: Box<BulkSummary>,
    },
}

// ------------------------------------------------------------- validación

/// Valida la petición antes de tocar nada.
pub fn validate(req: &BulkRequest, server_exists: impl Fn(&str) -> bool) -> CmdResult<()> {
    if !req.confirmed {
        return Err(EngineError::NotConfirmed.into());
    }
    if req.operation.is_destructive() && req.confirm_count != Some(req.targets.len() as u32) {
        return Err(CommandError::new(
            "bulk.notConfirmed",
            "para eliminar hay que escribir el número de contenedores afectados",
        ));
    }
    req.operation.validate()?;
    validate_targets(&req.targets, server_exists)
}

/// Destinos de una operación o de un comando masivo: entre 1 y
/// `MAX_TARGETS`, con el ID completo, de un servidor conocido y sin repetir
/// el mismo (servidor, ID).
pub fn validate_targets(
    targets: &[BulkTarget],
    server_exists: impl Fn(&str) -> bool,
) -> CmdResult<()> {
    let invalid = |m: &str| CommandError::new("bulk.invalid", m);
    if targets.is_empty() {
        return Err(invalid("no hay contenedores seleccionados"));
    }
    if targets.len() > MAX_TARGETS {
        return Err(invalid(&format!(
            "una operación masiva admite como máximo {MAX_TARGETS} contenedores"
        )));
    }
    let mut seen = HashSet::new();
    for t in targets {
        if !cmc_engine::bulk::is_full_id(&t.id) {
            return Err(invalid(&format!("{}: hace falta el ID completo", t.name)));
        }
        if !server_exists(&t.server) {
            return Err(CommandError::new(
                "server.unknown",
                format!("servidor desconocido: {}", t.server),
            ));
        }
        if !seen.insert((t.server.as_str(), t.id.as_str())) {
            return Err(invalid(&format!("{} aparece dos veces", t.name)));
        }
    }
    Ok(())
}

// ----------------------------------------------- contenedores en operación

/// Contenedores que forman parte de una operación masiva en curso.
#[derive(Default, Clone)]
pub struct BulkJobs {
    active: Arc<Mutex<HashSet<(String, String)>>>,
}

/// Mientras exista, los contenedores están reservados.
pub struct Reservation {
    jobs: BulkJobs,
    keys: Vec<(String, String)>,
}

impl Drop for Reservation {
    fn drop(&mut self) {
        let mut a = self.jobs.active.lock();
        for k in &self.keys {
            a.remove(k);
        }
    }
}

impl BulkJobs {
    /// Reserva todos los contenedores o ninguno. Si alguno ya está en otra
    /// operación masiva, devuelve `bulk.busy` con sus nombres.
    pub fn reserve(&self, targets: &[BulkTarget]) -> CmdResult<Reservation> {
        let mut a = self.active.lock();
        let busy: Vec<&str> = targets
            .iter()
            .filter(|t| a.contains(&(t.server.clone(), t.id.clone())))
            .map(|t| t.name.as_str())
            .collect();
        if !busy.is_empty() {
            return Err(CommandError::new(
                "bulk.busy",
                format!(
                    "ya hay una operación masiva en curso sobre: {}",
                    busy.join(", ")
                ),
            ));
        }
        let keys: Vec<_> = targets
            .iter()
            .map(|t| (t.server.clone(), t.id.clone()))
            .collect();
        a.extend(keys.iter().cloned());
        Ok(Reservation {
            jobs: self.clone(),
            keys,
        })
    }
}

// --------------------------------------------------------------- ejecución

fn cancelled(index: u32, target: BulkTarget) -> BulkItemResult {
    BulkItemResult {
        index,
        target,
        status: BulkItemStatus::Cancelled,
        state: None,
        skip: None,
        error: None,
        stage: None,
        details: None,
        duration_ms: 0,
    }
}

/// Ejecuta `exec` para cada destino con como mucho `concurrency` a la vez
/// (común a las operaciones y a los comandos masivos, 1.11.0). Lo que no ha
/// empezado cuando se cancela no empieza (`cancelled`). `on_running` y
/// `on_item` informan del progreso. Los resultados vuelven en el orden de la
/// petición.
pub async fn run_pool<R, F, Fut>(
    targets: Vec<BulkTarget>,
    concurrency: u8,
    cancel: CancellationToken,
    exec: F,
    on_running: impl Fn(u32) + Send + Sync + 'static,
    on_item: impl Fn(&R) + Send + Sync + 'static,
) -> Vec<R>
where
    R: PoolItem,
    F: Fn(u32, BulkTarget) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = R> + Send + 'static,
{
    let sem = Arc::new(Semaphore::new(usize::from(concurrency.max(1))));
    let exec = Arc::new(exec);
    let on_running = Arc::new(on_running);
    let on_item = Arc::new(on_item);
    let mut set = tokio::task::JoinSet::new();
    for (i, t) in targets.into_iter().enumerate() {
        let index = i as u32;
        let (sem, exec, on_running, on_item, cancel) = (
            sem.clone(),
            exec.clone(),
            on_running.clone(),
            on_item.clone(),
            cancel.clone(),
        );
        set.spawn(async move {
            let permit = tokio::select! {
                // `biased`: si ya se ha cancelado, no se empieza aunque haya sitio.
                biased;
                _ = cancel.cancelled() => None,
                p = sem.acquire_owned() => p.ok(),
            };
            let r = match permit {
                Some(_permit) if !cancel.is_cancelled() => {
                    on_running(index);
                    exec(index, t).await
                }
                _ => R::cancelled(index, t),
            };
            on_item(&r);
            r
        });
    }
    let mut out = Vec::new();
    while let Some(r) = set.join_next().await {
        match r {
            Ok(r) => out.push(r),
            // Una tarea que entra en pánico no debe ocultar el resto.
            Err(e) => tracing::error!("tarea de la operación masiva: {e}"),
        }
    }
    out.sort_by_key(R::index);
    out
}

/// Resultado de un elemento del grupo de ejecución.
pub trait PoolItem: Send + 'static {
    fn index(&self) -> u32;
    /// Resultado de uno que no llegó a empezar por la cancelación.
    fn cancelled(index: u32, target: BulkTarget) -> Self;
}

impl PoolItem for BulkItemResult {
    fn index(&self) -> u32 {
        self.index
    }

    fn cancelled(index: u32, target: BulkTarget) -> Self {
        cancelled(index, target)
    }
}

/// Operaciones masivas sobre [`run_pool`], con los eventos de la interfaz.
pub async fn run_items<F, Fut>(
    targets: Vec<BulkTarget>,
    concurrency: u8,
    cancel: CancellationToken,
    exec: F,
    on_event: impl Fn(BulkEvent) + Send + Sync + 'static,
) -> Vec<BulkItemResult>
where
    F: Fn(u32, BulkTarget) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = BulkItemResult> + Send + 'static,
{
    let on_event = Arc::new(on_event);
    let ev = on_event.clone();
    run_pool(
        targets,
        concurrency,
        cancel,
        exec,
        move |index| ev(BulkEvent::Running { index }),
        move |r| {
            on_event(BulkEvent::Item {
                result: Box::new(r.clone()),
            })
        },
    )
    .await
}

/// Convierte el resultado del motor en el de la interfaz.
pub fn item_result(
    index: u32,
    target: BulkTarget,
    started: Instant,
    r: Result<ItemDone, cmc_engine::bulk::ItemError>,
) -> BulkItemResult {
    let duration_ms = started.elapsed().as_millis() as u64;
    let base = BulkItemResult {
        index,
        target,
        status: BulkItemStatus::Succeeded,
        state: None,
        skip: None,
        error: None,
        stage: None,
        details: None,
        duration_ms,
    };
    match r {
        Ok(ItemDone::Applied { state, details }) => BulkItemResult {
            state: state.map(|s| s.status),
            details,
            ..base
        },
        Ok(ItemDone::Skipped { reason, state }) => BulkItemResult {
            status: BulkItemStatus::Skipped,
            state: (!state.is_empty()).then_some(state),
            skip: Some(reason),
            ..base
        },
        Err(e) => BulkItemResult {
            status: if e.outcome_unknown() {
                BulkItemStatus::Unknown
            } else {
                BulkItemStatus::Failed
            },
            stage: Some(e.stage),
            error: Some(e.error.into()),
            ..base
        },
    }
}

/// Fallo antes de llegar a Docker (servidor desconocido o sin conexión).
pub fn not_sent(index: u32, target: BulkTarget, error: CommandError) -> BulkItemResult {
    BulkItemResult {
        index,
        target,
        status: BulkItemStatus::Failed,
        state: None,
        skip: None,
        error: Some(error),
        stage: Some(BulkStage::Check),
        details: None,
        duration_ms: 0,
    }
}

/// Texto del resumen en el registro (en castellano, como el resto del registro).
pub fn describe(op: &BulkOperation, n: usize) -> String {
    let what = match op {
        BulkOperation::Start => "iniciar".to_owned(),
        BulkOperation::Stop => "detener".to_owned(),
        BulkOperation::Restart => "reiniciar".to_owned(),
        BulkOperation::Pause => "pausar".to_owned(),
        BulkOperation::Unpause => "reanudar".to_owned(),
        BulkOperation::Remove { force: false } => "eliminar".to_owned(),
        BulkOperation::Remove { force: true } => "eliminar (forzando)".to_owned(),
        BulkOperation::RestartPolicy { policy } => {
            format!("cambiar la política de reinicio a {}", policy.cli_value())
        }
    };
    format!("Operación masiva: {what} {n} contenedor(es)")
}

/// Abre el resumen en el registro de actividad, etiquetado con el lote.
pub fn begin_summary(
    activity: &ActivityLog,
    batch: Uuid,
    req: &BulkRequest,
    concurrency: u8,
) -> cmc_activity::OperationHandle {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Body<'a> {
        operation: &'a BulkOperation,
        concurrency: u8,
        targets: &'a [BulkTarget],
    }
    let body = serde_json::to_string_pretty(&Body {
        operation: &req.operation,
        concurrency,
        targets: &req.targets,
    })
    .ok();
    activity.with_batch(batch).begin(OperationSpec {
        category: Category::Container,
        kind: "container.bulk".into(),
        target: Some(format!("{} contenedor(es)", req.targets.len())),
        transport: Transport::App {
            action: describe(&req.operation, req.targets.len()),
        },
        request_body: body,
        equivalent_command: None,
    })
}

pub fn finish_summary(handle: cmc_activity::OperationHandle, counts: &BulkCounts) {
    if counts.clean() {
        handle.succeed(Outcome {
            summary: Some(counts.text()),
            ..Default::default()
        });
    } else {
        handle.fail(cmc_activity::Failure {
            error: counts.text(),
            ..Default::default()
        });
    }
}

// -------------------------------------------------------------- inventario

/// Datos de `inspect` ya consultados, por (servidor, ID). Evita repetir una
/// consulta por contenedor en cada actualización automática; «Actualizar»
/// los vuelve a pedir todos.
#[derive(Default)]
pub struct DetailsCache {
    map: Mutex<HashMap<(String, String), BulkDetails>>,
}

impl DetailsCache {
    pub fn get(&self, server: &str, id: &str) -> Option<BulkDetails> {
        self.map
            .lock()
            .get(&(server.to_owned(), id.to_owned()))
            .copied()
    }
    pub fn set(&self, server: &str, id: &str, d: BulkDetails) {
        self.map
            .lock()
            .insert((server.to_owned(), id.to_owned()), d);
    }
    pub fn remove(&self, server: &str, id: &str) {
        self.map.lock().remove(&(server.to_owned(), id.to_owned()));
    }
    /// Quita los contenedores de `server` que ya no existen.
    pub fn retain(&self, server: &str, ids: &HashSet<&str>) {
        self.map
            .lock()
            .retain(|(s, id), _| s != server || ids.contains(id.as_str()));
    }
}

/// Contenedores de un servidor.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BulkEnvironment {
    pub server: String,
    pub remote: bool,
    /// Nombre del perfil; vacío para este equipo (la interfaz lo traduce).
    pub name: String,
    pub address: String,
    /// `connected`, `connecting` o `disconnected`.
    pub connection: String,
    pub containers: Vec<BulkContainer>,
    /// Error al listar (servidor conectado que no respondió).
    pub error: Option<CommandError>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BulkInventory {
    pub fetched_at: DateTime<Utc>,
    pub environments: Vec<BulkEnvironment>,
}

pub fn connection_label(s: &ConnectionState) -> &'static str {
    match s {
        ConnectionState::Connected { .. } => "connected",
        ConnectionState::Connecting => "connecting",
        _ => "disconnected",
    }
}

#[cfg(test)]
#[path = "bulk_tests.rs"]
mod tests;
