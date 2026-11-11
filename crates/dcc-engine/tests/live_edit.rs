//! Actualización 0.9.2 contra Docker real (`DCC_DOCKER_TESTS=1`): cambios
//! directos (rename, update), recreación segura con copia y vuelta atrás,
//! restauración de la copia e imagen a partir de un contenedor.
//!
//! Todo lo creado lleva `dcc-test-` en el nombre y se elimina al terminar.

mod common;

use std::collections::HashMap;

use bollard::models::{
    ContainerCreateBody, EndpointIpamConfig, EndpointSettings, HostConfig, Ipam, IpamConfig,
    NetworkCreateRequest, NetworkingConfig, PortBinding, RestartPolicy, RestartPolicyNameEnum,
};
use bollard::query_parameters::{
    CreateContainerOptions, InspectContainerOptions, ListContainersOptions, RemoveContainerOptions,
    RemoveImageOptions,
};
use common::*;
use dcc_activity::{ActivityLog, OpStatus, Transport};
use dcc_engine::edit::{ChangeKind, EditRequest, DESCRIPTION_LABEL};
use dcc_engine::edit_ops::{ApplyOptions, CommitRequest};
use dcc_engine::spec::{
    KeyValue, MountKind, MountSpec, PortSpec, Protocol, RestartName, RestartSpec, MIB,
};
use dcc_engine::{ConnectionState, Engine, EngineError};
use futures_util::StreamExt;

async fn engine_via_proxy(
    dir: &tempfile::TempDir,
) -> (Engine, ActivityLog, Proxy, bollard::Docker) {
    let sock = real_socket().await;
    let docker =
        bollard::Docker::connect_with_unix(&sock, 60, bollard::API_DEFAULT_VERSION).unwrap();
    fixture::ensure_image(&docker).await;
    fixture::ensure_shell_image(&docker).await;
    let proxy = Proxy::start(&dir.path().join("p.sock"), sock).await;
    let log = ActivityLog::new(2000);
    // Local o, con DCC_SSH_TEST, por SSH con el proxy detrás (0.10.0).
    let m = common::manager(log.clone(), proxy.path.display().to_string());
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    (m.engine().await.unwrap(), log, proxy, docker)
}

/// Elimina todo lo que empiece por el prefijo de la prueba (también las copias).
struct Cleanup {
    docker: bollard::Docker,
    prefix: String,
    volumes: Vec<String>,
    networks: Vec<String>,
    images: Vec<String>,
}

