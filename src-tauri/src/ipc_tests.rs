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

/// Invoca como la interfaz con el equipo local seleccionado: si la llamada no
/// indica servidor, se añade `server: "local"` (la interfaz siempre lo envía).
/// Las pruebas de aislamiento usan [`invoke_exact`].
fn invoke(
    h: &Harness,
    cmd: &str,
    body: serde_json::Value,
) -> Result<serde_json::Value, serde_json::Value> {
    let mut body = body;
    if let Some(o) = body.as_object_mut() {
        o.entry("server").or_insert_with(|| json!("local"));
    }
    invoke_exact(h, cmd, body)
}

/// Invoca exactamente con los argumentos dados.
fn invoke_exact(
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
fn backup_restore_requires_explicit_confirmation() {
    let h = harness();
    let err = invoke(
        &h,
        "container_restore_backup",
        json!({ "id": "web-dcc-backup-20261111-000000", "start": true, "confirmed": false }),
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
    let all_kinds: Vec<_> = records.iter().rev().map(|r| r.kind.as_str()).collect();
    // Fase 8 (D-057): al conectar, la aplicación abre una suscripción a los
    // eventos de Docker, que se registra una sola vez, correcta, después de la
    // conexión (ping, versión, info). Llega en paralelo a las consultas de la
    // prueba, así que su posición exacta entre ellas no es fija.
    let watch_at: Vec<_> = all_kinds
        .iter()
        .enumerate()
        .filter(|(_, k)| **k == "events.watch")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        watch_at.len(),
        1,
        "una sola suscripción a eventos: {all_kinds:?}"
    );
    assert!(
        watch_at[0] >= 3,
        "la suscripción se abre tras conectar: {all_kinds:?}"
    );
    let kinds: Vec<_> = all_kinds
        .iter()
        .copied()
        .filter(|k| *k != "events.watch")
        .collect();
    // El resto son exactamente las operaciones que ha pedido la prueba.
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
        // 0.9.2
        ("container_edit_config", json!({ "id": "web" })),
        (
            "container_edit_plan",
            json!({ "id": "web", "request": { "name": "web" } }),
        ),
        (
            "container_edit_apply",
            json!({ "id": "web", "request": { "name": "x" }, "options": { "confirmRecreate": true } }),
        ),
        (
            "container_commit_preview",
            json!({ "id": "web", "request": { "repository": "a" } }),
        ),
        (
            "container_commit",
            json!({ "id": "web", "request": { "repository": "a", "tag": "1" } }),
        ),
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
    // Una imagen local con etiqueta y con comando o entrypoint: Docker no crea
    // contenedores de una imagen sin ninguno de los dos (las importadas con
    // `docker import`, por ejemplo), y aquí no se indica comando.
    let Some(image) = images.as_array().unwrap().iter().find_map(|i| {
        let tag = i["repoTags"].as_array()?.first()?.as_str()?.to_owned();
        let d = invoke(&h, "image_details", json!({ "reference": i["id"] })).ok()?;
        let runnable = d["command"].as_array().is_some_and(|c| !c.is_empty())
            || d["entrypoint"].as_array().is_some_and(|c| !c.is_empty());
        runnable.then_some(tag)
    }) else {
        eprintln!("omitida: no hay imágenes locales con comando");
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

#[test]
fn network_preview_validates_and_shows_exact_request() {
    let h = harness();
    let v = invoke(
        &h,
        "network_preview",
        json!({ "spec": { "name": "front", "ipv4": { "subnet": "10.240.0.0/24", "gateway": "10.240.0.1" }, "labels": [{ "key": "app", "value": "mi web" }], "internal": true } }),
    )
    .unwrap();
    assert_eq!(v["issues"], json!([]));
    assert_eq!(
        v["command"],
        "docker network create --driver bridge --subnet 10.240.0.0/24 --gateway 10.240.0.1 --label 'app=mi web' --internal front"
    );
    assert_eq!(v["request"], "POST /networks/create");
    let body: serde_json::Value = serde_json::from_str(v["body"].as_str().unwrap()).unwrap();
    assert_eq!(
        body,
        json!({ "Name": "front", "Driver": "bridge", "Internal": true, "Labels": { "app": "mi web" }, "CheckDuplicate": true,
                "IPAM": { "Driver": "default", "Config": [{ "Subnet": "10.240.0.0/24", "Gateway": "10.240.0.1" }] } })
    );
    let v = invoke(
        &h,
        "network_preview",
        json!({ "spec": { "name": "bridge", "ipv4": { "subnet": "10.241.0.5/24" } } }),
    )
    .unwrap();
    let codes: Vec<_> = v["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["code"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(codes, ["name.reserved", "subnet.hostBits"]);
    assert_eq!(v["issues"][1]["params"]["fixed"], "10.241.0.0/24");
    // Nada de esto habla con Docker.
    let list = invoke(&h, "activity_list", json!({})).unwrap();
    assert!(list
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["category"] == "system"));
}

#[test]
fn phase5_commands_fail_cleanly_without_connection() {
    let h = harness();
    wait_for("estado desconectado", || {
        engine_state(&h)["state"] == "disconnected"
    });
    for (cmd, args) in [
        ("networks_list", json!({})),
        ("network_details", json!({ "id": "front", "name": null })),
        ("network_create", json!({ "spec": { "name": "front" } })),
        (
            "network_remove",
            json!({ "id": "front", "name": "front", "confirmed": true }),
        ),
        ("networks_prune", json!({ "confirmed": true })),
        (
            "network_connect",
            json!({ "network": "front", "networkName": "front", "container": "web", "containerName": "web", "options": {}, "subnets": [] }),
        ),
        (
            "network_disconnect",
            json!({ "network": "front", "networkName": "front", "container": "web", "containerName": "web", "force": false, "confirmed": true }),
        ),
    ] {
        let err = invoke(&h, cmd, args).unwrap_err();
        assert_eq!(err["code"], "engine.notConnected", "{cmd}: {err}");
    }
    // Confirmación y validación antes de necesitar Docker.
    for (cmd, args) in [
        (
            "network_remove",
            json!({ "id": "front", "name": null, "confirmed": false }),
        ),
        ("networks_prune", json!({ "confirmed": false })),
        (
            "network_disconnect",
            json!({ "network": "front", "networkName": null, "container": "web", "containerName": null, "force": false, "confirmed": false }),
        ),
    ] {
        let err = invoke(&h, cmd, args).unwrap_err();
        assert_eq!(err["code"], "engine.notConfirmed", "{cmd}: {err}");
    }
    let e = invoke(
        &h,
        "network_create",
        json!({ "spec": { "name": "mi red" } }),
    )
    .unwrap_err();
    assert_eq!(e["code"], "engine.invalidSpec");
    assert_eq!(e["issues"][0]["code"], "name.invalid");
    let e = invoke(
        &h,
        "network_connect",
        json!({ "network": "front", "networkName": null, "container": "web", "containerName": null,
                "options": { "ipv4": "10.9.0.5" }, "subnets": ["10.8.0.0/24"] }),
    )
    .unwrap_err();
    assert_eq!(e["code"], "engine.invalidInput", "{e}");
}

/// Contra Docker real: crear, listar, conectar, ver el detalle, desconectar y
/// eliminar una red con los comandos que usa la ventana.
#[test]
fn live_networks_via_ipc() {
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
    let name = format!("dcc-test-ipc-net-{}", std::process::id());
    let created = invoke(
        &h,
        "network_create",
        json!({ "spec": { "name": name, "labels": [{ "key": "dcc.test", "value": "true" }] } }),
    )
    .unwrap();
    let id = created["id"].as_str().unwrap().to_owned();
    /// Elimina la red y el contenedor de prueba aunque la prueba falle.
    struct Guard<'a>(&'a Harness, String, std::cell::RefCell<Option<String>>);
    impl Drop for Guard<'_> {
        fn drop(&mut self) {
            if let Some(c) = self.2.borrow().as_ref() {
                let _ = invoke(
                    self.0,
                    "container_remove",
                    json!({ "id": c, "name": null, "force": true, "volumes": true, "confirmed": true }),
                );
            }
            let _ = invoke(
                self.0,
                "network_remove",
                json!({ "id": self.1, "name": null, "confirmed": true }),
            );
        }
    }
    let guard = Guard(&h, id.clone(), std::cell::RefCell::new(None));
    let list = invoke(&h, "networks_list", json!({})).unwrap();
    let n = list
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["name"] == name.as_str())
        .cloned();
    let n = n.expect("la red aparece en la lista");
    assert_eq!(n["predefined"], false);
    assert_eq!(n["labels"]["dcc.test"], "true");
    let subnets = n["subnets"].clone();

    // Un contenedor detenido cualquiera de prueba: se crea con la imagen de prueba
    // si existe; si no, solo se prueba la red.
    let has_fixture = invoke(&h, "images_list", json!({}))
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .any(|i| {
            i["repoTags"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t == "dcc-test/sleeper:2")
        });
    if has_fixture {
        let cname = format!("dcc-test-ipc-net-c-{}", std::process::id());
        let c = invoke(
            &h,
            "container_create",
            json!({ "spec": { "name": cname, "image": "dcc-test/sleeper:2", "labels": [{ "key": "dcc.test", "value": "true" }] }, "start": false }),
        )
        .unwrap();
        let cid = c["id"].as_str().unwrap().to_owned();
        *guard.2.borrow_mut() = Some(cid.clone());
        invoke(
            &h,
            "network_connect",
            json!({ "network": id, "networkName": name, "container": cid, "containerName": cname,
                    "options": { "aliases": ["web"] }, "subnets": subnets }),
        )
        .unwrap();
        let d = invoke(&h, "network_details", json!({ "id": id, "name": name })).unwrap();
        assert_eq!(d["summary"]["containers"][0]["name"], cname.as_str(), "{d}");
        invoke(
            &h,
            "network_disconnect",
            json!({ "network": id, "networkName": name, "container": cid, "containerName": cname, "force": false, "confirmed": true }),
        )
        .unwrap();
        let d = invoke(&h, "network_details", json!({ "id": id, "name": name })).unwrap();
        assert_eq!(d["summary"]["containers"], json!([]));
        invoke(
            &h,
            "container_remove",
            json!({ "id": cid, "name": cname, "force": true, "volumes": true, "confirmed": true }),
        )
        .unwrap();
        *guard.2.borrow_mut() = None;
    } else {
        eprintln!("omitida (parcial): sin la imagen de prueba no se conecta ningún contenedor");
    }
    invoke(
        &h,
        "network_remove",
        json!({ "id": id, "name": name, "confirmed": true }),
    )
    .unwrap();
    let e = invoke(&h, "network_details", json!({ "id": id, "name": name })).unwrap_err();
    assert_eq!(e["httpStatus"], 404, "{e}");
    let kinds: Vec<String> = invoke(&h, "activity_list", json!({}))
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["kind"].as_str().unwrap().to_owned())
        .filter(|k| k.starts_with("network."))
        .collect();
    for k in ["network.create", "network.list", "network.remove"] {
        assert!(kinds.iter().any(|x| x == k), "{k}: {kinds:?}");
    }
}

