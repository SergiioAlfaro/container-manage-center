//! Estado compartido del backend.

use dcc_activity::ActivityLog;
use dcc_store::{AppConfig, ConfigStore};
use parking_lot::Mutex;

use crate::error::CmdResult;

pub struct AppState {
    pub activity: ActivityLog,
    pub store: ConfigStore,
    config: Mutex<AppConfig>,
    /// Aviso generado al cargar la configuración (archivo corrupto, etc.).
    pub startup_warning: Option<String>,
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
        Self {
            activity,
            store,
            config: Mutex::new(config),
            startup_warning: warning,
        }
    }

    pub fn config(&self) -> AppConfig {
        self.config.lock().clone()
    }

    /// Normaliza, guarda en disco y aplica la nueva configuración.
    /// Si la escritura falla, la configuración en memoria no cambia.
    pub fn replace_config(&self, new: AppConfig) -> CmdResult<AppConfig> {
        let new = new.normalized();
        self.store.save(&new)?;
        self.activity
            .set_capacity(new.activity.max_records as usize);
        *self.config.lock() = new.clone();
        Ok(new)
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
        let saved = state.replace_config(c).unwrap();
        assert_eq!(state.activity.capacity(), 75);
        assert_eq!(store.load().unwrap().config, saved);
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
