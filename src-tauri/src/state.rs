//! Estado compartido del backend.

use dcc_activity::ActivityLog;
use dcc_engine::{ConnectionMode, EngineManager};
use dcc_store::{AppConfig, ConfigStore, DockerConfig, DockerMode};
use parking_lot::Mutex;

use crate::error::CmdResult;

pub struct AppState {
    pub activity: ActivityLog,
    pub store: ConfigStore,
    /// Única conexión con Docker de la aplicación (D-004).
    pub engine: EngineManager,
    config: Mutex<AppConfig>,
    /// Aviso generado al cargar la configuración (archivo corrupto, etc.).
    pub startup_warning: Option<String>,
}

/// Traduce la configuración persistida al modo de conexión del motor.
pub fn connection_mode(c: &DockerConfig) -> ConnectionMode {
    match c.mode {
        DockerMode::Auto => ConnectionMode::Auto,
        DockerMode::Custom => ConnectionMode::Custom {
            socket_path: c.custom_socket.clone(),
        },
    }
}

impl AppState {
    pub fn new(store: ConfigStore) -> Self {
        let (config, warning) = match store.load() {
            Ok(l) => (l.config, l.warning),
            Err(e) => (
                AppConfig::default(),
                Some(format!(
                    "No se pudo leer la configuración ({e}). Se usan los valores por defecto."
                )),
            ),
        };
        let activity = ActivityLog::new(config.activity.max_records as usize);
        let engine = EngineManager::new(activity.clone(), connection_mode(&config.docker));
        Self {
            activity,
            store,
            engine,
            config: Mutex::new(config),
            startup_warning: warning,
        }
    }

    pub fn config(&self) -> AppConfig {
        self.config.lock().clone()
    }

    /// Normaliza, guarda en disco y aplica la nueva configuración.
    /// Si la escritura falla, la configuración en memoria no cambia.
    /// Devuelve también si cambió la conexión con Docker.
    pub fn replace_config(&self, new: AppConfig) -> CmdResult<(AppConfig, bool)> {
        let new = new.normalized();
        self.store.save(&new)?;
        self.activity
            .set_capacity(new.activity.max_records as usize);
        let mut cur = self.config.lock();
        let docker_changed = connection_mode(&cur.docker) != connection_mode(&new.docker);
        *cur = new.clone();
        Ok((new, docker_changed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_config_persists_and_resizes_log() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::at(dir.path().join("config.toml"));
        let state = AppState::new(store.clone());
        let mut c = state.config();
        c.activity.max_records = 75;
        c.ui.language = dcc_store::Language::En;
        let (saved, docker_changed) = state.replace_config(c).unwrap();
        assert!(!docker_changed);
        assert_eq!(state.activity.capacity(), 75);
        assert_eq!(store.load().unwrap().config, saved);
    }

    #[test]
    fn docker_change_is_detected_only_when_effective() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::new(ConfigStore::at(dir.path().join("config.toml")));
        let mut c = state.config();
        // Cambiar la ruta en modo automático no cambia la conexión.
        c.docker.custom_socket = "/tmp/x.sock".into();
        assert!(!state.replace_config(c.clone()).unwrap().1);
        c.docker.mode = DockerMode::Custom;
        assert!(state.replace_config(c).unwrap().1);
    }

    #[test]
    fn failed_save_leaves_config_untouched() {
        let dir = tempfile::tempdir().unwrap();
        // Un archivo donde debería ir el directorio provoca un error de E/S.
        std::fs::write(dir.path().join("blocker"), "").unwrap();
        let state = AppState::new(ConfigStore::at(
            dir.path().join("blocker").join("config.toml"),
        ));
        let before = state.config();
        let mut c = before.clone();
        c.ui.sidebar_collapsed = true;
        assert!(state.replace_config(c).is_err());
        assert_eq!(state.config(), before);
    }
}
