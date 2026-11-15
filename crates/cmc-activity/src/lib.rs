// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! # cmc-activity
//!
//! Registro de las operaciones que Container Manage Center ejecuta realmente.
//! Todas las capas que hablan con Docker (API o CLI) deben pasar por
//! [`ActivityLog::begin`], de modo que el panel de actividad solo pueda mostrar
//! operaciones que han ocurrido.

pub mod history;
mod log;
mod model;
pub mod quote;
pub mod redact;

pub use history::HistoryEntry;
pub use log::{
    capture, ActivityLog, Failure, OperationHandle, OperationSpec, Outcome, DEFAULT_CAPACITY,
    DEFAULT_CAPTURE_LIMIT,
};
pub use model::{
    ActivityEvent, CapturedText, Category, OpStatus, OperationRecord, ServerTag, Transport,
};

#[cfg(test)]
mod tests;
