//! Fase 3 contra Docker real (`DCC_DOCKER_TESTS=1`): creación, inspección,
//! duplicado, logs, estadísticas y terminal. Todo lo creado lleva la etiqueta
//! `dcc.test=true` y se elimina al terminar.

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use common::*;
use dcc_activity::{ActivityLog, OpStatus};
use dcc_engine::spec::{
    ContainerSpec, KeyValue, MountKind, MountSpec, PortSpec, Protocol, RestartName, RestartSpec,
    MIB,
};
use dcc_engine::streams::{EndReason, LogStream, ShellChoice, TerminalInput};
use dcc_engine::{ConnectionState, ContainerAction, Engine, EngineError};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Motor conectado a través de un proxy que registra el tráfico real.
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

/// Elimina contenedores y volúmenes de prueba aunque la prueba falle.
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
                            Some(bollard::query_parameters::RemoveContainerOptions {
                                force: true,
                                v: true,
                                ..Default::default()
                            }),
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
        .ok();
    }
}

fn test_spec(tag: &str) -> ContainerSpec {
    ContainerSpec {
        name: format!("dcc-test-{tag}-{}", std::process::id()),
        image: fixture::IMAGE.into(),
        labels: vec![KeyValue::new("dcc.test", "true")],
        ..Default::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn create_full_spec_inspect_and_duplicate_roundtrip() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (e, log, proxy, docker) = engine_via_proxy(&dir).await;
    let bind_dir = tempfile::tempdir().unwrap();
    let volume = format!("dcc-test-vol-{}", std::process::id());
    let mut clean = Cleanup {
        docker: docker.clone(),
        containers: vec![],
        volumes: vec![volume.clone()],
    };

    let spec = ContainerSpec {
        command: vec!["talk".into(), "2".into()],
        entrypoint: vec!["/sleeper".into()],
        working_dir: "/".into(),
        hostname: "dcc-web".into(),
        env: vec![
            KeyValue::new("MODO", "prueba"),
            KeyValue::new("MSG", "hola mundo; $(id)"),
            KeyValue::new("DB_PASSWORD", "no-debe-aparecer"),
        ],
        labels: vec![
            KeyValue::new("dcc.test", "true"),
            KeyValue::new("equipo", "qa"),
        ],
        ports: vec![PortSpec {
            host_ip: "127.0.0.1".into(),
            host_port: None,
            container_port: 8080,
            protocol: Protocol::Tcp,
        }],
        mounts: vec![
            MountSpec {
                kind: MountKind::Volume,
                source: volume.clone(),
                target: "/data".into(),
                read_only: false,
            },
            MountSpec {
                kind: MountKind::Bind,
                source: bind_dir.path().display().to_string(),
                target: "/host".into(),
                read_only: true,
            },
        ],
        cpus: Some(0.5),
        memory_bytes: Some(64 * MIB),
        restart: RestartSpec {
            name: RestartName::OnFailure,
            max_retries: 2,
        },
        init: true,
        read_only_rootfs: true,
        cap_drop: vec!["ALL".into()],
        extra_hosts: vec![KeyValue::new("api.local", "10.0.0.5")],
        ..test_spec("full")
    };
    let created = e.create_container(&spec).await.expect("crear");
    clean.containers.push(created.id.clone());

    // Lo registrado coincide con el tráfico y lleva el cuerpo y el equivalente
    // exactos, salvo los valores sensibles, que se ocultan.
    let rec = log
        .snapshot()
        .into_iter()
        .find(|r| r.kind == "container.create")
        .unwrap();
    assert_eq!(rec.status, OpStatus::Succeeded);
    assert_eq!(
        rec.equivalent_command.as_deref(),
        Some(
            spec.redacted()
                .0
                .cli_command(false)
                .replacen(
                    "docker ",
                    &format!("docker --host {} ", docker_host(&proxy.path)),
                    1
                )
                .as_str()
        )
    );
    let body: serde_json::Value =
        serde_json::from_str(&rec.request_body.as_ref().unwrap().text).unwrap();
    assert_eq!(body["HostConfig"]["Memory"], 64 * 1024 * 1024);
    assert!(body["Env"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("DB_PASSWORD=********")));
    for r in log.snapshot() {
        let all = format!("{r:?}");
        assert!(
            !all.contains("no-debe-aparecer"),
            "secreto en el registro: {all}"
        );
    }
    assert_eq!(recorded(&log), proxy.requests());
    assert!(
        proxy.requests().iter().any(|(m, p)| m == "POST"
            && p == &format!("/containers/create?name={}&platform=", spec.name)),
        "{:?}",
        proxy.requests()
    );

    // Inspect refleja la configuración pedida.
    let d = e
        .inspect_container(&created.id, Some(&spec.name))
        .await
        .unwrap();
    assert_eq!(d.name, spec.name);
    assert_eq!(d.status, "created");
    assert_eq!(d.entrypoint, vec!["/sleeper"]);
    assert_eq!(d.command, vec!["talk", "2"]);
    assert!(d.env.contains(&KeyValue::new("MSG", "hola mundo; $(id)")));
    // A Docker sí se le envía el valor real.
    assert!(d
        .env
        .contains(&KeyValue::new("DB_PASSWORD", "no-debe-aparecer")));
    assert!(
        d.env.contains(&KeyValue::new("DCC_FIXTURE", "2")),
        "hereda el ENV de la imagen"
    );
    assert_eq!(d.cpus, Some(0.5));
    assert_eq!(d.memory_bytes, Some(64 * MIB));
    assert_eq!(d.restart, spec.restart);
    assert!(d.init && d.read_only_rootfs);
    assert_eq!(d.mounts.len(), 2);
    assert!(d.raw.contains("\"HostConfig\""));

    // Duplicar recupera exactamente la especificación (sin el nombre).
    let dup = e
        .duplicate_spec(&created.id, Some(&spec.name))
        .await
        .unwrap();
    let mut expected = spec.clone();
    expected.name = String::new();
    // La imagen de prueba ya define LABEL dcc.test=true: el duplicado la heredará,
    // así que no se repite en la especificación.
    expected.labels.retain(|l| l.key != "dcc.test");
    assert_eq!(dup.spec, expected);
    assert_eq!(dup.source_name, spec.name);
    assert!(
        dup.notes.contains(&"namedVolumesShared".to_string()),
        "{:?}",
        dup.notes
    );
    assert!(
        !dup.notes.contains(&"portsConflict".to_string()),
        "puerto sin fijar: no hay conflicto"
    );
    assert_eq!(recorded(&log), proxy.requests());

    // Y el duplicado se puede crear tal cual.
    let copy = ContainerSpec {
        name: format!("{}-copia", spec.name),
        ..dup.spec
    };
    let c2 = e.create_container(&copy).await.expect("crear copia");
    clean.containers.push(c2.id);
    proxy.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_spec_never_reaches_docker_and_missing_image_is_reported() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (e, log, proxy, _docker) = engine_via_proxy(&dir).await;
    let before = proxy.requests().len();
    let bad = ContainerSpec {
        mounts: vec![MountSpec {
            kind: MountKind::Bind,
            source: "/no/existe/dcc-test".into(),
            target: "/x".into(),
            read_only: false,
        }],
        ..test_spec("bad")
    };
    match e.create_container(&bad).await {
        Err(EngineError::InvalidSpec { issues }) => {
            assert!(issues.iter().any(|i| i.code == "bind.missing"))
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(proxy.requests().len(), before, "nada viaja a Docker");

    let missing = ContainerSpec {
        image: "dcc-test/no-existe:0".into(),
        ..test_spec("noimg")
    };
    let err = e.create_container(&missing).await.unwrap_err();
    assert_eq!(err.http_status(), Some(404), "{err:?}");
    let rec = log
        .snapshot()
        .into_iter()
        .find(|r| r.kind == "container.create")
        .unwrap();
    assert_eq!(rec.status, OpStatus::Failed);
    assert_eq!(rec.http_status, Some(404));
    proxy.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn logs_stream_follows_stdout_and_stderr_and_closes_cleanly() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (e, log, proxy, docker) = engine_via_proxy(&dir).await;
    let spec = ContainerSpec {
        command: vec!["/sleeper".into(), "talk".into(), "3".into()],
        ..test_spec("logs")
    };
    let c = e.create_container(&spec).await.unwrap();
    let _clean = Cleanup {
        docker,
        containers: vec![c.id.clone()],
        volumes: vec![],
    };
    e.container_action(&c.id, Some(&spec.name), ContainerAction::Start)
        .await
        .unwrap();

    let lines: Arc<Mutex<Vec<dcc_engine::streams::LogLine>>> = Default::default();
    let cancel = CancellationToken::new();
    let (sink_lines, stopper) = (lines.clone(), cancel.clone());
    let watcher = tokio::spawn(async move {
        for _ in 0..100 {
            if sink_lines.lock().unwrap().len() >= 6 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        // Ya se recibió todo; el flujo sigue abierto (follow) hasta cerrarlo.
        tokio::time::sleep(Duration::from_millis(200)).await;
        stopper.cancel();
    });
    let sink = lines.clone();
    let end = e
        .stream_logs(&c.id, Some(&spec.name), Some(100), cancel, move |batch| {
            sink.lock().unwrap().extend(batch);
            true
        })
        .await
        .unwrap();
    watcher.await.unwrap();
    assert_eq!(end.reason, EndReason::Closed);
    let got = lines.lock().unwrap().clone();
    let out: Vec<_> = got
        .iter()
        .filter(|l| l.stream == LogStream::Stdout)
        .map(|l| l.text.as_str())
        .collect();
    let err: Vec<_> = got
        .iter()
        .filter(|l| l.stream == LogStream::Stderr)
        .map(|l| l.text.as_str())
        .collect();
    assert_eq!(out, vec!["linea 1", "linea 2", "linea 3"]);
    assert_eq!(err, vec!["aviso 1", "aviso 2", "aviso 3"]);
    assert!(
        got.iter().all(|l| l.ts.is_some()),
        "todas las líneas llevan marca de tiempo"
    );
    let rec = log
        .snapshot()
        .into_iter()
        .find(|r| r.kind == "container.logs")
        .unwrap();
    assert_eq!(rec.status, OpStatus::Succeeded);
    assert_eq!(
        rec.summary.as_deref(),
        Some("Flujo cerrado por el usuario; 6 líneas recibidas")
    );
    assert!(rec
        .equivalent_command
        .unwrap()
        .ends_with(&format!("logs --follow --timestamps --tail 100 {}", c.id)));

    // Con el contenedor detenido, Docker entrega el historial y cierra el flujo.
    e.container_action(&c.id, None, ContainerAction::Stop)
        .await
        .unwrap();
    let n = Arc::new(Mutex::new(0usize));
    let n2 = n.clone();
    let end = e
        .stream_logs(&c.id, None, None, CancellationToken::new(), move |b| {
            *n2.lock().unwrap() += b.len();
            true
        })
        .await
        .unwrap();
    assert_eq!(end.reason, EndReason::Finished);
    assert_eq!(*n.lock().unwrap(), 6);
    assert_eq!(recorded(&log), proxy.requests());
    proxy.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn stats_stream_delivers_samples() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (e, log, proxy, docker) = engine_via_proxy(&dir).await;
    let spec = test_spec("stats");
    let c = e.create_container(&spec).await.unwrap();
    let _clean = Cleanup {
        docker,
        containers: vec![c.id.clone()],
        volumes: vec![],
    };
    e.container_action(&c.id, None, ContainerAction::Start)
        .await
        .unwrap();
    let samples: Arc<Mutex<Vec<dcc_engine::streams::StatsSample>>> = Default::default();
    let s2 = samples.clone();
    let end = tokio::time::timeout(
        Duration::from_secs(20),
        e.stream_stats(&c.id, None, CancellationToken::new(), move |s| {
            let mut v = s2.lock().unwrap();
            v.push(s);
            v.len() < 3
        }),
    )
    .await
    .expect("las muestras llegan a tiempo")
    .unwrap();
    assert_eq!(end.reason, EndReason::Closed);
    let v = samples.lock().unwrap();
    assert_eq!(v.len(), 3);
    assert!(v[2].memory_used > 0);
    assert!(v[2].memory_limit > 0);
    assert!(
        v[2].cpu_percent.is_some(),
        "a partir de la segunda muestra hay CPU: {:?}",
        v[2]
    );
    let rec = log
        .snapshot()
        .into_iter()
        .find(|r| r.kind == "container.stats")
        .unwrap();
    assert_eq!(
        rec.summary.as_deref(),
        Some("Flujo cerrado por el usuario; 3 muestras")
    );
    assert_eq!(recorded(&log), proxy.requests());
    proxy.stop();
}

type Terminal = (
    tokio::task::JoinHandle<Result<dcc_engine::streams::StreamEnd, EngineError>>,
    mpsc::Sender<TerminalInput>,
    Arc<Mutex<Vec<u8>>>,
    CancellationToken,
);

/// Lanza una terminal y devuelve (manejador, envío de entrada, salida acumulada, cancelación).
fn spawn_terminal(e: &Engine, id: &str, shell: ShellChoice) -> Terminal {
    let (tx, rx) = mpsc::channel(16);
    let out: Arc<Mutex<Vec<u8>>> = Default::default();
    let cancel = CancellationToken::new();
    let (e2, id2, out2, c2) = (e.clone(), id.to_owned(), out.clone(), cancel.clone());
    let h = tokio::spawn(async move {
        e2.run_terminal(
            &id2,
            None,
            shell.argv(),
            (100, 30),
            c2,
            rx,
            move |b: Bytes| {
                out2.lock().unwrap().extend_from_slice(&b);
                true
            },
        )
        .await
    });
    (h, tx, out, cancel)
}

async fn wait_output(out: &Arc<Mutex<Vec<u8>>>, needle: &str) -> bool {
    for _ in 0..100 {
        if String::from_utf8_lossy(&out.lock().unwrap()).contains(needle) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    false
}

#[tokio::test(flavor = "multi_thread")]
async fn terminal_runs_commands_and_reports_exit_code() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (e, log, proxy, docker) = engine_via_proxy(&dir).await;
    let spec = test_spec("term");
    let c = e.create_container(&spec).await.unwrap();
    let _clean = Cleanup {
        docker: docker.clone(),
        containers: vec![c.id.clone()],
        volumes: vec![],
    };

    // Con el contenedor parado, Docker rechaza el exec con 409 y queda registrado.
    let (h, _tx, _out, _c) = spawn_terminal(&e, &c.id, ShellChoice::Sh);
    let err = h.await.unwrap().unwrap_err();
    assert_eq!(err.http_status(), Some(409), "{err:?}");

    e.container_action(&c.id, None, ContainerAction::Start)
        .await
        .unwrap();
    let (h, tx, out, _cancel) = spawn_terminal(&e, &c.id, ShellChoice::Auto);
    tx.send(TerminalInput::Resize {
        cols: 120,
        rows: 40,
    })
    .await
    .unwrap();
    tx.send(TerminalInput::Data(b"echo dcc-$((40+2))\n".to_vec()))
        .await
        .unwrap();
    assert!(
        wait_output(&out, "dcc-42").await,
        "salida: {:?}",
        String::from_utf8_lossy(&out.lock().unwrap())
    );
    tx.send(TerminalInput::Data(b"exit 7\n".to_vec()))
        .await
        .unwrap();
    let end = tokio::time::timeout(Duration::from_secs(10), h)
        .await
        .expect("termina")
        .unwrap()
        .unwrap();
    assert_eq!(end.reason, EndReason::Finished);
    assert_eq!(end.exit_code, Some(7));
    let rec = log
        .snapshot()
        .into_iter()
        .find(|r| r.kind == "container.exec" && r.status != OpStatus::Failed)
        .unwrap();
    assert_eq!(rec.status, OpStatus::Succeeded);
    assert_eq!(rec.exit_code, Some(7));
    assert!(
        rec.summary
            .as_deref()
            .unwrap()
            .starts_with("El proceso terminó con código 7"),
        "{:?}",
        rec.summary
    );
    assert!(
        rec.summary
            .as_deref()
            .unwrap()
            .contains("2 redimensionados"),
        "{:?}",
        rec.summary
    );
    assert!(rec
        .equivalent_command
        .unwrap()
        .contains(&format!("exec --interactive --tty {} /bin/sh -c", c.id)));
    // Todo lo que viaja queda registrado salvo los redimensionados de la ventana,
    // que se resumen en la sesión (D-036): aquí, exactamente 2.
    let wire = proxy.requests();
    let resizes = wire.iter().filter(|(_, p)| p.contains("/resize?")).count();
    assert_eq!(resizes, 2);
    let wire_logged: Vec<_> = wire
        .into_iter()
        .filter(|(_, p)| !p.contains("/resize?"))
        .collect();
    assert_eq!(recorded(&log), wire_logged);
    proxy.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn closing_the_terminal_ends_the_shell_in_the_container() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (e, log, proxy, docker) = engine_via_proxy(&dir).await;
    let spec = test_spec("termclose");
    let c = e.create_container(&spec).await.unwrap();
    let _clean = Cleanup {
        docker: docker.clone(),
        containers: vec![c.id.clone()],
        volumes: vec![],
    };
    e.container_action(&c.id, None, ContainerAction::Start)
        .await
        .unwrap();
    let (h, tx, out, cancel) = spawn_terminal(&e, &c.id, ShellChoice::Sh);
    tx.send(TerminalInput::Data(b"echo listo\n".to_vec()))
        .await
        .unwrap();
    assert!(wait_output(&out, "listo").await);
    cancel.cancel();
    let end = h.await.unwrap().unwrap();
    assert_eq!(end.reason, EndReason::Closed);
    let rec = log
        .snapshot()
        .into_iter()
        .find(|r| r.kind == "container.exec")
        .unwrap();
    assert!(
        rec.summary
            .as_deref()
            .unwrap()
            .starts_with("Sesión cerrada por el usuario; la shell terminó"),
        "{:?}",
        rec.summary
    );

    // La shell recibe EOF y termina: no quedan procesos exec en marcha.
    let mut running = true;
    for _ in 0..60 {
        let info = docker.inspect_container(&c.id, None).await.unwrap();
        let mut any = false;
        for ex in info.exec_ids.unwrap_or_default() {
            if docker
                .inspect_exec(&ex)
                .await
                .ok()
                .and_then(|i| i.running)
                .unwrap_or(false)
            {
                any = true;
            }
        }
        if !any {
            running = false;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(!running, "la shell sigue en marcha tras cerrar la terminal");
    proxy.stop();
}

#[tokio::test(flavor = "multi_thread")]
async fn catalog_lists_are_real() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (e, log, proxy, _docker) = engine_via_proxy(&dir).await;
    let nets = e.networks_brief().await.unwrap();
    assert!(nets
        .iter()
        .any(|n| n.name == "bridge" && n.driver == "bridge"));
    let _vols = e.volumes_brief().await.unwrap();
    let d = e.image_defaults(fixture::IMAGE).await.unwrap();
    assert_eq!(d.command, vec!["/sleeper"]);
    assert!(d.env.contains(&"DCC_FIXTURE=2".to_string()));
    let err = e.image_defaults("dcc-test/no-existe:0").await.unwrap_err();
    assert_eq!(err.http_status(), Some(404));
    assert!(matches!(
        e.image_defaults("a?b").await,
        Err(EngineError::InvalidInput { .. })
    ));
    assert_eq!(recorded(&log), proxy.requests());
    proxy.stop();
}