#[test]
fn volume_preview_validates_and_shows_exact_request() {
    let h = harness();
    let v = invoke(
        &h,
        "volume_preview",
        json!({ "spec": { "name": "datos", "driverOpts": [{ "key": "type", "value": "tmpfs" }, { "key": "device", "value": "tmpfs" }], "labels": [{ "key": "app", "value": "web" }] } }),
    )
    .unwrap();
    assert_eq!(v["issues"], json!([]));
    assert_eq!(
        v["command"],
        "docker volume create --driver local --opt device=tmpfs --opt type=tmpfs --label app=web datos"
    );
    assert_eq!(v["request"], "POST /volumes/create");
    let body: serde_json::Value = serde_json::from_str(v["body"].as_str().unwrap()).unwrap();
    assert_eq!(
        body,
        json!({ "Name": "datos", "Driver": "local", "DriverOpts": { "type": "tmpfs", "device": "tmpfs" }, "Labels": { "app": "web" } })
    );
    let v = invoke(&h, "volume_preview", json!({ "spec": { "name": "a" } })).unwrap();
    assert_eq!(v["issues"][0]["code"], "volumeName.short");
    let list = invoke(&h, "activity_list", json!({})).unwrap();
    assert!(list
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["category"] == "system"));
}

#[test]
fn phase6_commands_fail_cleanly_without_connection() {
    let h = harness();
    wait_for("estado desconectado", || {
        engine_state(&h)["state"] == "disconnected"
    });
    for (cmd, args) in [
        ("volumes_list", json!({})),
        ("volume_details", json!({ "name": "datos" })),
        ("volume_sizes", json!({})),
        ("volume_create", json!({ "spec": { "name": "datos" } })),
        (
            "volume_remove",
            json!({ "name": "datos", "confirmed": true }),
        ),
        ("volumes_prune", json!({ "all": false, "confirmed": true })),
        ("binds_list", json!({})),
    ] {
        let err = invoke(&h, cmd, args).unwrap_err();
        assert_eq!(err["code"], "engine.notConnected", "{cmd}: {err}");
    }
    for (cmd, args) in [
        (
            "volume_remove",
            json!({ "name": "datos", "confirmed": false }),
        ),
        ("volumes_prune", json!({ "all": true, "confirmed": false })),
    ] {
        let err = invoke(&h, cmd, args).unwrap_err();
        assert_eq!(err["code"], "engine.notConfirmed", "{cmd}: {err}");
    }
    let e = invoke(&h, "volume_create", json!({ "spec": { "name": "a b" } })).unwrap_err();
    assert_eq!(e["code"], "engine.invalidSpec");
    assert_eq!(e["issues"][0]["code"], "volumeName.invalid");
}

/// Contra Docker real: crear, listar, ver el detalle, medir y eliminar un
/// volumen, y listar directorios vinculados, con los comandos de la ventana.
#[test]
fn live_volumes_via_ipc() {
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
    let name = format!("dcc-test-ipc-vol-{}", std::process::id());
    struct Guard<'a>(&'a Harness, String);
    impl Drop for Guard<'_> {
        fn drop(&mut self) {
            let _ = invoke(
                self.0,
                "volume_remove",
                json!({ "name": self.1, "confirmed": true }),
            );
        }
    }
    let created = invoke(
        &h,
        "volume_create",
        json!({ "spec": { "name": name, "labels": [{ "key": "dcc.test", "value": "true" }] } }),
    )
    .unwrap();
    let _g = Guard(&h, name.clone());
    assert_eq!(created["name"], name.as_str());
    // Repetido: error de validación con el código volumeName.exists.
    let e = invoke(&h, "volume_create", json!({ "spec": { "name": name } })).unwrap_err();
    assert_eq!(e["code"], "engine.invalidSpec", "{e}");
    assert_eq!(e["issues"][0]["code"], "volumeName.exists");
    let list = invoke(&h, "volumes_list", json!({})).unwrap();
    assert!(list
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["name"] == name.as_str()));
    let d = invoke(&h, "volume_details", json!({ "name": name })).unwrap();
    assert_eq!(d["summary"]["labels"]["dcc.test"], "true");
    let sizes = invoke(&h, "volume_sizes", json!({})).unwrap();
    assert!(
        sizes
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["name"] == name.as_str()),
        "{sizes}"
    );
    assert!(invoke(&h, "binds_list", json!({})).unwrap().is_array());
    invoke(
        &h,
        "volume_remove",
        json!({ "name": name, "confirmed": true }),
    )
    .unwrap();
    let e = invoke(&h, "volume_details", json!({ "name": name })).unwrap_err();
    assert_eq!(e["httpStatus"], 404, "{e}");
}

