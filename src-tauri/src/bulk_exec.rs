// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Comandos masivos (1.11.0, D-121 a D-124): coordinación de un mismo comando
//! dentro de varios contenedores.
//!
//! Reutiliza lo de las operaciones masivas: destinos (servidor, ID completo),
//! validación, reservas para no duplicar, el grupo de ejecución con
//! concurrencia limitada y el lote en la actividad. Lo propio de los comandos:
//!
//! - el comando, el usuario, el directorio y el tiempo máximo se validan una
//!   vez para todos; la concurrencia la elige quien lo ejecuta (1–16);
//! - el nivel de riesgo lo calcula el backend; desde «administrativo» hay que
//!   escribir el número de contenedores, como al eliminar;
//! - dos cancelaciones distintas: la de lo pendiente (no se lanza nada más) y
//!   la interrupción de lo activo (se deja de esperar; Docker Exec no permite
//!   detener el proceso, que puede seguir en el contenedor).

use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, Utc};
use cmc_activity::{ActivityLog, Category, OperationSpec, Outcome, Transport};
use cmc_engine::bulk::{BulkStage, ItemError, SkipReason};
use cmc_engine::bulk_exec::{
    classify, BulkCommand, CommandRisk, CommandRiskLevel, ExecDone, ExecEnd, ExecOutput,
};
use cmc_engine::EngineError;
use cmc_store::{ExecTemplate, ExecTemplateMode};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;
use uuid::Uuid;

use crate::bulk::{validate_targets, BulkTarget};
use crate::error::{CmdResult, CommandError};

pub const MIN_CONCURRENCY: u8 = 1;
pub const MAX_CONCURRENCY: u8 = 16;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BulkExecRequest {
    pub command: BulkCommand,
    pub targets: Vec<BulkTarget>,
    /// Ejecuciones a la vez (1–16).
    pub concurrency: u8,
    /// Confirmación del diálogo (obligatoria).
    pub confirmed: bool,
    /// Desde el nivel «administrativo», el número de contenedores escrito.
    pub confirm_count: Option<u32>,
}

/// Lo que verá la confirmación: los argumentos exactos, el comando
/// equivalente y el riesgo. Lo calcula el backend (un único lugar).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExecPreview {
    pub argv: Vec<String>,
    /// `docker exec …` equivalente, con `<contenedor>` en lugar del ID.
    pub equivalent: String,
    pub risk: CommandRisk,
    /// Hay que escribir el número de contenedores para confirmar.
    pub needs_count: bool,
}

pub fn preview(cmd: &BulkCommand) -> CmdResult<ExecPreview> {
    cmd.validate()?;
    let risk = classify(cmd);
    let args = cmd.exec_args("<contenedor>");
    Ok(ExecPreview {
        argv: cmd.argv(),
        equivalent: format!(
            "docker {}",
            cmc_activity::quote::render_argv(&args[0], &args[1..])
        ),
        needs_count: needs_count(&risk),
        risk,
    })
}

