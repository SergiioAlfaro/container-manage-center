// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
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
    /// Catalán (0.9.2).
    Ca,
    /// Gallego (0.9.2).
    Gl,
    /// Euskera (0.9.2).
    Eu,
    /// Portugués (0.9.2).
    Pt,
}

impl Language {
    /// Orden del selector de idioma (0.9.2): inglés primero.
    pub const ORDER: [Language; 8] = [
        Language::En,
        Language::Es,
        Language::Ca,
        Language::Gl,
        Language::Eu,
        Language::Fr,
        Language::It,
        Language::Pt,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Self::Es => "es",
            Self::En => "en",
            Self::Fr => "fr",
            Self::It => "it",
            Self::Ca => "ca",
            Self::Gl => "gl",
            Self::Eu => "eu",
            Self::Pt => "pt",
        }
    }
}

/// Color principal de la interfaz (0.9.2, D-072; 1.0.0, D-082).
///
/// Desde la 1.0.0 el único color principal es el morado: el verde azulado
/// original ya no se puede elegir. Se sigue guardando (`accent = "purple"`) para
/// que una versión anterior lea el mismo color. Al leer, **cualquier** valor
/// guardado —el `"teal"` de la 0.9.2–0.10.0, uno desconocido o uno con otro
/// tipo— se convierte en morado sin invalidar el resto de la configuración.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Accent {
    #[default]
    Purple,
}

impl<'de> Deserialize<'de> for Accent {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        serde::de::IgnoredAny::deserialize(d)?;
        Ok(Self::Purple)
    }
}

impl Accent {
    pub fn code(self) -> &'static str {
        match self {
            Self::Purple => "purple",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct UiConfig {
    pub theme: Theme,
    pub language: Language,
    /// Color principal (0.9.2).
    pub accent: Accent,
    pub sidebar_collapsed: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            language: Language::Es,
            accent: Accent::Purple,
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

/// Gestión masiva de contenedores (1.10.0, D-118).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct BulkSettings {
    /// Operaciones que se ejecutan a la vez en una operación masiva.
    pub concurrency: u8,
    /// Actualizar la lista de contenedores automáticamente.
    pub auto_refresh: bool,
    /// Segundos entre actualizaciones automáticas.
    pub refresh_secs: u16,
}

impl BulkSettings {
    pub const MIN_CONCURRENCY: u8 = 1;
    pub const MAX_CONCURRENCY: u8 = 16;
    pub const MIN_REFRESH_SECS: u16 = 5;
    pub const MAX_REFRESH_SECS: u16 = 300;

    pub fn normalized(self) -> Self {
        Self {
            concurrency: self
                .concurrency
                .clamp(Self::MIN_CONCURRENCY, Self::MAX_CONCURRENCY),
            auto_refresh: self.auto_refresh,
            refresh_secs: self
                .refresh_secs
                .clamp(Self::MIN_REFRESH_SECS, Self::MAX_REFRESH_SECS),
        }
    }
}

impl Default for BulkSettings {
    fn default() -> Self {
        Self {
            concurrency: 4,
            auto_refresh: true,
            refresh_secs: 15,
        }
    }
}

/// Modalidad de una plantilla de comando masivo (1.11.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ExecTemplateMode {
    /// Ejecutable con argumentos.
    Direct,
    /// Secuencia para un intérprete.
    Script,
}

/// Comando frecuente guardado (1.11.0, D-124). Es una configuración que se
/// reutiliza, no la garantía de que funcione en cualquier imagen. Nunca guarda
/// secretos: la aplicación rechaza las que los contienen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct ExecTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub mode: ExecTemplateMode,
    /// Modalidad directa.
    pub program: String,
    pub args: Vec<String>,
    /// Modalidad con intérprete.
    pub interpreter: String,
    pub flag: String,
    pub script: String,
    /// Opcionales; vacío = lo de la imagen.
    pub user: String,
    pub workdir: String,
    /// Tiempo máximo recomendado.
    pub timeout_secs: u32,
}

impl Default for ExecTemplate {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            description: String::new(),
            mode: ExecTemplateMode::Direct,
            program: String::new(),
            args: Vec::new(),
            interpreter: String::new(),
            flag: String::new(),
            script: String::new(),
            user: String::new(),
            workdir: String::new(),
            timeout_secs: 30,
        }
    }
}

/// Comandos frecuentes de la gestión masiva (1.11.0). Se cambian solo con su
/// propia orden, que los valida; guardar el resto de la configuración no los
/// toca (igual que los proyectos Compose).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct ExecTemplates {
    pub templates: Vec<ExecTemplate>,
}

impl ExecTemplates {
    pub const MAX_TEMPLATES: usize = 200;
    pub const MAX_NAME: usize = 80;
    pub const MAX_DESCRIPTION: usize = 500;

    /// Quita las que no tienen nombre o identificador, los repetidos y lo que
    /// pase del máximo (un archivo editado a mano no rompe la aplicación).
    pub fn normalized(mut self) -> Self {
        let mut seen = std::collections::HashSet::new();
        self.templates.retain(|t| {
            !t.id.trim().is_empty()
                && !t.name.trim().is_empty()
                && t.name.chars().count() <= Self::MAX_NAME
                && t.description.chars().count() <= Self::MAX_DESCRIPTION
                && seen.insert(t.id.clone())
        });
        self.templates.truncate(Self::MAX_TEMPLATES);
        self
    }
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
    /// Gestión masiva (1.10.0).
    pub bulk: BulkSettings,
    /// Comandos frecuentes de la gestión masiva (1.11.0).
    pub exec: ExecTemplates,
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
            bulk: BulkSettings::default(),
            exec: ExecTemplates::default(),
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
        self.bulk = self.bulk.normalized();
        self.exec = self.exec.normalized();
        self
    }
}
