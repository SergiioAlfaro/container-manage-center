// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Suscripción a los eventos de Docker: interrupción y rechazo (determinista).
//! Con Docker real solo se podría provocar reiniciando el daemon.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use cmc_activity::{ActivityLog, OpStatus};
use cmc_engine::{ConnectionMode, ConnectionState, EngineError, EngineManager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
use tokio_util::sync::CancellationToken;

const EVENT: &str = r#"{"Type":"container","Action":"start","Actor":{"ID":"abc","Attributes":{"name":"web"}},"scope":"local","time":1,"timeNano":1000000001}"#;

/// `mode`: 0 = envía un evento y corta la conexión; 1 = responde 500.
async fn fake_docker(path: std::path::PathBuf, mode: u32) -> Arc<AtomicU32> {
    let events_requests = Arc::new(AtomicU32::new(0));
    let counter = events_requests.clone();
    let listener = UnixListener::bind(&path).unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut s, _)) = listener.accept().await else {
                break;
            };
            let counter = counter.clone();
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
                    let path = head
                        .lines()
                        .next()
                        .unwrap_or("")
                        .split(' ')
                        .nth(1)
                        .unwrap_or("")
                        .to_owned();
                    if path.starts_with("/events") {
                        counter.fetch_add(1, Ordering::SeqCst);
                        if mode == 1 {
                            let body = r#"{"message":"error interno simulado"}"#;
                            let _ = s
                                .write_all(format!("HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes())
                                .await;
                            return;
                        }
                        let ev = format!("{EVENT}\n");
                        let _ = s
                            .write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{ev}\r\n", ev.len()).as_bytes())
                            .await;
                        let _ = s.flush().await;
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        // Docker se detiene: la conexión se corta sin el bloque final.
                        return;
                    }
                    let body = match path.split('?').next().unwrap_or("") {
                        "/_ping" => "OK".to_owned(),
                        "/version" => r#"{"Version":"26.1.5","ApiVersion":"1.45","MinAPIVersion":"1.24","Os":"linux","Arch":"amd64"}"#.to_owned(),
                        "/info" => r#"{"Containers":0,"ContainersRunning":0,"ContainersPaused":0,"ContainersStopped":0,"Images":0,"SecurityOptions":[]}"#.to_owned(),
                        _ => r#"{"message":"no implementado"}"#.to_owned(),
                    };
                    if s
                        .write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes())
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            });
        }
    });
    events_requests
}

async fn engine(
    mode: u32,
) -> (
    cmc_engine::Engine,
    ActivityLog,
    Arc<AtomicU32>,
    tempfile::TempDir,
) {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("d.sock");
    let counter = fake_docker(sock.clone(), mode).await;
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
    (m.engine().await.unwrap(), log, counter, dir)
}

#[tokio::test(flavor = "multi_thread")]
async fn an_interrupted_subscription_is_reported_and_recorded() {
    let (e, log, counter, _dir) = engine(0).await;
    let mut got = Vec::new();
    let r = e
        .events_watch(CancellationToken::new(), |ev| {
            got.push(ev);
            true
        })
        .await;
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    assert_eq!(got.len(), 1, "el evento enviado antes del corte se entrega");
    assert_eq!(
        (got[0].action.as_str(), got[0].name.as_deref()),
        ("start", Some("web"))
    );
    assert!(
        matches!(
            r,
            Err(EngineError::Io { .. }) | Err(EngineError::InvalidResponse { .. })
        ),
        "{r:?}"
    );
    let recs: Vec<_> = log
        .snapshot()
        .into_iter()
        .filter(|r| r.kind == "events.watch")
        .collect();
    assert_eq!(recs.len(), 2, "apertura y corte: {recs:?}");
    assert!(recs
        .iter()
        .any(|r| r.status == OpStatus::Succeeded && r.http_status == Some(200)));
    let failed = recs.iter().find(|r| r.status == OpStatus::Failed).unwrap();
    assert!(failed
        .error
        .as_deref()
        .unwrap()
        .starts_with("Suscripción a los eventos interrumpida"));
    // Nunca queda una operación "en curso".
    assert!(!log.snapshot().iter().any(|r| r.status == OpStatus::Running));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rejected_subscription_is_a_recorded_failure() {
    let (e, log, _c, _dir) = engine(1).await;
    let r = e.events_watch(CancellationToken::new(), |_| true).await;
    assert_eq!(
        r,
        Err(EngineError::Docker {
            status: 500,
            message: "error interno simulado".into()
        })
    );
    let rec = log
        .snapshot()
        .into_iter()
        .find(|r| r.kind == "events.watch")
        .unwrap();
    assert_eq!((rec.status, rec.http_status), (OpStatus::Failed, Some(500)));
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelling_closes_the_subscription_cleanly() {
    let (e, log, _c, _dir) = engine(0).await;
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert_eq!(e.events_watch(cancel, |_| true).await, Ok(()));
    assert!(!log.snapshot().iter().any(|r| r.status == OpStatus::Running));
}