// ------------------------------------------------------------------ Fase 7

#[test]
fn compose_files_via_ipc_create_read_save_and_conflict() {
    let h = harness();
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().display().to_string();
    assert!(invoke(&h, "compose_find_file", json!({ "dir": dir }))
        .unwrap()
        .is_null());
    let c = invoke(
        &h,
        "compose_file_create",
        json!({ "dir": dir, "content": "services: {}\n" }),
    )
    .unwrap();
    let path = c["path"].as_str().unwrap().to_owned();
    assert!(path.ends_with("/compose.yaml"));
    assert_eq!(
        invoke(&h, "compose_find_file", json!({ "dir": dir })).unwrap(),
        path.as_str()
    );
    let e = invoke(
        &h,
        "compose_file_create",
        json!({ "dir": dir, "content": "x" }),
    )
    .unwrap_err();
    assert_eq!(e["code"], "engine.alreadyExists", "{e}");
    let r = invoke(&h, "compose_file_read", json!({ "path": path })).unwrap();
    assert_eq!(r["content"], "services: {}\n");
    let v1 = r["version"].as_str().unwrap().to_owned();
    let s = invoke(
        &h,
        "compose_file_save",
        json!({ "path": path, "content": "services:\n  a:\n    image: x\n", "version": v1 }),
    )
    .unwrap();
    assert_ne!(s["version"], v1.as_str());
    // Guardar con la versión antigua: conflicto, el archivo no cambia.
    let e = invoke(
        &h,
        "compose_file_save",
        json!({ "path": path, "content": "pisado\n", "version": v1 }),
    )
    .unwrap_err();
    assert_eq!(e["code"], "engine.fileChanged", "{e}");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "services:\n  a:\n    image: x\n"
    );
    let e = invoke(&h, "compose_file_read", json!({ "path": "relativo.yaml" })).unwrap_err();
    assert_eq!(e["code"], "engine.invalidInput");
}

#[test]
fn phase7_commands_fail_cleanly_without_connection() {
    let h = harness();
    wait_for("estado desconectado", || {
        engine_state(&h)["state"] == "disconnected"
    });
    let p = json!({ "name": "tienda", "files": ["/srv/tienda/compose.yaml"] });
    for (cmd, body) in [
        ("compose_projects", json!({})),
        ("compose_config", json!({ "project": p })),
        (
            "compose_run_open",
            json!({ "project": p, "action": { "action": "stop" }, "confirmed": false, "onEvent": "__CHANNEL__:70" }),
        ),
        (
            "compose_logs_open",
            json!({ "project": p, "services": [], "tail": 10, "timestamps": false, "onEvent": "__CHANNEL__:71" }),
        ),
        (
            "compose_register",
            json!({ "files": ["/srv/tienda/compose.yaml"] }),
        ),
    ] {
        let e = invoke(&h, cmd, body).unwrap_err();
        assert_eq!(e["code"], "engine.notConnected", "{cmd}: {e}");
    }
    // Lo que no necesita Docker funciona igual.
    assert_eq!(
        invoke(&h, "compose_unregister", json!({ "name": "nada" })).unwrap(),
        false
    );
}

#[test]
fn registered_projects_survive_a_stale_config_set() {
    let h = harness();
    let stale = h.app.state::<AppState>().config();
    h.app
        .state::<AppState>()
        .update_compose(|ps| {
            ps.push(dcc_store::SavedProject {
                name: "tienda".into(),
                files: vec!["/srv/tienda/compose.yaml".into()],
            });
            Ok(())
        })
        .unwrap();
    let mut cfg = stale;
    cfg.ui.theme = Theme::Dark;
    let v = invoke(&h, "config_set", json!({ "config": cfg })).unwrap();
    assert_eq!(v["compose"]["projects"][0]["name"], "tienda");
    assert_eq!(h.store.load().unwrap().config.compose.projects.len(), 1);
    assert_eq!(
        invoke(&h, "compose_unregister", json!({ "name": "tienda" })).unwrap(),
        true
    );
    assert!(h.store.load().unwrap().config.compose.projects.is_empty());
}

/// Contra Docker real con el Compose del equipo: añadir un proyecto (nombre
/// decidido por Compose), validar, desplegar y eliminar por los comandos de la
/// ventana. Eliminar sin confirmar se rechaza antes de ejecutar nada.
#[test]
fn live_compose_via_ipc() {
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
    let status = invoke(&h, "compose_status", json!({})).unwrap();
    if status["status"] != "available" {
        eprintln!(
            "omitida: Docker Compose no está instalado en este equipo ({})",
            status["status"]
        );
        return;
    }
    let images = invoke(&h, "images_list", json!({})).unwrap();
    let tags: Vec<String> = images
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|i| i["repoTags"].as_array().cloned().unwrap_or_default())
        .filter_map(|t| t.as_str().map(str::to_owned))
        .filter(|t| !t.ends_with(":<none>"))
        .collect();
    let image = tags
        .iter()
        .find(|t| t.as_str() == "dcc-test/sleeper:2")
        .or(tags.first())
        .cloned()
        .expect("hace falta al menos una imagen local con etiqueta (las pruebas del motor crean dcc-test/sleeper:2)");

    let proj_dir = tempfile::Builder::new()
        .prefix("dcc-test-ipc7-")
        .tempdir()
        .unwrap();
    let name = format!("dcc-test-ipc7-{}", std::process::id());
    let created = invoke(
        &h,
        "compose_file_create",
        json!({
            "dir": proj_dir.path().display().to_string(),
            "content": format!("name: {name}\nservices:\n  app:\n    image: {image}\n    labels: {{dcc.test: \"true\"}}\nnetworks:\n  default:\n    labels: {{dcc.test: \"true\"}}\n"),
        }),
    )
    .unwrap();
    let file = created["path"].as_str().unwrap().to_owned();
    struct Guard<'a>(&'a Harness, serde_json::Value);
    impl Drop for Guard<'_> {
        fn drop(&mut self) {
            let _ = invoke(
                self.0,
                "compose_run_open",
                json!({ "project": self.1, "action": { "action": "down" }, "confirmed": true, "onEvent": "__CHANNEL__:79" }),
            );
            std::thread::sleep(Duration::from_secs(5));
        }
    }

    // Archivos no válidos: no se añade nada y se devuelve el mensaje de Compose.
    let bad = proj_dir.path().join("mal.yaml");
    std::fs::write(&bad, "services:\n  a:\n    ports: 12\n").unwrap();
    let r = invoke(
        &h,
        "compose_register",
        json!({ "files": [bad.display().to_string()] }),
    )
    .unwrap();
    assert!(r["project"].is_null());
    assert!(r["error"].as_str().unwrap().contains("ports"), "{r}");

    let r = invoke(&h, "compose_register", json!({ "files": [file] })).unwrap();
    assert_eq!(
        r["project"]["name"],
        name.as_str(),
        "el nombre lo decide Compose (name:)"
    );
    let project = r["project"].clone();
    let _g = Guard(&h, project.clone());
    let e = invoke(&h, "compose_register", json!({ "files": [file] })).unwrap_err();
    assert_eq!(e["code"], "compose.duplicate");
    let listed = invoke(&h, "compose_projects", json!({})).unwrap();
    let mine = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == name.as_str())
        .unwrap()
        .clone();
    assert_eq!(mine["registered"], true);
    assert_eq!(mine["state"], "notDeployed");

    let cfg = invoke(&h, "compose_config", json!({ "project": project })).unwrap();
    assert_eq!(cfg["model"]["services"][0]["name"], "app");
    let cfg = invoke(&h, "compose_config", json!({ "project": project, "unsaved": { "index": 0, "content": "services:\n  a:\n    ports: 12\n" } })).unwrap();
    assert!(cfg["error"].as_str().unwrap().contains("ports"));
    let prev = invoke(
        &h,
        "compose_preview",
        json!({ "project": project, "action": { "action": "up", "build": true } }),
    )
    .unwrap();
    let cmd = prev["command"].as_str().unwrap();
    assert!(
        cmd.contains(&format!("-p {name}")) && cmd.contains("up --detach --build"),
        "{cmd}"
    );

    let sid = invoke(&h, "compose_run_open", json!({ "project": project, "action": { "action": "up" }, "confirmed": false, "onEvent": "__CHANNEL__:72" })).unwrap();
    assert!(sid.as_u64().is_some());
    wait_long("despliegue", 120, || {
        invoke(&h, "compose_projects", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["name"] == name.as_str() && p["total"] == 1)
    });
    wait_long("fin de la operación", 60, || {
        invoke(&h, "activity_list", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .any(|r| {
                r["kind"] == "compose.up"
                    && r["target"] == name.as_str()
                    && r["status"] == "succeeded"
            })
    });
    // El registro se cierra al terminar el proceso, pero la operación sigue
    // (con el proyecto bloqueado) mientras compose_settle confirma el estado;
    // la ventana espera el evento End. Aquí: hasta que la tarea termina de
    // verdad. Sin esto, con un daemon lento, «down» llegaba con el proyecto
    // aún ocupado (engine.busy), como pasó una vez en la verificación de 0.9.2.
    wait_long("fin real de la operación", 60, || {
        h.app.state::<AppState>().streams.running() == 0
    });

    // Eliminar sin confirmar: error inmediato, nada ejecutado.
    let downs = || {
        invoke(&h, "activity_list", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["kind"] == "compose.down")
            .count()
    };
    let before = downs();
    let e = invoke(&h, "compose_run_open", json!({ "project": project, "action": { "action": "down" }, "confirmed": false, "onEvent": "__CHANNEL__:73" })).unwrap_err();
    assert_eq!(e["code"], "engine.notConfirmed");
    assert_eq!(downs(), before);
    invoke(&h, "compose_run_open", json!({ "project": project, "action": { "action": "down", "volumes": true }, "confirmed": true, "onEvent": "__CHANNEL__:74" })).unwrap();
    wait_long("eliminación", 120, || {
        invoke(&h, "compose_projects", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["name"] == name.as_str() && p["state"] == "notDeployed")
    });
    assert_eq!(
        invoke(&h, "compose_unregister", json!({ "name": name })).unwrap(),
        true
    );
}

