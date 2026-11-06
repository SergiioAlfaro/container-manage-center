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

// ------------------------------------------------------------------ Fase 3

#[test]
fn preview_validates_and_translates_without_docker() {
    let h = harness();
    let v = invoke(
        &h,
        "container_preview",
        json!({ "spec": { "name": "web", "image": "nginx:1.27", "ports": [{ "hostIp": "", "hostPort": 8080, "containerPort": 80, "protocol": "tcp" }], "mounts": [{ "kind": "bind", "source": "/no/existe/dcc", "target": "/x", "readOnly": false }] } }),
    )
    .unwrap();
    assert_eq!(
        v["runCommand"],
        "docker run --detach --name web --publish 8080:80/tcp --mount type=bind,source=/no/existe/dcc,target=/x --restart no nginx:1.27"
    );
    assert!(v["createCommand"]
        .as_str()
        .unwrap()
        .starts_with("docker create --name web"));
    assert!(v["body"].as_str().unwrap().contains("\"PortBindings\""));
    let issues = v["issues"].as_array().unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0]["code"], "bind.missing");
    assert_eq!(issues[0]["severity"], "error");
    assert_eq!(issues[0]["field"], "mounts.0.source");
    // Los secretos no aparecen en lo que se muestra.
    let v = invoke(
        &h,
        "container_preview",
        json!({ "spec": { "image": "postgres:17", "env": [{ "key": "POSTGRES_PASSWORD", "value": "s3creto" }] } }),
    )
    .unwrap();
    assert_eq!(v["secretsHidden"], 1);
    for k in ["runCommand", "createCommand", "body"] {
        assert!(!v[k].as_str().unwrap().contains("s3creto"), "{k}: {}", v[k]);
    }
    // Nada de esto habla con Docker: el registro de actividad sigue vacío.
    let list = invoke(&h, "activity_list", json!({})).unwrap();
    assert!(list
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["category"] == "system"));
}

#[test]
fn stream_close_of_unknown_stream_is_harmless() {
    let h = harness();
    assert_eq!(
        invoke(&h, "stream_close", json!({ "stream": 999 })).unwrap(),
        json!(false)
    );
    let err = invoke(
        &h,
        "terminal_input",
        json!({ "stream": 999, "data": "ls\n" }),
    )
    .unwrap_err();
    assert_eq!(err["code"], "stream.closed");
    invoke(
        &h,
        "terminal_resize",
        json!({ "stream": 999, "cols": 80, "rows": 24 }),
    )
    .unwrap();
}

#[test]
fn phase3_commands_fail_cleanly_without_connection() {
    let h = harness();
    wait_for("estado desconectado", || {
        engine_state(&h)["state"] == "disconnected"
    });
    for (cmd, args) in [
        ("container_inspect", json!({ "id": "web", "name": null })),
        (
            "container_duplicate_spec",
            json!({ "id": "web", "name": null }),
        ),
        (
            "container_create",
            json!({ "spec": { "image": "nginx" }, "start": true }),
        ),
        ("image_defaults", json!({ "image": "nginx" })),
        ("networks_brief", json!({})),
        ("volumes_brief", json!({})),
    ] {
        let err = invoke(&h, cmd, args).unwrap_err();
        assert_eq!(err["code"], "engine.notConnected", "{cmd}: {err}");
    }
}

