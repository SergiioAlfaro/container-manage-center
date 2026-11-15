// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! # cmc-store
//!
//! Lectura y escritura de la configuración en
//! `$XDG_CONFIG_HOME/container-manage-center/config.toml`.
//!
//! - La escritura es atómica: archivo temporal en el mismo directorio + `rename`.
//! - Un archivo corrupto no impide arrancar: se renombra a `config.toml.corrupt`
//!   y se usan los valores por defecto.
//! - Nunca se guardan credenciales aquí (ver DECISIONS.md, D-011).

mod config;
pub mod journal;
pub mod servers;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub use config::{
    Accent, ActivityConfig, AppConfig, ComposeSettings, DockerConfig, DockerMode, Language,
    SavedProject, Theme, UiConfig, UpdatesConfig, CONFIG_VERSION,
};
pub use servers::{ServerAuthMethod, ServerConfig, ServerDockerTarget, ServerSeen, ServerStore};

mod fsx;
pub mod legacy;
pub mod migrate;

/// Carpeta de la configuración y del estado (1.8.0, D-113).
pub const APP_DIR: &str = "container-manage-center";
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
        let body = toml::to_string_pretty(config)?;
        write_atomic(&self.path, &body)
    }
}

/// Escritura atómica (temporal en el mismo directorio + `rename`) con permisos 0600.
pub fn write_atomic(path: &Path, body: &str) -> Result<(), StoreError> {
    let dir = path.parent().ok_or(StoreError::NoConfigDir)?;
    fs::create_dir_all(dir).map_err(|source| StoreError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    let tmp = path.with_extension("toml.tmp");
    let io = |source| StoreError::Io {
        path: tmp.clone(),
        source,
    };
    {
        let mut f = fs::File::create(&tmp).map_err(io)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600)).map_err(io)?;
        }
        f.write_all(body.as_bytes()).map_err(io)?;
        f.sync_all().map_err(io)?;
    }
    fs::rename(&tmp, path).map_err(|source| StoreError::Io {
        path: path.to_path_buf(),
        source,
    })
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

    #[test]
    fn french_and_italian_are_saved_and_loaded() {
        let (_d, s) = store();
        // 0.9.2: los ocho idiomas, con su código en el archivo.
        for lang in Language::ORDER {
            let mut c = AppConfig::default();
            c.ui.language = lang;
            s.save(&c).unwrap();
            assert_eq!(s.load().unwrap().config.ui.language, lang);
            assert!(fs::read_to_string(s.path())
                .unwrap()
                .contains(&format!("language = \"{}\"", lang.code())));
        }
        fs::write(s.path(), "version = 1\n[ui]\nlanguage = \"fr\"\n").unwrap();
        assert_eq!(s.load().unwrap().config.ui.language, Language::Fr);
    }

    #[test]
    fn language_order_and_accent_roundtrip() {
        let codes: Vec<&str> = Language::ORDER.iter().map(|l| l.code()).collect();
        assert_eq!(codes, ["en", "es", "ca", "gl", "eu", "fr", "it", "pt"]);
        let (_d, s) = store();
        // El valor predeterminado es el morado y así se guarda.
        assert_eq!(AppConfig::default().ui.accent, Accent::Purple);
        s.save(&AppConfig::default()).unwrap();
        assert!(fs::read_to_string(s.path())
            .unwrap()
            .contains("accent = \"purple\""));
        assert_eq!(s.load().unwrap().config.ui.accent, Accent::Purple);
        // Un archivo anterior a la 0.9.2 (sin `accent`) también usa el morado.
        fs::write(s.path(), "version = 1\n[ui]\nlanguage = \"gl\"\n").unwrap();
        let c = s.load().unwrap().config;
        assert_eq!((c.ui.language, c.ui.accent), (Language::Gl, Accent::Purple));
    }

    /// 1.0.0 (D-082): el verde azulado ya no existe como color principal.
    #[test]
    fn saved_green_accent_migrates_to_purple_keeping_the_rest() {
        let (_d, s) = store();
        fs::create_dir_all(s.path().parent().unwrap()).unwrap();
        for accent in ["\"teal\"", "\"pink\"", "\"\"", "3", "true"] {
            let toml = format!(
                "version = 1\n[ui]\ntheme = \"dark\"\nlanguage = \"eu\"\naccent = {accent}\nsidebarCollapsed = true\n[activity]\nmaxRecords = 321\n"
            );
            fs::write(s.path(), &toml).unwrap();
            let loaded = s.load().unwrap();
            let c = loaded.config;
            assert_eq!(c.ui.accent, Accent::Purple, "{accent}");
            // El resto de preferencias se conserva: no se trata como archivo dañado.
            assert_eq!(
                (c.ui.theme, c.ui.language, c.ui.sidebar_collapsed),
                (Theme::Dark, Language::Eu, true),
                "{accent}"
            );
            assert_eq!(c.activity.max_records, 321, "{accent}");
            assert!(loaded.warning.is_none(), "{accent}");
            assert!(
                !s.path().with_extension("toml.corrupt").exists(),
                "{accent}"
            );
            // Al guardar queda el morado: el verde no vuelve.
            s.save(&c).unwrap();
            let text = fs::read_to_string(s.path()).unwrap();
            assert!(
                text.contains("accent = \"purple\"") && !text.contains("teal"),
                "{accent}"
            );
        }
    }

    #[test]
    fn updates_section_roundtrip_and_normalization() {
        let (_d, s) = store();
        // Un archivo de una versión anterior (sin [updates]) sigue siendo válido.
        fs::create_dir_all(s.path().parent().unwrap()).unwrap();
        fs::write(s.path(), "version = 1\n[ui]\ntheme = \"dark\"\n").unwrap();
        let l = s.load().unwrap();
        assert_eq!(l.config.updates, UpdatesConfig::default());
        assert!(!l.config.updates.check_on_start, "desactivado por defecto");

        let c = AppConfig {
            updates: UpdatesConfig {
                check_on_start: true,
                source: "  mi-org/cmc \n".into(),
            },
            ..AppConfig::default()
        }
        .normalized();
        assert_eq!(c.updates.source, "mi-org/cmc");
        assert!(c.updates.check_on_start);
        s.save(&c).unwrap();
        assert_eq!(s.load().unwrap().config.updates, c.updates);

        // Sin fuente no se comprueba al iniciar; texto con controles o enorme: se descarta.
        for bad in [
            "",
            "a\u{7}b",
            &"x".repeat(UpdatesConfig::MAX_SOURCE_LEN + 1),
        ] {
            let c = AppConfig {
                updates: UpdatesConfig {
                    check_on_start: true,
                    source: bad.to_owned(),
                },
                ..AppConfig::default()
            }
            .normalized();
            assert_eq!(c.updates, UpdatesConfig::default(), "{bad:?}");
        }
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