// ------------------------------------------------------------------ Fase 8

/// Simula una operación terminada (las operaciones reales se prueban en el motor).
fn record_op(h: &Harness, kind: &str, ok: bool) {
    let st = h.app.state::<AppState>();
    let op = st.activity.begin(dcc_activity::OperationSpec {
        category: dcc_activity::Category::Container,
        kind: kind.into(),
        target: Some("web".into()),
        transport: dcc_activity::Transport::Api {
            method: "POST".into(),
            path: "/build?t=web&buildargs=%7B%22NPM_TOKEN%22%3A%22token-secreto%22%7D".into(),
            endpoint: "/var/run/docker.sock".into(),
        },
        request_body: Some(r#"{"Env":["CLAVE=valor-secreto"]}"#.into()),
        equivalent_command: Some("docker run -e CLAVE=valor-secreto web".into()),
    });
    if ok {
        op.succeed(dcc_activity::Outcome {
            summary: Some("hecho".into()),
            stdout: Some("salida valor-secreto".into()),
            ..Default::default()
        });
    } else {
        op.fail(dcc_activity::Failure {
            error: "falló".into(),
            http_status: Some(500),
            ..Default::default()
        });
    }
}

fn restart(h: Harness) -> Harness {
    let Harness {
        app, window, _dir, ..
    } = h;
    drop(window);
    drop(app);
    let docker = DockerConfig {
        mode: DockerMode::Custom,
        custom_socket: _dir.path().join("no-docker.sock").display().to_string(),
    };
    harness_with(_dir, docker)
}

fn wait_history(h: &Harness, n: u64) -> serde_json::Value {
    let mut last = json!(null);
    for _ in 0..100 {
        last = invoke(h, "history_list", json!({ "filter": {} })).unwrap();
        if last["stored"].as_u64() == Some(n) {
            return last;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("el historial no llegó a {n}: {last}");
}

#[test]
fn history_survives_a_restart_without_secrets() {
    let h = harness();
    record_op(&h, "image.build", true);
    record_op(&h, "container.start", false);
    let page = wait_history(&h, 2);
    assert_eq!(page["persistent"], true);
    assert_eq!(
        page["entries"][0]["kind"], "container.start",
        "los más recientes primero"
    );
    assert_eq!(page["entries"][0]["status"], "failed");
    assert_eq!(page["entries"][0]["httpStatus"], 500);
    let dir = page["location"].as_str().unwrap().to_owned();
    let file = std::path::Path::new(&dir).join(crate::journal::HISTORY_FILE);
    let raw = std::fs::read_to_string(&file).unwrap();
    assert_eq!(raw.lines().count(), 2);
    assert!(
        !raw.contains("valor-secreto") && !raw.contains("token-secreto"),
        "{raw}"
    );
    assert!(
        raw.contains("NPM_TOKEN"),
        "se conservan los nombres de los argumentos"
    );
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let first_session = page["sessionStart"].as_str().unwrap().to_owned();

    // Otra sesión: sigue ahí, con la sesión anterior identificable.
    std::thread::sleep(Duration::from_millis(20));
    let h = restart(h);
    let page = invoke(&h, "history_list", json!({ "filter": {} })).unwrap();
    assert_eq!(page["stored"], 2);
    assert!(page["sessionStart"].as_str().unwrap() > first_session.as_str());
    assert!(
        page["entries"][0]["startedAt"].as_str().unwrap() < page["sessionStart"].as_str().unwrap()
    );
    // El panel de actividad de la nueva sesión empieza vacío (no se mezcla).
    assert!(invoke(&h, "activity_list", json!({}))
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["kind"] != "image.build"));

    // Filtros.
    let f = |filter: serde_json::Value| {
        invoke(&h, "history_list", json!({ "filter": filter })).unwrap()["total"]
            .as_u64()
            .unwrap()
    };
    assert_eq!(f(json!({ "statuses": ["failed"] })), 1);
    assert_eq!(f(json!({ "text": "BUILD" })), 1);
    assert_eq!(f(json!({ "categories": ["network"] })), 0);
    assert_eq!(
        f(json!({ "since": chrono::Utc::now().timestamp() + 10 })),
        0
    );
    assert_eq!(
        f(json!({ "until": chrono::Utc::now().timestamp() + 10, "since": 0 })),
        2
    );

    invoke(&h, "history_clear", json!({})).unwrap();
    assert_eq!(
        invoke(&h, "history_list", json!({ "filter": {} })).unwrap()["stored"],
        0
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "");
}

#[test]
fn history_is_not_kept_when_disabled() {
    let h = harness();
    let mut cfg = h.app.state::<AppState>().config();
    cfg.activity.keep_history = false;
    invoke(&h, "config_set", json!({ "config": cfg })).unwrap();
    record_op(&h, "container.stop", true);
    std::thread::sleep(Duration::from_millis(200));
    let page = invoke(&h, "history_list", json!({ "filter": {} })).unwrap();
    assert_eq!(page["stored"], 0);
    assert_eq!(page["persistent"], false);
    // El panel de actividad de la sesión sí la tiene.
    assert!(invoke(&h, "activity_list", json!({}))
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["kind"] == "container.stop"));
}

#[test]
fn phase8_commands_without_connection() {
    let h = harness();
    wait_for("estado desconectado", || {
        engine_state(&h)["state"] == "disconnected"
    });
    let now = chrono::Utc::now().timestamp();
    let e = invoke(
        &h,
        "events_query",
        json!({ "query": { "since": now - 60, "until": now } }),
    )
    .unwrap_err();
    assert_eq!(e["code"], "engine.notConnected", "{e}");
    let e = invoke(
        &h,
        "events_query",
        json!({ "query": { "since": now, "until": now - 60 } }),
    )
    .unwrap_err();
    assert!(
        e["code"] == "engine.invalidInput" || e["code"] == "engine.notConnected",
        "{e}"
    );
    let p = invoke(&h, "events_recent", json!({ "filter": {} })).unwrap();
    assert_eq!(p["total"], 0);
    let p = invoke(&h, "events_journal", json!({ "filter": {} })).unwrap();
    assert_eq!(
        (p["total"].as_u64(), p["persistent"].as_bool()),
        (Some(0), Some(true))
    );
    invoke(&h, "events_journal_clear", json!({})).unwrap();
}

/// Fase 9: información del sistema, comprobación de versiones nuevas (con un
/// servidor local real) y validación de la fuente y de los enlaces.
#[test]
fn phase9_system_info_and_updates() {
    let h = harness();
    let info = invoke(&h, "system_info", json!({})).unwrap();
    assert_eq!(info["appVersion"], env!("CARGO_PKG_VERSION"));
    assert_eq!(info["arch"], std::env::consts::ARCH);
    assert!(
        info["glibc"].as_str().is_some_and(|v| v.starts_with("2.")),
        "{info}"
    );
    assert!(info["configPath"]
        .as_str()
        .unwrap()
        .ends_with("config.toml"));
    assert!(info["stateDir"].as_str().is_some(), "{info}");

    // Sin fuente, fuente insegura o mal escrita: no sale ninguna petición.
    let e = invoke(&h, "updates_check", json!({ "source": null })).unwrap_err();
    assert_eq!(e["code"], "updates.noSource", "{e}");
    let e = invoke(
        &h,
        "updates_check",
        json!({ "source": "http://ejemplo.es/v.json" }),
    )
    .unwrap_err();
    assert_eq!(e["code"], "updates.insecureSource", "{e}");
    let e = invoke(&h, "updates_check", json!({ "source": "no es una fuente" })).unwrap_err();
    assert_eq!(e["code"], "updates.invalidSource", "{e}");
    let e = invoke(
        &h,
        "open_release_page",
        json!({ "url": "file:///etc/passwd" }),
    )
    .unwrap_err();
    assert_eq!(e["code"], "app.open", "{e}");

    // Servidor local que publica una versión más nueva.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut c, _) = listener.accept().unwrap();
        let mut buf = [0u8; 2048];
        let n = c.read(&mut buf).unwrap();
        let req = String::from_utf8_lossy(&buf[..n]).to_string();
        let body = r#"{"version":"99.0.0","url":"https://ejemplo.es/v99","notes":"Nueva"}"#;
        write!(c, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        req
    });
    let src = format!("http://{addr}/latest.json");
    let r = invoke(&h, "updates_check", json!({ "source": src })).unwrap();
    assert_eq!(
        (r["latest"].as_str(), r["newer"].as_bool()),
        (Some("99.0.0"), Some(true)),
        "{r}"
    );
    assert_eq!(r["current"], env!("CARGO_PKG_VERSION"));
    let req = server.join().unwrap();
    assert!(req.starts_with("GET /latest.json HTTP/1.1"), "{req}");
    assert!(
        req.to_lowercase().contains(&format!(
            "user-agent: docker-control-center/{}",
            env!("CARGO_PKG_VERSION")
        )),
        "{req}"
    );
    let last = invoke(&h, "updates_last", json!({})).unwrap();
    assert_eq!(last["check"]["latest"], "99.0.0");
    assert_eq!(last["automatic"], false);

    // La sección [updates] se guarda normalizada.
    let mut cfg = invoke(&h, "config_get", json!({})).unwrap();
    cfg["updates"] = json!({ "checkOnStart": true, "source": "  mi-org/dcc  " });
    let saved = invoke(&h, "config_set", json!({ "config": cfg })).unwrap();
    assert_eq!(
        saved["updates"],
        json!({ "checkOnStart": true, "source": "mi-org/dcc" })
    );
}