/// Contra Docker real: crear con el asistente, inspeccionar, duplicar y eliminar.
#[test]
fn live_create_inspect_duplicate_via_ipc() {
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
    let images = invoke(&h, "images_list", json!({})).unwrap();
    let Some(image) = images.as_array().unwrap().iter().find_map(|i| {
        i["repoTags"]
            .as_array()
            .and_then(|t| t.first())
            .and_then(|t| t.as_str())
            .map(str::to_owned)
    }) else {
        eprintln!("omitida: no hay imágenes locales");
        return;
    };
    let name = format!("dcc-test-ipc-{}", std::process::id());
    let spec = json!({ "name": name, "image": image, "labels": [{ "key": "dcc.test", "value": "true" }], "env": [{ "key": "A", "value": "1" }] });
    // Sin iniciar: algunas imágenes terminan enseguida y no importa aquí.
    let r = invoke(
        &h,
        "container_create",
        json!({ "spec": spec, "start": false }),
    )
    .unwrap();
    let id = r["id"].as_str().unwrap().to_owned();
    assert_eq!(r["name"], name.as_str());
    assert!(r["state"].is_null() && r["startError"].is_null());
    let d = invoke(&h, "container_inspect", json!({ "id": id, "name": name })).unwrap();
    assert_eq!(d["status"], "created");
    assert!(d["env"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["key"] == "A" && e["value"] == "1"));
    let dup = invoke(
        &h,
        "container_duplicate_spec",
        json!({ "id": id, "name": name }),
    )
    .unwrap();
    assert_eq!(dup["sourceName"], name.as_str());
    assert!(dup["spec"]["env"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["key"] == "A"));
    // Validación en el backend: una especificación inválida devuelve los problemas.
    let err = invoke(
        &h,
        "container_create",
        json!({ "spec": { "image": image, "name": "mal nombre" }, "start": false }),
    )
    .unwrap_err();
    assert_eq!(err["code"], "engine.invalidSpec");
    assert_eq!(err["issues"][0]["code"], "name.invalid");
    invoke(
        &h,
        "container_remove",
        json!({ "id": id, "name": name, "force": true, "volumes": true, "confirmed": true }),
    )
    .unwrap();
}

// ------------------------------------------------------------------ Fase 4

#[test]
fn image_ref_parse_normalizes_and_reports_stable_codes() {
    let h = harness();
    let v = invoke(&h, "image_ref_parse", json!({ "reference": "nginx" })).unwrap();
    assert_eq!(
        v,
        json!({ "registry": "docker.io", "repository": "library/nginx", "tag": null, "digest": null })
    );
    let e = invoke(&h, "image_ref_parse", json!({ "reference": "Nginx:1" })).unwrap_err();
    assert_eq!(e["code"], "ref.invalidRepository");
    let e = invoke(&h, "image_ref_parse", json!({ "reference": "nginx:-x" })).unwrap_err();
    assert_eq!(e["code"], "ref.invalidTag");
}

#[test]
fn build_preview_validates_scans_context_and_hides_secrets_without_docker() {
    let h = harness();
    let v = invoke(
        &h,
        "build_preview",
        json!({ "spec": { "context": "/no/existe/dcc", "tags": ["app:1"] } }),
    )
    .unwrap();
    assert_eq!(v["issues"][0]["code"], "context.missing");
    assert!(v["context"].is_null());

    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("Dockerfile"), "FROM alpine\n").unwrap();
    std::fs::write(d.path().join(".dockerignore"), "*.log\n").unwrap();
    std::fs::write(d.path().join("a.log"), "x").unwrap();
    std::fs::write(d.path().join("app.txt"), "hola").unwrap();
    let ctx = d.path().to_string_lossy().into_owned();
    let v = invoke(
        &h,
        "build_preview",
        json!({ "spec": { "context": ctx, "tags": ["app:1"], "buildArgs": [{ "key": "NPM_TOKEN", "value": "s3creto" }], "builder": "classic" } }),
    )
    .unwrap();
    assert_eq!(v["context"]["files"], 3, "{v}");
    assert_eq!(v["context"]["excluded"], 1);
    assert_eq!(v["secretsHidden"], 1);
    let cmd = v["command"].as_str().unwrap();
    assert!(
        cmd.starts_with(
            "DOCKER_BUILDKIT=0 docker build --tag app:1 --build-arg 'NPM_TOKEN=********'"
        ),
        "{cmd}"
    );
    assert!(!v["request"].as_str().unwrap().contains("s3creto"));
    assert_eq!(v["issues"][0]["code"], "arg.secret");
    // Nada de esto habla con Docker.
    let list = invoke(&h, "activity_list", json!({})).unwrap();
    assert!(list
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["category"] == "system"));
}

