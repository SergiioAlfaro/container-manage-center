//! Modelo de las copias de seguridad (E4, 1.4.0, D-096).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Versión del formato de copia (`manifest.json`).
pub const FORMAT: u32 = 1;
/// Nombre del manifiesto.
pub const MANIFEST: &str = "manifest.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "BackupItemKind")]
pub enum ItemKind {
    /// Contenido de un volumen (`tar`).
    Volume,
    /// Volcado de una base de datos hecho dentro de su contenedor.
    Database,
    /// Configuración de un contenedor (sin secretos).
    Container,
    /// Archivos de un proyecto Compose.
    Compose,
    /// Configuración de la aplicación (sin secretos).
    AppConfig,
}

/// Motores de bases de datos con volcado propio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DbEngine {
    Mariadb,
    Mysql,
    Postgres,
    Redis,
    Mongo,
}

impl DbEngine {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mariadb => "mariadb",
            Self::Mysql => "mysql",
            Self::Postgres => "postgres",
            Self::Redis => "redis",
            Self::Mongo => "mongo",
        }
    }
    /// Extensión del archivo de volcado.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mariadb | Self::Mysql | Self::Postgres => "sql",
            Self::Redis => "rdb",
            Self::Mongo => "archive",
        }
    }
}

/// Cómo de coherente es una copia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Consistency {
    /// Volcado del propio motor (transacción coherente).
    Dump,
    /// Los contenedores que usan el volumen estaban detenidos durante la copia.
    Stopped,
    /// Ningún contenedor en ejecución usaba el volumen.
    Idle,
    /// Copia «en caliente»: **no garantizada** (un contenedor escribía).
    Hot,
    /// Archivos de configuración: no aplica.
    NotApplicable,
}

/// Un archivo de la copia, con su suma.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "BackupFile")]
pub struct ManifestFile {
    /// Ruta relativa a la carpeta de la copia (`volumes/datos.tar`).
    pub path: String,
    pub sha256: String,
    #[ts(type = "number")]
    pub size: u64,
}

/// Un elemento copiado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BackupItem {
    pub kind: ItemKind,
    /// Volumen, contenedor o proyecto.
    pub name: String,
    pub files: Vec<ManifestFile>,
    pub consistency: Consistency,
    /// Datos para restaurar: motor, imagen, ruta del volumen, contenedores
    /// que lo usaban, variables secretas quitadas…
    pub details: BTreeMap<String, String>,
    /// Códigos traducibles (`backup.warn.hot`…).
    pub warnings: Vec<String>,
}

/// Manifiesto de una copia completa. Se escribe el último: sin él la copia no
/// existe (D-096).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "BackupManifest")]
pub struct Manifest {
    pub format: u32,
    pub id: String,
    /// Id del servidor (`local` o `srv-…`).
    pub server: String,
    pub created_at: String,
    pub finished_at: String,
    pub app_version: String,
    /// Tarea programada que la creó; `None` en una copia manual.
    pub job: Option<String>,
    pub items: Vec<BackupItem>,
}

impl Manifest {
    #[allow(clippy::cast_possible_truncation)]
    pub fn size(&self) -> u64 {
        self.items
            .iter()
            .flat_map(|i| &i.files)
            .map(|f| f.size)
            .sum()
    }
}

/// Estado de una copia en el destino.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BackupState {
    /// Manifiesto válido.
    Complete,
    /// Copia interrumpida (`.partial`): nunca se da por buena.
    Incomplete,
    /// Manifiesto ilegible o de otro formato.
    Unreadable,
}

/// Una copia encontrada en el destino.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BackupEntry {
    pub id: String,
    pub path: String,
    pub state: BackupState,
    pub manifest: Option<Manifest>,
    #[ts(type = "number")]
    pub size: u64,
}

/// Resultado de comprobar la integridad.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IntegrityReport {
    pub ok: bool,
    pub checked: u32,
    /// Archivos cuya suma no coincide.
    pub altered: Vec<String>,
    /// Archivos del manifiesto que faltan.
    pub missing: Vec<String>,
    /// Archivos que no están en el manifiesto.
    pub unexpected: Vec<String>,
}

// ---------------------------------------------------------------- petición

/// Cómo copiar un volumen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum VolumeMode {
    /// En caliente: si algún contenedor en ejecución lo usa, la copia se marca
    /// como no garantizada.
    Hot,
    /// Detener los contenedores que lo usan, copiar y volver a arrancarlos.
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VolumeSelection {
    pub name: String,
    pub mode: VolumeMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ComposeSelection {
    pub project: String,
    /// Incluir los `.env` (pueden tener secretos): solo con aceptación expresa.
    pub include_env: bool,
}

/// Qué copiar.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BackupRequest {
    pub volumes: Vec<VolumeSelection>,
    /// Contenedores de bases de datos que se vuelcan.
    pub databases: Vec<String>,
    /// Contenedores cuya configuración se guarda.
    pub containers: Vec<String>,
    pub compose: Vec<ComposeSelection>,
    pub app_config: bool,
}

impl BackupRequest {
    pub fn is_empty(&self) -> bool {
        self.volumes.is_empty()
            && self.databases.is_empty()
            && self.containers.is_empty()
            && self.compose.is_empty()
            && !self.app_config
    }
}

/// Avance de una copia o restauración.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BackupProgress {
    pub kind: ItemKind,
    pub name: String,
    pub done: u32,
    pub total: u32,
    #[ts(type = "number")]
    pub bytes: u64,
}

// ---------------------------------------------------------------- retención

/// Política de retención (D-097): las últimas `daily` copias diarias, `weekly`
/// semanales y `monthly` mensuales de una misma tarea.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Retention {
    pub daily: u32,
    pub weekly: u32,
    pub monthly: u32,
}

impl Default for Retention {
    fn default() -> Self {
        Self {
            daily: 7,
            weekly: 4,
            monthly: 3,
        }
    }
}