/// Contra Docker real: la aplicación se suscribe sola al conectar; los eventos
/// llegan a la ventana, se guardan y siguen ahí tras reiniciar la aplicación.
#[test]
fn live_events_via_ipc() {
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
    let h = harness_with(dir, docker.clone());
    let got: Arc<Mutex<Vec<serde_json::Value>>> = Default::default();
    let g2 = got.clone();
    h.app.listen_any(crate::journal::DOCKER_EVENT, move |e| {
        g2.lock()
            .unwrap()
            .push(serde_json::from_str(e.payload()).unwrap());
    });
    wait_for("conexión", || engine_state(&h)["state"] == "connected");
    wait_for("suscripción a eventos", || {
        invoke(&h, "activity_list", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "events.watch" && r["status"] == "succeeded")
    });
    let vol = format!("dcc-test-ipc8-{}", std::process::id());
    let created = invoke(
        &h,
        "volume_create",
        json!({ "spec": { "name": vol, "driver": "local", "driverOpts": [], "labels": [{ "key": "dcc.test", "value": "true" }] } }),
    );
    assert!(created.is_ok(), "{created:?}");
    invoke(
        &h,
        "volume_remove",
        json!({ "name": vol, "confirmed": true }),
    )
    .unwrap();
    wait_for("eventos en la ventana", || {
        got.lock().unwrap().iter().any(|e| {
            e["kind"] == "volume" && e["action"] == "destroy" && e["actorId"] == vol.as_str()
        })
    });
    let recent = invoke(
        &h,
        "events_recent",
        json!({ "filter": { "types": ["volume"], "text": vol } }),
    )
    .unwrap();
    assert_eq!(
        recent["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["action"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["destroy", "create"]
    );
    let now = chrono::Utc::now().timestamp();
    let q = invoke(&h, "events_query", json!({ "query": { "since": now - 120, "until": now, "types": ["volume"], "resource": vol } })).unwrap();
    assert_eq!(q["events"].as_array().unwrap().len(), 2, "{q}");

    // Tras reiniciar, el diario conserva los eventos de la sesión anterior.
    let Harness {
        app, window, _dir, ..
    } = h;
    drop(window);
    drop(app);
    let h = harness_with(_dir, docker);
    let j = invoke(&h, "events_journal", json!({ "filter": { "text": vol } })).unwrap();
    assert_eq!(j["total"], 2, "{j}");
    let since_now = invoke(
        &h,
        "events_journal",
        json!({ "filter": { "text": vol, "since": now + 5 } }),
    )
    .unwrap();
    assert_eq!(since_now["total"], 0);
}

/// 0.9.1 · Varias terminales por IPC contra Docker real: independientes, cierre
/// individual sin afectar a las demás y cierre de todas (como al cerrar la
/// ventana) sin dejar procesos en el contenedor (D-063).
#[test]
fn live_multiple_terminals_via_ipc() {
    if std::env::var("DCC_DOCKER_TESTS").as_deref() != Ok("1") {
        eprintln!("omitida: define DCC_DOCKER_TESTS=1");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let docker_cfg = match std::env::var("DCC_DOCKER_SOCKET") {
        Ok(p) => DockerConfig {
            mode: DockerMode::Custom,
            custom_socket: p,
        },
        Err(_) => DockerConfig::default(),
    };
    let h = harness_with(dir, docker_cfg);
    wait_for("conexión", || engine_state(&h)["state"] == "connected");
    let images = invoke(&h, "images_list", json!({})).unwrap();
    assert!(
        images.as_array().unwrap().iter().any(|i| i["repoTags"]
            .as_array()
            .is_some_and(|t| t.iter().any(|t| t == "dcc-test/sleeper:2"))),
        "hace falta dcc-test/sleeper:2 (la crean las pruebas del motor)"
    );
    let name = format!("dcc-test-ipc-term-{}", std::process::id());
    let c = invoke(
        &h,
        "container_create",
        json!({ "spec": { "name": name, "image": "dcc-test/sleeper:2", "labels": [{ "key": "dcc.test", "value": "true" }] }, "start": true }),
    )
    .unwrap();
    let id = c["id"].as_str().unwrap().to_owned();
    let socket = tauri::async_runtime::block_on(async {
        let st = h.app.state::<AppState>();
        st.servers
            .local
            .engine()
            .await
            .unwrap()
            .endpoint()
            .socket_path
            .clone()
    });
    let docker =
        bollard::Docker::connect_with_unix(&socket, 60, bollard::API_DEFAULT_VERSION).unwrap();
    // El contenedor se elimina al final, pase lo que pase.
    struct Guard(bollard::Docker, String);
    impl Drop for Guard {
        fn drop(&mut self) {
            let (d, id) = (self.0.clone(), self.1.clone());
            std::thread::spawn(move || {
                tauri::async_runtime::block_on(async {
                    let _ = d
                        .remove_container(
                            &id,
                            Some(bollard::query_parameters::RemoveContainerOptions {
                                force: true,
                                ..Default::default()
                            }),
                        )
                        .await;
                })
            })
            .join()
            .ok();
        }
    }
    let _guard = Guard(docker.clone(), id.clone());

    // Ejecuta en el contenedor (sin TTY) y devuelve la salida.
    let run = |script: &str| -> String {
        let (d, id, script) = (docker.clone(), id.clone(), script.to_owned());
        tauri::async_runtime::block_on(async move {
            use futures_util::StreamExt;
            let ex = d
                .create_exec(
                    &id,
                    bollard::models::ExecConfig {
                        attach_stdout: Some(true),
                        cmd: Some(vec!["/bin/sh".into(), "-c".into(), script]),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            let mut out = String::new();
            if let bollard::exec::StartExecResults::Attached { mut output, .. } =
                d.start_exec(&ex.id, None).await.unwrap()
            {
                while let Some(Ok(f)) = output.next().await {
                    out.push_str(&f.to_string());
                }
            }
            out
        })
    };
    // Procesos vivos del contenedor (sin zombis) llamados así.
    let alive = |comm: &str| -> usize {
        run(r#"for d in /proc/[0-9]*; do st=; read -r st < "$d/stat" 2>/dev/null && echo "$st"; done"#)
            .lines()
            .filter(|l| l.contains(&format!("({comm})")) && !l.contains(") Z "))
            .count()
    };
    let file = |p: &str| run(&format!("read -r x < {p} 2>/dev/null && echo \"$x\""));

    // Se abre con el mismo comando que usa la ventana, con un canal real de
    // Tauri que recoge la salida (en el entorno simulado, un canal
    // "__CHANNEL__:n" no entrega nada y la sesión se cerraría al primer envío,
    // como hace cuando la ventana deja de escuchar).
    let outputs: Arc<Mutex<std::collections::HashMap<u32, String>>> = Default::default();
    let open = |n: u32| -> u64 {
        let o = outputs.clone();
        let ch =
            tauri::ipc::Channel::<crate::commands::containers::TerminalEvent>::new(move |body| {
                if let tauri::ipc::InvokeResponseBody::Json(j) = body {
                    let v: serde_json::Value = serde_json::from_str(&j).unwrap();
                    if let Some(d) = v["data"].as_str() {
                        use base64::Engine as _;
                        let bytes = base64::engine::general_purpose::STANDARD.decode(d).unwrap();
                        o.lock()
                            .unwrap()
                            .entry(n)
                            .or_default()
                            .push_str(&String::from_utf8_lossy(&bytes));
                    }
                }
                Ok(())
            });
        tauri::async_runtime::block_on(crate::commands::containers::terminal_open(
            h.app.state::<AppState>(),
            "local".into(),
            id.clone(),
            Some(name.clone()),
            dcc_engine::streams::ShellChoice::Sh,
            100,
            30,
            ch,
        ))
        .unwrap()
    };
    let input = |sid: u64, s: &str| {
        invoke(&h, "terminal_input", json!({ "stream": sid, "data": s })).unwrap();
    };
    let s1 = open(1);
    let s2 = open(2);
    assert_ne!(s1, s2);
    input(s1, "echo uno > /t1; /sleeper &\n");
    input(s2, "echo dos > /t2\n");
    wait_for("las dos sesiones escriben", || {
        file("/t1").trim() == "uno" && file("/t2").trim() == "dos"
    });
    invoke(
        &h,
        "terminal_resize",
        json!({ "stream": s1, "cols": 80, "rows": 20 }),
    )
    .unwrap();
    wait_for("sleeper en segundo plano", || alive("sleeper") == 2);
    // Cada canal recibe solo la salida de su sesión (el eco de lo tecleado).
    {
        let o = outputs.lock().unwrap();
        assert!(
            o[&1].contains("echo uno") && !o[&1].contains("echo dos"),
            "{o:?}"
        );
        assert!(
            o[&2].contains("echo dos") && !o[&2].contains("echo uno"),
            "{o:?}"
        );
        assert!(
            !o[&1].contains("\u{1b}]6973;"),
            "la marca del PID no llega a la ventana"
        );
    }

    // Cerrar la 1: su proceso en segundo plano desaparece; la 2 sigue.
    assert!(invoke(&h, "stream_close", json!({ "stream": s1 }))
        .unwrap()
        .as_bool()
        .unwrap());
    wait_for("la sesión 1 termina y limpia", || alive("sleeper") == 1);
    let e = invoke(&h, "terminal_input", json!({ "stream": s1, "data": "x" })).unwrap_err();
    assert_eq!(e["code"], "stream.closed");
    input(s2, "echo tres > /t3\n");
    wait_for("la sesión 2 sigue", || file("/t3").trim() == "tres");

    // Cerrar todas y esperar, como al cerrar la ventana.
    let s3 = open(3);
    input(s3, "/sleeper &\n");
    wait_for("sleeper de la sesión 3", || alive("sleeper") == 2);
    let left = tauri::async_runtime::block_on(async {
        h.app
            .state::<AppState>()
            .streams
            .close_all_and_wait(std::time::Duration::from_secs(15))
            .await
    });
    assert_eq!(left, 0, "todas las sesiones terminaron");
    assert_eq!(alive("sleeper"), 1, "solo el proceso principal");
    assert_eq!(alive("sh"), 1, "solo la shell que lista");
    let acts = invoke(&h, "activity_list", json!({})).unwrap();
    let execs: Vec<_> = acts
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["kind"] == "container.exec")
        .collect();
    assert_eq!(execs.len(), 3, "tres sesiones registradas");
    assert!(
        execs.iter().all(|r| r["status"] == "succeeded"),
        "{execs:?}"
    );
}

// ------------------------------------------------------------ servidores (0.10.0)

/// Toda orden con Docker exige indicar el servidor: sin él, la llamada se
/// rechaza; con uno desconocido, también, y nunca se usa otro servidor.
#[test]
fn docker_commands_are_bound_to_an_explicit_server() {
    let h = harness();
    let calls: Vec<(&str, serde_json::Value)> = vec![
        ("engine_state", json!({})),
        ("engine_reconnect", json!({})),
        ("engine_overview", json!({})),
        ("compose_status", json!({})),
        ("containers_list", json!({})),
        ("images_list", json!({})),
        ("networks_list", json!({})),
        ("volumes_list", json!({})),
        ("container_inspect", json!({ "id": "web", "name": null })),
        (
            "image_tag",
            json!({ "source": "a", "repo": "b", "tag": "c" }),
        ),
        ("events_query", json!({ "query": {} })),
        (
            "container_remove",
            json!({ "id": "web", "name": null, "force": true, "volumes": true, "confirmed": true }),
        ),
        (
            "image_remove",
            json!({ "reference": "nginx", "force": true, "confirmed": true }),
        ),
        ("images_prune", json!({ "all": true, "confirmed": true })),
        (
            "network_remove",
            json!({ "id": "n", "name": null, "confirmed": true }),
        ),
    ];
    for (cmd, args) in &calls {
        let err = invoke_exact(&h, cmd, args.clone()).unwrap_err();
        assert!(
            err.to_string().contains("server"),
            "{cmd} sin servidor debe rechazarse: {err}"
        );
        for bad in ["srv-0123456789abcdef", "", "LOCAL", "../local"] {
            let mut a = args.clone();
            a["server"] = json!(bad);
            let err = invoke_exact(&h, cmd, a).unwrap_err();
            assert_eq!(err["code"], "server.unknown", "{cmd} con «{bad}»: {err}");
        }
    }
    // Ninguna de esas llamadas llegó a registrar una operación con Docker.
    let recs: Vec<OperationRecord> =
        serde_json::from_value(invoke(&h, "activity_list", json!({})).unwrap()).unwrap();
    assert!(
        recs.iter()
            .all(|r| !matches!(r.transport, Transport::Api { .. })),
        "{recs:?}"
    );
}

fn server_cfg(name: &str, port: u16) -> serde_json::Value {
    json!({
        "id": "", "name": name, "host": "127.0.0.1", "port": port, "user": "nadie",
        "description": "", "auth": "agent", "keyPath": "", "dockerTarget": "default",
        "dockerSocket": "", "dockerContext": "", "connectTimeout": 3, "proxyJump": "",
        "autoConnect": false, "rememberSecret": false
    })
}

/// Un puerto local sin nadie escuchando.
fn closed_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

#[test]
fn server_profiles_are_managed_without_touching_anything_else() {
    let h = harness();
    // Validación del asistente.
    let issues = invoke(
        &h,
        "server_validate",
        json!({ "config": server_cfg("", 22) }),
    )
    .unwrap();
    assert_eq!(issues[0]["field"], "name");
    let mut bad = server_cfg("X", 22);
    bad["host"] = json!("-oProxyCommand=touch /tmp/pwn");
    assert_eq!(
        invoke(&h, "server_save", json!({ "config": bad })).unwrap_err()["code"],
        "server.invalid"
    );
    // Alta.
    let a = invoke(
        &h,
        "server_save",
        json!({ "config": server_cfg("Servidor A", closed_port()) }),
    )
    .unwrap();
    let id = a["id"].as_str().unwrap().to_owned();
    assert!(id.starts_with("srv-"));
    let list = invoke(&h, "servers_list", json!({})).unwrap();
    assert_eq!(list[0]["id"], "local");
    assert_eq!(list[0]["remote"], false);
    assert_eq!(list[1]["id"], id.as_str());
    assert_eq!(
        list[1]["state"]["state"], "idle",
        "un remoto no conecta solo"
    );
    // Conectar a un puerto cerrado: error de conexión de ese servidor; el
    // equipo local no cambia de estado.
    let local_before =
        invoke(&h, "engine_state", json!({ "server": "local" })).unwrap()["state"].clone();
    let out = invoke(
        &h,
        "server_connect",
        json!({ "server": id, "secret": null, "remember": false }),
    )
    .unwrap();
    assert_eq!(out["state"]["state"], "disconnected");
    assert_eq!(out["state"]["diagnosis"]["problem"]["kind"], "remote");
    assert_eq!(
        out["state"]["diagnosis"]["problem"]["remote"]["kind"],
        "sshRefused"
    );
    assert_eq!(
        invoke(&h, "engine_state", json!({ "server": "local" })).unwrap()["state"],
        local_before
    );
    let err = invoke(&h, "containers_list", json!({ "server": id })).unwrap_err();
    assert_eq!(err["code"], "engine.notConnected");
    // Eliminar exige confirmación; el equipo local no se elimina.
    assert_eq!(
        invoke(
            &h,
            "server_delete",
            json!({ "server": id, "confirmed": false })
        )
        .unwrap_err()["code"],
        "engine.notConfirmed"
    );
    assert_eq!(
        invoke(
            &h,
            "server_delete",
            json!({ "server": "local", "confirmed": true })
        )
        .unwrap_err()["code"],
        "server.localFixed"
    );
    invoke(&h, "server_disconnect", json!({ "server": id })).unwrap();
    invoke(
        &h,
        "server_delete",
        json!({ "server": id, "confirmed": true }),
    )
    .unwrap();
    assert_eq!(
        invoke(&h, "servers_list", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        invoke(&h, "containers_list", json!({ "server": id })).unwrap_err()["code"],
        "server.unknown"
    );
    // Lista de servidores aparte de la configuración y sin secretos.
    let raw = std::fs::read_to_string(h.store.path().with_file_name("servers.toml")).unwrap();
    assert!(!raw.contains("Servidor A"), "{raw}");
    let cfg = std::fs::read_to_string(h.store.path()).unwrap();
    assert!(!cfg.contains("srv-"), "{cfg}");
    // Alta, cambio y baja quedan en actividad con su servidor.
    let recs: Vec<OperationRecord> =
        serde_json::from_value(invoke(&h, "activity_list", json!({})).unwrap()).unwrap();
    let mine: Vec<&str> = recs
        .iter()
        .filter(|r| r.server.as_ref().is_some_and(|s| s.id == id))
        .map(|r| r.kind.as_str())
        .collect();
    assert!(
        mine.contains(&"server.add") && mine.contains(&"server.delete"),
        "{mine:?}"
    );
}

#[test]
fn a_password_test_never_leaks_the_secret() {
    let h = harness();
    let mut c = server_cfg("Con contraseña", closed_port());
    c["auth"] = json!("password");
    let secret = "Secreto-IPC-Único-9";
    let report = invoke(&h, "server_test", json!({ "config": c, "secret": secret })).unwrap();
    assert_eq!(report["problem"]["kind"], "sshRefused");
    assert_eq!(report["steps"].as_array().unwrap().len(), 7);
    let recs = invoke(&h, "activity_list", json!({})).unwrap().to_string();
    assert!(!recs.contains(secret));
    assert!(!report.to_string().contains(secret));
    for f in std::fs::read_dir(h.store.path().parent().unwrap()).unwrap() {
        let p = f.unwrap().path();
        if p.is_file() {
            let body = std::fs::read(&p).unwrap();
            assert!(
                !String::from_utf8_lossy(&body).contains(secret),
                "{} contiene el secreto",
                p.display()
            );
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LabServer {
    user: String,
    host: String,
    port: u16,
    socket: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LabPassword {
    server: usize,
    user: String,
    password: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Lab {
    key: String,
    servers: Vec<LabServer>,
    password: LabPassword,
}

fn docker_cli(socket: &str, args: &[&str]) -> String {
    let out = std::process::Command::new("docker")
        .arg("-H")
        .arg(format!("unix://{socket}"))
        .args(args)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn ssh_processes_for(user: &str) -> usize {
    let out = std::process::Command::new("pgrep")
        .args(["-f", &format!("ssh .*{user}@")])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).lines().count()
}

/// Tres servidores remotos reales por SSH (Docker 29, 26 y 24) y el equipo
/// local a la vez, por IPC como la interfaz: identidad confirmada, clave y
/// contraseña, cada operación en su servidor, vista general, desconexión sin
/// procesos huérfanos y eliminación de un perfil sin tocar el servidor.
/// Necesita `DCC_DOCKER_TESTS=1`, `DCC_SSH_LAB=<lab.json>` y
/// `DCC_TEST_ASKPASS=<dcc-askpass>`.
#[test]
fn live_three_remote_servers_through_ipc() {
    if std::env::var("DCC_DOCKER_TESTS").as_deref() != Ok("1") {
        eprintln!("omitida: define DCC_DOCKER_TESTS=1");
        return;
    }
    let Ok(lab) = std::env::var("DCC_SSH_LAB") else {
        eprintln!("omitida: define DCC_SSH_LAB=<lab.json>");
        return;
    };
    assert!(
        std::env::var_os("DCC_TEST_ASKPASS").is_some(),
        "define DCC_TEST_ASKPASS"
    );
    let lab: Lab = serde_json::from_str(&std::fs::read_to_string(lab).unwrap()).unwrap();
    let keyring = std::env::var_os("DCC_KEYRING_TEST").is_some();
    let dir = tempfile::tempdir().unwrap();
    let h = harness_with(dir, DockerConfig::default());
    wait_for("conexión local", || {
        engine_state(&h)["state"] == "connected"
    });

    // Alta de los tres: A y C con clave, B con contraseña.
    let mut ids = Vec::new();
    for (i, s) in lab.servers.iter().enumerate() {
        let mut c = server_cfg(&format!("Lab {}", ["A", "B", "C"][i]), s.port);
        c["host"] = json!(s.host);
        c["dockerTarget"] = json!("socket");
        c["dockerSocket"] = json!(s.socket);
        c["connectTimeout"] = json!(10);
        if i == lab.password.server {
            c["user"] = json!(lab.password.user);
            c["auth"] = json!("password");
        } else {
            c["user"] = json!(s.user);
            c["auth"] = json!("key");
            c["keyPath"] = json!(lab.key);
        }
        let saved = invoke(&h, "server_save", json!({ "config": c })).unwrap();
        ids.push((saved["id"].as_str().unwrap().to_owned(), saved));
    }

    // Primera conexión: identidad desconocida, sin reintentos; se confirma la
    // huella presentada y entonces conecta. (Las claves del laboratorio no
    // están en ~/.ssh/known_hosts: este camino se ejecuta siempre.)
    for (i, (id, cfg)) in ids.iter().enumerate() {
        let trusted = invoke(&h, "server_trusted_keys", json!({ "config": cfg })).unwrap();
        assert!(trusted.as_array().unwrap().is_empty(), "{trusted}");
        {
            let out = invoke(
                &h,
                "server_connect",
                json!({ "server": id, "secret": null, "remember": false }),
            )
            .unwrap();
            assert_eq!(
                out["state"]["diagnosis"]["problem"]["remote"]["kind"], "hostKeyUnknown",
                "{out}"
            );
            let key = invoke(&h, "server_host_key", json!({ "config": cfg })).unwrap();
            assert!(key["fingerprint"].as_str().unwrap().starts_with("SHA256:"));
            invoke(
                &h,
                "server_trust",
                json!({ "config": cfg, "key": key, "replace": false }),
            )
            .unwrap();
        }
        let secret = (i == lab.password.server).then(|| lab.password.password.clone());
        let out = invoke(
            &h,
            "server_connect",
            json!({ "server": id, "secret": secret, "remember": keyring && secret.is_some() }),
        )
        .unwrap();
        assert_eq!(out["state"]["state"], "connected", "{out}");
        assert!(out["state"]["latencyMs"].as_u64().is_some());
    }
    let versions: Vec<String> = ids
        .iter()
        .map(|(id, _)| {
            invoke(&h, "engine_state", json!({ "server": id })).unwrap()["engine"]["apiVersion"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    let distinct: std::collections::HashSet<&String> = versions.iter().collect();
    assert_eq!(distinct.len(), 3, "tres daemons distintos: {versions:?}");

    // Una operación en B ocurre solo en B.
    let (b, _) = &ids[1];
    let name = format!("dcc-ipc-remote-{}", std::process::id());
    let created = invoke(
        &h,
        "container_create",
        json!({ "server": b, "spec": { "name": name, "image": "dcc-test/sleeper:2", "labels": [{ "key": "dcc.test", "value": "true" }] }, "start": true }),
    )
    .unwrap();
    let cid = created["id"].as_str().unwrap().to_owned();
    let names_on = |server: &str| -> Vec<String> {
        invoke(&h, "containers_list", json!({ "server": server }))
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap_or_default().to_owned())
            .collect()
    };
    assert!(names_on(b).contains(&name));
    for other in ["local", ids[0].0.as_str(), ids[2].0.as_str()] {
        assert!(!names_on(other).contains(&name), "apareció en {other}");
    }
    assert_eq!(
        docker_cli(
            &lab.servers[1].socket,
            &["inspect", "-f", "{{.State.Running}}", &cid]
        ),
        "true"
    );
    // Registrada con su servidor.
    let recs: Vec<OperationRecord> =
        serde_json::from_value(invoke(&h, "activity_list", json!({})).unwrap()).unwrap();
    let create = recs
        .iter()
        .find(|r| r.kind == "container.create" && r.target.as_deref() == Some(name.as_str()))
        .expect("registro de la creación");
    assert_eq!(create.server.as_ref().unwrap().id, *b);
    assert!(create.server.as_ref().unwrap().address.contains(":2202"));
    assert!(!serde_json::to_string(&recs)
        .unwrap()
        .contains(&lab.password.password));

    // Vista general: cuatro servidores, cada uno con sus datos.
    let ov = invoke(&h, "servers_overview", json!({})).unwrap();
    assert_eq!(ov.as_array().unwrap().len(), 4);
    for o in ov.as_array().unwrap() {
        assert!(o["overview"]["info"]["ncpu"].as_i64().unwrap() > 0, "{o}");
        assert!(o["error"].is_null());
    }

    // Desconectar B: sin procesos ssh de B; los demás siguen.
    invoke(&h, "server_disconnect", json!({ "server": b })).unwrap();
    assert_eq!(
        invoke(&h, "engine_state", json!({ "server": b })).unwrap()["state"],
        "idle"
    );
    wait_for("sin procesos ssh de B", || {
        ssh_processes_for(&lab.password.user) == 0
    });
    assert_eq!(
        invoke(&h, "containers_list", json!({ "server": b })).unwrap_err()["code"],
        "engine.notConnected"
    );
    invoke(&h, "containers_list", json!({ "server": ids[2].0 })).unwrap();
    // El contenedor sigue en B: desconectar no toca el servidor.
    assert_eq!(
        docker_cli(
            &lab.servers[1].socket,
            &["inspect", "-f", "{{.State.Running}}", &cid]
        ),
        "true"
    );

    // Con keyring: se vuelve a conectar sin escribir la contraseña.
    if keyring {
        let st = invoke(&h, "server_secret_status", json!({ "server": b })).unwrap();
        assert_eq!(st["stored"], true, "{st}");
        let out = invoke(
            &h,
            "server_connect",
            json!({ "server": b, "secret": null, "remember": false }),
        )
        .unwrap();
        assert_eq!(out["state"]["state"], "connected", "{out}");
        assert_eq!(
            invoke(&h, "server_forget_secret", json!({ "server": b })).unwrap(),
            1
        );
        invoke(&h, "server_disconnect", json!({ "server": b })).unwrap();
    } else {
        let out = invoke(
            &h,
            "server_connect",
            json!({ "server": b, "secret": null, "remember": false }),
        )
        .unwrap();
        assert_eq!(
            out["state"]["diagnosis"]["problem"]["remote"]["kind"], "secretRequired",
            "sin keyring la contraseña no se guarda: {out}"
        );
    }

    // Eliminar el perfil de B: el contenedor sigue en el servidor.
    invoke(
        &h,
        "server_delete",
        json!({ "server": b, "confirmed": true }),
    )
    .unwrap();
    assert_eq!(
        docker_cli(
            &lab.servers[1].socket,
            &["inspect", "-f", "{{.State.Running}}", &cid]
        ),
        "true"
    );
    docker_cli(&lab.servers[1].socket, &["rm", "-f", &cid]);

    // Al cerrar, no queda ninguna sesión.
    tauri::async_runtime::block_on(h.app.state::<AppState>().servers.close_all());
    for s in &lab.servers {
        wait_for("sin procesos ssh", || ssh_processes_for(&s.user) == 0);
    }
}
