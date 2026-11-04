//! Prueba determinista de la carrera de Docker Engine descrita en D-031.
//!
//! En moby, `handleContainerExit` llama a `SetStopped` (que hace responder 204 a
//! `POST /containers/{id}/stop`) antes de `CheckpointTo(containersReplica)`, la
//! réplica de la que lee `GET /containers/json`. Durante esa ventana el listado
//! dice "running" mientras inspect (que toma el bloqueo del contenedor) dice
//! "exited". La ventana depende del disco y de la carga: se observó en Debian 13
//! (Docker 26.1.5) y no se puede provocar a voluntad con un daemon real.
//!
//! Este servidor responde como lo haría Docker justo en esa ventana, de forma
//! reproducible, para comprobar que la aplicación informa del estado real.

use std::sync::{Arc, Mutex};

use dcc_activity::{ActivityLog, OpStatus};
use dcc_engine::{ConnectionMode, ConnectionState, ContainerAction, EngineManager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;

const ID: &str = "abc123";

fn respond(method: &str, path: &str) -> (u16, &'static str, String) {
    let json = "application/json";
    match (method, path.split('?').next().unwrap_or("")) {
        ("GET", "/_ping") => (200, "text/plain", "OK".into()),
        ("GET", "/version") => (200, json, r#"{"Version":"26.1.5","ApiVersion":"1.45","MinAPIVersion":"1.24","Os":"linux","Arch":"amd64"}"#.into()),
        ("GET", "/info") => (200, json, r#"{"Containers":1,"ContainersRunning":1,"ContainersPaused":0,"ContainersStopped":0,"Images":1,"SecurityOptions":["name=seccomp,profile=builtin"]}"#.into()),
        ("POST", p) if p == format!("/containers/{ID}/stop") => (204, json, String::new()),
        // Réplica todavía sin actualizar: el listado dice "running".
        ("GET", "/containers/json") => (200, json, format!(r#"[{{"Id":"{ID}","Names":["/web"],"Image":"nginx","State":"running","Status":"Up 1 minute"}}]"#)),
        // Inspect ve el estado real.
        ("GET", p) if p == format!("/containers/{ID}/json") => (200, json, format!(r#"{{"Id":"{ID}","State":{{"Status":"exited","Running":false,"ExitCode":0}}}}"#)),
        _ => (404, json, r#"{"message":"no implementado en el servidor de prueba"}"#.into()),
    }
}

async fn fake_docker(path: std::path::PathBuf) -> Arc<Mutex<Vec<String>>> {
    let seen: Arc<Mutex<Vec<String>>> = Default::default();
    let log = seen.clone();
    let listener = UnixListener::bind(&path).unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut s, _)) = listener.accept().await else {
                break;
            };
            let log = log.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    // Una petición por iteración (conexión persistente, sin cuerpo).
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
                    buf.drain(..end);
                    let mut parts = head.lines().next().unwrap_or("").split(' ');
                    let (m, p) = (
                        parts.next().unwrap_or("").to_owned(),
                        parts.next().unwrap_or("").to_owned(),
                    );
                    log.lock().unwrap().push(format!("{m} {p}"));
                    let (code, ctype, body) = respond(&m, &p);
                    let reason = match code {
                        200 => "OK",
                        204 => "No Content",
                        _ => "Not Found",
                    };
                    let resp = if code == 204 {
                        format!("HTTP/1.1 204 {reason}\r\n\r\n")
                    } else {
                        format!("HTTP/1.1 {code} {reason}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\n\r\n{body}", body.len())
                    };
                    if s.write_all(resp.as_bytes()).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    seen
}

#[tokio::test(flavor = "multi_thread")]
async fn stop_reports_real_state_even_if_list_is_stale() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("docker26.sock");
    let seen = fake_docker(sock.clone()).await;
    let log = ActivityLog::new(50);
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

    // Así se manifestaba el fallo: justo tras la parada el listado sigue diciendo "running".
    let confirmed = e
        .container_action(ID, Some("web"), ContainerAction::Stop)
        .await
        .unwrap();
    assert_eq!(
        confirmed.status, "exited",
        "la aplicación debe informar del estado real, no del listado desfasado"
    );
    assert_eq!(confirmed.exit_code, Some(0));
    // En el mismo instante el listado está desfasado: fiarse de él (lo que hacía
    // la versión 0.2.0 al refrescar la tabla) mostraría un estado falso.
    assert_eq!(e.list_containers().await.unwrap()[0].state, "running");

    // Las peticiones fueron exactamente parada + inspect, y ambas quedaron registradas.
    let wire: Vec<String> = seen
        .lock()
        .unwrap()
        .iter()
        .skip(3)
        .take(2)
        .cloned()
        .collect();
    assert_eq!(
        wire,
        vec![
            format!("POST /containers/{ID}/stop?t=10"),
            format!("GET /containers/{ID}/json?size=false")
        ]
    );
    let kinds: Vec<_> = log
        .snapshot()
        .into_iter()
        .rev()
        .skip(3)
        .take(2)
        .map(|r| (r.kind, r.status))
        .collect();
    assert_eq!(
        kinds,
        vec![
            ("container.stop".into(), OpStatus::Succeeded),
            ("container.state".into(), OpStatus::Succeeded)
        ]
    );
    let state_rec = log
        .snapshot()
        .into_iter()
        .find(|r| r.kind == "container.state")
        .unwrap();
    assert_eq!(state_rec.summary.as_deref(), Some("Estado: exited"));
    assert!(state_rec.equivalent_command.unwrap().ends_with(&format!(
        "container inspect --format '{{{{.State.Status}}}}' {ID}"
    )));
}
