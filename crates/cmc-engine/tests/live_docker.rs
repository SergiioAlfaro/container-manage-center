// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Pruebas de integración contra un Docker Engine **real**.
//!
//! Se activan con `CMC_DOCKER_TESTS=1`. El socket se toma de `CMC_DOCKER_SOCKET`
//! o de la detección automática. Sin la variable, cada prueba se omite.
//!
//! Un proxy intercepta el tráfico del socket para comprobar que lo registrado
//! en el panel de actividad es exactamente lo que viajó a Docker.

use std::os::unix::fs::PermissionsExt;

use std::sync::Arc;
use std::time::Duration;

use cmc_activity::{ActivityLog, OpStatus, Transport};
use cmc_engine::endpoint::HostEnv;
use cmc_engine::manager::{attempt, Attempt, Timing};
use cmc_engine::{ComposeStatus, ConnectionMode, ConnectionState, EngineManager, Problem};
mod common;
use common::*;

#[tokio::test(flavor = "multi_thread")]
async fn connects_and_records_successful_handshake() {
    if !enabled() {
        return;
    }
    let sock = real_socket().await;
    let log = ActivityLog::new(100);
    let m = EngineManager::new(
        log.clone(),
        ConnectionMode::Custom {
            socket_path: sock.clone(),
        },
    );
    match m.connect().await {
        ConnectionState::Connected {
            endpoint, engine, ..
        } => {
            assert_eq!(endpoint.socket_path, sock);
            assert!(!engine.version.is_empty());
            assert!(cmc_engine::client::api_at_least(
                &engine.api_version,
                (1, 41)
            ));
        }
        other => panic!("se esperaba conexión: {other:?}"),
    }
    let kinds: Vec<_> = log
        .snapshot()
        .into_iter()
        .rev()
        .map(|r| (r.kind, r.status))
        .collect();
    assert_eq!(
        kinds,
        vec![
            ("system.ping".to_owned(), OpStatus::Succeeded),
            ("system.version".to_owned(), OpStatus::Succeeded),
            ("system.info".to_owned(), OpStatus::Succeeded),
        ]
    );
    let version = &log.snapshot()[1];
    assert!(version
        .equivalent_command
        .as_deref()
        .unwrap()
        .ends_with("version"));
}

