//! Pruebas de integración de la capa IPC con el runtime simulado de Tauri:
//! comandos reales invocados como lo haría la interfaz, y eventos de actividad
//! recibidos por la ventana.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use dcc_activity::{
    ActivityEvent, Category, OpStatus, OperationRecord, OperationSpec, Outcome, Transport,
};
use dcc_store::{AppConfig, ConfigStore, Theme};
use serde_json::json;
use tauri::ipc::{CallbackFn, InvokeBody, InvokeResponseBody};
use tauri::test::{get_ipc_response, mock_builder, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{Listener, Manager};

use crate::state::AppState;

struct Harness {
    app: tauri::App<tauri::test::MockRuntime>,
    window: tauri::WebviewWindow<tauri::test::MockRuntime>,
    _dir: tempfile::TempDir,
    store: ConfigStore,
}

fn harness() -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let store = ConfigStore::at(dir.path().join("config.toml"));
    let mut app = super::configure(mock_builder(), store.clone())
        .build(tauri::generate_context!(test = true))
        .unwrap();
    // Ejecuta `setup` una sola vez: crea la ventana `main` de tauri.conf.json y
    // arranca el puente de eventos. El aviso de obsolescencia se refiere a usarlo
    // en bucle, que aquí no ocurre.
    #[allow(deprecated)]
    app.run_iteration(|_, _| {});
    let window = app.get_webview_window("main").expect("ventana main");
    Harness {
        app,
        window,
        _dir: dir,
        store,
    }
}

fn invoke(
    h: &Harness,
    cmd: &str,
    body: serde_json::Value,
) -> Result<serde_json::Value, serde_json::Value> {
    let res = get_ipc_response(
        &h.window,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    );
    match res {
        Ok(InvokeResponseBody::Json(s)) => Ok(serde_json::from_str(&s).unwrap()),
        Ok(InvokeResponseBody::Raw(b)) => Ok(serde_json::from_slice(&b).unwrap()),
        Err(e) => Err(e),
    }
}

#[test]
fn config_get_returns_defaults() {
    let h = harness();
    let v = invoke(&h, "config_get", json!({})).unwrap();
    assert_eq!(v["ui"]["language"], "es");
    assert_eq!(v["activity"]["maxRecords"], 500);
}

#[test]
fn config_set_persists_to_disk() {
    let h = harness();
    let mut cfg = AppConfig::default();
    cfg.ui.theme = Theme::Dark;
    cfg.activity.max_records = 2;
    let v = invoke(&h, "config_set", json!({ "config": cfg })).unwrap();
    // Valor fuera de rango corregido por el backend.
    assert_eq!(v["activity"]["maxRecords"], 50);
    let disk = h.store.load().unwrap().config;
    assert_eq!(disk.ui.theme, Theme::Dark);
    assert_eq!(disk.activity.max_records, 50);
}

#[test]
fn config_set_rejects_malformed_payload() {
    let h = harness();
    let err = invoke(
        &h,
        "config_set",
        json!({ "config": { "ui": { "theme": "purple" } } }),
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("purple") || err.to_string().contains("variant"),
        "{err}"
    );
}

#[test]
fn app_info_reports_version_and_path() {
    let h = harness();
    let v = invoke(&h, "app_info", json!({})).unwrap();
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    assert!(v["configPath"].as_str().unwrap().ends_with("config.toml"));
}

fn spec() -> OperationSpec {
    OperationSpec {
        category: Category::System,
        kind: "system.ping".into(),
        target: None,
        transport: Transport::Api {
            method: "GET".into(),
            path: "/_ping".into(),
            endpoint: "/var/run/docker.sock".into(),
        },
        request_body: None,
        equivalent_command: None,
    }
}

#[test]
fn activity_list_and_clear_reflect_backend_log() {
    let h = harness();
    let state = h.app.state::<AppState>();
    state.activity.begin(spec()).succeed(Outcome {
        http_status: Some(200),
        ..Default::default()
    });

    let list = invoke(&h, "activity_list", json!({})).unwrap();
    let records: Vec<OperationRecord> = serde_json::from_value(list).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, "system.ping");
    assert_eq!(records[0].status, OpStatus::Succeeded);

    invoke(&h, "activity_clear", json!({})).unwrap();
    let list = invoke(&h, "activity_list", json!({})).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 0);
}

#[test]
fn activity_events_reach_the_window() {
    let h = harness();
    let received: Arc<Mutex<Vec<ActivityEvent>>> = Default::default();
    let sink = received.clone();
    h.window.listen(crate::events::ACTIVITY_EVENT, move |e| {
        sink.lock()
            .unwrap()
            .push(serde_json::from_str(e.payload()).unwrap());
    });

    let state = h.app.state::<AppState>();
    state.activity.begin(spec()).succeed(Outcome::default());

    // El puente corre en el runtime asíncrono: espera acotada.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while received.lock().unwrap().len() < 2 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let events = received.lock().unwrap();
    assert_eq!(events.len(), 2, "se esperaban inicio y fin: {events:?}");
    match (&events[0], &events[1]) {
        (ActivityEvent::Upsert { record: a }, ActivityEvent::Upsert { record: b }) => {
            assert_eq!(a.status, OpStatus::Running);
            assert_eq!(b.status, OpStatus::Succeeded);
        }
        other => panic!("eventos inesperados: {other:?}"),
    }
}
