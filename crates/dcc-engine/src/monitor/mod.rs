// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Monitorización (E5, 1.5.0, D-102 a D-107): muestreo por servidor sin
//! agentes, históricos, y alertas con histéresis.

pub mod alerts;
pub mod history;
pub mod model;
pub mod sampler;

pub use model::*;
