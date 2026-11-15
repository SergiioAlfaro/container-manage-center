// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Fase 8 contra Docker real (`DCC_DOCKER_TESTS=1`): suscripción a los eventos
//! de Docker en tiempo real y consulta de los eventos que conserva, con sus
//! filtros. Las peticiones pasan por un proxy que registra lo que llega a
//! Docker. Todo lo creado lleva la etiqueta `dcc.test=true` y se elimina al
//! terminar, aunque la prueba falle.

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::*;
use dcc_activity::{ActivityLog, OpStatus, Transport};
use dcc_engine::events::{DockerEvent, EventQuery};
use dcc_engine::{ConnectionState, Engine, EngineError};
use tokio_util::sync::CancellationToken;

async fn engine_via_proxy(
    dir: &tempfile::TempDir,
) -> (Engine, ActivityLog, Proxy, bollard::Docker) {
    let sock = real_socket().await;
    let docker =
        bollard::Docker::connect_with_unix(&sock, 60, bollard::API_DEFAULT_VERSION).unwrap();
    fixture::ensure_image(&docker).await;
    let proxy = Proxy::start(&dir.path().join("p.sock"), sock).await;
    let log = ActivityLog::new(1000);
    // Local o, con DCC_SSH_TEST, por SSH con el proxy detrás (0.10.0).
    let m = common::manager(log.clone(), proxy.path.display().to_string());
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    (m.engine().await.unwrap(), log, proxy, docker)
}

struct Cleanup {
    docker: bollard::Docker,
    containers: Vec<String>,
    volumes: Vec<String>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        let d = self.docker.clone();
        let (cs, vs) = (
            std::mem::take(&mut self.containers),
            std::mem::take(&mut self.volumes),
        );
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                for c in cs {
                    let _ = d
                        .remove_container(
                            &c,
                            Some(
                                bollard::query_parameters::RemoveContainerOptionsBuilder::default()
                                    .force(true)
                                    .build(),
                            ),
                        )
                        .await;
                }
                for v in vs {
                    let _ = d
                        .remove_volume(&v, None::<bollard::query_parameters::RemoveVolumeOptions>)
                        .await;
                }
            });
        })
        .join()
        .unwrap();
    }
}

fn labels() -> std::collections::HashMap<String, String> {
    [("dcc.test".to_owned(), "true".to_owned())].into()
}

