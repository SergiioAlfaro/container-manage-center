// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Gestión masiva (1.10.0): qué peticiones llegan a Docker en cada caso, con
//! un Docker simulado con estado (contenedores, políticas, fallos y cortes).
//!
//! Comprueba que una operación incompatible no envía nada, que la eliminación
//! nunca borra volúmenes, que se confirma el estado posterior, que un corte
//! durante la orden deja el resultado como desconocido (sin reintentar) y que
//! cada petición queda registrada con el identificador de la operación masiva.
// Pruebas contra un socket Unix (Linux); no en Windows (D-114).
#![cfg(unix)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use cmc_activity::{ActivityLog, OpStatus};
use cmc_engine::bulk::{
    BulkOperation, BulkStage, ItemDone, RestartPolicy, RestartPolicyName, SkipReason,
};
use cmc_engine::{ConnectionMode, ConnectionState, Engine, EngineError, EngineManager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
use uuid::Uuid;

fn id(c: char) -> String {
    c.to_string().repeat(64)
}

#[derive(Clone)]
struct Ctr {
    state: &'static str,
    auto_remove: bool,
    policy: (&'static str, i64),
}

#[derive(Default)]
struct World {
    containers: HashMap<String, Ctr>,
    /// Peticiones recibidas (`MÉTODO ruta`), sin las de conexión.
    seen: Vec<String>,
    /// Cuerpos de `POST /containers/{id}/update`.
    bodies: Vec<String>,
    /// La orden a este contenedor responde 500.
    fail: Option<String>,
    /// La orden a este contenedor corta la conexión sin responder.
    drop: Option<String>,
    /// La actualización se acepta pero no cambia nada.
    ignore_update: bool,
    /// Inspect devuelve otro contenedor.
    impostor: Option<String>,
}

type Shared = Arc<Mutex<World>>;

enum Reply {
    Json(u16, String),
    Empty(u16),
    Hangup,
}

fn inspect(id: &str, c: &Ctr) -> String {
    format!(
        r#"{{"Id":"{id}","Name":"/c-{}","State":{{"Status":"{}","ExitCode":0}},"HostConfig":{{"AutoRemove":{},"RestartPolicy":{{"Name":"{}","MaximumRetryCount":{}}}}}}}"#,
        &id[..1],
        c.state,
        c.auto_remove,
        c.policy.0,
        c.policy.1
    )
}

fn respond(w: &mut World, method: &str, path: &str, body: &str) -> Reply {
    let p = path.split('?').next().unwrap_or("");
    match (method, p) {
        ("GET", "/_ping") => return Reply::Json(200, "OK".into()),
        ("GET", "/version") => {
            return Reply::Json(
                200,
                r#"{"Version":"29.0.0","ApiVersion":"1.52","MinAPIVersion":"1.24","Os":"linux","Arch":"amd64"}"#.into(),
            )
        }
        ("GET", "/info") => return Reply::Json(200, r#"{"Containers":0,"Images":0}"#.into()),
        _ => {}
    }
    w.seen.push(format!("{method} {path}"));
    let rest = p.strip_prefix("/containers/").unwrap_or("");
    let (cid, verb) = rest.split_once('/').unwrap_or((rest, ""));
    let cid = cid.to_owned();
    let not_found = || Reply::Json(404, r#"{"message":"No such container"}"#.into());
    if method != "GET" {
        if w.drop.as_deref() == Some(cid.as_str()) {
            return Reply::Hangup;
        }
        if w.fail.as_deref() == Some(cid.as_str()) {
            return Reply::Json(500, r#"{"message":"fallo simulado del daemon"}"#.into());
        }
    }
    match (method, verb) {
        ("GET", "json") => {
            if let Some(other) = w.impostor.clone().filter(|i| *i == cid) {
                let c = w.containers[&other].clone();
                return Reply::Json(200, inspect(&id('f'), &c));
            }
            match w.containers.get(&cid) {
                Some(c) => Reply::Json(200, inspect(&cid, c)),
                None => not_found(),
            }
        }
        ("POST", v @ ("start" | "stop" | "restart" | "pause" | "unpause")) => {
            let Some(c) = w.containers.get_mut(&cid) else {
                return not_found();
            };
            c.state = match v {
                "start" | "restart" | "unpause" => "running",
                "stop" => "exited",
                _ => "paused",
            };
            Reply::Empty(204)
        }
        ("POST", "update") => {
            w.bodies.push(body.to_owned());
            if w.ignore_update {
                return Reply::Json(200, r#"{"Warnings":[]}"#.into());
            }
            let v: serde_json::Value = serde_json::from_str(body).unwrap();
            let name: &'static str = match v["RestartPolicy"]["Name"].as_str().unwrap() {
                "always" => "always",
                "unless-stopped" => "unless-stopped",
                "on-failure" => "on-failure",
                _ => "no",
            };
            let n = v["RestartPolicy"]["MaximumRetryCount"]
                .as_i64()
                .unwrap_or(0);
            match w.containers.get_mut(&cid) {
                Some(c) => {
                    c.policy = (name, n);
                    Reply::Json(200, r#"{"Warnings":[]}"#.into())
                }
                None => not_found(),
            }
        }
        ("DELETE", "") => {
            if w.containers.remove(&cid).is_some() {
                Reply::Empty(204)
            } else {
                not_found()
            }
        }
        _ => Reply::Json(404, r#"{"message":"no implementado"}"#.into()),
    }
}

async fn fake_docker(path: std::path::PathBuf, world: Shared) {
    let listener = UnixListener::bind(&path).unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut s, _)) = listener.accept().await else {
                break;
            };
            let world = world.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 8192];
                loop {
                    let end = loop {
                        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            break i + 4;
                        }
                        match s.read(&mut chunk).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => buf.extend_from_slice(&chunk[..n]),
                        }
                    };
                    let head = String::from_utf8_lossy(&buf[..end]).to_string();
                    let len: usize = head
                        .lines()
                        .find_map(|l| {
                            let (k, v) = l.split_once(':')?;
                            k.eq_ignore_ascii_case("content-length")
                                .then(|| v.trim().parse().ok())?
                        })
                        .unwrap_or(0);
                    while buf.len() < end + len {
                        match s.read(&mut chunk).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => buf.extend_from_slice(&chunk[..n]),
                        }
                    }
                    let body = String::from_utf8_lossy(&buf[end..end + len]).to_string();
                    buf.drain(..end + len);
                    let mut parts = head.lines().next().unwrap_or("").split(' ');
                    let (m, p) = (
                        parts.next().unwrap_or("").to_owned(),
                        parts.next().unwrap_or("").to_owned(),
                    );
                    let reply = respond(&mut world.lock().unwrap(), &m, &p, &body);
                    let resp = match reply {
                        Reply::Hangup => return,
                        Reply::Empty(code) => format!("HTTP/1.1 {code} No Content\r\n\r\n"),
                        Reply::Json(code, b) => format!(
                            "HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{b}",
                            b.len()
                        ),
                    };
                    if s.write_all(resp.as_bytes()).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
}

struct Harness {
    world: Shared,
    log: ActivityLog,
    engine: Engine,
    _dir: tempfile::TempDir,
}

async fn harness(containers: &[(char, &'static str)]) -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("docker.sock");
    let world: Shared = Default::default();
    for (c, st) in containers {
        world.lock().unwrap().containers.insert(
            id(*c),
            Ctr {
                state: st,
                auto_remove: false,
                policy: ("no", 0),
            },
        );
    }
    fake_docker(sock.clone(), world.clone()).await;
    let log = ActivityLog::new(200);
    let m = EngineManager::new(
        log.clone(),
        ConnectionMode::Custom {
            socket_path: sock.display().to_string(),
        },
    );
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    let engine = m.engine().await.unwrap();
    world.lock().unwrap().seen.clear();
    Harness {
        world,
        log,
        engine,
        _dir: dir,
    }
}

impl Harness {
    fn seen(&self) -> Vec<String> {
        self.world.lock().unwrap().seen.clone()
    }
    fn tagged(&self, batch: Uuid) -> Engine {
        self.engine
            .with_activity(self.engine.activity().with_batch(batch))
    }
}

/// Operaciones sobre contenedores (sin las de la conexión).
fn containers(log: &ActivityLog) -> Vec<cmc_activity::OperationRecord> {
    log.snapshot()
        .into_iter()
        .filter(|r| r.kind.starts_with("container."))
        .collect()
}

fn policy(name: RestartPolicyName, n: u32) -> BulkOperation {
    BulkOperation::RestartPolicy {
        policy: RestartPolicy {
            name,
            maximum_retry_count: n,
        },
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn start_is_sent_confirmed_and_tagged_with_the_batch() {
    let h = harness(&[('a', "exited")]).await;
    let batch = Uuid::new_v4();
    let done = h
        .tagged(batch)
        .bulk_apply(&BulkOperation::Start, &id('a'), "c-a")
        .await
        .unwrap();
    let ItemDone::Applied {
        state: Some(st), ..
    } = done
    else {
        panic!("debía aplicarse: {done:?}");
    };
    assert_eq!(st.status, "running", "estado confirmado por inspect");
    let a = id('a');
    assert_eq!(
        h.seen(),
        [
            format!("GET /containers/{a}/json?size=false"),
            format!("POST /containers/{a}/start"),
            format!("GET /containers/{a}/json?size=false"),
        ],
        "comprobación (sin registrar), orden y confirmación"
    );
    // La orden y la confirmación quedan registradas con el lote; la comprobación previa no.
    // (El registro lista primero lo más reciente.)
    let mut recs = containers(&h.log);
    recs.reverse();
    let kinds: Vec<_> = recs.iter().map(|r| r.kind.as_str()).collect();
    assert_eq!(kinds, ["container.start", "container.state"]);
    assert!(recs.iter().all(|r| r.batch == Some(batch)));
    assert!(recs.iter().all(|r| r.status == OpStatus::Succeeded));
}

#[tokio::test(flavor = "multi_thread")]
async fn incompatible_state_sends_nothing() {
    let h = harness(&[('a', "running"), ('b', "exited")]).await;
    let r = h
        .engine
        .bulk_apply(&BulkOperation::Start, &id('a'), "c-a")
        .await
        .unwrap();
    assert_eq!(
        r,
        ItemDone::Skipped {
            reason: SkipReason::State {
                state: "running".into()
            },
            state: "running".into()
        }
    );
    let r = h
        .engine
        .bulk_apply(&BulkOperation::Pause, &id('b'), "c-b")
        .await
        .unwrap();
    assert!(matches!(r, ItemDone::Skipped { .. }));
    assert!(
        h.seen().iter().all(|s| s.starts_with("GET ")),
        "solo consultas: {:?}",
        h.seen()
    );
    assert!(containers(&h.log).is_empty(), "nada que registrar");
}

#[tokio::test(flavor = "multi_thread")]
async fn remove_never_deletes_volumes_and_needs_force_when_running() {
    let h = harness(&[('a', "running"), ('b', "exited")]).await;
    let soft = BulkOperation::Remove { force: false };
    let r = h.engine.bulk_apply(&soft, &id('a'), "c-a").await.unwrap();
    assert!(matches!(
        r,
        ItemDone::Skipped {
            reason: SkipReason::NeedsForce { .. },
            ..
        }
    ));
    assert!(!h.seen().iter().any(|s| s.starts_with("DELETE")));

    let r = h.engine.bulk_apply(&soft, &id('b'), "c-b").await.unwrap();
    assert_eq!(
        r,
        ItemDone::Applied {
            state: None,
            details: None
        }
    );
    let hard = BulkOperation::Remove { force: true };
    h.engine.bulk_apply(&hard, &id('a'), "c-a").await.unwrap();
    let deletes: Vec<_> = h
        .seen()
        .into_iter()
        .filter(|s| s.starts_with("DELETE"))
        .collect();
    assert_eq!(
        deletes,
        [
            format!(
                "DELETE /containers/{}?v=false&force=false&link=false",
                id('b')
            ),
            format!(
                "DELETE /containers/{}?v=false&force=true&link=false",
                id('a')
            ),
        ]
    );
    assert!(h.world.lock().unwrap().containers.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn restart_policy_update_is_confirmed() {
    let h = harness(&[('a', "running")]).await;
    let op = policy(RestartPolicyName::OnFailure, 3);
    let r = h.engine.bulk_apply(&op, &id('a'), "c-a").await.unwrap();
    let ItemDone::Applied {
        details: Some(d), ..
    } = r
    else {
        panic!("{r:?}");
    };
    assert_eq!(d.restart_policy.cli_value(), "on-failure:3");
    let body: serde_json::Value = serde_json::from_str(&h.world.lock().unwrap().bodies[0]).unwrap();
    assert_eq!(
        body["RestartPolicy"],
        serde_json::json!({"Name": "on-failure", "MaximumRetryCount": 3})
    );
    let upd = h
        .log
        .snapshot()
        .into_iter()
        .find(|r| r.kind == "container.update")
        .unwrap();
    let eq = upd.equivalent_command.unwrap();
    assert!(eq.ends_with(" update --restart on-failure:3 c-a"), "{eq}");

    // Otra vez la misma: no se envía nada.
    let again = h.engine.bulk_apply(&op, &id('a'), "c-a").await.unwrap();
    assert!(matches!(
        again,
        ItemDone::Skipped {
            reason: SkipReason::Unchanged,
            ..
        }
    ));
    assert_eq!(h.world.lock().unwrap().bodies.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_remove_containers_are_skipped_and_ignored_updates_are_not_success() {
    let h = harness(&[('a', "running"), ('b', "running")]).await;
    h.world
        .lock()
        .unwrap()
        .containers
        .get_mut(&id('a'))
        .unwrap()
        .auto_remove = true;
    let op = policy(RestartPolicyName::Always, 0);
    let r = h.engine.bulk_apply(&op, &id('a'), "c-a").await.unwrap();
    assert!(matches!(
        r,
        ItemDone::Skipped {
            reason: SkipReason::AutoRemove,
            ..
        }
    ));
    // Docker acepta la orden pero la política no cambia: no se da por buena.
    h.world.lock().unwrap().ignore_update = true;
    let e = h.engine.bulk_apply(&op, &id('b'), "c-b").await.unwrap_err();
    assert_eq!(e.stage, BulkStage::Confirm);
    assert!(e.outcome_unknown());
}

#[tokio::test(flavor = "multi_thread")]
async fn docker_errors_and_connection_cuts_are_reported_without_retry() {
    let h = harness(&[('a', "exited"), ('b', "exited")]).await;
    h.world.lock().unwrap().fail = Some(id('a'));
    h.world.lock().unwrap().drop = Some(id('b'));

    let e = h
        .engine
        .bulk_apply(&BulkOperation::Start, &id('a'), "c-a")
        .await
        .unwrap_err();
    assert_eq!(e.stage, BulkStage::Execute);
    assert!(matches!(e.error, EngineError::Docker { status: 500, .. }));
    assert!(!e.outcome_unknown(), "Docker la rechazó: no cambió nada");

    let e = h
        .engine
        .bulk_apply(&BulkOperation::Start, &id('b'), "c-b")
        .await
        .unwrap_err();
    assert_eq!(e.stage, BulkStage::Execute);
    assert!(e.outcome_unknown(), "corte durante la orden: {e:?}");
    // Una sola orden por contenedor: nunca se reintenta.
    let posts = h.seen().iter().filter(|s| s.starts_with("POST")).count();
    assert_eq!(posts, 2);
    let failed: Vec<_> = h
        .log
        .snapshot()
        .into_iter()
        .filter(|r| r.status == OpStatus::Failed)
        .collect();
    assert_eq!(failed.len(), 2, "los dos fallos quedan registrados");
}

#[tokio::test(flavor = "multi_thread")]
async fn gone_busy_bad_id_and_impostor() {
    let h = harness(&[('a', "exited"), ('b', "exited")]).await;
    // Ya no existe.
    let r = h
        .engine
        .bulk_apply(&BulkOperation::Start, &id('e'), "c-e")
        .await
        .unwrap();
    assert!(matches!(
        r,
        ItemDone::Skipped {
            reason: SkipReason::Gone,
            ..
        }
    ));
    // Otra operación en curso sobre el mismo contenedor: no se duplica.
    let _held = h
        .engine
        .locks()
        .acquire(&format!("container:{}", id('a')))
        .unwrap();
    let e = h
        .engine
        .bulk_apply(&BulkOperation::Start, &id('a'), "c-a")
        .await
        .unwrap_err();
    assert!(matches!(e.error, EngineError::Busy { .. }));
    assert_eq!(e.stage, BulkStage::Check);
    // Nombres o prefijos: rechazados antes de hablar con Docker.
    let before = h.seen().len();
    for bad in ["c-b", &id('b')[..12]] {
        let e = h
            .engine
            .bulk_apply(&BulkOperation::Start, bad, "x")
            .await
            .unwrap_err();
        assert!(matches!(e.error, EngineError::InvalidInput { .. }));
    }
    assert_eq!(h.seen().len(), before);
    // Docker devuelve otro contenedor para el ID: no se toca nada.
    h.world.lock().unwrap().impostor = Some(id('b'));
    h.world.lock().unwrap().containers.insert(
        id('f'),
        Ctr {
            state: "exited",
            auto_remove: false,
            policy: ("no", 0),
        },
    );
    let e = h
        .engine
        .bulk_apply(&BulkOperation::Start, &id('b'), "c-b")
        .await
        .unwrap_err();
    assert!(matches!(e.error, EngineError::InvalidResponse { .. }));
    assert!(!h.seen().iter().any(|s| s.starts_with("POST")));
}