#[test]
fn phase4_commands_fail_cleanly_without_connection() {
    let h = harness();
    wait_for("estado desconectado", || {
        engine_state(&h)["state"] == "disconnected"
    });
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("Dockerfile"), "FROM alpine\n").unwrap();
    let ctx = d.path().to_string_lossy().into_owned();
    for (cmd, args) in [
        ("image_details", json!({ "reference": "nginx" })),
        ("image_history", json!({ "reference": "nginx" })),
        (
            "image_tag",
            json!({ "source": "nginx", "repo": "web", "tag": "1" }),
        ),
        (
            "image_remove",
            json!({ "reference": "nginx", "force": false, "confirmed": true }),
        ),
        ("images_prune", json!({ "all": false, "confirmed": true })),
        (
            "images_search",
            json!({ "term": "nginx", "limit": 10, "officialOnly": false }),
        ),
        (
            "image_pull_open",
            json!({ "reference": "nginx", "auth": { "mode": "dockerConfig" }, "onEvent": "__CHANNEL__:7" }),
        ),
        (
            "image_pull_open",
            json!({ "reference": "reg.io/app", "auth": { "mode": "manual", "username": "ana", "password": "x" }, "onEvent": "__CHANNEL__:8" }),
        ),
        (
            "build_open",
            json!({ "spec": { "context": ctx, "tags": ["app:1"] }, "onEvent": "__CHANNEL__:9" }),
        ),
    ] {
        let err = invoke(&h, cmd, args).unwrap_err();
        assert_eq!(err["code"], "engine.notConnected", "{cmd}: {err}");
    }
    // La confirmación y la validación se comprueban antes de necesitar Docker.
    let e = invoke(
        &h,
        "image_remove",
        json!({ "reference": "nginx", "force": false, "confirmed": false }),
    )
    .unwrap_err();
    assert_eq!(e["code"], "engine.notConfirmed");
    let e = invoke(
        &h,
        "images_prune",
        json!({ "all": true, "confirmed": false }),
    )
    .unwrap_err();
    assert_eq!(e["code"], "engine.notConfirmed");
    let e = invoke(
        &h,
        "image_pull_open",
        json!({ "reference": "Mal", "auth": { "mode": "anonymous" }, "onEvent": "__CHANNEL__:10" }),
    )
    .unwrap_err();
    assert_eq!(e["code"], "ref.invalidRepository");
    let e = invoke(
        &h,
        "build_open",
        json!({ "spec": { "context": "/no/existe/dcc" }, "onEvent": "__CHANNEL__:11" }),
    )
    .unwrap_err();
    assert_eq!(e["code"], "engine.invalidSpec");
    assert_eq!(e["issues"][0]["code"], "context.missing");
}

/// Contra Docker real: inspeccionar, capas y construir a través de la IPC.
#[test]
fn live_images_via_ipc() {
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
    let images = invoke(&h, "images_list", json!({})).unwrap();
    let Some(image) = images.as_array().unwrap().iter().find_map(|i| {
        i["repoTags"]
            .as_array()
            .and_then(|t| t.first())
            .and_then(|t| t.as_str())
            .map(str::to_owned)
    }) else {
        eprintln!("omitida: no hay imágenes locales con etiqueta");
        return;
    };
    let d = invoke(&h, "image_details", json!({ "reference": image })).unwrap();
    assert!(d["repoTags"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t == image.as_str()));
    assert!(d["raw"].as_str().unwrap().contains("\"Id\""));
    let layers = invoke(&h, "image_history", json!({ "reference": image })).unwrap();
    assert!(!layers.as_array().unwrap().is_empty());

    // Construcción real: el resultado se observa en Docker.
    let ctx = tempfile::tempdir().unwrap();
    std::fs::write(
        ctx.path().join("Dockerfile"),
        format!("FROM {image}\nLABEL dcc.test=true\n"),
    )
    .unwrap();
    let tag = format!("dcc-test/ipc-construida-{}:1", std::process::id());
    let sid = invoke(
        &h,
        "build_open",
        json!({ "spec": { "context": ctx.path().to_string_lossy(), "tags": [tag] }, "onEvent": "__CHANNEL__:21" }),
    )
    .unwrap();
    assert!(sid.as_u64().is_some());
    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    let built = loop {
        if let Ok(v) = invoke(&h, "image_details", json!({ "reference": tag })) {
            break v;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "la construcción no terminó"
        );
        std::thread::sleep(Duration::from_millis(500));
    };
    assert_eq!(built["labels"]["dcc.test"], "true");
    let r = invoke(
        &h,
        "image_remove",
        json!({ "reference": tag, "force": false, "confirmed": true }),
    )
    .unwrap();
    assert!(r["untagged"]
        .as_array()
        .unwrap()
        .iter()
        .any(|u| u.as_str().unwrap().ends_with(&tag)));
}

#[path = "../../crates/dcc-engine/tests/common/registry.rs"]
mod test_registry;

