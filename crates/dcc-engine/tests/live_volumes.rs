//! Fase 6 contra Docker real (`DCC_DOCKER_TESTS=1`): volúmenes (creación,
//! listado con contenedores, tamaño, inspección, eliminación, limpieza) y
//! directorios vinculados. Todo lo creado lleva la etiqueta `dcc.test=true` y
//! se elimina al terminar, aunque la prueba falle.

mod common;

use std::collections::HashMap;

use common::*;
use dcc_activity::{ActivityLog, OpStatus};
use dcc_engine::binds::{BindRisk, PathKind};
use dcc_engine::build::BuildSpec;
use dcc_engine::spec::KeyValue;
use dcc_engine::volumes::VolumeSpec;
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

/// Elimina contenedores, volúmenes e imágenes de prueba aunque la prueba falle.
struct Cleanup {
    docker: Option<bollard::Docker>,
    containers: Vec<String>,
    volumes: Vec<String>,
    images: Vec<String>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        let Some(d) = self.docker.clone() else { return };
        let (cs, vs, is) = (
            std::mem::take(&mut self.containers),
            std::mem::take(&mut self.volumes),
            std::mem::take(&mut self.images),
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
                for i in is {
                    let _ = d
                        .remove_image(
                            &i,
                            Some(bollard::query_parameters::RemoveImageOptions {
                                force: true,
                                ..Default::default()
                            }),
                            None,
                        )
                        .await;
                }
            });
        })
        .join()
        .ok();
    }
}

fn tag(s: &str) -> String {
    format!("dcc-test-{s}-{}", std::process::id())
}

fn records(log: &ActivityLog, kind: &str) -> Vec<dcc_activity::OperationRecord> {
    log.snapshot()
        .into_iter()
        .filter(|r| r.kind == kind)
        .collect()
}

fn test_labels() -> HashMap<String, String> {
    let mut l = HashMap::new();
    l.insert("dcc.test".to_owned(), "true".to_owned());
    l
}

