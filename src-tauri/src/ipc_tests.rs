//! Pruebas de integración de la capa IPC con el runtime simulado de Tauri:
//! comandos reales invocados como lo haría la interfaz, y eventos de actividad
//! recibidos por la ventana.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use dcc_activity::{
    ActivityEvent, Category, OpStatus, OperationRecord, OperationSpec, Outcome, Transport,
};
use dcc_store::{AppConfig, ConfigStore, DockerConfig, DockerMode, Theme};
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

/// Por defecto la aplicación apunta a un socket inexistente: las pruebas son
/// deterministas y no dependen de que haya Docker en el equipo.
fn harness() -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let docker = DockerConfig {
        mode: DockerMode::Custom,
        custom_socket: dir.path().join("no-docker.sock").display().to_string(),
    };
    harness_with(dir, docker)
}

fn harness_with(dir: tempfile::TempDir, docker: DockerConfig) -> Harness {
    let store = ConfigStore::at(dir.path().join("config.toml"));
    store
        .save(&AppConfig {
            docker,
            ..Default::default()
        })
        .unwrap();
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
    let mut cfg = h.app.state::<AppState>().config();
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

// ------------------------------------------------------------------ Docker

fn wait_for<F: Fn() -> bool>(what: &str, f: F) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !f() {
        assert!(
            std::time::Instant::now() < deadline,
            "tiempo agotado esperando: {what}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn engine_state(h: &Harness) -> serde_json::Value {
    invoke(h, "engine_state", json!({})).unwrap()
}

#[test]
fn missing_socket_is_reported_as_disconnected_with_diagnosis() {
    let h = harness();
    wait_for("estado desconectado", || {
        engine_state(&h)["state"] == "disconnected"
    });
    let s = engine_state(&h);
    assert_eq!(s["diagnosis"]["problem"]["kind"], "noSocket");
    assert_eq!(s["diagnosis"]["mode"]["mode"], "custom");
    assert_eq!(
        s["diagnosis"]["candidates"][0]["result"]["result"],
        "missing"
    );
}

#[test]
fn resource_commands_fail_cleanly_without_connection() {
    let h = harness();
    wait_for("estado desconectado", || {
        engine_state(&h)["state"] == "disconnected"
    });
    for cmd in ["containers_list", "images_list", "engine_overview"] {
        let err = invoke(&h, cmd, json!({})).unwrap_err();
        assert_eq!(err["code"], "engine.notConnected", "{cmd}: {err}");
    }
}

#[test]
fn removal_requires_explicit_confirmation() {
    let h = harness();
    let err = invoke(
        &h,
        "container_remove",
        json!({ "id": "web", "name": null, "force": true, "volumes": true, "confirmed": false }),
    )
    .unwrap_err();
    assert_eq!(err["code"], "engine.notConfirmed");
}

#[test]
fn engine_test_reports_invalid_path_without_saving() {
    let h = harness();
    let before = h.store.load().unwrap().config.docker;
    let v = invoke(
        &h,
        "engine_test",
        json!({ "docker": { "mode": "custom", "customSocket": "relativa.sock" } }),
    )
    .unwrap();
    assert_eq!(v["result"], "failed");
    assert_eq!(v["diagnosis"]["problem"]["kind"], "invalidPath");
    assert_eq!(h.store.load().unwrap().config.docker, before);
}

#[test]
fn changing_docker_config_reconnects_and_emits_state() {
    let h = harness();
    wait_for("estado desconectado", || {
        engine_state(&h)["state"] == "disconnected"
    });
    let states: Arc<Mutex<Vec<serde_json::Value>>> = Default::default();
    let sink = states.clone();
    h.window.listen(crate::events::ENGINE_STATE, move |e| {
        sink.lock()
            .unwrap()
            .push(serde_json::from_str(e.payload()).unwrap())
    });
    let other = h._dir.path().join("otro.sock").display().to_string();
    let mut cfg = h.app.state::<AppState>().config();
    cfg.docker.custom_socket = other.clone();
    invoke(&h, "config_set", json!({ "config": cfg })).unwrap();
    wait_for("evento con la nueva ruta", || {
        states.lock().unwrap().iter().any(|s| {
            s["state"] == "disconnected"
                && s["diagnosis"]["candidates"][0]["endpoint"]["socketPath"] == other.as_str()
        })
    });
    assert_eq!(h.store.load().unwrap().config.docker.custom_socket, other);
}

/// Contra Docker real (DCC_DOCKER_TESTS=1): la app arranca, conecta y sirve datos.
#[test]
fn live_app_connects_and_serves_real_data() {
    if std::env::var("DCC_DOCKER_TESTS").as_deref() != Ok("1") {
        eprintln!("omitida: define DCC_DOCKER_TESTS=1");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let docker = match std::env::var("DCC_DOCKER_SOCKET") {
        Ok(p) => DockerConfig {
            mode: DockerMode::Custom,
            custom_socket: p,
        },
        Err(_) => DockerConfig::default(),
    };
    let h = harness_with(dir, docker);
    wait_for("conexión", || engine_state(&h)["state"] == "connected");
    let s = engine_state(&h);
    assert!(!s["engine"]["version"].as_str().unwrap().is_empty());

    let o = invoke(&h, "engine_overview", json!({})).unwrap();
    assert!(o["info"]["ncpu"].as_i64().unwrap() > 0);
    let containers = invoke(&h, "containers_list", json!({})).unwrap();
    assert_eq!(
        containers.as_array().unwrap().len() as i64,
        o["info"]["containers"].as_i64().unwrap()
    );
    let images = invoke(&h, "images_list", json!({})).unwrap();
    assert!(images.is_array());

    // La actividad contiene exactamente las operaciones ejecutadas, todas correctas.
    let records: Vec<OperationRecord> =
        serde_json::from_value(invoke(&h, "activity_list", json!({})).unwrap()).unwrap();
    let kinds: Vec<_> = records.iter().rev().map(|r| r.kind.as_str()).collect();
    assert_eq!(
        kinds,
        [
            "system.ping",
            "system.version",
            "system.info",
            "system.version",
            "system.info",
            "container.list",
            "image.list",
            "container.list"
        ]
    );
    assert!(records.iter().all(|r| r.status == OpStatus::Succeeded));

    // Una acción sobre un contenedor inexistente devuelve el error real de Docker.
    let err = invoke(
        &h,
        "container_action",
        json!({ "id": "dcc-no-existe-xyz", "name": null, "action": "start" }),
    )
    .unwrap_err();
    assert_eq!(err["code"], "engine.docker");
    assert_eq!(err["httpStatus"], 404);
}
