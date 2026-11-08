//! Esquema de la configuración de la aplicación.
//!
//! Todos los campos tienen valor por defecto (`#[serde(default)]`) para que un
//! archivo antiguo o incompleto siga siendo válido al añadir opciones nuevas.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Language {
    #[default]
    Es,
    En,
    /// Francés (0.9.1).
    Fr,
    /// Italiano (0.9.1).
    It,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct UiConfig {
    pub theme: Theme,
    pub language: Language,
    pub sidebar_collapsed: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            language: Language::Es,
            sidebar_collapsed: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct ActivityConfig {
    /// Panel abierto al arrancar.
    pub panel_open: bool,
    /// Abrir el panel automáticamente cuando una operación falla.
    pub open_on_error: bool,
    /// Número máximo de operaciones conservadas en el historial.
    pub max_records: u32,
    /// Guardar en disco el historial de operaciones y el diario de eventos
    /// para consultarlos en sesiones posteriores (Fase 8, D-058).
    pub keep_history: bool,
}

impl Default for ActivityConfig {
    fn default() -> Self {
        Self {
            panel_open: false,
            open_on_error: true,
            max_records: 500,
            keep_history: true,
        }
    }
}

impl ActivityConfig {
    pub const MIN_RECORDS: u32 = 50;
    pub const MAX_RECORDS: u32 = 10_000;
}

/// Cómo localizar Docker Engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DockerMode {
    /// DOCKER_HOST, contexto de Docker, rootless y socket del sistema, en ese orden.
    #[default]
    Auto,
    /// Solo el socket indicado en `custom_socket`.
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct DockerConfig {
    pub mode: DockerMode,
    /// Ruta absoluta del socket Unix (o `unix:///ruta`). Se conserva aunque el
    /// modo sea automático, para no perderla al alternar.
    pub custom_socket: String,
}

impl DockerConfig {
    pub const MAX_PATH_LEN: usize = 4096;
}

/// Proyecto Compose añadido por el usuario a la lista (Fase 7).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct SavedProject {
    /// Nombre del proyecto (regla de Compose).
    pub name: String,
    /// Archivos Compose (rutas absolutas), en orden.
    pub files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct ComposeSettings {
    pub projects: Vec<SavedProject>,
}

impl ComposeSettings {
    pub const MAX_PROJECTS: usize = 200;
    pub const MAX_FILES: usize = 16;

    pub fn valid_name(name: &str) -> bool {
        let mut c = name.chars();
        matches!(c.next(), Some(f) if f.is_ascii_lowercase() || f.is_ascii_digit())
            && name.len() <= 255
            && c.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    }

    fn valid_path(p: &str) -> bool {
        p.starts_with('/') && !p.contains('\0') && p.len() <= DockerConfig::MAX_PATH_LEN
    }

    /// Descarta entradas no válidas o repetidas (gana la primera) y limita el tamaño.
    pub fn normalized(self) -> Self {
        let mut out: Vec<SavedProject> = Vec::new();
        for p in self.projects {
            let ok = Self::valid_name(&p.name)
                && !p.files.is_empty()
                && p.files.len() <= Self::MAX_FILES
                && p.files.iter().all(|f| Self::valid_path(f));
            if ok && !out.iter().any(|o| o.name == p.name) && out.len() < Self::MAX_PROJECTS {
                out.push(p);
            }
        }
        Self { projects: out }
    }
}

/// Comprobación de versiones nuevas de la aplicación (Fase 9, D-060). La
/// aplicación solo consulta y avisa: nunca descarga ni instala nada.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct UpdatesConfig {
    /// Comprobar al iniciar la aplicación (desactivado por defecto: la
    /// aplicación no se conecta a Internet si no se pide).
    pub check_on_start: bool,
    /// Dónde se publican las versiones: `propietario/repositorio` de GitHub o
    /// la dirección HTTPS de un archivo JSON. Vacío: sin fuente.
    pub source: String,
}

impl UpdatesConfig {
    pub const MAX_SOURCE_LEN: usize = 2048;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct AppConfig {
    pub version: u32,
    pub ui: UiConfig,
    pub activity: ActivityConfig,
    pub docker: DockerConfig,
    pub compose: ComposeSettings,
    pub updates: UpdatesConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            ui: UiConfig::default(),
            activity: ActivityConfig::default(),
            docker: DockerConfig::default(),
            compose: ComposeSettings::default(),
            updates: UpdatesConfig::default(),
        }
    }
}

impl AppConfig {
    /// Corrige valores fuera de rango en lugar de rechazarlos.
    pub fn normalized(mut self) -> Self {
        self.version = CONFIG_VERSION;
        self.activity.max_records = self
            .activity
            .max_records
            .clamp(ActivityConfig::MIN_RECORDS, ActivityConfig::MAX_RECORDS);
        let trimmed = self.docker.custom_socket.trim();
        self.docker.custom_socket =
            if trimmed.len() > DockerConfig::MAX_PATH_LEN || trimmed.contains('\0') {
                String::new()
            } else {
                trimmed.to_owned()
            };
        self.compose = self.compose.normalized();
        let source = self.updates.source.trim();
        self.updates.source = if source.len() > UpdatesConfig::MAX_SOURCE_LEN
            || source.chars().any(char::is_control)
        {
            String::new()
        } else {
            source.to_owned()
        };
        // Sin fuente no hay nada que comprobar al iniciar.
        if self.updates.source.is_empty() {
            self.updates.check_on_start = false;
        }
        self
    }
}
