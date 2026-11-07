//! # dcc-store
//!
//! Lectura y escritura de la configuración en
//! `$XDG_CONFIG_HOME/docker-control-center/config.toml`.
//!
//! - La escritura es atómica: archivo temporal en el mismo directorio + `rename`.
//! - Un archivo corrupto no impide arrancar: se renombra a `config.toml.corrupt`
//!   y se usan los valores por defecto.
//! - Nunca se guardan credenciales aquí (ver DECISIONS.md, D-011).

mod config;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub use config::{
    ActivityConfig, AppConfig, ComposeSettings, DockerConfig, DockerMode, Language, SavedProject,
    Theme, UiConfig, CONFIG_VERSION,
};

pub const APP_DIR: &str = "docker-control-center";
pub const CONFIG_FILE: &str = "config.toml";

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("no se pudo determinar el directorio de configuración del usuario")]
    NoConfigDir,
    #[error("error de E/S en {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("no se pudo serializar la configuración: {0}")]
    Serialize(#[from] toml::ser::Error),
}

/// Resultado de cargar la configuración.
#[derive(Debug)]
pub struct Loaded {
    pub config: AppConfig,
    /// Aviso para el usuario si se recuperó de un archivo corrupto.
    pub warning: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    /// Almacén en la ubicación XDG estándar del usuario.
    pub fn default_location() -> Result<Self, StoreError> {
        let dir = dirs::config_dir().ok_or(StoreError::NoConfigDir)?;
        Ok(Self::at(dir.join(APP_DIR).join(CONFIG_FILE)))
    }

    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<Loaded, StoreError> {
        let raw = match fs::read_to_string(&self.path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Loaded {
                    config: AppConfig::default(),
                    warning: None,
                });
            }
            Err(source) => {
                return Err(StoreError::Io {
                    path: self.path.clone(),
                    source,
                })
            }
        };
        match toml::from_str::<AppConfig>(&raw) {
            Ok(cfg) => Ok(Loaded {
                config: cfg.normalized(),
                warning: None,
            }),
            Err(parse_err) => {
                let backup = self.path.with_extension("toml.corrupt");
                tracing::warn!(error = %parse_err, backup = %backup.display(), "configuración corrupta");
                fs::rename(&self.path, &backup).map_err(|source| StoreError::Io {
                    path: backup.clone(),
                    source,
                })?;
                Ok(Loaded {
                    config: AppConfig::default(),
                    warning: Some(format!(
                        "El archivo de configuración no era válido y se ha guardado una copia en {}. Se han restaurado los valores por defecto.",
                        backup.display()
                    )),
                })
            }
        }
    }

    pub fn save(&self, config: &AppConfig) -> Result<(), StoreError> {
        let dir = self.path.parent().ok_or(StoreError::NoConfigDir)?;
        fs::create_dir_all(dir).map_err(|source| StoreError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let body = toml::to_string_pretty(config)?;
        let tmp = self.path.with_extension("toml.tmp");
        let io = |source| StoreError::Io {
            path: tmp.clone(),
            source,
        };
        {
            let mut f = fs::File::create(&tmp).map_err(io)?;
            f.write_all(body.as_bytes()).map_err(io)?;
            f.sync_all().map_err(io)?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600)).map_err(io)?;
        }
        fs::rename(&tmp, &self.path).map_err(|source| StoreError::Io {
            path: self.path.clone(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, ConfigStore) {
        let dir = tempfile::tempdir().unwrap();
        let s = ConfigStore::at(dir.path().join("sub").join(CONFIG_FILE));
        (dir, s)
    }

    #[test]
    fn missing_file_gives_defaults() {
        let (_d, s) = store();
        let l = s.load().unwrap();
        assert_eq!(l.config, AppConfig::default());
        assert!(l.warning.is_none());
        assert_eq!(l.config.ui.language, Language::Es);
    }

    #[test]
    fn save_then_load_roundtrip() {
        let (_d, s) = store();
        let mut c = AppConfig::default();
        c.ui.theme = Theme::Dark;
        c.ui.language = Language::En;
        c.ui.sidebar_collapsed = true;
        c.activity.max_records = 1234;
        s.save(&c).unwrap();
        assert_eq!(s.load().unwrap().config, c);
        assert!(!s.path().with_extension("toml.tmp").exists());
    }

    #[cfg(unix)]
    #[test]
    fn saved_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let (_d, s) = store();
        s.save(&AppConfig::default()).unwrap();
        let mode = fs::metadata(s.path()).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn partial_file_fills_defaults() {
        let (_d, s) = store();
        fs::create_dir_all(s.path().parent().unwrap()).unwrap();
        fs::write(s.path(), "[ui]\ntheme = \"light\"\n").unwrap();
        let c = s.load().unwrap().config;
        assert_eq!(c.ui.theme, Theme::Light);
        assert_eq!(c.ui.language, Language::Es);
        assert_eq!(c.activity, ActivityConfig::default());
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        let (_d, s) = store();
        fs::create_dir_all(s.path().parent().unwrap()).unwrap();
        fs::write(s.path(), "[activity]\nmaxRecords = 3\n").unwrap();
        assert_eq!(
            s.load().unwrap().config.activity.max_records,
            ActivityConfig::MIN_RECORDS
        );
    }

    #[test]
    fn corrupt_file_is_backed_up_and_defaults_used() {
        let (_d, s) = store();
        fs::create_dir_all(s.path().parent().unwrap()).unwrap();
        fs::write(s.path(), "esto no es [toml").unwrap();
        let l = s.load().unwrap();
        assert_eq!(l.config, AppConfig::default());
        assert!(l.warning.is_some());
        assert!(s.path().with_extension("toml.corrupt").exists());
        assert!(!s.path().exists());
    }

    #[test]
    fn docker_section_roundtrip_and_old_files_get_defaults() {
        let (_d, s) = store();
        fs::create_dir_all(s.path().parent().unwrap()).unwrap();
        // Archivo de la fase 1, sin sección [docker].
        fs::write(s.path(), "version = 1\n[ui]\ntheme = \"dark\"\n").unwrap();
        let c = s.load().unwrap().config;
        assert_eq!(c.docker, DockerConfig::default());
        let mut c = c;
        c.docker = DockerConfig {
            mode: DockerMode::Custom,
            custom_socket: "  /run/user/1000/docker.sock ".into(),
        };
        let c = c.normalized();
        assert_eq!(c.docker.custom_socket, "/run/user/1000/docker.sock");
        s.save(&c).unwrap();
        assert_eq!(s.load().unwrap().config.docker, c.docker);
    }

    #[test]
    fn json_shape_matches_frontend_contract() {
        let v = serde_json::to_value(AppConfig::default()).unwrap();
        assert_eq!(v["ui"]["theme"], "system");
        assert_eq!(v["ui"]["language"], "es");
        assert_eq!(v["activity"]["openOnError"], true);
    }

    #[test]
    fn compose_projects_roundtrip_and_invalid_entries_are_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::at(dir.path().join("config.toml"));
        let mut c = AppConfig::default();
        let p = |name: &str, files: &[&str]| SavedProject {
            name: name.into(),
            files: files.iter().map(|s| s.to_string()).collect(),
        };
        c.compose.projects = vec![
            p("tienda", &["/srv/tienda/compose.yaml"]),
            p("tienda", &["/otra/compose.yaml"]),
            p("Mal", &["/x/compose.yaml"]),
            p("rel", &["compose.yaml"]),
            p("vacio", &[]),
            p("ok-2", &["/a/compose.yaml", "/a/override.yaml"]),
        ];
        store.save(&c.normalized()).unwrap();
        let loaded = store.load().unwrap().config;
        assert_eq!(
            loaded.compose.projects,
            [
                p("tienda", &["/srv/tienda/compose.yaml"]),
                p("ok-2", &["/a/compose.yaml", "/a/override.yaml"])
            ]
        );
        // Un archivo de la versión 0.6 (sin sección compose) sigue siendo válido.
        std::fs::write(
            dir.path().join("config.toml"),
            "version = 1\n[ui]\ntheme = \"dark\"\n",
        )
        .unwrap();
        assert!(store.load().unwrap().config.compose.projects.is_empty());
    }
}
