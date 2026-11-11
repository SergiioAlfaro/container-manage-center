//! Fase 5 contra Docker real (`DCC_DOCKER_TESTS=1`): creación, listado,
//! inspección, conexión y desconexión de contenedores, eliminación y limpieza
//! de redes. Todo lo creado lleva la etiqueta `dcc.test=true` y se elimina al
//! terminar, aunque la prueba falle.

mod common;

use std::collections::HashMap;
use std::time::Duration;

use common::*;
use dcc_activity::{ActivityLog, OpStatus};
use dcc_engine::networks::{Cidr, ConnectOptions, IpamPool, NetworkSpec};
use dcc_engine::spec::KeyValue;
use dcc_engine::{ConnectionState, Engine, EngineError};

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

/// Elimina contenedores y redes de prueba aunque la prueba falle.
struct Cleanup {
    docker: bollard::Docker,
    containers: Vec<String>,
    networks: Vec<String>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        let d = self.docker.clone();
        let (cs, ns) = (
            std::mem::take(&mut self.containers),
            std::mem::take(&mut self.networks),
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
                for n in ns {
                    let _ = d.remove_network(&n).await;
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

/// Una subred /24 libre en 10.213.0.0/16 (no solapa con ninguna red existente).
async fn free_subnet(docker: &bollard::Docker, skip: usize) -> String {
    let used: Vec<Cidr> = docker
        .list_networks(None::<bollard::query_parameters::ListNetworksOptions>)
        .await
        .unwrap()
        .into_iter()
        .flat_map(|n| n.ipam.and_then(|i| i.config).unwrap_or_default())
        .filter_map(|c| c.subnet.and_then(|s| Cidr::parse(&s).ok()))
        .collect();
    let start = (std::process::id() as usize % 200) + skip;
    for i in 0..256 {
        let s = format!("10.213.{}.0/24", (start + i) % 256);
        let c = Cidr::parse(&s).unwrap();
        if !used
            .iter()
            .any(|u| u.contains_cidr(&c) || c.contains_cidr(u))
        {
            return s;
        }
    }
    panic!("no hay subredes libres en 10.213.0.0/16");
}

/// Crea un contenedor de prueba (sin red explícita: queda en `bridge`).
async fn container(docker: &bollard::Docker, name: &str, start: bool) -> String {
    let mut labels = HashMap::new();
    labels.insert("dcc.test".to_owned(), "true".to_owned());
    let id = docker
        .create_container(
            Some(bollard::query_parameters::CreateContainerOptions {
                name: Some(name.to_owned()),
                platform: String::new(),
            }),
            bollard::models::ContainerCreateBody {
                image: Some(fixture::IMAGE.into()),
                labels: Some(labels),
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

async fn container_networks(
    docker: &bollard::Docker,
    id: &str,
) -> HashMap<String, bollard::models::EndpointSettings> {
    docker
        .inspect_container(
            id,
            None::<bollard::query_parameters::InspectContainerOptions>,
        )
        .await
        .unwrap()
        .network_settings
        .and_then(|n| n.networks)
        .unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread")]
async fn create_list_inspect_connect_disconnect_and_remove() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (engine, log, proxy, docker) = engine_via_proxy(&dir).await;
    let name = tag("net");
    let subnet = free_subnet(&docker, 0).await;
    let prefix = subnet.trim_end_matches(".0/24").to_owned();
    let (a_name, b_name) = (tag("net-a"), tag("net-b"));
    let mut cleanup = Cleanup {
        docker: docker.clone(),
        containers: vec![a_name.clone(), b_name.clone()],
        networks: vec![name.clone()],
    };

    // Creación con subred, puerta de enlace, rango, etiqueta y opción.
    let spec = NetworkSpec {
        name: name.clone(),
        ipv4: IpamPool {
            subnet: subnet.clone(),
            gateway: format!("{prefix}.1"),
            ip_range: format!("{prefix}.128/25"),
        },
        labels: vec![
            KeyValue::new("dcc.test", "true"),
            KeyValue::new("dcc.app", "mi web"),
        ],
        options: vec![KeyValue::new("com.docker.network.driver.mtu", "1400")],
        attachable: true,
        ..Default::default()
    };
    assert!(spec.validate().is_empty(), "{:?}", spec.validate());
    let created = engine.create_network(&spec).await.unwrap();
    assert_eq!(created.id.len(), 64);
    let r = &records(&log, "network.create")[0];
    assert_eq!(r.status, OpStatus::Succeeded);
    assert_eq!(
        r.equivalent_command.as_deref(),
        Some(
            format!(
                "docker --host {} network create --driver bridge --subnet {subnet} \
                 --ip-range {prefix}.128/25 --gateway {prefix}.1 --label 'dcc.app=mi web' \
                 --label dcc.test=true --opt com.docker.network.driver.mtu=1400 --attachable {name}",
                docker_host(&proxy.path)
            )
            .as_str()
        )
    );

    // Listado: la red aparece con sus datos y sin contenedores.
    let list = engine.list_networks(None).await.unwrap();
    let n = list
        .iter()
        .find(|n| n.name == name)
        .expect("red en la lista");
    assert_eq!(n.id, created.id);
    assert_eq!(n.driver, "bridge");
    assert_eq!(n.subnets, std::slice::from_ref(&subnet));
    assert_eq!(n.gateways, [format!("{prefix}.1")]);
    assert!(n.attachable && !n.internal && !n.predefined);
    assert_eq!(n.labels["dcc.app"], "mi web");
    assert!(n.containers.is_empty());
    for p in ["bridge", "host", "none"] {
        assert!(
            list.iter().any(|n| n.name == p && n.predefined),
            "red predefinida {p}"
        );
    }

    // Contenedor A en marcha, B detenido.
    let a = container(&docker, &a_name, true).await;
    let b = container(&docker, &b_name, false).await;

    // Conectar A con alias e IP fija.
    let ip = format!("{prefix}.50");
    let opts = ConnectOptions {
        aliases: vec!["web".into(), "api".into()],
        ipv4: ip.clone(),
        ipv6: String::new(),
    };
    engine
        .connect_network(
            &created.id,
            Some(&name),
            &a,
            Some(&a_name),
            &opts,
            &n.subnets,
        )
        .await
        .unwrap();
    let nets = container_networks(&docker, &a).await;
    let ep = nets.get(&name).expect("A conectado");
    assert_eq!(ep.ip_address.as_deref(), Some(ip.as_str()));
    let aliases = ep.aliases.clone().unwrap_or_default();
    assert!(
        aliases.contains(&"web".to_owned()) && aliases.contains(&"api".to_owned()),
        "{aliases:?}"
    );
    let r = &records(&log, "network.connect")[0];
    assert_eq!(
        r.equivalent_command
            .as_deref()
            .map(|c| c.split(" network ").nth(1).unwrap_or_default()),
        Some(format!("connect --alias web --alias api --ip {ip} {name} {a_name}").as_str())
    );
    // Conectar otra vez: Docker lo rechaza y se registra el error.
    let e = engine
        .connect_network(
            &created.id,
            Some(&name),
            &a,
            Some(&a_name),
            &ConnectOptions::default(),
            &n.subnets,
        )
        .await
        .unwrap_err();
    assert!(matches!(e, EngineError::Docker { .. }), "{e:?}");
    assert!(e.to_string().contains("already exists"), "{e}");
    assert_eq!(records(&log, "network.connect")[0].status, OpStatus::Failed);

    // Conectar B (detenido).
    engine
        .connect_network(
            &created.id,
            Some(&name),
            &b,
            Some(&b_name),
            &ConnectOptions::default(),
            &n.subnets,
        )
        .await
        .unwrap();

    // Detalle: A en marcha con su IP; B detenido (Docker no lo incluye en el
    // inspect de la red; sale de la lista de contenedores).
    let d = engine
        .network_details(&created.id, Some(&name))
        .await
        .unwrap();
    assert_eq!(d.summary.name, name);
    assert_eq!(d.ipam_driver, "default");
    assert_eq!(d.pools.len(), 1);
    assert_eq!(d.pools[0].subnet, subnet);
    assert_eq!(d.pools[0].ip_range, format!("{prefix}.128/25"));
    assert_eq!(d.options["com.docker.network.driver.mtu"], "1400");
    let members: Vec<_> = d
        .summary
        .containers
        .iter()
        .map(|m| (m.name.as_str(), m.state.as_str(), m.ipv4.clone()))
        .collect();
    assert_eq!(
        members,
        [
            (a_name.as_str(), "running", Some(format!("{ip}/24"))),
            (b_name.as_str(), "created", None),
        ]
    );
    let raw: serde_json::Value = serde_json::from_str(&d.raw).unwrap();
    assert_eq!(raw["Id"], created.id.as_str());
    assert_eq!(
        raw["Containers"].as_object().unwrap().len(),
        1,
        "solo los que están en marcha"
    );
    // El listado también indica los dos contenedores.
    let list = engine.list_networks(None).await.unwrap();
    let n = list.iter().find(|n| n.name == name).unwrap();
    assert_eq!(n.containers.len(), 2);

    // Eliminar con A en marcha: Docker lo rechaza y la red sigue.
    let e = engine
        .remove_network(&created.id, Some(&name), true)
        .await
        .unwrap_err();
    assert_eq!(e.http_status(), Some(403), "{e:?}");
    assert!(e.to_string().contains("active endpoints"), "{e}");
    assert_eq!(records(&log, "network.remove")[0].status, OpStatus::Failed);
    docker
        .inspect_network(
            &created.id,
            None::<bollard::query_parameters::InspectNetworkOptions>,
        )
        .await
        .expect("la red sigue existiendo");

    // Desconectar sin confirmación: no se envía nada.
    let before = proxy.requests().len();
    assert_eq!(
        engine
            .disconnect_network(&created.id, Some(&name), &a, Some(&a_name), false, false)
            .await,
        Err(EngineError::NotConfirmed)
    );
    assert_eq!(proxy.requests().len(), before);
    // Desconectar A.
    engine
        .disconnect_network(&created.id, Some(&name), &a, Some(&a_name), false, true)
        .await
        .unwrap();
    assert!(!container_networks(&docker, &a).await.contains_key(&name));
    let r = &records(&log, "network.disconnect")[0];
    assert_eq!(r.status, OpStatus::Succeeded);
    assert!(r
        .equivalent_command
        .as_deref()
        .unwrap()
        .ends_with(&format!("network disconnect {name} {a_name}")));
    // Desconectar algo que ya no está conectado: error real de Docker.
    let e = engine
        .disconnect_network(&created.id, Some(&name), &a, Some(&a_name), false, true)
        .await
        .unwrap_err();
    assert!(e.to_string().contains("is not connected"), "{e}");

    // Eliminar con solo B (detenido) conectado: Docker la elimina, y B ya no
    // puede arrancar. Es lo que la interfaz avisa antes de eliminar (D-047).
    engine
        .remove_network(&created.id, Some(&name), true)
        .await
        .unwrap();
    assert!(docker
        .inspect_network(
            &created.id,
            None::<bollard::query_parameters::InspectNetworkOptions>
        )
        .await
        .is_err());
    cleanup.networks.clear();
    let start_b = docker
        .start_container(&b, None::<bollard::query_parameters::StartContainerOptions>)
        .await;
    assert!(start_b.is_err(), "B no debería poder arrancar sin su red");
    let r = &records(&log, "network.remove")[0];
    assert_eq!(r.status, OpStatus::Succeeded);
    assert!(r
        .equivalent_command
        .as_deref()
        .unwrap()
        .ends_with(&format!("network rm {name}")));
    // La petición usa el ID; el equivalente, el nombre.
    assert!(proxy
        .requests()
        .contains(&("DELETE".to_owned(), format!("/networks/{}", created.id))));

    assert_eq!(recorded(&log), proxy.requests());
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_input_never_reaches_docker_and_docker_errors_are_reported() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (engine, log, proxy, docker) = engine_via_proxy(&dir).await;
    let name = tag("err");
    let other = tag("err-overlap");
    let a_name = tag("err-a");
    let _c = Cleanup {
        docker: docker.clone(),
        containers: vec![a_name.clone()],
        networks: vec![name.clone(), other.clone()],
    };
    let before = proxy.requests().len();

    // Especificación inválida: InvalidSpec, sin tráfico.
    let bad = NetworkSpec {
        name: "mi red".into(),
        ipv4: IpamPool {
            subnet: "10.241.0.5/24".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    match engine.create_network(&bad).await {
        Err(EngineError::InvalidSpec { issues }) => {
            let codes: Vec<_> = issues.iter().map(|i| i.code.as_str()).collect();
            assert_eq!(codes, ["name.invalid", "subnet.hostBits"]);
        }
        other => panic!("{other:?}"),
    }
    // Referencias inválidas, IP fuera de la subred, sin confirmar: sin tráfico.
    assert!(matches!(
        engine.remove_network("../x", None, true).await,
        Err(EngineError::InvalidInput { .. })
    ));
    assert_eq!(
        engine.remove_network("x", None, false).await,
        Err(EngineError::NotConfirmed)
    );
    assert_eq!(
        engine.prune_networks(None, false).await,
        Err(EngineError::NotConfirmed)
    );
    let e = engine
        .connect_network(
            "x",
            None,
            "y",
            None,
            &ConnectOptions {
                ipv4: "10.99.0.5".into(),
                ..Default::default()
            },
            &["10.98.0.0/24".into()],
        )
        .await
        .unwrap_err();
    assert!(matches!(e, EngineError::InvalidInput { .. }), "{e:?}");
    assert!(matches!(
        engine
            .connect_network(
                "x",
                None,
                "y",
                None,
                &ConnectOptions {
                    aliases: vec!["a b".into()],
                    ..Default::default()
                },
                &[]
            )
            .await,
        Err(EngineError::InvalidInput { .. })
    ));
    assert_eq!(
        proxy.requests().len(),
        before,
        "nada inválido llega a Docker"
    );
    assert!(log
        .snapshot()
        .iter()
        .all(|r| !r.kind.starts_with("network.")));

    // Errores reales de Docker, registrados.
    let subnet = free_subnet(&docker, 7).await;
    let ok = NetworkSpec {
        name: name.clone(),
        ipv4: IpamPool {
            subnet: subnet.clone(),
            ..Default::default()
        },
        labels: vec![KeyValue::new("dcc.test", "true")],
        ..Default::default()
    };
    engine.create_network(&ok).await.unwrap();
    // Nombre repetido (sin subred, para que Docker compruebe el nombre y no el
    // solapamiento): 409, y en Docker sigue habiendo una sola red con ese
    // nombre. Docker < 25 crearía otra sin `CheckDuplicate` (D-048).
    let dup = NetworkSpec {
        name: name.clone(),
        labels: vec![KeyValue::new("dcc.test", "true")],
        ..Default::default()
    };
    let e = engine.create_network(&dup).await.unwrap_err();
    assert_eq!(e.http_status(), Some(409), "{e:?}");
    assert!(e.to_string().contains("already exists"), "{e}");
    let same_name = docker
        .list_networks(None::<bollard::query_parameters::ListNetworksOptions>)
        .await
        .unwrap()
        .into_iter()
        .filter(|n| n.name.as_deref() == Some(name.as_str()))
        .count();
    assert_eq!(
        same_name, 1,
        "no debe crearse una segunda red con el mismo nombre"
    );
    // Misma subred con otro nombre: Docker rechaza el solapamiento.
    let overlap = NetworkSpec {
        name: other.clone(),
        ..ok.clone()
    };
    let e = engine.create_network(&overlap).await.unwrap_err();
    assert!(e.to_string().to_lowercase().contains("overlap"), "{e}");
    // Red predefinida: Docker no deja eliminarla.
    let bridge_id = docker
        .inspect_network(
            "bridge",
            None::<bollard::query_parameters::InspectNetworkOptions>,
        )
        .await
        .unwrap()
        .id
        .unwrap();
    let e = engine
        .remove_network(&bridge_id, Some("bridge"), true)
        .await
        .unwrap_err();
    assert_eq!(e.http_status(), Some(403), "{e:?}");
    // Alias en la red bridge: Docker solo los admite en redes de usuario.
    let a = container(&docker, &a_name, false).await;
    let e = engine
        .connect_network(
            &bridge_id,
            Some("bridge"),
            &a,
            Some(&a_name),
            &ConnectOptions {
                aliases: vec!["web".into()],
                ..Default::default()
            },
            &[],
        )
        .await
        .unwrap_err();
    assert!(e.to_string().contains("alias"), "{e}");
    // Red que no existe: 404.
    let e = engine
        .network_details("dcc-test-no-existe-12345", None)
        .await
        .unwrap_err();
    assert_eq!(e.http_status(), Some(404), "{e:?}");

    let creates = records(&log, "network.create");
    assert_eq!(creates.len(), 3);
    assert_eq!(
        creates
            .iter()
            .filter(|r| r.status == OpStatus::Failed)
            .count(),
        2
    );
    assert!(log
        .snapshot()
        .iter()
        .filter(|r| r.kind.starts_with("network.") && r.status == OpStatus::Failed)
        .all(|r| r.error.as_deref().is_some_and(|e| !e.is_empty())));
    assert_eq!(recorded(&log), proxy.requests());
}

#[tokio::test(flavor = "multi_thread")]
async fn prune_removes_networks_without_running_containers() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (engine, log, proxy, docker) = engine_via_proxy(&dir).await;
    let label = format!("dcc.prune={}", std::process::id());
    let (free, stopped, running) = (tag("p-free"), tag("p-stopped"), tag("p-running"));
    let (cs, cr) = (tag("p-cs"), tag("p-cr"));
    let _c = Cleanup {
        docker: docker.clone(),
        containers: vec![cs.clone(), cr.clone()],
        networks: vec![free.clone(), stopped.clone(), running.clone()],
    };
    let mut ids = HashMap::new();
    for n in [&free, &stopped, &running] {
        let spec = NetworkSpec {
            name: n.clone(),
            labels: vec![
                KeyValue::new("dcc.test", "true"),
                KeyValue::new("dcc.prune", std::process::id().to_string()),
            ],
            ..Default::default()
        };
        ids.insert(n.clone(), engine.create_network(&spec).await.unwrap().id);
    }
    let s = container(&docker, &cs, false).await;
    let r = container(&docker, &cr, true).await;
    engine
        .connect_network(
            &ids[&stopped],
            Some(&stopped),
            &s,
            Some(&cs),
            &ConnectOptions::default(),
            &[],
        )
        .await
        .unwrap();
    engine
        .connect_network(
            &ids[&running],
            Some(&running),
            &r,
            Some(&cr),
            &ConnectOptions::default(),
            &[],
        )
        .await
        .unwrap();

    // Lo que la interfaz estima antes de limpiar: las redes sin contenedores en
    // marcha (entre ellas, la que solo usa un contenedor detenido).
    let list = engine.list_networks(None).await.unwrap();
    let mut estimate: Vec<String> = list
        .iter()
        .filter(|n| {
            n.labels.get("dcc.prune").map(String::as_str) == Some(&std::process::id().to_string())
        })
        .filter(|n| !n.predefined && !n.containers.iter().any(|m| m.state == "running"))
        .map(|n| n.name.clone())
        .collect();
    estimate.sort();
    let mut expected = vec![free.clone(), stopped.clone()];
    expected.sort();
    assert_eq!(estimate, expected);

    let mut deleted = engine.prune_networks(Some(&label), true).await.unwrap();
    deleted.sort();
    // Docker elimina también la red que solo usaba el contenedor detenido
    // (comprobado con Docker 29 y 24); la estimación coincide con lo eliminado.
    assert_eq!(deleted, expected);
    assert!(docker
        .inspect_network(
            &running,
            None::<bollard::query_parameters::InspectNetworkOptions>
        )
        .await
        .is_ok());
    let rec = &records(&log, "network.prune")[0];
    assert_eq!(rec.status, OpStatus::Succeeded);
    assert!(rec
        .equivalent_command
        .as_deref()
        .unwrap()
        .ends_with(&format!("network prune --force --filter label={label}")));
    // Segunda vez: nada que eliminar.
    assert!(engine
        .prune_networks(Some(&label), true)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(recorded(&log), proxy.requests());
    drop(r);
}

/// IPv6 depende de que el kernel y el daemon lo admitan: si Docker lo rechaza,
/// se comprueba que el error real queda registrado y la red no existe.
#[tokio::test(flavor = "multi_thread")]
async fn ipv6_network_is_created_or_the_real_error_is_reported() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (engine, log, proxy, docker) = engine_via_proxy(&dir).await;
    let name = tag("v6");
    let _c = Cleanup {
        docker: docker.clone(),
        containers: vec![],
        networks: vec![name.clone()],
    };
    let sub = format!("fd00:dcc:{:x}::/64", std::process::id() % 0xffff);
    let spec = NetworkSpec {
        name: name.clone(),
        enable_ipv6: true,
        ipv6: IpamPool {
            subnet: sub.clone(),
            ..Default::default()
        },
        labels: vec![KeyValue::new("dcc.test", "true")],
        ..Default::default()
    };
    match engine.create_network(&spec).await {
        Ok(c) => {
            let d = engine.network_details(&c.id, Some(&name)).await.unwrap();
            assert!(d.summary.enable_ipv6);
            assert!(d.summary.subnets.contains(&sub), "{:?}", d.summary.subnets);
            eprintln!("ipv6: red IPv6 creada con {sub}");
        }
        Err(e) => {
            assert!(matches!(e, EngineError::Docker { .. }), "{e:?}");
            assert_eq!(records(&log, "network.create")[0].status, OpStatus::Failed);
            tokio::time::sleep(Duration::from_millis(200)).await;
            assert!(docker
                .inspect_network(
                    &name,
                    None::<bollard::query_parameters::InspectNetworkOptions>
                )
                .await
                .is_err());
            eprintln!("ipv6: el daemon no admite IPv6 en este equipo: {e}");
        }
    }
    assert_eq!(recorded(&log), proxy.requests());
}
