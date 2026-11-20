// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Registro de actividad en memoria, seguro entre hilos, con notificación a suscriptores.

use std::collections::VecDeque;
use std::sync::Arc;

use chrono::Utc;
use parking_lot::Mutex;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::model::ServerTag;
use crate::model::{ActivityEvent, CapturedText, Category, OpStatus, OperationRecord, Transport};
use crate::redact::{redact_api_path, redact_args, redact_text};

/// Texto que se guarda o se emite: siempre sin secretos (E1, D-085).
fn clean(s: String) -> String {
    match redact_text(&s) {
        std::borrow::Cow::Borrowed(_) => s,
        std::borrow::Cow::Owned(o) => o,
    }
}

/// Salida capturada sin secretos y recortada a `limit` bytes. Primero se oculta
/// y después se recorta, para no partir un secreto y dejar media parte visible.
fn clean_capture(s: String, limit: usize) -> CapturedText {
    capture(clean(s), limit)
}

fn clean_transport(t: Transport) -> Transport {
    match t {
        Transport::Api {
            method,
            path,
            endpoint,
        } => Transport::Api {
            method,
            path: redact_api_path(&path, false),
            endpoint,
        },
        Transport::Cli { program, args } => Transport::Cli {
            program,
            args: redact_args(&args),
        },
        other => other,
    }
}

/// Límite por defecto de bytes guardados de cada salida capturada.
pub const DEFAULT_CAPTURE_LIMIT: usize = 64 * 1024;
/// Número de registros por defecto que se conservan en memoria.
pub const DEFAULT_CAPACITY: usize = 500;

/// Recorta un texto a `limit` bytes respetando los límites de carácter UTF-8.
pub fn capture(text: impl Into<String>, limit: usize) -> CapturedText {
    let mut text = text.into();
    let original = text.len();
    let truncated = original > limit;
    if truncated {
        let mut cut = limit;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
    }
    CapturedText {
        text,
        truncated,
        original_bytes: original as u64,
    }
}

/// Descripción de una operación que va a ejecutarse.
#[derive(Debug, Clone)]
pub struct OperationSpec {
    pub category: Category,
    pub kind: String,
    pub target: Option<String>,
    pub transport: Transport,
    pub request_body: Option<String>,
    pub equivalent_command: Option<String>,
}

/// Resultado de una operación correcta.
#[derive(Debug, Clone, Default)]
pub struct Outcome {
    pub summary: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub http_status: Option<u16>,
    pub exit_code: Option<i32>,
}

/// Resultado de una operación fallida.
#[derive(Debug, Clone, Default)]
pub struct Failure {
    pub error: String,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub http_status: Option<u16>,
    pub exit_code: Option<i32>,
}

struct Inner {
    records: VecDeque<OperationRecord>,
    capacity: usize,
    capture_limit: usize,
}

/// Registro compartido. Clonar es barato (`Arc`).
#[derive(Clone)]
pub struct ActivityLog {
    inner: Arc<Mutex<Inner>>,
    tx: broadcast::Sender<ActivityEvent>,
    /// Servidor con el que se etiquetan las operaciones de esta vista (0.10.0).
    server: Option<ServerTag>,
    /// Operación masiva con la que se etiquetan (1.10.0, D-117).
    batch: Option<Uuid>,
}

