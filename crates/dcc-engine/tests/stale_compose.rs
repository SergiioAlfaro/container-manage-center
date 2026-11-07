//! Prueba determinista de la carrera de D-031 aplicada a Docker Compose (D-056).
//!
//! Observado en Debian 13 (Docker 26.1.5, Compose 2.26.1) y reproducido con
//! Docker 26.1.3: justo después de `docker compose stop`, `GET /containers/json`
//! puede decir que un contenedor sigue "running" (Docker responde a la parada
//! antes de actualizar la réplica del listado). El proyecto aparecía "Parcial"
//! en lugar de "Detenido". Inspect espera al bloqueo del contenedor: una vez
//! respondido, el listado es coherente.
//!
//! Este servidor responde como Docker en esa ventana: el listado da el estado
//! antiguo de cada contenedor hasta que se consulta con inspect.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use dcc_activity::ActivityLog;
use dcc_engine::compose::project::ProjectState;
use dcc_engine::{ConnectionMode, ConnectionState, EngineManager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;

/// (id, nombre, servicio, estado en el listado desfasado, estado real)
const CONTAINERS: [(&str, &str, &str, &str, &str); 3] = [
    ("aaa111", "tienda-web-1", "web", "running", "exited"),
    ("bbb222", "tienda-db-1", "db", "exited", "exited"),
    ("ccc333", "otro-app-1", "app", "running", "running"),
];

#[derive(Default)]
struct World {
    inspected: HashSet<String>,
    requests: Vec<String>,
}

fn labels(project: &str, service: &str) -> String {
    format!(
        r#"{{"com.docker.compose.project":"{project}","com.docker.compose.service":"{service}","com.docker.compose.container-number":"1","com.docker.compose.oneoff":"False"}}"#
    )
}

fn respond(w: &mut World, method: &str, path: &str) -> (u16, String) {
    let route = path.split('?').next().unwrap_or("");
    match (method, route) {
        ("GET", "/_ping") => (200, "OK".into()),
        ("GET", "/version") => (200, r#"{"Version":"26.1.5","ApiVersion":"1.45","MinAPIVersion":"1.24","Os":"linux","Arch":"amd64"}"#.into()),
        ("GET", "/info") => (200, r#"{"Containers":3,"ContainersRunning":1,"ContainersPaused":0,"ContainersStopped":2,"Images":1,"SecurityOptions":[]}"#.into()),
        ("GET", "/containers/json") => {
            let items: Vec<String> = CONTAINERS
                .iter()
                .map(|(id, name, svc, stale, real)| {
                    let st = if w.inspected.contains(*id) { real } else { stale };
                    let project = name.split('-').next().unwrap();
                    format!(
                        r#"{{"Id":"{id}","Names":["/{name}"],"Image":"img","State":"{st}","Status":"","Labels":{}}}"#,
                        labels(project, svc)
                    )
                })
                .collect();
            (200, format!("[{}]", items.join(",")))
        }
        ("GET", p) if p.starts_with("/containers/") && p.ends_with("/json") => {
            let id = p.trim_start_matches("/containers/").trim_end_matches("/json");
            match CONTAINERS.iter().find(|c| c.0 == id) {
                Some((id, name, _, _, real)) => {
                    w.inspected.insert((*id).to_owned());
                    (200, format!(r#"{{"Id":"{id}","Name":"/{name}","State":{{"Status":"{real}","Running":{},"ExitCode":0}}}}"#, *real == "running"))
                }
                None => (404, r#"{"message":"No such container"}"#.into()),
            }
        }
        _ => (404, r#"{"message":"no implementado en el servidor de prueba"}"#.into()),
    }
}

async fn fake_docker(path: std::path::PathBuf) -> Arc<Mutex<World>> {
    let world: Arc<Mutex<World>> = Default::default();
    let w2 = world.clone();
    let listener = UnixListener::bind(&path).unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut s, _)) = listener.accept().await else {
                break;
            };
            let w = w2.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    let end = loop {
                        if let Some(i) = buf.windows(4).position(|x| x == b"\r\n\r\n") {
                            break i + 4;
                        }
                        match s.read(&mut chunk).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => buf.extend_from_slice(&chunk[..n]),
                        }
                    };
                    let head = String::from_utf8_lossy(&buf[..end]).to_string();
                    buf.drain(..end);
                    let mut parts = head.lines().next().unwrap_or("").split(' ');
                    let (m, p) = (
                        parts.next().unwrap_or("").to_owned(),
                        parts.next().unwrap_or("").to_owned(),
                    );
                    let (code, body) = {
                        let mut g = w.lock().unwrap();
                        g.requests.push(format!("{m} {p}"));
                        respond(&mut g, &m, &p)
                    };
                    let reason = if code == 200 { "OK" } else { "Not Found" };
                    let resp = format!(
                        "HTTP/1.1 {code} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                        body.len()
                    );
                    if s.write_all(resp.as_bytes()).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    world
}

#[tokio::test(flavor = "multi_thread")]
async fn project_state_is_confirmed_with_inspect_after_an_action() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("docker26.sock");
    let world = fake_docker(sock.clone()).await;
    let log = ActivityLog::new(100);
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
    let e = m.engine().await.unwrap();

    // Así fallaba: con el listado desfasado, el proyecto detenido sale "Parcial".
    let before = e.compose_projects(&[]).await.unwrap();
    let tienda = before
        .iter()
        .find(|p| p.reference.name == "tienda")
        .unwrap();
    assert_eq!(
        (tienda.state, tienda.running, tienda.total),
        (ProjectState::Partial, 1, 2)
    );

    // Lo que hace la aplicación tras desplegar, iniciar, detener o reiniciar.
    let settled = e.compose_settle("tienda").await.unwrap();
    let mut states: Vec<_> = settled
        .iter()
        .map(|s| (s.name.as_str(), s.status.as_str()))
        .collect();
    states.sort();
    assert_eq!(
        states,
        [("tienda-db-1", "exited"), ("tienda-web-1", "exited")]
    );

    // Ahora el listado es coherente y el estado del proyecto es el real.
    let after = e.compose_projects(&[]).await.unwrap();
    let tienda = after.iter().find(|p| p.reference.name == "tienda").unwrap();
    assert_eq!(
        (tienda.state, tienda.running, tienda.total),
        (ProjectState::Stopped, 0, 2)
    );
    // Otros proyectos no se tocan ni se consultan.
    let otro = after.iter().find(|p| p.reference.name == "otro").unwrap();
    assert_eq!(otro.state, ProjectState::Running);
    let reqs = world.lock().unwrap().requests.clone();
    assert!(reqs
        .iter()
        .any(|r| r == "GET /containers/aaa111/json?size=false"));
    assert!(reqs
        .iter()
        .any(|r| r == "GET /containers/bbb222/json?size=false"));
    assert!(!reqs.iter().any(|r| r.contains("ccc333")), "{reqs:?}");

    // Cada confirmación es una operación registrada.
    let checks = log
        .snapshot()
        .into_iter()
        .filter(|r| r.kind == "container.state")
        .count();
    assert_eq!(checks, 2);
}