impl Cleanup {
    fn new(docker: &bollard::Docker, prefix: &str) -> Self {
        Self {
            docker: docker.clone(),
            prefix: prefix.to_owned(),
            volumes: vec![],
            networks: vec![],
            images: vec![],
        }
    }
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        let d = self.docker.clone();
        let prefix = self.prefix.clone();
        let (vs, ns, is) = (
            std::mem::take(&mut self.volumes),
            std::mem::take(&mut self.networks),
            std::mem::take(&mut self.images),
        );
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                let all = d
                    .list_containers(Some(ListContainersOptions {
                        all: true,
                        ..Default::default()
                    }))
                    .await
                    .unwrap_or_default();
                for c in all {
                    let mine = c
                        .names
                        .unwrap_or_default()
                        .iter()
                        .any(|n| n.trim_start_matches('/').starts_with(&prefix));
                    if mine {
                        let _ = d
                            .remove_container(
                                &c.id.unwrap_or_default(),
                                Some(RemoveContainerOptions {
                                    force: true,
                                    v: true,
                                    ..Default::default()
                                }),
                            )
                            .await;
                    }
                }
                for v in vs {
                    let _ = d
                        .remove_volume(&v, None::<bollard::query_parameters::RemoveVolumeOptions>)
                        .await;
                }
                for n in ns {
                    let _ = d.remove_network(&n).await;
                }
                for i in is {
                    let _ = d
                        .remove_image(
                            &i,
                            Some(RemoveImageOptions {
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

fn prefix(tag: &str) -> String {
    format!("dcc-test-{tag}-{}", std::process::id())
}

fn labels() -> HashMap<String, String> {
    HashMap::from([("dcc.test".to_string(), "true".to_string())])
}

async fn create(docker: &bollard::Docker, name: &str, body: ContainerCreateBody) -> String {
    docker
        .create_container(
            Some(CreateContainerOptions {
                name: Some(name.to_owned()),
                ..Default::default()
            }),
            body,
        )
        .await
        .expect("crear contenedor de prueba")
        .id
}

async fn inspect(docker: &bollard::Docker, id: &str) -> bollard::models::ContainerInspectResponse {
    docker
        .inspect_container(id, None::<InspectContainerOptions>)
        .await
        .unwrap()
}

fn status(c: &bollard::models::ContainerInspectResponse) -> String {
    serde_json::to_value(c.state.as_ref().unwrap().status.as_ref().unwrap())
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned()
}

/// Ejecuta una orden de shell dentro del contenedor: salida y código.
async fn sh(docker: &bollard::Docker, id: &str, script: &str) -> (String, i64) {
    let ex = docker
        .create_exec(
            id,
            bollard::models::ExecConfig {
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                cmd: Some(vec!["/bin/sh".into(), "-c".into(), script.into()]),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let mut text = String::new();
    if let bollard::exec::StartExecResults::Attached { mut output, .. } =
        docker.start_exec(&ex.id, None).await.unwrap()
    {
        while let Some(Ok(f)) = output.next().await {
            text.push_str(&f.to_string());
        }
    }
    let code = docker
        .inspect_exec(&ex.id)
        .await
        .unwrap()
        .exit_code
        .unwrap_or(-1);
    (text, code)
}

async fn names(docker: &bollard::Docker, prefix: &str) -> Vec<String> {
    let mut v: Vec<String> = docker
        .list_containers(Some(ListContainersOptions {
            all: true,
            ..Default::default()
        }))
        .await
        .unwrap()
        .into_iter()
        .flat_map(|c| c.names.unwrap_or_default())
        .map(|n| n.trim_start_matches('/').to_owned())
        .filter(|n| n.starts_with(prefix))
        .collect();
    v.sort();
    v
}

fn confirm(plan: &dcc_engine::edit::EditPlan) -> ApplyOptions {
    ApplyOptions {
        confirm_recreate: true,
        accepted_risks: plan.risks.iter().map(|r| r.code.clone()).collect(),
        keep_mac: false,
    }
}

fn kinds(log: &ActivityLog) -> Vec<String> {
    log.snapshot().into_iter().rev().map(|r| r.kind).collect()
}

// ------------------------------------------------------------ cambios directos

#[tokio::test(flavor = "multi_thread")]
async fn direct_changes_rename_and_update_keep_the_same_container() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (e, log, _proxy, docker) = engine_via_proxy(&dir).await;
    let p = prefix("direct");
    let _clean = Cleanup::new(&docker, &p);
    let id = create(
        &docker,
        &p,
        ContainerCreateBody {
            image: Some(fixture::SHELL_IMAGE.into()),
            labels: Some(labels()),
            host_config: Some(HostConfig {
                memory: Some(64 * MIB as i64),
                nano_cpus: Some(500_000_000),
                ..Default::default()
            }),
            ..Default::default()
        },
    )
    .await;
    docker.start_container(&id, None).await.unwrap();
    let started = inspect(&docker, &id).await.state.unwrap().started_at;

    let cfg = e.edit_config(&id).await.unwrap();
    assert_eq!(cfg.status, "running");
    assert_eq!(cfg.current.memory_bytes, Some(64 * MIB));
    assert_eq!(cfg.current.cpus, Some(0.5));
    assert!(cfg.host_cpus > 0 && cfg.host_memory > 0);

    let new_name = format!("{p}-renamed");
    let req = EditRequest {
        name: new_name.clone(),
        // Mayor que memoria+swap actuales (128 MiB): hay que subir el swap a la vez.
        memory_bytes: Some(200 * MIB),
        memory_reservation_bytes: Some(32 * MIB),
        cpus: Some(1.0),
        restart: RestartSpec {
            name: RestartName::OnFailure,
            max_retries: 3,
        },
        ..cfg.current.clone()
    };
    let plan = e.edit_plan(&id, &req).await.unwrap();
    assert!(!plan.needs_recreate, "{plan:?}");
    assert!(plan.changes.iter().all(|c| c.kind == ChangeKind::Direct));
    let fields: Vec<&str> = plan.changes.iter().map(|c| c.field.as_str()).collect();
    assert_eq!(
        fields,
        ["name", "restart", "cpus", "memory", "memoryReservation"]
    );
    assert!(plan.kept.contains(&"env".to_string()));
    assert!(plan.equivalent.iter().any(|c| c.contains("update")
        && c.contains("--memory 200m")
        && c.contains("--restart on-failure:3")));
    assert!(plan.equivalent.iter().any(|c| c.contains("rename")));

    // Sin confirmación de recreación: los cambios directos no la necesitan.
    let out = e
        .edit_apply(&id, &req, &ApplyOptions::default())
        .await
        .unwrap();
    assert_eq!(out.error, None, "{out:?}");
    assert!(!out.recreated && out.backup_name.is_none());
    assert_eq!(out.container_id, id);
    assert_eq!(out.name, new_name);

    let after = inspect(&docker, &id).await;
    let h = after.host_config.clone().unwrap();
    assert_eq!(after.name.as_deref(), Some(format!("/{new_name}").as_str()));
    assert_eq!(h.memory, Some(200 * MIB as i64));
    assert_eq!(h.memory_reservation, Some(32 * MIB as i64));
    assert_eq!(h.nano_cpus, Some(1_000_000_000));
    let rp = h.restart_policy.unwrap();
    assert_eq!(rp.name, Some(RestartPolicyNameEnum::ON_FAILURE));
    assert_eq!(rp.maximum_retry_count, Some(3));
    // No se ha reiniciado ni recreado.
    assert_eq!(after.state.unwrap().started_at, started);
    assert_eq!(names(&docker, &p).await, vec![new_name.clone()]);

    let k = kinds(&log);
    assert!(k.contains(&"container.update".to_string()));
    assert!(k.contains(&"container.rename".to_string()));
    assert!(!k.contains(&"container.recreate".to_string()));
    let upd = log
        .snapshot()
        .into_iter()
        .find(|r| r.kind == "container.update")
        .unwrap();
    assert_eq!(upd.status, OpStatus::Succeeded);
    assert!(upd.equivalent_command.unwrap().contains("docker"));
    assert!(matches!(upd.transport, Transport::Api { .. }));

    // Nada que aplicar: se rechaza sin tocar nada.
    let cfg2 = e.edit_config(&id).await.unwrap();
    match e
        .edit_apply(&id, &cfg2.current, &ApplyOptions::default())
        .await
    {
        Err(EngineError::InvalidInput { .. }) => {}
        other => panic!("{other:?}"),
    }

    // Quitar un límite no se puede hacer con update: exige recreación.
    let plan = e
        .edit_plan(
            &id,
            &EditRequest {
                cpus: None,
                ..cfg2.current.clone()
            },
        )
        .await
        .unwrap();
    assert!(plan.needs_recreate);
    assert_eq!(plan.changes[0].reason.as_deref(), Some("limitRemoval"));

    // Un contenedor detenido también admite los cambios directos.
    docker.stop_container(&id, None).await.unwrap();
    let back = EditRequest {
        name: p.clone(),
        ..cfg2.current.clone()
    };
    let out = e
        .edit_apply(&id, &back, &ApplyOptions::default())
        .await
        .unwrap();
    assert_eq!(out.error, None);
    assert_eq!(names(&docker, &p).await, vec![p.clone()]);
}

// ------------------------------------------------------------ recreación

struct Rich {
    id: String,
    name: String,
    volume: String,
    net_a: String,
    net_b: String,
    bind: tempfile::TempDir,
    host_port: u16,
}

/// Contenedor en ejecución con mucha configuración que la aplicación no edita.
async fn rich(docker: &bollard::Docker, p: &str, clean: &mut Cleanup, subnet: u8) -> Rich {
    let volume = format!("{p}-vol");
    let (net_a, net_b) = (format!("{p}-neta"), format!("{p}-netb"));
    clean.volumes.push(volume.clone());
    clean.networks.extend([net_a.clone(), net_b.clone()]);
    for (n, third) in [(&net_a, subnet), (&net_b, subnet + 1)] {
        docker
            .create_network(NetworkCreateRequest {
                name: n.clone(),
                labels: Some(labels()),
                ipam: Some(Ipam {
                    config: Some(vec![IpamConfig {
                        subnet: Some(format!("10.213.{third}.0/24")),
                        ..Default::default()
                    }]),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .await
            .unwrap();
    }
    let bind = tempfile::tempdir().unwrap();
    std::fs::write(bind.path().join("host.txt"), "desde el equipo\n").unwrap();
    // Puerto libre del equipo.
    let host_port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mut lbl = labels();
    lbl.insert("app.tier".into(), "backend".into());
    let body = ContainerCreateBody {
        image: Some(fixture::SHELL_IMAGE.into()),
        labels: Some(lbl),
        hostname: Some("dcc-rich".into()),
        user: Some("0:0".into()),
        working_dir: Some("/tmp".into()),
        env: Some(vec![
            "MODE=prod".into(),
            "API_TOKEN=s3cr3t-original".into(),
            "REMOVE_ME=1".into(),
        ]),
        cmd: Some(vec!["/bin/sleep".into(), "infinity".into()]),
        exposed_ports: Some(vec!["8080/tcp".into()]),
        // Volumen anónimo declarado con `-v /anon`.
        volumes: Some(vec!["/anon".into()]),
        stop_signal: Some("SIGKILL".into()),
        host_config: Some(HostConfig {
            binds: Some(vec![
                format!("{volume}:/data"),
                format!("{}:/host:ro", bind.path().display()),
            ]),
            port_bindings: Some(HashMap::from([(
                "8080/tcp".to_string(),
                Some(vec![PortBinding {
                    host_ip: Some("127.0.0.1".into()),
                    host_port: Some(host_port.to_string()),
                }]),
            )])),
            network_mode: Some(net_a.clone()),
            restart_policy: Some(RestartPolicy {
                name: Some(RestartPolicyNameEnum::UNLESS_STOPPED),
                maximum_retry_count: None,
            }),
            memory: Some(96 * MIB as i64),
            nano_cpus: Some(750_000_000),
            dns: Some(vec!["9.9.9.9".into()]),
            extra_hosts: Some(vec!["db.internal:10.9.8.7".into()]),
            cap_add: Some(vec!["NET_ADMIN".into()]),
            cap_drop: Some(vec!["MKNOD".into()]),
            tmpfs: Some(HashMap::from([(
                "/scratch".to_string(),
                "size=1m".to_string(),
            )])),
            ulimits: Some(vec![bollard::models::ResourcesUlimits {
                name: Some("nofile".into()),
                soft: Some(2048),
                hard: Some(4096),
            }]),
            pids_limit: Some(200),
            shm_size: Some(32 * MIB as i64),
            init: Some(true),
            log_config: Some(bollard::models::HostConfigLogConfig {
                typ: Some("json-file".into()),
                config: Some(HashMap::from([("max-size".to_string(), "1m".to_string())])),
            }),
            ..Default::default()
        }),
        networking_config: Some(NetworkingConfig {
            endpoints_config: Some(HashMap::from([(
                net_a.clone(),
                EndpointSettings {
                    ipam_config: Some(EndpointIpamConfig {
                        ipv4_address: Some(format!("10.213.{subnet}.50")),
                        ..Default::default()
                    }),
                    aliases: Some(vec!["rich-alias".into()]),
                    ..Default::default()
                },
            )])),
        }),
        ..Default::default()
    };
    let id = create(docker, p, body).await;
    docker
        .connect_network(
            &net_b,
            bollard::models::NetworkConnectRequest {
                container: id.clone(),
                endpoint_config: Some(EndpointSettings {
                    aliases: Some(vec!["second".into()]),
                    ..Default::default()
                }),
            },
        )
        .await
        .unwrap();
    docker.start_container(&id, None).await.unwrap();
    Rich {
        id,
        name: p.to_owned(),
        volume,
        net_a,
        net_b,
        bind,
        host_port,
    }
}

/// `Config` y `HostConfig` como JSON, sin los campos que se esperan distintos.
fn comparable(
    c: &bollard::models::ContainerInspectResponse,
    drop_cfg: &[&str],
    drop_host: &[&str],
) -> (serde_json::Value, serde_json::Value) {
    let mut cfg = serde_json::to_value(c.config.as_ref().unwrap()).unwrap();
    let mut host = serde_json::to_value(c.host_config.as_ref().unwrap()).unwrap();
    for k in drop_cfg {
        cfg.as_object_mut().unwrap().remove(*k);
    }
    for k in drop_host {
        host.as_object_mut().unwrap().remove(*k);
    }
    (cfg, host)
}

/// `GET /containers/{id}/json` sin pasar por los tipos de bollard: así la
/// comparación incluye también los campos que la biblioteca no conoce.
async fn raw_inspect(id: &str) -> serde_json::Value {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut s = tokio::net::UnixStream::connect(real_socket().await)
        .await
        .unwrap();
    s.write_all(format!("GET /containers/{id}/json HTTP/1.0\r\nHost: docker\r\n\r\n").as_bytes())
        .await
        .unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).await.unwrap();
    let text = String::from_utf8_lossy(&buf);
    let body = text.split_once("\r\n\r\n").expect("respuesta HTTP").1;
    serde_json::from_str(body).expect("JSON de inspect")
}

fn without(v: &serde_json::Value, keys: &[&str]) -> serde_json::Value {
    let mut v = v.clone();
    for k in keys {
        v.as_object_mut().unwrap().remove(*k);
    }
    v
}

fn mounts_of(c: &bollard::models::ContainerInspectResponse) -> Vec<(String, String, String, bool)> {
    let mut v: Vec<_> = c
        .mounts
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|m| {
            (
                m.destination.unwrap_or_default(),
                m.typ.unwrap_or_default(),
                m.name
                    .filter(|n| !n.is_empty())
                    .or(m.source)
                    .unwrap_or_default(),
                m.rw.unwrap_or(true),
            )
        })
        .collect();
    v.sort();
    v
}

#[tokio::test(flavor = "multi_thread")]
async fn recreation_changes_only_what_was_edited_and_keeps_data_and_a_backup() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (e, log, proxy, docker) = engine_via_proxy(&dir).await;
    let p = prefix("recreate");
    let mut clean = Cleanup::new(&docker, &p);
    let r = rich(&docker, &p, &mut clean, 10).await;

    // Datos en cada sitio: volumen con nombre, volumen anónimo, carpeta y capa.
    let (_, code) = sh(
        &docker,
        &r.id,
        "echo vol > /data/v.txt && echo anon > /anon/a.txt && echo layer > /layer.txt",
    )
    .await;
    assert_eq!(code, 0);
    let before = inspect(&docker, &r.id).await;
    let raw_before = raw_inspect(&r.id).await;
    let anon = mounts_of(&before)
        .into_iter()
        .find(|m| m.0 == "/anon")
        .unwrap()
        .2;

    let cfg = e.edit_config(&r.id).await.unwrap();
    assert_eq!(cfg.anonymous_volumes, vec![anon.clone()]);
    assert_eq!(
        cfg.other_mounts,
        Vec::<String>::new(),
        "tmpfs va en HostConfig.Tmpfs"
    );
    assert_eq!(cfg.networks.len(), 2);
    assert_eq!(
        cfg.networks
            .iter()
            .find(|n| n.name == r.net_a)
            .unwrap()
            .fixed_ipv4,
        "10.213.10.50"
    );
    let own: Vec<&str> = cfg.current.env.iter().map(|k| k.key.as_str()).collect();
    assert_eq!(own, ["MODE", "API_TOKEN", "REMOVE_ME"]);
    assert_eq!(cfg.current.mounts.len(), 3);
    assert!(cfg.size_rw.is_some());

    let new_port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let req = EditRequest {
        description: "Servicio de pruebas: «recreación»".into(),
        env: vec![
            KeyValue::new("MODE", "staging"),
            KeyValue::new("API_TOKEN", "s3cr3t-nuevo"),
            KeyValue::new("ADDED", "con espacios y $(id)"),
        ],
        ports: vec![
            PortSpec {
                host_ip: "127.0.0.1".into(),
                host_port: Some(new_port),
                container_port: 8080,
                protocol: Protocol::Tcp,
            },
            PortSpec {
                host_ip: "127.0.0.1".into(),
                host_port: None,
                container_port: 9090,
                protocol: Protocol::Udp,
            },
        ],
        ..cfg.current.clone()
    };
    let plan = e.edit_plan(&r.id, &req).await.unwrap();
    assert!(plan.needs_recreate && plan.will_stop);
    assert!(plan.blockers.is_empty(), "{:?}", plan.blockers);
    assert!(!plan.has_errors(), "{:?}", plan.issues);
    assert_eq!(plan.image_ref.as_deref(), Some(fixture::SHELL_IMAGE));
    let fields: Vec<&str> = plan.changes.iter().map(|c| c.field.as_str()).collect();
    assert_eq!(fields, ["description", "env", "ports"]);
    assert!(plan.kept.contains(&"mounts".to_string()));
    assert!(plan.risks.iter().any(|x| x.code == "writableLayer"));
    // Con la API 1.44 o posterior se ofrece conservar la MAC; antes, se avisa.
    assert!(plan
        .notices
        .iter()
        .any(|x| x.code == "macChoice" || x.code == "macLegacy"));
    // Los secretos no aparecen en el plan.
    let plan_json = serde_json::to_string(&plan).unwrap();
    assert!(!plan_json.contains("s3cr3t"), "{plan_json}");
    assert!(plan_json.contains("staging"));

    // Sin confirmación no se toca nada: ni una petición que modifique.
    let posts = |px: &Proxy| {
        px.requests()
            .into_iter()
            .filter(|(m, _)| m != "GET" && m != "HEAD")
            .count()
    };
    let n = posts(&proxy);
    for opts in [
        ApplyOptions::default(),
        ApplyOptions {
            confirm_recreate: true,
            accepted_risks: vec![],
            keep_mac: false,
        },
    ] {
        match e.edit_apply(&r.id, &req, &opts).await {
            Err(EngineError::NotConfirmed) => {}
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(posts(&proxy), n);
    assert_eq!(status(&inspect(&docker, &r.id).await), "running");

    // Recreación confirmada.
    let out = e.edit_apply(&r.id, &req, &confirm(&plan)).await.unwrap();
    assert_eq!(out.error, None, "{out:?}");
    assert!(out.recreated && out.started && !out.rolled_back);
    assert_ne!(out.container_id, r.id);
    assert_eq!(out.name, r.name);
    assert_eq!(out.backup_id.as_deref(), Some(r.id.as_str()));
    let backup = out.backup_name.clone().unwrap();
    assert_eq!(
        dcc_engine::edit::backup_origin(&backup),
        Some(r.name.as_str())
    );
    assert_eq!(
        names(&docker, &p).await,
        vec![r.name.clone(), backup.clone()]
    );

    let new = inspect(&docker, &out.container_id).await;
    let old = inspect(&docker, &r.id).await;
    assert_eq!(status(&new), "running");
    assert_eq!(status(&old), "exited");
    assert_eq!(old.name.as_deref(), Some(format!("/{backup}").as_str()));

    // Lo editado.
    let env = new.config.clone().unwrap().env.unwrap();
    assert!(env.contains(&"MODE=staging".to_string()));
    assert!(env.contains(&"API_TOKEN=s3cr3t-nuevo".to_string()));
    assert!(env.contains(&"ADDED=con espacios y $(id)".to_string()));
    assert!(!env.iter().any(|x| x.starts_with("REMOVE_ME")));
    assert_eq!(
        new.config
            .clone()
            .unwrap()
            .labels
            .unwrap()
            .get(DESCRIPTION_LABEL)
            .map(String::as_str),
        Some("Servicio de pruebas: «recreación»")
    );
    let pb = new.host_config.clone().unwrap().port_bindings.unwrap();
    assert_eq!(
        pb["8080/tcp"].as_ref().unwrap()[0].host_port.as_deref(),
        Some(new_port.to_string().as_str())
    );
    assert!(pb.contains_key("9090/udp"));

    // Todo lo demás, idéntico: Config y HostConfig completos.
    let (c_old, h_old) = comparable(
        &before,
        &["Env", "Labels", "ExposedPorts"],
        &["PortBindings", "Binds"],
    );
    let (c_new, h_new) = comparable(
        &new,
        &["Env", "Labels", "ExposedPorts"],
        &["PortBindings", "Binds"],
    );
    assert_eq!(c_new, c_old);
    assert_eq!(h_new, h_old);
    // Lo mismo con el JSON tal como lo da Docker (campos que bollard no modela).
    let raw_new = raw_inspect(&out.container_id).await;
    assert_eq!(
        without(&raw_new["HostConfig"], &["PortBindings", "Binds"]),
        without(&raw_before["HostConfig"], &["PortBindings", "Binds"])
    );
    assert_eq!(
        without(&raw_new["Config"], &["Env", "Labels", "ExposedPorts"]),
        without(&raw_before["Config"], &["Env", "Labels", "ExposedPorts"])
    );
    let mut l_old = before.config.clone().unwrap().labels.unwrap();
    let mut l_new = new.config.clone().unwrap().labels.unwrap();
    l_new.remove(DESCRIPTION_LABEL);
    l_old.remove(DESCRIPTION_LABEL);
    assert_eq!(l_new, l_old);
    // Los mismos montajes, con los mismos volúmenes (también el anónimo).
    assert_eq!(mounts_of(&new), mounts_of(&before));
    let binds = new.host_config.clone().unwrap().binds.unwrap();
    assert!(binds.contains(&format!("{}:/data", r.volume)));
    assert!(binds.contains(&format!("{}:/host:ro", r.bind.path().display())));
    assert!(binds.contains(&format!("{anon}:/anon")));

    // Redes: las dos, con la IP fija y los alias.
    let nets = new.network_settings.clone().unwrap().networks.unwrap();
    assert_eq!(nets.len(), 2);
    assert_eq!(nets[&r.net_a].ip_address.as_deref(), Some("10.213.10.50"));
    assert!(
        nets[&r.net_a]
            .aliases
            .clone()
            .unwrap_or_default()
            .contains(&"rich-alias".to_string())
            || nets[&r.net_a]
                .dns_names
                .clone()
                .unwrap_or_default()
                .contains(&"rich-alias".to_string())
    );
    assert!(
        nets[&r.net_b]
            .aliases
            .clone()
            .unwrap_or_default()
            .contains(&"second".to_string())
            || nets[&r.net_b]
                .dns_names
                .clone()
                .unwrap_or_default()
                .contains(&"second".to_string())
    );
    let old_short: String = r.id.chars().take(12).collect();
    for n in nets.values() {
        assert!(!n.aliases.clone().unwrap_or_default().contains(&old_short));
    }

    // Datos persistentes intactos; la capa de escritura se queda en la copia.
    let (o, code) = sh(
        &docker,
        &out.container_id,
        "cat /data/v.txt /anon/a.txt /host/host.txt; echo $MODE; cat /layer.txt",
    )
    .await;
    assert!(o.contains("vol\nanon\ndesde el equipo\nstaging\n"), "{o}");
    assert_ne!(code, 0, "la capa de escritura no pasa al contenedor nuevo");
    assert!(!o.contains("layer\n"));
    assert!(docker.inspect_volume(&r.volume).await.is_ok());
    assert!(docker.inspect_volume(&anon).await.is_ok());

    // Actividad: registro de conjunto y pasos reales, sin secretos.
    let recs = log.snapshot();
    let umbrella = recs
        .iter()
        .find(|x| x.kind == "container.recreate")
        .unwrap();
    assert_eq!(umbrella.status, OpStatus::Succeeded);
    assert!(matches!(umbrella.transport, Transport::App { .. }));
    assert!(umbrella.summary.clone().unwrap().contains(&backup));
    assert!(umbrella.duration_ms.is_some());
    for k in [
        "container.stop",
        "container.rename",
        "container.create",
        "network.connect",
        "container.start",
    ] {
        assert!(recs.iter().any(|x| x.kind == k), "falta {k}");
    }
    let all = serde_json::to_string(&recs).unwrap();
    assert!(
        !all.contains("s3cr3t"),
        "un secreto llegó al registro de actividad"
    );
    assert!(all.contains("API_TOKEN=********"));
    // Ninguna petición elimina volúmenes, imágenes ni el original.
    for (m, path) in proxy.requests() {
        assert!(m != "DELETE", "petición inesperada: {m} {path}");
    }

    // La pantalla de edición del nuevo ofrece la copia.
    let cfg_new = e.edit_config(&out.container_id).await.unwrap();
    assert_eq!(cfg_new.backups.len(), 1);
    assert_eq!(cfg_new.backups[0].id, r.id);
    assert_eq!(
        cfg_new.current.description,
        "Servicio de pruebas: «recreación»"
    );

    // Restaurar la copia: vuelve el original (con su capa) y el nuevo se aparta.
    match e.restore_backup(&r.id, true, false).await {
        Err(EngineError::NotConfirmed) => {}
        other => panic!("{other:?}"),
    }
    let res = e.restore_backup(&r.id, true, true).await.unwrap();
    assert_eq!(res.error, None, "{res:?}");
    assert_eq!(res.container_id, r.id);
    assert_eq!(res.name, r.name);
    let restored = inspect(&docker, &r.id).await;
    assert_eq!(status(&restored), "running");
    assert_eq!(
        restored.name.as_deref(),
        Some(format!("/{}", r.name).as_str())
    );
    let aside = res.backup_name.clone().unwrap();
    assert!(aside.contains("-dcc-replaced-"));
    assert_eq!(status(&inspect(&docker, &out.container_id).await), "exited");
    let (o, code) = sh(&docker, &r.id, "cat /layer.txt /data/v.txt; echo $MODE").await;
    assert_eq!((o.as_str(), code), ("layer\nvol\nprod\n", 0));
    assert_eq!(names(&docker, &p).await, vec![r.name.clone(), aside]);
    for (m, path) in proxy.requests() {
        assert!(m != "DELETE", "petición inesperada: {m} {path}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn recreation_rolls_back_when_the_new_container_cannot_start() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (e, log, _proxy, docker) = engine_via_proxy(&dir).await;
    let p = prefix("rollback");
    let mut clean = Cleanup::new(&docker, &p);
    let r = rich(&docker, &p, &mut clean, 20).await;
    sh(
        &docker,
        &r.id,
        "echo vol > /data/v.txt; echo layer > /layer.txt",
    )
    .await;
    let before = inspect(&docker, &r.id).await;

    // Un programa del equipo ocupa el puerto: Docker no puede saberlo hasta iniciar.
    let busy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let busy_port = busy.local_addr().unwrap().port();
    let cfg = e.edit_config(&r.id).await.unwrap();
    let req = EditRequest {
        ports: vec![PortSpec {
            host_ip: "127.0.0.1".into(),
            host_port: Some(busy_port),
            container_port: 8080,
            protocol: Protocol::Tcp,
        }],
        env: vec![KeyValue::new("MODE", "nunca")],
        ..cfg.current.clone()
    };
    let plan = e.edit_plan(&r.id, &req).await.unwrap();
    assert!(plan.needs_recreate && plan.blockers.is_empty() && !plan.has_errors());

    let out = e.edit_apply(&r.id, &req, &confirm(&plan)).await.unwrap();
    assert!(out.error.is_some(), "{out:?}");
    assert!(
        out.rolled_back && !out.rollback_failed && !out.recreated,
        "{out:?}"
    );
    assert_eq!(out.container_id, r.id);
    assert_eq!(out.name, r.name);
    assert!(out.started);
    let codes: Vec<&str> = out.steps.iter().map(|s| s.code.as_str()).collect();
    assert!(codes.contains(&"rollbackRemove") && codes.contains(&"rollbackRename"));
    assert!(codes.contains(&"rollbackStart"));

    // Solo queda el original, con su nombre, en ejecución y con sus datos.
    assert_eq!(names(&docker, &p).await, vec![r.name.clone()]);
    let after = inspect(&docker, &r.id).await;
    assert_eq!(status(&after), "running");
    assert_eq!(
        comparable(&after, &[], &[]),
        comparable(&before, &[], &[]),
        "el original no cambia"
    );
    let (o, code) = sh(&docker, &r.id, "cat /data/v.txt /layer.txt; echo $MODE").await;
    assert_eq!((o.as_str(), code), ("vol\nlayer\nprod\n", 0));
    assert!(docker.inspect_volume(&r.volume).await.is_ok());
    // El puerto original vuelve a estar publicado.
    let pb = after.network_settings.unwrap().ports.unwrap();
    assert_eq!(
        pb["8080/tcp"].as_ref().unwrap()[0].host_port.as_deref(),
        Some(r.host_port.to_string().as_str())
    );

    let recs = log.snapshot();
    let umbrella = recs
        .iter()
        .find(|x| x.kind == "container.recreate")
        .unwrap();
    assert_eq!(umbrella.status, OpStatus::Failed);
    assert!(umbrella
        .error
        .clone()
        .unwrap()
        .contains("contenedor original"));
    drop(busy);
}

#[tokio::test(flavor = "multi_thread")]
async fn stopped_container_is_recreated_stopped_and_mount_changes_keep_volumes() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (e, _log, proxy, docker) = engine_via_proxy(&dir).await;
    let p = prefix("stopped");
    let mut clean = Cleanup::new(&docker, &p);
    let (vol_a, vol_b) = (format!("{p}-a"), format!("{p}-b"));
    clean.volumes.extend([vol_a.clone(), vol_b.clone()]);
    let id = create(
        &docker,
        &p,
        ContainerCreateBody {
            image: Some(fixture::SHELL_IMAGE.into()),
            labels: Some(labels()),
            host_config: Some(HostConfig {
                binds: Some(vec![format!("{vol_a}:/a")]),
                nano_cpus: Some(250_000_000),
                memory: Some(64 * MIB as i64),
                ..Default::default()
            }),
            ..Default::default()
        },
    )
    .await;
    docker.start_container(&id, None).await.unwrap();
    sh(&docker, &id, "echo a > /a/f.txt").await;
    docker
        .stop_container(
            &id,
            Some(bollard::query_parameters::StopContainerOptions {
                t: Some(0),
                signal: None,
            }),
        )
        .await
        .unwrap();

    let cfg = e.edit_config(&id).await.unwrap();
    assert_eq!(cfg.status, "exited");
    let bind_dir = tempfile::tempdir().unwrap();
    let req = EditRequest {
        cpus: None,
        memory_bytes: None,
        mounts: vec![
            // /a pasa a solo lectura; se añade otro volumen y una carpeta.
            MountSpec {
                kind: MountKind::Volume,
                source: vol_a.clone(),
                target: "/a".into(),
                read_only: true,
            },
            MountSpec {
                kind: MountKind::Volume,
                source: vol_b.clone(),
                target: "/b".into(),
                read_only: false,
            },
            MountSpec {
                kind: MountKind::Bind,
                source: bind_dir.path().display().to_string(),
                target: "/host".into(),
                read_only: false,
            },
        ],
        ..cfg.current.clone()
    };
    let plan = e.edit_plan(&id, &req).await.unwrap();
    assert!(plan.needs_recreate && !plan.will_stop);
    assert!(plan.blockers.is_empty() && !plan.has_errors(), "{plan:?}");
    let m = plan.changes.iter().find(|c| c.field == "mounts").unwrap();
    let ops: Vec<(&str, &str)> = m
        .items
        .iter()
        .map(|i| (i.op.as_str(), i.label.as_str()))
        .collect();
    assert_eq!(
        ops,
        [("modified", "/a"), ("added", "/b"), ("added", "/host")]
    );

    let out = e.edit_apply(&id, &req, &confirm(&plan)).await.unwrap();
    assert_eq!(out.error, None, "{out:?}");
    assert!(out.recreated && !out.started);
    let new = inspect(&docker, &out.container_id).await;
    assert_eq!(
        status(&new),
        "created",
        "un contenedor detenido no se inicia"
    );
    let h = new.host_config.clone().unwrap();
    assert!(h.nano_cpus.unwrap_or(0) == 0 && h.memory.unwrap_or(0) == 0);
    assert_eq!(h.memory_swap.unwrap_or(0), 0);
    // Al iniciarlo, los datos del volumen siguen ahí y /a es de solo lectura.
    docker
        .start_container(&out.container_id, None)
        .await
        .unwrap();
    let (o, _) = sh(
        &docker,
        &out.container_id,
        "cat /a/f.txt; (echo x > /a/w.txt) 2>/dev/null || echo RO; echo b > /b/f.txt && echo OK",
    )
    .await;
    assert_eq!(o, "a\nRO\nOK\n");
    for (m, path) in proxy.requests() {
        assert!(m != "DELETE", "petición inesperada: {m} {path}");
    }

    // Quitar un montaje exige aceptar el riesgo; el volumen no se elimina.
    let cfg = e.edit_config(&out.container_id).await.unwrap();
    let req = EditRequest {
        mounts: cfg
            .current
            .mounts
            .iter()
            .filter(|m| m.target != "/b")
            .cloned()
            .collect(),
        ..cfg.current.clone()
    };
    let plan = e.edit_plan(&out.container_id, &req).await.unwrap();
    assert!(
        plan.risks.iter().any(|r| r.code == "mountDetached"),
        "{:?}",
        plan.risks
    );
    let out2 = e
        .edit_apply(&out.container_id, &req, &confirm(&plan))
        .await
        .unwrap();
    assert_eq!(out2.error, None, "{out2:?}");
    assert!(
        docker.inspect_volume(&vol_b).await.is_ok(),
        "el volumen se conserva"
    );
    let m = mounts_of(&inspect(&docker, &out2.container_id).await);
    assert!(!m.iter().any(|x| x.0 == "/b"));
}

#[tokio::test(flavor = "multi_thread")]
async fn unsafe_recreations_are_refused_before_touching_anything() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (e, _log, proxy, docker) = engine_via_proxy(&dir).await;
    let p = prefix("refuse");
    let _clean = Cleanup::new(&docker, &p);
    let mk = |name: String, extra: HashMap<String, String>, host: HostConfig| {
        let docker = docker.clone();
        async move {
            let mut l = labels();
            l.extend(extra);
            create(
                &docker,
                &name,
                ContainerCreateBody {
                    image: Some(fixture::SHELL_IMAGE.into()),
                    labels: Some(l),
                    // Docker 26 y anteriores solo publican puertos expuestos.
                    exposed_ports: host
                        .port_bindings
                        .as_ref()
                        .map(|p| p.keys().cloned().collect()),
                    host_config: Some(host),
                    ..Default::default()
                },
            )
            .await
        }
    };
    let rm = mk(
        format!("{p}-rm"),
        HashMap::new(),
        HostConfig {
            auto_remove: Some(true),
            ..Default::default()
        },
    )
    .await;
    let compose = mk(
        format!("{p}-compose"),
        HashMap::from([("com.docker.compose.project".to_string(), "demo".to_string())]),
        HostConfig::default(),
    )
    .await;
    let paused = mk(format!("{p}-paused"), HashMap::new(), HostConfig::default()).await;
    docker.start_container(&paused, None).await.unwrap();
    docker.pause_container(&paused).await.unwrap();
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let holder = mk(
        format!("{p}-holder"),
        HashMap::new(),
        HostConfig {
            port_bindings: Some(HashMap::from([(
                "80/tcp".to_string(),
                Some(vec![PortBinding {
                    host_ip: Some("127.0.0.1".into()),
                    host_port: Some(port.to_string()),
                }]),
            )])),
            ..Default::default()
        },
    )
    .await;
    docker.start_container(&holder, None).await.unwrap();

    let writes = |px: &Proxy| {
        px.requests()
            .into_iter()
            .filter(|(m, _)| m != "GET" && m != "HEAD")
            .count()
    };
    let n = writes(&proxy);
    for (id, code) in [
        (&rm, "autoRemove"),
        (&compose, "composeManaged"),
        (&paused, "state"),
    ] {
        let cfg = e.edit_config(id).await.unwrap();
        let req = EditRequest {
            env: vec![KeyValue::new("X", "1")],
            ..cfg.current.clone()
        };
        let plan = e.edit_plan(id, &req).await.unwrap();
        assert!(
            plan.blockers.iter().any(|b| b.code == code),
            "{code}: {:?}",
            plan.blockers
        );
        match e.edit_apply(id, &req, &confirm(&plan)).await {
            Err(EngineError::InvalidInput { message }) => assert!(message.contains(code)),
            other => panic!("{code}: {other:?}"),
        }
        // Un cambio directo sigue siendo posible en un contenedor de Compose.
        if code == "composeManaged" {
            let direct = EditRequest {
                restart: RestartSpec {
                    name: RestartName::Always,
                    max_retries: 0,
                },
                ..cfg.current.clone()
            };
            assert!(!e.edit_plan(id, &direct).await.unwrap().needs_recreate);
        }
    }
    assert_eq!(writes(&proxy), n, "no se envió ninguna modificación");

    // Nombre ocupado y puerto publicado por otro contenedor: errores de validación.
    let cfg = e.edit_config(&compose).await.unwrap();
    let plan = e
        .edit_plan(
            &compose,
            &EditRequest {
                name: format!("{p}-holder"),
                ..cfg.current.clone()
            },
        )
        .await
        .unwrap();
    assert!(plan.issues.iter().any(|i| i.code == "name.inUse"));
    let cfg = e.edit_config(&rm).await.unwrap();
    let req = EditRequest {
        ports: vec![PortSpec {
            host_ip: String::new(),
            host_port: Some(port),
            container_port: 80,
            protocol: Protocol::Tcp,
        }],
        ..cfg.current.clone()
    };
    let plan = e.edit_plan(&rm, &req).await.unwrap();
    assert!(
        plan.issues
            .iter()
            .any(|i| i.code == "port.inUseByContainer"),
        "{:?}",
        plan.issues
    );
    match e.edit_apply(&rm, &req, &confirm(&plan)).await {
        Err(EngineError::InvalidSpec { .. }) => {}
        other => panic!("{other:?}"),
    }
    // Nombre no válido.
    let bad = EditRequest {
        name: "no válido".into(),
        ..cfg.current.clone()
    };
    match e.edit_apply(&rm, &bad, &ApplyOptions::default()).await {
        Err(EngineError::InvalidSpec { issues }) => {
            assert!(issues.iter().any(|i| i.code == "name.invalid"))
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(writes(&proxy), n);
    docker.unpause_container(&paused).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn fixed_mac_address_is_kept_only_when_asked_or_when_it_is_known() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (e, _log, _proxy, docker) = engine_via_proxy(&dir).await;
    let p = prefix("mac");
    let mut clean = Cleanup::new(&docker, &p);
    let net = format!("{p}-net");
    clean.networks.push(net.clone());
    docker
        .create_network(NetworkCreateRequest {
            name: net.clone(),
            labels: Some(labels()),
            ..Default::default()
        })
        .await
        .unwrap();
    const MAC: &str = "02:aa:bb:cc:dd:01";
    let body = || ContainerCreateBody {
        image: Some(fixture::SHELL_IMAGE.into()),
        labels: Some(labels()),
        host_config: Some(HostConfig {
            network_mode: Some(net.clone()),
            ..Default::default()
        }),
        networking_config: Some(NetworkingConfig {
            endpoints_config: Some(HashMap::from([(
                net.clone(),
                EndpointSettings {
                    mac_address: Some(MAC.into()),
                    ..Default::default()
                },
            )])),
        }),
        ..Default::default()
    };
    let mac_of = |c: &bollard::models::ContainerInspectResponse| {
        c.network_settings.clone().unwrap().networks.unwrap()[&net]
            .mac_address
            .clone()
            .unwrap_or_default()
    };
    let id = create(&docker, &p, body()).await;
    let api: Vec<u32> = docker
        .version()
        .await
        .unwrap()
        .api_version
        .unwrap()
        .split('.')
        .map(|x| x.parse().unwrap())
        .collect();
    if (api[0], api[1]) < (1, 44) {
        // Docker 24 (API 1.43) no admite la MAC por red: la aplicación lo
        // dice y no acepta «conservar MAC» en lugar de perderla en silencio.
        docker.start_container(&id, None).await.unwrap();
        let cfg = e.edit_config(&id).await.unwrap();
        let req = EditRequest {
            env: vec![KeyValue::new("STEP", "1")],
            ..cfg.current.clone()
        };
        let plan = e.edit_plan(&id, &req).await.unwrap();
        assert!(plan.notices.iter().any(|n| n.code == "macLegacy"));
        match e
            .edit_apply(
                &id,
                &req,
                &ApplyOptions {
                    keep_mac: true,
                    ..confirm(&plan)
                },
            )
            .await
        {
            Err(EngineError::InvalidInput { .. }) => {}
            other => panic!("{other:?}"),
        }
        assert_eq!(status(&inspect(&docker, &id).await), "running");
        assert_eq!(names(&docker, &p).await, vec![p.clone()]);
        return;
    }

    // Nunca iniciado: la MAC que figura es la configurada y se conserva sola.
    let cfg = e.edit_config(&id).await.unwrap();
    let req = EditRequest {
        env: vec![KeyValue::new("STEP", "1")],
        ..cfg.current.clone()
    };
    let plan = e.edit_plan(&id, &req).await.unwrap();
    assert!(plan.notices.is_empty(), "{:?}", plan.notices);
    let out = e.edit_apply(&id, &req, &confirm(&plan)).await.unwrap();
    assert_eq!(out.error, None, "{out:?}");
    assert_eq!(mac_of(&inspect(&docker, &out.container_id).await), MAC);

    // En ejecución y con «conservar MAC»: la misma dirección.
    let id = out.container_id;
    docker.start_container(&id, None).await.unwrap();
    assert_eq!(mac_of(&inspect(&docker, &id).await), MAC);
    let cfg = e.edit_config(&id).await.unwrap();
    let req = EditRequest {
        env: vec![KeyValue::new("STEP", "2")],
        ..cfg.current.clone()
    };
    let plan = e.edit_plan(&id, &req).await.unwrap();
    assert!(plan.notices.iter().any(|n| n.code == "macChoice"));
    let out = e
        .edit_apply(
            &id,
            &req,
            &ApplyOptions {
                keep_mac: true,
                ..confirm(&plan)
            },
        )
        .await
        .unwrap();
    assert_eq!(out.error, None, "{out:?}");
    assert_eq!(mac_of(&inspect(&docker, &out.container_id).await), MAC);

    // En ejecución y sin pedirlo: Docker asigna una dirección nueva.
    let id = out.container_id;
    let cfg = e.edit_config(&id).await.unwrap();
    let req = EditRequest {
        env: vec![KeyValue::new("STEP", "3")],
        ..cfg.current.clone()
    };
    let plan = e.edit_plan(&id, &req).await.unwrap();
    let out = e.edit_apply(&id, &req, &confirm(&plan)).await.unwrap();
    assert_eq!(out.error, None, "{out:?}");
    let now = mac_of(&inspect(&docker, &out.container_id).await);
    assert!(!now.is_empty() && now != MAC, "{now}");

    // Detenido: se avisa de que no se puede conocer.
    docker
        .stop_container(
            &out.container_id,
            Some(bollard::query_parameters::StopContainerOptions {
                t: Some(0),
                signal: None,
            }),
        )
        .await
        .unwrap();
    let cfg = e.edit_config(&out.container_id).await.unwrap();
    let plan = e
        .edit_plan(
            &out.container_id,
            &EditRequest {
                env: vec![KeyValue::new("STEP", "4")],
                ..cfg.current.clone()
            },
        )
        .await
        .unwrap();
    assert!(plan.notices.iter().any(|n| n.code == "macUnknown"));
}

// ------------------------------------------------------------ imagen desde contenedor

#[tokio::test(flavor = "multi_thread")]
async fn commit_creates_an_image_without_volume_data_and_leaves_the_container_alone() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (e, log, proxy, docker) = engine_via_proxy(&dir).await;
    let p = prefix("commit");
    let mut clean = Cleanup::new(&docker, &p);
    let volume = format!("{p}-vol");
    clean.volumes.push(volume.clone());
    let bind = tempfile::tempdir().unwrap();
    let id = create(
        &docker,
        &p,
        ContainerCreateBody {
            image: Some(fixture::SHELL_IMAGE.into()),
            labels: Some(labels()),
            host_config: Some(HostConfig {
                binds: Some(vec![
                    format!("{volume}:/data"),
                    format!("{}:/host", bind.path().display()),
                ]),
                ..Default::default()
            }),
            ..Default::default()
        },
    )
    .await;
    docker.start_container(&id, None).await.unwrap();
    let (_, code) = sh(
        &docker,
        &id,
        "echo capa > /tmp/layer.txt && echo volumen > /data/v.txt && echo carpeta > /host/h.txt",
    )
    .await;
    assert_eq!(code, 0);
    let started = inspect(&docker, &id).await.state.unwrap().started_at;

    let repo = format!("dcc-test/commit-{}", std::process::id());
    let reference = format!("{repo}:v1");
    clean.images.push(reference.clone());
    let req = CommitRequest {
        repository: repo.clone(),
        tag: "v1".into(),
        author: "Prueba <prueba@example.com>".into(),
        message: "Estado tras la configuración".into(),
        pause: false,
        replace_existing: false,
    };
    let pre = e.commit_preview(&id, &req).await.unwrap();
    assert_eq!(pre.reference.as_deref(), Some(reference.as_str()));
    assert_eq!(pre.status, "running");
    assert!(pre.issues.is_empty() && pre.existing_image_id.is_none());
    let mut ex: Vec<(String, String)> = pre
        .excluded_mounts
        .iter()
        .map(|m| (m.kind.clone(), m.target.clone()))
        .collect();
    ex.sort();
    assert_eq!(
        ex,
        [
            ("bind".to_string(), "/host".to_string()),
            ("volume".to_string(), "/data".to_string())
        ]
    );
    assert!(pre.equivalent.contains("commit") && pre.equivalent.contains("--pause=false"));

    // Validación: nada llega a Docker.
    let n = proxy.requests().len();
    for bad in [
        CommitRequest {
            repository: "Con Mayúsculas".into(),
            ..req.clone()
        },
        CommitRequest {
            repository: String::new(),
            ..req.clone()
        },
        CommitRequest {
            tag: "mal tag".into(),
            ..req.clone()
        },
        CommitRequest {
            message: "línea\nnueva".into(),
            ..req.clone()
        },
    ] {
        match e.commit_image(&id, &bad).await {
            Err(EngineError::InvalidSpec { .. }) => {}
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(proxy.requests().len(), n);

    let res = e.commit_image(&id, &req).await.unwrap();
    assert_eq!(res.reference, reference);
    assert_eq!(res.replaced_image_id, None);
    let img = docker.inspect_image(&reference).await.unwrap();
    assert_eq!(img.id.as_deref(), Some(res.image_id.as_str()));
    assert_eq!(img.author.as_deref(), Some("Prueba <prueba@example.com>"));
    assert_eq!(img.comment.as_deref(), Some("Estado tras la configuración"));
    // Aparece en la lista de imágenes de la aplicación.
    let list = e.list_images(None).await.unwrap();
    assert!(serde_json::to_string(&list).unwrap().contains(&reference));

    // El contenedor original sigue igual: en ejecución, sin reinicios ni pausas.
    let after = inspect(&docker, &id).await;
    assert_eq!(status(&after), "running");
    assert_eq!(after.state.unwrap().started_at, started);
    let (o, _) = sh(&docker, &id, "cat /tmp/layer.txt /data/v.txt /host/h.txt").await;
    assert_eq!(o, "capa\nvolumen\ncarpeta\n");

    // La imagen contiene la capa del contenedor, pero no el volumen ni la carpeta.
    let probe = create(
        &docker,
        &format!("{p}-probe"),
        ContainerCreateBody {
            image: Some(reference.clone()),
            labels: Some(labels()),
            ..Default::default()
        },
    )
    .await;
    docker.start_container(&probe, None).await.unwrap();
    let (o, _) = sh(
        &docker,
        &probe,
        "cat /tmp/layer.txt; cat /data/v.txt 2>/dev/null || echo SIN-VOLUMEN; cat /host/h.txt 2>/dev/null || echo SIN-CARPETA",
    )
    .await;
    assert_eq!(o, "capa\nSIN-VOLUMEN\nSIN-CARPETA\n");

    // El nombre ya existe: no se reutiliza sin aceptarlo expresamente.
    let pre = e.commit_preview(&id, &req).await.unwrap();
    assert_eq!(
        pre.existing_image_id.as_deref(),
        Some(res.image_id.as_str())
    );
    match e.commit_image(&id, &req).await {
        Err(EngineError::AlreadyExists { path }) => assert_eq!(path, reference),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        docker
            .inspect_image(&reference)
            .await
            .unwrap()
            .id
            .as_deref(),
        Some(res.image_id.as_str())
    );
    sh(&docker, &id, "echo dos > /tmp/second.txt").await;
    let res2 = e
        .commit_image(
            &id,
            &CommitRequest {
                replace_existing: true,
                pause: true,
                ..req.clone()
            },
        )
        .await
        .unwrap();
    assert_ne!(res2.image_id, res.image_id);
    assert_eq!(
        res2.replaced_image_id.as_deref(),
        Some(res.image_id.as_str())
    );
    clean.images.push(res.image_id.clone());
    assert_eq!(
        status(&inspect(&docker, &id).await),
        "running",
        "la pausa es temporal"
    );

    // Actividad: la operación real, con su orden equivalente.
    let rec = log
        .snapshot()
        .into_iter()
        .rfind(|r| r.kind == "container.commit")
        .unwrap();
    assert_eq!(rec.status, OpStatus::Succeeded);
    match &rec.transport {
        Transport::Api { method, path, .. } => {
            assert_eq!(method, "POST");
            assert!(path.starts_with("/commit?"), "{path}");
        }
        other => panic!("{other:?}"),
    }
    assert!(rec.equivalent_command.unwrap().contains("commit"));

    // Un contenedor que no existe: error claro de Docker.
    match e.commit_image("dcc-test-no-existe", &req).await {
        Err(EngineError::Docker { status: 404, .. }) => {}
        other => panic!("{other:?}"),
    }
    // Y uno detenido también se puede convertir en imagen.
    docker
        .stop_container(
            &id,
            Some(bollard::query_parameters::StopContainerOptions {
                t: Some(0),
                signal: None,
            }),
        )
        .await
        .unwrap();
    let stopped_ref = format!("{repo}:stopped");
    clean.images.push(stopped_ref.clone());
    let r3 = e
        .commit_image(
            &id,
            &CommitRequest {
                tag: "stopped".into(),
                ..req.clone()
            },
        )
        .await
        .unwrap();
    assert_eq!(r3.reference, stopped_ref);
    assert_eq!(status(&inspect(&docker, &id).await), "exited");
}

// ------------------------------------------------- recrear y renombrar a la vez

#[tokio::test(flavor = "multi_thread")]
async fn recreation_with_rename_lists_the_backup_under_the_new_name_and_restores_it() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (e, _log, _proxy, docker) = engine_via_proxy(&dir).await;
    let p = prefix("renrec");
    let mut clean = Cleanup::new(&docker, &p);
    let vol = format!("{p}-vol");
    clean.volumes.push(vol.clone());
    let id = create(
        &docker,
        &p,
        ContainerCreateBody {
            image: Some(fixture::SHELL_IMAGE.into()),
            labels: Some(labels()),
            env: Some(vec!["MODO=uno".into()]),
            host_config: Some(HostConfig {
                binds: Some(vec![format!("{vol}:/datos")]),
                ..Default::default()
            }),
            ..Default::default()
        },
    )
    .await;
    docker.start_container(&id, None).await.unwrap();
    assert_eq!(sh(&docker, &id, "echo dato > /datos/f").await.1, 0);

    let cfg = e.edit_config(&id).await.unwrap();
    let new_name = format!("{p}-nuevo");
    let req = EditRequest {
        name: new_name.clone(),
        env: vec![KeyValue {
            key: "MODO".into(),
            value: "dos".into(),
        }],
        ..cfg.current.clone()
    };
    let plan = e.edit_plan(&id, &req).await.unwrap();
    assert!(plan.needs_recreate, "{plan:?}");
    let backup = plan.backup_name.clone().unwrap();
    assert!(
        backup.starts_with(&format!("{new_name}-dcc-backup-")),
        "la copia lleva el nombre nuevo: {backup}"
    );
    let out = e.edit_apply(&id, &req, &confirm(&plan)).await.unwrap();
    assert_eq!(out.error, None, "{out:?}");
    assert!(out.recreated);
    assert_eq!(out.name, new_name);
    assert_eq!(out.backup_name.as_deref(), Some(backup.as_str()));
    assert_ne!(out.container_id, id);
    // El original es la copia (mismo ID), detenido.
    let b = inspect(&docker, &id).await;
    assert_eq!(b.name.unwrap().trim_start_matches('/'), backup);
    assert_eq!(status(&inspect(&docker, &id).await), "exited");

    // La pantalla del contenedor nuevo muestra su copia.
    let cfg2 = e.edit_config(&out.container_id).await.unwrap();
    assert_eq!(cfg2.current.name, new_name);
    assert_eq!(cfg2.backups.len(), 1, "{:?}", cfg2.backups);
    assert_eq!(cfg2.backups[0].id, id);
    assert_eq!(cfg2.backups[0].original_name, new_name);
    let (data, _) = sh(&docker, &out.container_id, "cat /datos/f; echo $MODO").await;
    assert_eq!(data.split_whitespace().collect::<Vec<_>>(), ["dato", "dos"]);

    // Restaurar: la copia recupera el nombre y el recreado se aparta sin borrarse.
    let r = e.restore_backup(&id, true, true).await.unwrap();
    assert_eq!(r.error, None, "{r:?}");
    let restored = inspect(&docker, &id).await;
    assert_eq!(
        restored.name.clone().unwrap().trim_start_matches('/'),
        new_name
    );
    assert_eq!(status(&restored), "running");
    let (data, _) = sh(&docker, &id, "cat /datos/f; echo $MODO").await;
    assert_eq!(data.split_whitespace().collect::<Vec<_>>(), ["dato", "uno"]);
    let aside = inspect(&docker, &out.container_id).await;
    assert!(aside
        .name
        .unwrap()
        .trim_start_matches('/')
        .starts_with(&format!("{new_name}-dcc-replaced-")));
    assert!(docker.inspect_volume(&vol).await.is_ok());
}
