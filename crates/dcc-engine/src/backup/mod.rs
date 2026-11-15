// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Copias de seguridad y restauración (E4, 1.4.0, D-096 a D-101).

pub mod db;
pub mod docker;
pub mod model;
pub mod restore;
pub mod run;
pub mod store;

pub use model::*;
pub use restore::{
    RestoreItem, RestoreItemResult, RestoreOutcome, RestorePlanItem, RestorePreview, RestoreRequest,
};
pub use run::BackupContext;
