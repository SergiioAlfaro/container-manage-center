//! # dcc-engine
//!
//! Comunicación con Docker Engine a través de su socket Unix (D-004):
//!
//! - [`endpoint`]: localiza los sockets candidatos (DOCKER_HOST, contexto, rootless, sistema).
//! - [`probe`]: distingue socket inexistente, sin permiso o con el daemon detenido.
//! - [`client::Engine`]: peticiones a la API registradas en el panel de actividad.
//! - [`cli`]: ejecución sin shell del CLI de Docker (Compose, D-005).
//! - [`manager::EngineManager`]: conexión, diagnóstico, vigilancia y reconexión.

pub mod build;
pub mod catalog;
pub mod cli;
pub mod client;
pub mod compose;
pub mod containers;
pub mod details;
pub mod endpoint;
mod error;
pub mod images;
pub mod manager;
mod model;
pub mod probe;
pub mod pull;
pub mod reference;
pub mod registry_auth;
pub mod resources;
pub mod spec;
pub mod streams;
pub mod sys;

pub use client::{ApiOp, Engine};
pub use compose::{ComposeKind, ComposeStatus};
pub use containers::{
    ContainerAction, ContainerState, ContainerSummary, CreatedContainer, PortMapping,
};
pub use error::EngineError;
pub use images::{ImageSummary, ImageUser};
pub use manager::EngineManager;
pub use model::*;
