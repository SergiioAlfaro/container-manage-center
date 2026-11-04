use serde::Serialize;
use tauri::State;
use ts_rs::TS;

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub config_path: String,
    pub startup_warning: Option<String>,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        name: "Docker Control Center".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        config_path: state.store.path().display().to_string(),
        startup_warning: state.startup_warning.clone(),
    }
}