#[tokio::test(flavor = "multi_thread")]
async fn recorded_requests_match_what_travels_on_the_socket() {
    if !enabled() {
        return;
    }
    let sock = real_socket().await;
    let dir = tempfile::tempdir().unwrap();
    let proxy = Proxy::start(&dir.path().join("proxy.sock"), sock).await;
    let log = ActivityLog::new(100);
    let m = EngineManager::new(
        log.clone(),
        ConnectionMode::Custom {
            socket_path: proxy.path.display().to_string(),
        },
    );
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    let overview = m.overview().await.unwrap();
    assert!(overview.info.containers.is_some());
    assert_eq!(
        recorded(&log),
        proxy.requests(),
        "el registro debe coincidir con el tráfico real"
    );
    assert_eq!(proxy.requests().len(), 5);
    proxy.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn overview_contains_real_engine_data() {
    if !enabled() {
        return;
    }
    let sock = real_socket().await;
    let m = EngineManager::new(
        ActivityLog::new(100),
        ConnectionMode::Custom { socket_path: sock },
    );
    m.connect().await;
    let o = m.overview().await.unwrap();
    assert!(o.version.version.is_some());
    assert!(o.info.ncpu.unwrap_or(0) > 0);
    assert!(o.info.mem_total.unwrap_or(0) > 0);
    assert!(o.info.storage_driver.is_some());
    let total = o.info.containers.unwrap();
    let parts = o.info.containers_running.unwrap()
        + o.info.containers_paused.unwrap()
        + o.info.containers_stopped.unwrap();
    assert_eq!(total, parts);
}

#[tokio::test(flavor = "multi_thread")]
async fn monitor_detects_loss_and_reconnects() {
    if !enabled() {
        return;
    }
    let sock = real_socket().await;
    let dir = tempfile::tempdir().unwrap();
    let ppath = dir.path().join("proxy.sock");
    let proxy = Proxy::start(&ppath, sock.clone()).await;
    let log = ActivityLog::new(200);
    let timing = Timing {
        heartbeat: Duration::from_millis(300),
        retry_min: Duration::from_millis(200),
        retry_max: Duration::from_millis(400),
    };
    let m = EngineManager::with_options(
        log.clone(),
        ConnectionMode::Custom {
            socket_path: ppath.display().to_string(),
        },
        Arc::new(HostEnv::from_process),
        timing,
    );
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    let mut rx = m.subscribe();
    let _monitor = m.spawn_monitor();

    // "Cae" Docker: el socket desaparece.
    let path = proxy.stop();
    tokio::time::timeout(
        Duration::from_secs(10),
        rx.wait_for(|s| matches!(s, ConnectionState::Disconnected { .. })),
    )
    .await
    .expect("no se detectó la pérdida")
    .unwrap();
    if let ConnectionState::Disconnected { diagnosis, .. } = m.state() {
        assert_eq!(diagnosis.problem, Problem::NoSocket);
    }
    assert!(m.engine().await.is_err());
    // La pérdida queda registrada con un ping fallido.
    assert!(log
        .snapshot()
        .iter()
        .any(|r| r.kind == "system.ping" && r.status == OpStatus::Failed));

    // Vuelve Docker.
    let _proxy = Proxy::start(&path, sock).await;
    tokio::time::timeout(
        Duration::from_secs(10),
        rx.wait_for(|s| matches!(s, ConnectionState::Connected { .. })),
    )
    .await
    .expect("no se reconectó")
    .unwrap();
    assert!(m.engine().await.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn socket_without_permission_is_diagnosed() {
    if !enabled() {
        return;
    }
    if cmc_engine::sys::uid() == 0 {
        eprintln!("omitida: root ignora los permisos de archivo");
        return;
    }
    let sock = real_socket().await;
    let dir = tempfile::tempdir().unwrap();
    let ppath = dir.path().join("cerrado.sock");
    let proxy = Proxy::start(&ppath, sock).await;
    std::fs::set_permissions(&ppath, std::fs::Permissions::from_mode(0o000)).unwrap();
    let log = ActivityLog::new(10);
    let mode = ConnectionMode::Custom {
        socket_path: ppath.display().to_string(),
    };
    match attempt(&log, &mode, &HostEnv::from_process(), true).await {
        Attempt::Failed(d) => match d.problem {
            Problem::PermissionDenied { permissions } => {
                assert_eq!(permissions.mode, "0");
                assert_eq!(permissions.socket_path, ppath.display().to_string());
            }
            other => panic!("se esperaba PermissionDenied: {other:?}"),
        },
        Attempt::Connected(..) => panic!("no debería conectar"),
    }
    assert!(
        proxy.requests().is_empty(),
        "sin permiso no debe viajar ninguna petición"
    );
    proxy.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn compose_version_is_obtained_from_the_real_cli() {
    if !enabled() {
        return;
    }
    let sock = real_socket().await;
    let log = ActivityLog::new(10);
    let m = EngineManager::new(log.clone(), ConnectionMode::Custom { socket_path: sock });
    m.connect().await;
    let status = m.compose_status().await;
    let runs: Vec<_> = log
        .snapshot()
        .into_iter()
        .rev()
        .filter(|r| r.kind == "compose.version")
        .collect();
    // Nunca se usan opciones como --short o --format para detectar (D-032).
    for r in &runs {
        match &r.transport {
            Transport::Cli { args, .. } => assert!(
                args == &["compose", "version"] || args == &["version"],
                "{args:?}"
            ),
            other => panic!("{other:?}"),
        }
    }
    match status {
        ComposeStatus::Available { version, kind, .. } => {
            let last = runs.last().expect("ejecución registrada");
            assert_eq!(last.status, OpStatus::Succeeded);
            assert!(last
                .stdout
                .as_ref()
                .unwrap()
                .text
                .contains(version.split('+').next().unwrap()));
            eprintln!("Compose {version} ({kind:?})");
        }
        ComposeStatus::NotInstalled { searched, .. } => {
            assert!(!searched.is_empty());
            eprintln!("omitida: Docker Compose no está instalado en este equipo (se buscó en {} ubicaciones)", searched.len());
        }
        other => panic!("estado de Compose inesperado en este equipo: {other:?}"),
    }
}

/// Caso real: usuario fuera del grupo `docker` frente al socket del sistema.
/// Se activa con `CMC_EXPECT_PERMISSION_DENIED=1` en un equipo preparado para ello.
#[tokio::test(flavor = "multi_thread")]
async fn system_socket_denied_for_user_outside_docker_group() {
    if std::env::var("CMC_EXPECT_PERMISSION_DENIED").as_deref() != Ok("1") {
        eprintln!("omitida: define CMC_EXPECT_PERMISSION_DENIED=1");
        return;
    }
    let env = HostEnv {
        docker_host: None,
        docker_context: None,
        ..HostEnv::from_process()
    };
    match attempt(&ActivityLog::new(10), &ConnectionMode::Auto, &env, true).await {
        Attempt::Failed(d) => match d.problem {
            Problem::PermissionDenied { permissions } => {
                assert!(permissions.socket_path.ends_with("docker.sock"));
                assert_eq!(permissions.group, "docker");
                assert!(!permissions.user_in_group);
                assert!(!permissions.group_active_in_session);
                // El rootless no existe y no debe tapar el problema real.
                assert_eq!(
                    d.candidates.first().map(|c| &c.result),
                    Some(&cmc_engine::ProbeResult::Missing)
                );
            }
            other => panic!("se esperaba PermissionDenied: {other:?}"),
        },
        Attempt::Connected(..) => panic!("no debería conectar"),
    }
}

// ------------------------------------------------------------------ contenedores

async fn state_of(engine: &cmc_engine::Engine, id: &str) -> String {
    engine
        .list_containers()
        .await
        .unwrap()
        .into_iter()
        .find(|c| c.id == id)
        .map(|c| c.state)
        .unwrap_or_else(|| "absent".into())
}

#[tokio::test(flavor = "multi_thread")]
async fn container_lifecycle_against_real_docker_matches_wire() {
    use cmc_engine::ContainerAction::*;
    if !enabled() {
        return;
    }
    let sock = real_socket().await;
    let docker =
        bollard::Docker::connect_with_unix(&sock, 60, bollard::API_DEFAULT_VERSION).unwrap();
    let c = fixture::TestContainer::create(&docker, "life").await;

    let dir = tempfile::tempdir().unwrap();
    let proxy = Proxy::start(&dir.path().join("p.sock"), sock).await;
    let log = ActivityLog::new(500);
    let m = EngineManager::new(
        log.clone(),
        ConnectionMode::Custom {
            socket_path: proxy.path.display().to_string(),
        },
    );
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    let e = m.engine().await.unwrap();

    let listed = e.list_containers().await.unwrap();
    let mine = listed
        .iter()
        .find(|x| x.id == c.id)
        .expect("el contenedor de prueba aparece en la lista");
    assert_eq!(mine.name, c.name);
    assert_eq!(mine.image, fixture::IMAGE);
    assert_eq!(mine.state, "created");

    // Cada acción devuelve el estado confirmado por inspect; después el listado
    // también debe reflejarlo (inspect sincroniza con la réplica del listado, D-031).
    for (action, expected) in [
        (Start, "running"),
        (Pause, "paused"),
        (Unpause, "running"),
        (Restart, "running"),
    ] {
        let confirmed = e
            .container_action(&c.id, Some(&c.name), action)
            .await
            .unwrap();
        assert_eq!(
            confirmed.status, expected,
            "estado confirmado tras {action:?}"
        );
        assert_eq!(
            state_of(&e, &c.id).await,
            expected,
            "listado tras {action:?}"
        );
    }

    // Eliminar en ejecución sin forzar: Docker lo rechaza con 409 y queda registrado.
    let err = e
        .remove_container(&c.id, Some(&c.name), false, false)
        .await
        .unwrap_err();
    assert_eq!(err.http_status(), Some(409), "{err:?}");
    let failed = log
        .snapshot()
        .into_iter()
        .find(|r| r.kind == "container.remove")
        .unwrap();
    assert_eq!(failed.status, OpStatus::Failed);
    assert_eq!(failed.http_status, Some(409));
    assert!(failed.error.as_deref().unwrap().contains("409"));

    let confirmed = e
        .container_action(&c.id, Some(&c.name), Stop)
        .await
        .unwrap();
    assert_eq!(confirmed.status, "exited", "estado confirmado tras Stop");
    assert_eq!(state_of(&e, &c.id).await, "exited", "listado tras Stop");
    e.remove_container(&c.id, Some(&c.name), false, true)
        .await
        .unwrap();
    assert_eq!(state_of(&e, &c.id).await, "absent");

    // Todo lo registrado coincide, en orden, con lo que viajó por el socket.
    assert_eq!(recorded(&log), proxy.requests());
    let paths: Vec<_> = recorded(&log)
        .into_iter()
        .map(|(m, p)| format!("{m} {p}"))
        .collect();
    assert!(
        paths.contains(&format!("POST /containers/{}/stop?t=10", c.id)),
        "{paths:#?}"
    );
    assert!(
        paths.contains(&format!(
            "DELETE /containers/{}?v=true&force=false&link=false",
            c.id
        )),
        "{paths:#?}"
    );

    let eq: Vec<_> = log
        .snapshot()
        .into_iter()
        .rev()
        .filter(|r| r.kind.starts_with("container.") && r.kind != "container.list")
        .filter_map(|r| r.equivalent_command)
        .collect();
    assert!(
        eq.iter()
            .any(|c2| c2.ends_with(&format!("stop --time 10 {}", c.id))),
        "{eq:#?}"
    );
    assert!(
        eq.iter()
            .any(|c2| c2.ends_with(&format!("rm --volumes {}", c.id))),
        "{eq:#?}"
    );
    proxy.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_actions_on_same_container_are_rejected() {
    if !enabled() {
        return;
    }
    let sock = real_socket().await;
    let docker =
        bollard::Docker::connect_with_unix(&sock, 60, bollard::API_DEFAULT_VERSION).unwrap();
    let c = fixture::TestContainer::create(&docker, "busy").await;
    let m = EngineManager::new(
        ActivityLog::new(100),
        ConnectionMode::Custom { socket_path: sock },
    );
    m.connect().await;
    let e = m.engine().await.unwrap();
    e.container_action(&c.id, None, cmc_engine::ContainerAction::Start)
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        e.container_action(&c.id, None, cmc_engine::ContainerAction::Stop),
        e.container_action(&c.id, None, cmc_engine::ContainerAction::Restart)
    );
    let busy = [&a, &b]
        .iter()
        .filter(|r| matches!(r, Err(cmc_engine::EngineError::Busy { .. })))
        .count();
    assert_eq!(busy, 1, "una de las dos debe rechazarse: {a:?} {b:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_container_reference_never_reaches_docker() {
    if !enabled() {
        return;
    }
    let sock = real_socket().await;
    let dir = tempfile::tempdir().unwrap();
    let proxy = Proxy::start(&dir.path().join("p.sock"), sock).await;
    let m = EngineManager::new(
        ActivityLog::new(100),
        ConnectionMode::Custom {
            socket_path: proxy.path.display().to_string(),
        },
    );
    m.connect().await;
    let before = proxy.requests().len();
    let e = m.engine().await.unwrap();
    for bad in ["../../images/json", "a/b", "x?force=true"] {
        assert!(matches!(
            e.remove_container(bad, None, true, true).await,
            Err(cmc_engine::EngineError::InvalidInput { .. })
        ));
    }
    assert_eq!(proxy.requests().len(), before);
    proxy.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn images_list_reports_real_usage() {
    if !enabled() {
        return;
    }
    let sock = real_socket().await;
    let docker =
        bollard::Docker::connect_with_unix(&sock, 60, bollard::API_DEFAULT_VERSION).unwrap();
    let c = fixture::TestContainer::create(&docker, "img").await;
    let dir = tempfile::tempdir().unwrap();
    let proxy = Proxy::start(&dir.path().join("p.sock"), sock).await;
    let log = ActivityLog::new(100);
    let m = EngineManager::new(
        log.clone(),
        ConnectionMode::Custom {
            socket_path: proxy.path.display().to_string(),
        },
    );
    m.connect().await;
    let e = m.engine().await.unwrap();
    let images = e.list_images(None).await.unwrap();
    let img = images
        .iter()
        .find(|i| i.repo_tags.iter().any(|t| t == fixture::IMAGE))
        .expect("imagen de prueba listada");
    assert!(img.size > 0);
    assert!(!img.dangling);
    assert!(img.used_by.iter().any(|u| u.id == c.id && u.name == c.name));
    assert_eq!(recorded(&log), proxy.requests());
    proxy.stop();
}