/// Crea un contenedor con los montajes dados (formato `HostConfig.Mounts`).
async fn container_with(
    docker: &bollard::Docker,
    name: &str,
    image: &str,
    mounts: Vec<bollard::models::Mount>,
    start: bool,
) -> String {
    let id = docker
        .create_container(
            Some(bollard::query_parameters::CreateContainerOptions {
                name: Some(name.to_owned()),
                platform: String::new(),
            }),
            bollard::models::ContainerCreateBody {
                image: Some(image.into()),
                labels: Some(test_labels()),
                host_config: Some(bollard::models::HostConfig {
                    mounts: Some(mounts),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .id;
    if start {
        docker
            .start_container(
                &id,
                None::<bollard::query_parameters::StartContainerOptions>,
            )
            .await
            .unwrap();
    }
    id
}

fn volume_mount(name: &str, target: &str, ro: bool) -> bollard::models::Mount {
    bollard::models::Mount {
        typ: Some(bollard::models::MountType::VOLUME),
        source: Some(name.into()),
        target: Some(target.into()),
        read_only: Some(ro),
        ..Default::default()
    }
}

fn bind_mount(src: &str, target: &str, ro: bool) -> bollard::models::Mount {
    bollard::models::Mount {
        typ: Some(bollard::models::MountType::BIND),
        source: Some(src.into()),
        target: Some(target.into()),
        read_only: Some(ro),
        ..Default::default()
    }
}

/// Tamaño del archivo que la imagen de prueba lleva en /data.
const DATA: usize = 64 * 1024;

/// Imagen con un archivo de 64 KiB en /data: al montar un volumen vacío en
/// /data, Docker copia ese contenido al volumen, así el tamaño es conocido.
async fn image_with_data(engine: &Engine, cleanup: &mut Cleanup) -> String {
    let ctx = tempfile::tempdir().unwrap();
    std::fs::write(ctx.path().join("datos.bin"), vec![7u8; DATA]).unwrap();
    std::fs::write(
        ctx.path().join("Dockerfile"),
        format!(
            "FROM {}\nCOPY datos.bin /data/datos.bin\nLABEL dcc.test=true\n",
            fixture::IMAGE
        ),
    )
    .unwrap();
    let image = format!("dcc-test/volumen-datos-{}:1", std::process::id());
    cleanup.images.push(image.clone());
    let spec = BuildSpec {
        context: ctx.path().to_string_lossy().into_owned(),
        dockerfile: "Dockerfile".into(),
        tags: vec![image.clone()],
        ..Default::default()
    };
    engine
        .build_image(&spec, CancellationToken::new(), |_| true)
        .await
        .unwrap();
    image
}

#[tokio::test(flavor = "multi_thread")]
async fn create_list_size_inspect_and_remove() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (engine, log, proxy, docker) = engine_via_proxy(&dir).await;
    let name = tag("vol");
    let (run, stopped) = (tag("vol-run"), tag("vol-stopped"));
    let mut cleanup = Cleanup {
        docker: Some(docker.clone()),
        containers: vec![run.clone(), stopped.clone()],
        volumes: vec![name.clone()],
        images: vec![],
    };
    let image = image_with_data(&engine, &mut cleanup).await;

    // Creación.
    let spec = VolumeSpec {
        name: name.clone(),
        labels: vec![
            KeyValue::new("dcc.test", "true"),
            KeyValue::new("dcc.app", "mi web"),
        ],
        ..Default::default()
    };
    assert!(spec.validate().is_empty());
    let v = engine.create_volume(&spec).await.unwrap();
    assert_eq!(v.name, name);
    assert_eq!(v.driver, "local");
    assert_eq!(v.labels["dcc.app"], "mi web");
    let r = &records(&log, "volume.create")[0];
    assert_eq!(r.status, OpStatus::Succeeded);
    assert!(r.equivalent_command.as_deref().unwrap().ends_with(&format!(
        "volume create --driver local --label 'dcc.app=mi web' --label dcc.test=true {name}"
    )));

    // Repetir el nombre: Docker devolvería el existente sin cambios; la
    // aplicación lo detecta y no envía la creación.
    let creates_before = proxy
        .requests()
        .iter()
        .filter(|(m, p)| m == "POST" && p == "/volumes/create")
        .count();
    let dup = VolumeSpec {
        name: name.clone(),
        labels: vec![KeyValue::new("otra", "x")],
        ..Default::default()
    };
    match engine.create_volume(&dup).await {
        Err(EngineError::InvalidSpec { issues }) => {
            assert_eq!(issues[0].code, "volumeName.exists");
        }
        other => panic!("{other:?}"),
    }
    let creates_after = proxy
        .requests()
        .iter()
        .filter(|(m, p)| m == "POST" && p == "/volumes/create")
        .count();
    assert_eq!(
        creates_before, creates_after,
        "no se envía la segunda creación"
    );
    let real = docker.inspect_volume(&name).await.unwrap();
    assert!(
        !real.labels.contains_key("otra"),
        "las etiquetas no cambian"
    );

    // Un contenedor en marcha (escritura) y otro detenido (solo lectura).
    container_with(
        &docker,
        &run,
        &image,
        vec![volume_mount(&name, "/data", false)],
        true,
    )
    .await;
    container_with(
        &docker,
        &stopped,
        fixture::IMAGE,
        vec![volume_mount(&name, "/copia", true)],
        false,
    )
    .await;

    let list = engine.list_volumes(None).await.unwrap();
    let s = list
        .iter()
        .find(|x| x.name == name)
        .expect("volumen en la lista");
    assert!(!s.anonymous);
    let users: Vec<_> = s
        .containers
        .iter()
        .map(|u| {
            (
                u.name.as_str(),
                u.state.as_str(),
                u.destination.as_str(),
                u.rw,
            )
        })
        .collect();
    let mut expected = vec![
        (run.as_str(), "running", "/data", true),
        (stopped.as_str(), "created", "/copia", false),
    ];
    expected.sort();
    assert_eq!(users, expected);

    // Tamaño real: el contenido de /data de la imagen se copió al volumen.
    let sizes = engine.volume_sizes().await.unwrap();
    let sz = sizes
        .iter()
        .find(|x| x.name == name)
        .expect("tamaño del volumen");
    assert!(
        sz.size.is_some_and(|b| b >= DATA as i64),
        "{sz:?} debería medir al menos {DATA} bytes"
    );
    assert_eq!(sz.ref_count, Some(2));
    assert_eq!(records(&log, "volume.usage")[0].status, OpStatus::Succeeded);

    // Detalle.
    let d = engine.volume_details(&name).await.unwrap();
    assert_eq!(d.summary.containers.len(), 2);
    let raw: serde_json::Value = serde_json::from_str(&d.raw).unwrap();
    assert_eq!(raw["Name"], name.as_str());
    assert!(d.summary.mountpoint.ends_with("/_data") || !d.summary.mountpoint.is_empty());

    // Eliminar en uso: Docker lo rechaza (409), incluso por el detenido.
    let e = engine.remove_volume(&name, true).await.unwrap_err();
    assert_eq!(e.http_status(), Some(409), "{e:?}");
    assert!(e.to_string().contains("in use"), "{e}");
    docker
        .remove_container(
            &run,
            Some(bollard::query_parameters::RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await
        .unwrap();
    let e = engine.remove_volume(&name, true).await.unwrap_err();
    assert_eq!(e.http_status(), Some(409), "solo con el detenido: {e:?}");
    docker
        .remove_container(
            &stopped,
            None::<bollard::query_parameters::RemoveContainerOptions>,
        )
        .await
        .unwrap();
    // Sin contenedores: se elimina.
    assert_eq!(
        engine.remove_volume(&name, false).await,
        Err(EngineError::NotConfirmed)
    );
    engine.remove_volume(&name, true).await.unwrap();
    assert!(docker.inspect_volume(&name).await.is_err());
    cleanup.volumes.clear();
    let removes = records(&log, "volume.remove");
    assert_eq!(removes.len(), 3);
    assert_eq!(removes[0].status, OpStatus::Succeeded);
    assert!(removes[1..].iter().all(|r| r.status == OpStatus::Failed));
    assert_eq!(recorded(&log), proxy.requests());
}

#[tokio::test(flavor = "multi_thread")]
async fn anonymous_volumes_and_prune_with_and_without_all() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (engine, log, proxy, docker) = engine_via_proxy(&dir).await;
    let mark = std::process::id().to_string();
    let label = format!("dcc.prune={mark}");
    let (named, used) = (tag("vp-named"), tag("vp-used"));
    let user = tag("vp-user");
    let mut cleanup = Cleanup {
        docker: Some(docker.clone()),
        containers: vec![user.clone()],
        volumes: vec![named.clone(), used.clone()],
        images: vec![],
    };
    let labels = vec![
        KeyValue::new("dcc.test", "true"),
        KeyValue::new("dcc.prune", mark.clone()),
    ];
    // Anónimo: sin nombre.
    let anon = engine
        .create_volume(&VolumeSpec {
            labels: labels.clone(),
            ..Default::default()
        })
        .await
        .unwrap();
    cleanup.volumes.push(anon.name.clone());
    assert_eq!(anon.name.len(), 64);
    assert!(anon.anonymous);
    for n in [&named, &used] {
        engine
            .create_volume(&VolumeSpec {
                name: n.clone(),
                labels: labels.clone(),
                ..Default::default()
            })
            .await
            .unwrap();
    }
    container_with(
        &docker,
        &user,
        fixture::IMAGE,
        vec![volume_mount(&used, "/x", false)],
        false,
    )
    .await;

    // Sin --all: solo el anónimo sin uso.
    let r = engine
        .prune_volumes(false, Some(&label), true)
        .await
        .unwrap();
    assert_eq!(r.deleted, std::slice::from_ref(&anon.name));
    // Con --all: también el que tiene nombre; nunca el que usa un contenedor.
    let r = engine
        .prune_volumes(true, Some(&label), true)
        .await
        .unwrap();
    assert_eq!(r.deleted, std::slice::from_ref(&named));
    assert!(docker.inspect_volume(&used).await.is_ok());
    let p = records(&log, "volume.prune");
    assert_eq!(p.len(), 2);
    assert!(p[0]
        .equivalent_command
        .as_deref()
        .unwrap()
        .ends_with(&format!(
            "volume prune --force --all --filter label={label}"
        )));
    assert!(p[1]
        .equivalent_command
        .as_deref()
        .unwrap()
        .ends_with(&format!("volume prune --force --filter label={label}")));
    assert_eq!(
        engine.prune_volumes(true, None, false).await,
        Err(EngineError::NotConfirmed)
    );
    assert_eq!(recorded(&log), proxy.requests());
}

#[tokio::test(flavor = "multi_thread")]
async fn binds_report_host_state_users_and_risks() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (engine, log, proxy, docker) = engine_via_proxy(&dir).await;
    let host = tempfile::tempdir().unwrap();
    let src = host.path().join("web");
    std::fs::create_dir(&src).unwrap();
    let src_s = src.to_string_lossy().into_owned();
    let file = host.path().join("app.conf");
    std::fs::write(&file, "x=1\n").unwrap();
    let file_s = file.to_string_lossy().into_owned();
    let (a, b) = (tag("bind-a"), tag("bind-b"));
    let _c = Cleanup {
        docker: Some(docker.clone()),
        containers: vec![a.clone(), b.clone()],
        volumes: vec![],
        images: vec![],
    };
    let sock = real_socket().await;
    container_with(
        &docker,
        &a,
        fixture::IMAGE,
        vec![
            bind_mount(&src_s, "/srv", false),
            bind_mount(&file_s, "/etc/app.conf", true),
        ],
        true,
    )
    .await;
    container_with(
        &docker,
        &b,
        fixture::IMAGE,
        vec![
            bind_mount(&src_s, "/copia", true),
            bind_mount(&sock, "/var/run/docker.sock", false),
        ],
        false,
    )
    .await;

    let binds = engine.list_binds(None).await.unwrap();
    let web = binds
        .iter()
        .find(|x| x.source == src_s)
        .expect("carpeta vinculada");
    assert_eq!(web.host.kind, PathKind::Directory);
    assert!(web.host.readable);
    assert!(web.writable, "a la monta con escritura");
    assert_eq!(web.risk, None);
    let users: Vec<_> = web
        .containers
        .iter()
        .map(|u| (u.name.as_str(), u.destination.as_str(), u.rw))
        .collect();
    let mut expected = vec![(a.as_str(), "/srv", true), (b.as_str(), "/copia", false)];
    expected.sort();
    assert_eq!(users, expected);
    let conf = binds.iter().find(|x| x.source == file_s).unwrap();
    assert_eq!(conf.host.kind, PathKind::File);
    assert!(!conf.writable);
    let s = binds
        .iter()
        .find(|x| x.source == sock)
        .expect("socket vinculado");
    assert_eq!(s.risk, Some(BindRisk::DockerSocket));
    assert_eq!(s.host.kind, PathKind::Socket);

    // Si la carpeta desaparece del equipo, se indica.
    std::fs::remove_dir(&src).unwrap();
    let binds = engine.list_binds(None).await.unwrap();
    assert_eq!(
        binds.iter().find(|x| x.source == src_s).unwrap().host.kind,
        PathKind::Missing
    );
    // Solo la lista de contenedores habla con Docker. En un servidor remoto
    // (0.10.0, D-080) las carpetas se comprueban además en el servidor con
    // `stat`/`find` por SSH (`bind.inspect`), que no son peticiones a Docker.
    let remote = ssh_target().is_some();
    let over_ssh = |r: &dcc_activity::OperationRecord| {
        r.kind == "bind.inspect"
            && matches!(&r.transport, dcc_activity::Transport::Cli { program, .. } if program == "ssh")
    };
    assert!(log.snapshot().iter().all(|r| r.kind == "container.list"
        || r.kind.starts_with("system.")
        || (remote && over_ssh(r))));
    assert_eq!(log.snapshot().iter().any(over_ssh), remote);
    assert_eq!(recorded(&log), proxy.requests());
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_input_never_reaches_docker() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (engine, log, proxy, _docker) = engine_via_proxy(&dir).await;
    let before = proxy.requests().len();
    for bad in ["a", "a b", "-x"] {
        let r = engine
            .create_volume(&VolumeSpec {
                name: bad.into(),
                ..Default::default()
            })
            .await;
        assert!(
            matches!(r, Err(EngineError::InvalidSpec { .. })),
            "{bad}: {r:?}"
        );
    }
    assert!(matches!(
        engine.volume_details("../etc").await,
        Err(EngineError::InvalidInput { .. })
    ));
    assert!(matches!(
        engine.remove_volume("a/b", true).await,
        Err(EngineError::InvalidInput { .. })
    ));
    assert_eq!(proxy.requests().len(), before);
    assert!(log
        .snapshot()
        .iter()
        .all(|r| !r.kind.starts_with("volume.")));
    // Volumen que no existe: 404 real.
    let e = engine
        .volume_details("dcc-test-no-existe-12345")
        .await
        .unwrap_err();
    assert_eq!(e.http_status(), Some(404), "{e:?}");
    assert_eq!(recorded(&log), proxy.requests());
}
