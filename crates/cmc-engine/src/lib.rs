// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! # cmc-engine
//!
//! Comunicación con Docker Engine a través de su socket Unix (D-004):
//!
//! - [`endpoint`]: localiza los sockets candidatos (DOCKER_HOST, contexto, rootless, sistema).
//! - [`probe`]: distingue socket inexistente, sin permiso o con el daemon detenido.
//! - [`client::Engine`]: peticiones a la API registradas en el panel de actividad.
//! - [`cli`]: ejecución sin shell del CLI de Docker (Compose, D-005).
//! - [`manager::EngineManager`]: conexión, diagnóstico, vigilancia y reconexión.

pub mod audit;
pub mod backup;
pub mod binds;
pub mod build;
pub mod bulk;
pub mod bulk_exec;
pub mod catalog;
pub mod cli;
pub mod client;
pub mod compose;
pub mod containers;
pub mod details;
pub mod edit;
pub mod edit_ops;
pub mod endpoint;
mod error;
pub mod events;
pub mod fsx;
pub mod hostpath;
pub mod images;
pub mod legacy;
pub mod manager;
mod model;
pub mod monitor;
pub mod networks;
pub mod probe;
pub mod pull;
pub mod reference;
pub mod registry_auth;
pub mod resources;
pub mod risk;
pub mod spec;
pub mod ssh;
pub mod streams;
pub mod sys;
pub mod volumes;

pub use client::{ApiOp, Engine};
pub use compose::file::ComposeFile;
pub use compose::ops::ComposeConfig;
pub use compose::project::{
    ComposeAction, ComposeCli, ComposeProject, DownImages, ProjectModel, ProjectRef, PullPolicy,
};
pub use compose::{ComposeKind, ComposeStatus};
pub use containers::{
    ContainerAction, ContainerState, ContainerSummary, CreatedContainer, MountPoint, PortMapping,
};
pub use error::EngineError;
pub use images::{ImageSummary, ImageUser};
pub use manager::EngineManager;
pub use model::*;