async fn wait_for(
    events: &Arc<Mutex<Vec<DockerEvent>>>,
    what: &str,
    f: impl Fn(&[DockerEvent]) -> bool,
) {
    for _ in 0..200 {
        if f(&events.lock().unwrap()) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!(
        "no llegó {what}: {:?}",
        events
            .lock()
            .unwrap()
            .iter()
            .map(|e| (&e.kind, &e.action, &e.name))
            .collect::<Vec<_>>()
    );
}

/// Suscripción en tiempo real y consulta posterior con filtros.
#[tokio::test(flavor = "multi_thread")]
async fn watch_in_real_time_then_query_with_filters() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (engine, log, proxy, docker) = engine_via_proxy(&dir).await;
    let tag = std::process::id();
    let vol = format!("dcc-test-ev-vol-{tag}");
    let name = format!("dcc-test-ev-ctr-{tag}");
    let mut cleanup = Cleanup {
        docker: docker.clone(),
        containers: vec![name.clone()],
        volumes: vec![vol.clone()],
    };

    let seen: Arc<Mutex<Vec<DockerEvent>>> = Default::default();
    let cancel = CancellationToken::new();
    let watch = {
        let (e, s, c) = (engine.clone(), seen.clone(), cancel.clone());
        tokio::spawn(async move {
            e.events_watch(c, move |ev| {
                s.lock().unwrap().push(ev);
                true
            })
            .await
        })
    };
    // La suscripción queda registrada en cuanto Docker la acepta, sin esperar eventos.
    for _ in 0..100 {
        if log
            .snapshot()
            .iter()
            .any(|r| r.kind == "events.watch" && r.status == OpStatus::Succeeded)
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let rec = log
        .snapshot()
        .into_iter()
        .find(|r| r.kind == "events.watch")
        .expect("suscripción registrada");
    assert_eq!(rec.status, OpStatus::Succeeded, "{rec:?}");
    match &rec.transport {
        Transport::Api { method, path, .. } => {
            assert_eq!((method.as_str(), path.as_str()), ("GET", "/events"))
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        rec.equivalent_command.as_deref(),
        Some(engine.docker_cli(&["events".to_owned()]).as_str())
    );
    let since = chrono::Utc::now().timestamp() - 1;

    // Cambios hechos fuera de la aplicación (bollard directamente, como el CLI).
    docker
        .create_volume(bollard::models::VolumeCreateRequest {
            name: Some(vol.clone()),
            labels: Some(labels()),
            ..Default::default()
        })
        .await
        .unwrap();
    docker
        .create_container(
            Some(
                bollard::query_parameters::CreateContainerOptionsBuilder::default()
                    .name(&name)
                    .build(),
            ),
            bollard::models::ContainerCreateBody {
                image: Some(fixture::IMAGE.into()),
                labels: Some(labels()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    docker
        .start_container(
            &name,
            None::<bollard::query_parameters::StartContainerOptions>,
        )
        .await
        .unwrap();
    docker
        .stop_container(
            &name,
            Some(
                bollard::query_parameters::StopContainerOptionsBuilder::default()
                    .t(1)
                    .build(),
            ),
        )
        .await
        .unwrap();
    docker
        .remove_container(
            &name,
            None::<bollard::query_parameters::RemoveContainerOptions>,
        )
        .await
        .unwrap();
    docker
        .remove_volume(&vol, None::<bollard::query_parameters::RemoveVolumeOptions>)
        .await
        .unwrap();
    cleanup.containers.clear();
    cleanup.volumes.clear();

    wait_for(&seen, "la eliminación del volumen", |evs| {
        evs.iter()
            .any(|e| e.kind == "volume" && e.action == "destroy" && e.actor_id == vol)
    })
    .await;
    let mine: Vec<(String, String)> = seen
        .lock()
        .unwrap()
        .iter()
        .filter(|e| e.name.as_deref() == Some(name.as_str()) || e.actor_id == vol)
        .map(|e| (e.kind.clone(), e.action.clone()))
        .collect();
    let pos = |k: &str, a: &str| {
        mine.iter()
            .position(|x| x.0 == k && x.1 == a)
            .unwrap_or_else(|| panic!("falta {k} {a}: {mine:?}"))
    };
    // En orden, tal como ocurrieron.
    assert!(pos("volume", "create") < pos("container", "create"));
    assert!(pos("container", "create") < pos("container", "start"));
    assert!(pos("container", "start") < pos("container", "die"));
    assert!(pos("container", "die") < pos("container", "destroy"));
    assert!(pos("container", "destroy") < pos("volume", "destroy"));
    let start = seen
        .lock()
        .unwrap()
        .iter()
        .find(|e| e.action == "start" && e.name.as_deref() == Some(name.as_str()))
        .cloned()
        .unwrap();
    assert_eq!(
        start.attributes.get("image").map(String::as_str),
        Some(fixture::IMAGE)
    );
    assert!(start.time_nano / 1_000_000_000 >= since);

    cancel.cancel();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), watch)
            .await
            .unwrap()
            .unwrap(),
        Ok(())
    );

    // Consulta: solo ese contenedor y solo `start` y `die`.
    let until = chrono::Utc::now().timestamp();
    let q = EventQuery {
        since,
        until,
        types: vec!["container".into()],
        actions: vec!["start".into(), "die".into()],
        resource: Some(name.clone()),
    };
    let r = engine.events_query(&q).await.unwrap();
    let sent = log
        .snapshot()
        .into_iter()
        .rev()
        .find(|r| r.kind == "events.query")
        .unwrap();
    let sent_path = match &sent.transport {
        Transport::Api { path, .. } => path.clone(),
        other => panic!("{other:?}"),
    };
    assert!(
        sent_path.contains("until=") && sent_path.contains("%2C%22event%22"),
        "{sent_path}"
    );
    assert_eq!(
        r.events
            .iter()
            .map(|e| e.action.as_str())
            .collect::<Vec<_>>(),
        ["start", "die"]
    );
    assert!(r
        .events
        .iter()
        .all(|e| e.name.as_deref() == Some(name.as_str())));
    assert!(!r.truncated);
    assert!(
        r.command
            .contains(&format!(" events --since {since} --until {until}.")),
        "{}",
        r.command
    );
    assert!(r.command.ends_with(&format!("--filter container={name} --filter event=start --filter event=die --filter type=container")), "{}", r.command);
    // Sin filtros aparecen también el volumen y el resto de acciones.
    let all = engine
        .events_query(&EventQuery {
            since,
            until,
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(all
        .events
        .iter()
        .any(|e| e.kind == "volume" && e.actor_id == vol));
    assert!(all.events.len() > r.events.len());

    // Lo que se registró es exactamente lo que llegó a Docker.
    let reqs = proxy.requests();
    assert!(
        reqs.contains(&("GET".to_owned(), "/events".to_owned())),
        "{reqs:?}"
    );
    assert!(
        reqs.contains(&("GET".to_owned(), sent_path.clone())),
        "{reqs:?}"
    );
    let qrec = log
        .snapshot()
        .into_iter()
        .find(|r| {
            r.kind == "events.query"
                && matches!(&r.transport, Transport::Api { path, .. } if *path == sent_path)
        })
        .expect("consulta filtrada registrada");
    assert_eq!(qrec.status, OpStatus::Succeeded);
    assert_eq!(qrec.summary.as_deref(), Some("2 eventos"));
}

/// Una consulta inválida nunca llega a Docker.
#[tokio::test(flavor = "multi_thread")]
async fn invalid_queries_never_reach_docker() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (engine, log, proxy, _docker) = engine_via_proxy(&dir).await;
    let now = chrono::Utc::now().timestamp();
    let before = proxy.requests().len();
    for q in [
        EventQuery {
            since: now,
            until: now - 10,
            ..Default::default()
        },
        EventQuery {
            since: 0,
            until: now + 3600,
            ..Default::default()
        },
        EventQuery {
            since: 0,
            until: now,
            types: vec!["bogus".into()],
            ..Default::default()
        },
        EventQuery {
            since: 0,
            until: now,
            types: vec!["container".into()],
            resource: Some("a\"b".into()),
            ..Default::default()
        },
    ] {
        let e = engine.events_query(&q).await.unwrap_err();
        assert!(matches!(e, EngineError::InvalidInput { .. }), "{e:?}");
    }
    assert_eq!(proxy.requests().len(), before);
    assert!(!log.snapshot().iter().any(|r| r.kind == "events.query"));
}
