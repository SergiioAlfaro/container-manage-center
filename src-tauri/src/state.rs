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
    /// Flujos abiertos (logs, estadísticas, terminales).
    pub streams: std::sync::Arc<crate::streams::StreamRegistry>,
    config: Mutex<AppConfig>,
    /// Aviso generado al cargar la configuración (archivo corrupto, etc.).
    pub startup_warning: Option<String>,
    /// Historial de operaciones y diario de eventos (Fase 8).
    pub journals: crate::journal::Journals,
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
        let (journals, journal_warning) =
            crate::journal::Journals::open(crate::journal::state_dir(store.path()));
        let startup_warning = match (warning, journal_warning) {
            (Some(a), Some(b)) => Some(format!("{a} {b}")),
            (a, b) => a.or(b),
        };
        Self {
            activity,
            store,
            engine,
            streams: Default::default(),
            config: Mutex::new(config),
            startup_warning,
            journals,
        }
    }

    pub fn config(&self) -> AppConfig {
        self.config.lock().clone()
    }

    /// Normaliza, guarda en disco y aplica la nueva configuración.
    /// Si la escritura falla, la configuración en memoria no cambia.
    /// Devuelve también si cambió la conexión con Docker.
    ///
    /// La lista de proyectos Compose no se cambia aquí: la interfaz envía la
    /// configuración completa y una copia antigua borraría proyectos añadidos
    /// mientras tanto. Solo la cambia [`AppState::update_compose`].
    pub fn replace_config(&self, new: AppConfig) -> CmdResult<(AppConfig, bool)> {
        let mut cur = self.config.lock();
        let mut new = new.normalized();
        new.compose = cur.compose.clone();
        self.store.save(&new)?;
        self.activity
            .set_capacity(new.activity.max_records as usize);
        let docker_changed = connection_mode(&cur.docker) != connection_mode(&new.docker);
        *cur = new.clone();
        Ok((new, docker_changed))
    }

    /// Cambia la lista de proyectos Compose registrados y la guarda.
    pub fn update_compose<T>(
        &self,
        f: impl FnOnce(&mut Vec<dcc_store::SavedProject>) -> CmdResult<T>,
    ) -> CmdResult<(T, AppConfig)> {
        let mut cur = self.config.lock();
        let mut next = cur.clone();
        let out = f(&mut next.compose.projects)?;
        let next = next.normalized();
        self.store.save(&next)?;
        *cur = next.clone();
        Ok((out, next))
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

    #[test]
    fn settings_screen_cannot_overwrite_compose_projects() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::at(dir.path().join("config.toml"));
        let state = AppState::new(store.clone());
        let stale = state.config();
        state
            .update_compose(|ps| {
                ps.push(dcc_store::SavedProject {
                    name: "tienda".into(),
                    files: vec!["/srv/tienda/compose.yaml".into()],
                });
                Ok(())
            })
            .unwrap();
        let mut c = stale;
        c.ui.language = dcc_store::Language::En;
        let (saved, _) = state.replace_config(c).unwrap();
        assert_eq!(
            saved.compose.projects.len(),
            1,
            "la copia antigua no borra el proyecto"
        );
        assert_eq!(saved.ui.language, dcc_store::Language::En);
        assert_eq!(store.load().unwrap().config, saved);
    }
}