impl Default for ActivityLog {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

impl ActivityLog {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            inner: Arc::new(Mutex::new(Inner {
                records: VecDeque::with_capacity(capacity.min(4096)),
                capacity: capacity.max(1),
                capture_limit: DEFAULT_CAPTURE_LIMIT,
            })),
            tx,
            server: None,
            batch: None,
        }
    }

    /// Vista del mismo registro que etiqueta con `server` cada operación que
    /// empieza. Comparte almacenamiento y suscriptores con el original.
    pub fn with_server(&self, server: ServerTag) -> Self {
        Self {
            inner: self.inner.clone(),
            tx: self.tx.clone(),
            server: Some(server),
            batch: self.batch,
        }
    }

    /// Vista que etiqueta además cada operación con la operación masiva
    /// `batch` (1.10.0, D-117). Conserva el servidor de la vista.
    pub fn with_batch(&self, batch: Uuid) -> Self {
        Self {
            inner: self.inner.clone(),
            tx: self.tx.clone(),
            server: self.server.clone(),
            batch: Some(batch),
        }
    }

    /// Operación masiva de esta vista.
    pub fn batch(&self) -> Option<Uuid> {
        self.batch
    }

    /// Servidor de esta vista.
    pub fn server(&self) -> Option<&ServerTag> {
        self.server.as_ref()
    }

    /// Suscripción a cambios. Un receptor lento puede perder eventos
    /// (`RecvError::Lagged`); en ese caso debe volver a pedir `snapshot()`.
    pub fn subscribe(&self) -> broadcast::Receiver<ActivityEvent> {
        self.tx.subscribe()
    }

    /// Cambia la capacidad; descarta los registros más antiguos si sobran.
    pub fn set_capacity(&self, capacity: usize) {
        let mut inner = self.inner.lock();
        inner.capacity = capacity.max(1);
        while inner.records.len() > inner.capacity {
            inner.records.pop_front();
        }
    }

    pub fn capacity(&self) -> usize {
        self.inner.lock().capacity
    }

    /// Copia de los registros, del más reciente al más antiguo.
    pub fn snapshot(&self) -> Vec<OperationRecord> {
        self.inner.lock().records.iter().rev().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.inner.lock().records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Vacía el historial visual. Las operaciones en curso se conservan, porque
    /// su finalización todavía debe poder registrarse.
    pub fn clear(&self) {
        {
            let mut inner = self.inner.lock();
            inner.records.retain(|r| r.status == OpStatus::Running);
        }
        let _ = self.tx.send(ActivityEvent::Cleared);
        for record in self.snapshot().into_iter().rev() {
            let _ = self.tx.send(ActivityEvent::Upsert {
                record: Box::new(record),
            });
        }
    }

    /// Registra el inicio de una operación. El `OperationHandle` devuelto debe
    /// cerrarse con `succeed` o `fail`; si se descarta sin cerrarse, la
    /// operación queda marcada como `Cancelled`.
    pub fn begin(&self, spec: OperationSpec) -> OperationHandle {
        let limit = self.inner.lock().capture_limit;
        let record = OperationRecord {
            id: Uuid::new_v4(),
            category: spec.category,
            kind: spec.kind,
            target: spec.target.map(clean),
            transport: clean_transport(spec.transport),
            request_body: spec.request_body.map(|b| clean_capture(b, limit)),
            equivalent_command: spec.equivalent_command.map(clean),
            status: OpStatus::Running,
            started_at: Utc::now(),
            finished_at: None,
            duration_ms: None,
            summary: None,
            stdout: None,
            stderr: None,
            error: None,
            http_status: None,
            exit_code: None,
            server: self.server.clone(),
            batch: self.batch,
        };
        let id = record.id;
        self.upsert(record);
        OperationHandle {
            log: self.clone(),
            id,
            done: false,
        }
    }

    fn upsert(&self, record: OperationRecord) {
        {
            let mut inner = self.inner.lock();
            if let Some(slot) = inner.records.iter_mut().find(|r| r.id == record.id) {
                *slot = record.clone();
            } else {
                inner.records.push_back(record.clone());
                while inner.records.len() > inner.capacity {
                    inner.records.pop_front();
                }
            }
        }
        // Sin suscriptores, `send` devuelve error: no es un fallo.
        let _ = self.tx.send(ActivityEvent::Upsert {
            record: Box::new(record),
        });
    }

    fn finish(&self, id: Uuid, apply: impl FnOnce(&mut OperationRecord, usize)) {
        let updated = {
            let mut inner = self.inner.lock();
            let limit = inner.capture_limit;
            inner.records.iter_mut().find(|r| r.id == id).map(|r| {
                let now = Utc::now();
                r.finished_at = Some(now);
                r.duration_ms = Some((now - r.started_at).num_milliseconds().max(0) as u64);
                apply(r, limit);
                r.clone()
            })
        };
        if let Some(record) = updated {
            let _ = self.tx.send(ActivityEvent::Upsert {
                record: Box::new(record),
            });
        }
    }
}

/// Operación en curso.
pub struct OperationHandle {
    log: ActivityLog,
    id: Uuid,
    done: bool,
}

impl OperationHandle {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn succeed(mut self, outcome: Outcome) {
        self.done = true;
        self.log.finish(self.id, |r, limit| {
            r.status = OpStatus::Succeeded;
            r.summary = outcome.summary.map(clean);
            r.stdout = outcome.stdout.map(|s| clean_capture(s, limit));
            r.stderr = outcome.stderr.map(|s| clean_capture(s, limit));
            r.http_status = outcome.http_status;
            r.exit_code = outcome.exit_code;
        });
    }

    pub fn fail(mut self, failure: Failure) {
        self.done = true;
        self.log.finish(self.id, |r, limit| {
            r.status = OpStatus::Failed;
            r.error = Some(clean(failure.error));
            r.stdout = failure.stdout.map(|s| clean_capture(s, limit));
            r.stderr = failure.stderr.map(|s| clean_capture(s, limit));
            r.http_status = failure.http_status;
            r.exit_code = failure.exit_code;
        });
    }
}

impl OperationHandle {
    /// Cancelación explícita (p. ej. el usuario detuvo un proceso): igual que
    /// descartar el handle, pero conservando la salida capturada hasta ese momento.
    pub fn cancel(mut self, failure: Failure) {
        self.done = true;
        self.log.finish(self.id, |r, limit| {
            r.status = OpStatus::Cancelled;
            r.error = Some(clean(failure.error));
            r.stdout = failure.stdout.map(|s| clean_capture(s, limit));
            r.stderr = failure.stderr.map(|s| clean_capture(s, limit));
            r.exit_code = failure.exit_code;
        });
    }
}

impl Drop for OperationHandle {
    fn drop(&mut self) {
        if !self.done {
            self.log.finish(self.id, |r, _| {
                r.status = OpStatus::Cancelled;
                r.error = Some("La operación se interrumpió antes de completarse".into());
            });
        }
    }
}