fn pull_records(h: &Harness) -> Vec<serde_json::Value> {
    invoke(h, "activity_list", json!({}))
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["kind"] == "image.pull")
        .cloned()
        .collect()
}

fn wait_long<F: Fn() -> bool>(what: &str, secs: u64, f: F) {
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    while !f() {
        assert!(std::time::Instant::now() < deadline, "sin {what}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Contra Docker real: descarga completa y cancelada desde un registro local a
/// través de los mismos comandos que usa la ventana (`image_pull_open` y
/// `stream_close`). Un registro de actividad por descarga.
#[test]
fn live_pull_and_cancel_via_ipc() {
    if std::env::var("DCC_DOCKER_TESTS").as_deref() != Ok("1") {
        eprintln!("omitida: define DCC_DOCKER_TESTS=1");
        return;
    }
    let rt = tokio::runtime::Runtime::new().unwrap();
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
    let remove = |r: &str| {
        let _ = invoke(
            &h,
            "image_remove",
            json!({ "reference": r, "force": true, "confirmed": true }),
        );
    };

    // Descarga completa.
    let served = test_registry::unique(1024 * 1024);
    let layer = served.layers()[0].0.clone();
    let reg = rt.block_on(test_registry::Registry::start(
        served,
        test_registry::Options {
            chunk_delay: Some(Duration::from_millis(5)),
            ..Default::default()
        },
    ));
    let reference = format!("{}/dcc-test/ipc-descarga:1", reg.addr);
    let sid = invoke(
        &h,
        "image_pull_open",
        json!({ "reference": reference, "auth": { "mode": "anonymous" }, "onEvent": "__CHANNEL__:31" }),
    )
    .unwrap();
    assert!(sid.as_u64().is_some());
    wait_long("fin de la descarga", 60, || {
        pull_records(&h)
            .first()
            .is_some_and(|r| r["status"] != "running")
    });
    let recs = pull_records(&h);
    assert_eq!(recs.len(), 1, "{recs:?}");
    assert_eq!(recs[0]["status"], "succeeded", "{:?}", recs[0]);
    let summary = recs[0]["summary"].as_str().unwrap();
    assert!(
        summary.contains("1 capas descargadas, 0 ya existían"),
        "{summary}"
    );
    assert_eq!(reg.layer_gets(&layer), 1);
    invoke(&h, "image_details", json!({ "reference": reference })).expect("imagen descargada");
    remove(&reference);

    // Descarga cancelada con stream_close, como el botón Cancelar.
    let served = test_registry::unique(1024 * 1024);
    let layer = served.layers()[0].0.clone();
    let slow = rt.block_on(test_registry::Registry::start(
        served,
        test_registry::Options {
            blob_delay: Some(Duration::from_secs(12)),
            chunk_delay: Some(Duration::from_millis(5)),
            ..Default::default()
        },
    ));
    let reference = format!("{}/dcc-test/ipc-cancelada:1", slow.addr);
    let sid = invoke(
        &h,
        "image_pull_open",
        json!({ "reference": reference, "auth": { "mode": "anonymous" }, "onEvent": "__CHANNEL__:32" }),
    )
    .unwrap();
    wait_long("petición de la capa", 10, || slow.layer_gets(&layer) == 1);
    let t0 = std::time::Instant::now();
    assert_eq!(
        invoke(&h, "stream_close", json!({ "stream": sid })).unwrap(),
        json!(true)
    );
    wait_long("registro cancelado", 5, || {
        pull_records(&h)
            .first()
            .is_some_and(|r| r["status"] != "running")
    });
    assert!(t0.elapsed() < Duration::from_secs(5));
    let recs = pull_records(&h);
    assert_eq!(recs.len(), 2, "un registro por descarga: {recs:?}");
    assert_eq!(recs[0]["status"], "failed");
    assert!(
        recs[0]["error"]
            .as_str()
            .unwrap()
            .contains("cancelada por el usuario"),
        "{:?}",
        recs[0]
    );
    // Docker no llega a recibir la capa entera (ver D-045).
    wait_long("cierre de la conexión con el registro", 25, || {
        !slow.aborted.lock().unwrap().is_empty()
    });
    assert_eq!(*slow.aborted.lock().unwrap(), vec![layer]);
    assert!(invoke(&h, "image_details", json!({ "reference": reference })).is_err());
    remove(&reference);
}