pub fn needs_count(risk: &CommandRisk) -> bool {
    risk.level >= CommandRiskLevel::Administrative
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ExecItemStatus {
    /// Terminó con código 0.
    Succeeded,
    /// Terminó con otro código, Docker no pudo lanzarlo o rechazó la ejecución.
    Failed,
    /// Se agotó el tiempo máximo (el proceso puede seguir).
    TimedOut,
    /// Se pidió la interrupción (el proceso puede seguir).
    Interrupted,
    /// No se ejecutó: no está en ejecución, ya no existe o se canceló antes.
    NotRun,
    /// Se perdió la conexión durante la ejecución: pudo ejecutarse total o
    /// parcialmente. Nunca se reintenta.
    Unknown,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExecItemResult {
    pub index: u32,
    pub target: BulkTarget,
    pub status: ExecItemStatus,
    pub output: Option<ExecOutput>,
    /// Por qué no se ejecutó (estado del contenedor o ya no existe).
    pub skip: Option<SkipReason>,
    /// Cancelado antes de empezar.
    pub cancelled: bool,
    pub error: Option<CommandError>,
    pub stage: Option<BulkStage>,
    #[ts(type = "number")]
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExecCounts {
    pub succeeded: u32,
    pub failed: u32,
    pub timed_out: u32,
    pub interrupted: u32,
    pub not_run: u32,
    pub unknown: u32,
}

impl ExecCounts {
    pub fn of(results: &[ExecItemResult]) -> Self {
        let mut c = Self::default();
        for r in results {
            match r.status {
                ExecItemStatus::Succeeded => c.succeeded += 1,
                ExecItemStatus::Failed => c.failed += 1,
                ExecItemStatus::TimedOut => c.timed_out += 1,
                ExecItemStatus::Interrupted => c.interrupted += 1,
                ExecItemStatus::NotRun => c.not_run += 1,
                ExecItemStatus::Unknown => c.unknown += 1,
            }
        }
        c
    }

    pub fn clean(&self) -> bool {
        self.failed + self.timed_out + self.interrupted + self.not_run + self.unknown == 0
    }

    pub fn text(&self) -> String {
        let mut parts = vec![format!("{} correctos", self.succeeded)];
        for (n, w) in [
            (self.failed, "con error"),
            (self.timed_out, "con tiempo agotado"),
            (self.interrupted, "interrumpidos"),
            (self.unknown, "con resultado desconocido"),
            (self.not_run, "no ejecutados"),
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
pub struct ExecSummary {
    pub batch: Uuid,
    pub command: BulkCommand,
    pub total: u32,
    pub counts: ExecCounts,
    #[ts(type = "string")]
    pub started_at: DateTime<Utc>,
    #[ts(type = "string")]
    pub finished_at: DateTime<Utc>,
    #[ts(type = "number")]
    pub duration_ms: u64,
    pub results: Vec<ExecItemResult>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ExecEvent {
    Started {
        batch: Uuid,
        total: u32,
        concurrency: u8,
    },
    Running {
        index: u32,
    },
    Item {
        result: Box<ExecItemResult>,
    },
    Finished {
        summary: Box<ExecSummary>,
    },
}

/// Valida la petición antes de tocar nada.
pub fn validate(req: &BulkExecRequest, server_exists: impl Fn(&str) -> bool) -> CmdResult<()> {
    if !req.confirmed {
        return Err(EngineError::NotConfirmed.into());
    }
    if !(MIN_CONCURRENCY..=MAX_CONCURRENCY).contains(&req.concurrency) {
        return Err(CommandError::new(
            "bulk.invalid",
            format!("la concurrencia debe estar entre {MIN_CONCURRENCY} y {MAX_CONCURRENCY}"),
        ));
    }
    req.command.validate()?;
    if needs_count(&classify(&req.command)) && req.confirm_count != Some(req.targets.len() as u32) {
        return Err(CommandError::new(
            "bulk.notConfirmed",
            "para un comando de riesgo hay que escribir el número de contenedores",
        ));
    }
    validate_targets(&req.targets, server_exists)
}

/// Resultado de la interfaz a partir del del motor.
pub fn item_result(
    index: u32,
    target: BulkTarget,
    started: Instant,
    r: Result<ExecDone, ItemError>,
) -> ExecItemResult {
    let base = ExecItemResult {
        index,
        target,
        status: ExecItemStatus::Succeeded,
        output: None,
        skip: None,
        cancelled: false,
        error: None,
        stage: None,
        duration_ms: started.elapsed().as_millis() as u64,
    };
    match r {
        Ok(ExecDone::NotRun { reason }) => ExecItemResult {
            status: ExecItemStatus::NotRun,
            skip: Some(reason),
            ..base
        },
        Ok(ExecDone::Ran(o)) => {
            let status = match (o.end, o.exit_code) {
                (ExecEnd::TimedOut, _) => ExecItemStatus::TimedOut,
                (ExecEnd::Interrupted, _) => ExecItemStatus::Interrupted,
                (ExecEnd::Exited, Some(0)) if !o.launch_failed => ExecItemStatus::Succeeded,
                _ => ExecItemStatus::Failed,
            };
            ExecItemResult {
                status,
                output: Some(o),
                ..base
            }
        }
        Err(e) => {
            let unknown = e.outcome_unknown();
            ExecItemResult {
                status: if unknown {
                    ExecItemStatus::Unknown
                } else {
                    ExecItemStatus::Failed
                },
                stage: Some(e.stage),
                error: Some(e.error.into()),
                ..base
            }
        }
    }
}

pub fn not_sent(index: u32, target: BulkTarget, error: CommandError) -> ExecItemResult {
    ExecItemResult {
        index,
        target,
        status: ExecItemStatus::Failed,
        output: None,
        skip: None,
        cancelled: false,
        error: Some(error),
        stage: Some(BulkStage::Check),
        duration_ms: 0,
    }
}

impl crate::bulk::PoolItem for ExecItemResult {
    fn index(&self) -> u32 {
        self.index
    }

    fn cancelled(index: u32, target: BulkTarget) -> Self {
        cancelled(index, target)
    }
}

pub fn cancelled(index: u32, target: BulkTarget) -> ExecItemResult {
    ExecItemResult {
        index,
        target,
        status: ExecItemStatus::NotRun,
        output: None,
        skip: None,
        cancelled: true,
        error: None,
        stage: None,
        duration_ms: 0,
    }
}

/// Registro resumen (`container.exec.bulk`): el comando, el usuario, el
/// directorio, el tiempo máximo, el riesgo, la concurrencia y los destinos. El
/// registro de actividad oculta los secretos que pudiera llevar el comando.
pub fn begin_summary(
    log: &ActivityLog,
    batch: Uuid,
    req: &BulkExecRequest,
) -> cmc_activity::OperationHandle {
    let risk = classify(&req.command);
    let body = serde_json::json!({
        "argv": req.command.argv(),
        "user": req.command.user(),
        "workdir": req.command.workdir(),
        "timeoutSecs": req.command.timeout_secs,
        "risk": risk.level,
        "concurrency": req.concurrency,
        "targets": req.targets.iter().map(|t| serde_json::json!({
            "server": t.server, "id": t.id, "name": t.name,
        })).collect::<Vec<_>>(),
    });
    log.with_batch(batch).begin(OperationSpec {
        category: Category::Container,
        kind: "container.exec.bulk".into(),
        target: Some(format!("{} contenedor(es)", req.targets.len())),
        transport: Transport::App {
            action: format!(
                "Comando masivo en {} contenedor(es): {}",
                req.targets.len(),
                redacted_command(&req.command)
            ),
        },
        request_body: Some(serde_json::to_string_pretty(&body).unwrap_or_default()),
        equivalent_command: None,
    })
}

/// Comando para mostrar con los secretos ocultos (argumentos `--password`,
/// `NOMBRE=valor`, URL con contraseña…).
pub fn redacted_command(cmd: &BulkCommand) -> String {
    let argv = cmc_activity::redact::redact_args(&cmd.argv());
    let text = match argv.split_first() {
        Some((p, rest)) => cmc_activity::quote::render_argv(p, rest),
        None => String::new(),
    };
    cmc_activity::redact::redact_text(&text).into_owned()
}

pub fn finish_summary(handle: cmc_activity::OperationHandle, counts: &ExecCounts) {
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

/// Interrupciones de las ejecuciones activas, por flujo.
#[derive(Default, Clone)]
pub struct ExecInterrupts {
    inner: Arc<Mutex<HashMap<u64, CancellationToken>>>,
}

impl ExecInterrupts {
    pub fn insert(&self, sid: u64, token: CancellationToken) {
        self.inner.lock().insert(sid, token);
    }

    pub fn remove(&self, sid: u64) {
        self.inner.lock().remove(&sid);
    }

    /// `true` si había una ejecución con ese flujo.
    pub fn interrupt(&self, sid: u64) -> bool {
        match self.inner.lock().get(&sid) {
            Some(t) => {
                t.cancel();
                true
            }
            None => false,
        }
    }
}

// ------------------------------------------------------------- plantillas

/// Comando de una plantilla (para validarla igual que un comando real).
pub fn template_command(t: &ExecTemplate) -> BulkCommand {
    use cmc_engine::bulk_exec::ExecCommand;
    BulkCommand {
        command: match t.mode {
            ExecTemplateMode::Direct => ExecCommand::Direct {
                program: t.program.clone(),
                args: t.args.clone(),
            },
            ExecTemplateMode::Script => ExecCommand::Script {
                interpreter: t.interpreter.clone(),
                flag: t.flag.clone(),
                script: t.script.clone(),
            },
        },
        user: Some(t.user.clone()),
        workdir: Some(t.workdir.clone()),
        timeout_secs: t.timeout_secs,
    }
}

/// Valida las plantillas que se van a guardar: nombre, comando válido y **sin
/// secretos** (si las reglas de ocultación cambiarían algo, se rechaza).
pub fn validate_templates(list: &[ExecTemplate]) -> CmdResult<()> {
    use cmc_store::ExecTemplates;
    let invalid = |m: String| CommandError::new("exec.templateInvalid", m);
    if list.len() > ExecTemplates::MAX_TEMPLATES {
        return Err(invalid(format!(
            "como máximo {} comandos guardados",
            ExecTemplates::MAX_TEMPLATES
        )));
    }
    let mut ids = std::collections::HashSet::new();
    for t in list {
        let name = t.name.trim();
        if name.is_empty() || name.chars().count() > ExecTemplates::MAX_NAME {
            return Err(invalid(format!(
                "el nombre es obligatorio y como máximo de {} caracteres",
                ExecTemplates::MAX_NAME
            )));
        }
        if t.description.chars().count() > ExecTemplates::MAX_DESCRIPTION {
            return Err(invalid(format!(
                "{name}: la descripción es demasiado larga"
            )));
        }
        if t.id.trim().is_empty() || !ids.insert(t.id.as_str()) {
            return Err(invalid(format!("{name}: identificador repetido o vacío")));
        }
        template_command(t)
            .validate()
            .map_err(|e| invalid(format!("{name}: {e}")))?;
        let fields = [
            t.program.as_str(),
            t.interpreter.as_str(),
            t.script.as_str(),
            t.user.as_str(),
            t.workdir.as_str(),
            t.description.as_str(),
        ];
        let secret = fields
            .iter()
            .any(|f| cmc_activity::redact::redact_text(f) != *f)
            || cmc_activity::redact::redact_args(&t.args) != t.args;
        if secret {
            return Err(CommandError::new(
                "exec.templateSecret",
                format!(
                    "{name}: parece contener una contraseña, un token u otro secreto; no se guarda"
                ),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "bulk_exec_tests.rs"]
mod tests;
