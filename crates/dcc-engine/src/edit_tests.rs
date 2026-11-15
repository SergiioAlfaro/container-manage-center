// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Pruebas deterministas de la edición: lectura, clasificación de cambios,
//! validación y cuerpos de `update` y de recreación.

use std::collections::HashMap;

use bollard::models::{
    ContainerConfig, ContainerInspectResponse, ContainerState, ContainerStateStatusEnum,
    EndpointIpamConfig, EndpointSettings, HostConfig, ImageConfig, Mount, MountPoint, MountType,
    NetworkSettings, PortBinding,
};

use super::*;
use crate::spec::MIB;
use crate::spec::REDACTED;

fn inspect() -> ContainerInspectResponse {
    let id = "0123456789abcdef0123456789abcdef".to_string();
    ContainerInspectResponse {
        id: Some(id),
        name: Some("/web".into()),
        image: Some("sha256:aaaa".into()),
        state: Some(ContainerState {
            status: Some(ContainerStateStatusEnum::RUNNING),
            ..Default::default()
        }),
        config: Some(ContainerConfig {
            hostname: Some("0123456789ab".into()),
            image: Some("nginx:1.27".into()),
            env: Some(vec![
                "PATH=/usr/bin".into(),
                "MODE=prod".into(),
                "DB_PASSWORD=hunter2".into(),
            ]),
            labels: Some(HashMap::from([
                ("maintainer".to_string(), "x".to_string()),
                ("team".to_string(), "ops".to_string()),
            ])),
            exposed_ports: Some(vec!["80/tcp".into(), "443/tcp".into()]),
            volumes: Some(vec!["/cache".into(), "/declared".into()]),
            cmd: Some(vec!["nginx".into()]),
            ..Default::default()
        }),
        host_config: Some(HostConfig {
            binds: Some(vec!["data:/data".into(), "/srv/www:/www:ro,z".into()]),
            mounts: Some(vec![
                Mount {
                    typ: Some(MountType::VOLUME),
                    source: Some("logs".into()),
                    target: Some("/logs".into()),
                    ..Default::default()
                },
                Mount {
                    typ: Some(MountType::TMPFS),
                    target: Some("/scratch".into()),
                    ..Default::default()
                },
            ]),
            port_bindings: Some(HashMap::from([(
                "80/tcp".to_string(),
                Some(vec![PortBinding {
                    host_ip: Some("".into()),
                    host_port: Some("8080".into()),
                }]),
            )])),
            network_mode: Some("front".into()),
            memory: Some(256 * MIB as i64),
            memory_swap: Some(512 * MIB as i64),
            nano_cpus: Some(1_500_000_000),
            dns: Some(vec!["9.9.9.9".into()]),
            links: Some(vec!["/db:/web/database".into()]),
            ..Default::default()
        }),
        mounts: Some(vec![
            MountPoint {
                typ: Some("volume".into()),
                name: Some("data".into()),
                destination: Some("/data".into()),
                rw: Some(true),
                ..Default::default()
            },
            MountPoint {
                typ: Some("bind".into()),
                source: Some("/srv/www".into()),
                destination: Some("/www".into()),
                rw: Some(false),
                ..Default::default()
            },
            MountPoint {
                typ: Some("volume".into()),
                name: Some("logs".into()),
                destination: Some("/logs".into()),
                rw: Some(true),
                ..Default::default()
            },
            MountPoint {
                typ: Some("volume".into()),
                name: Some("f00dfeed".into()),
                destination: Some("/cache".into()),
                rw: Some(true),
                ..Default::default()
            },
        ]),
        network_settings: Some(NetworkSettings {
            networks: Some(HashMap::from([
                (
                    "front".to_string(),
                    EndpointSettings {
                        ipam_config: Some(EndpointIpamConfig {
                            ipv4_address: Some("10.0.0.5".into()),
                            ..Default::default()
                        }),
                        aliases: Some(vec!["web".into(), "0123456789ab".into()]),
                        ip_address: Some("10.0.0.5".into()),
                        mac_address: Some("02:42:0a:00:00:05".into()),
                        network_id: Some("net1".into()),
                        ..Default::default()
                    },
                ),
                (
                    "back".to_string(),
                    EndpointSettings {
                        ip_address: Some("10.1.0.9".into()),
                        mac_address: Some("02:42:0a:01:00:09".into()),
                        ..Default::default()
                    },
                ),
            ])),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn image() -> ImageConfig {
    ImageConfig {
        env: Some(vec!["PATH=/usr/bin".into()]),
        labels: Some(HashMap::from([("maintainer".to_string(), "x".to_string())])),
        cmd: Some(vec!["nginx".into()]),
        volumes: Some(vec!["/cache".into()]),
        ..Default::default()
    }
}

fn current() -> EditRequest {
    current_request(&inspect(), Some(&image()))
}

fn no_macs() -> HashMap<String, String> {
    HashMap::new()
}

#[test]
fn reads_the_editable_state() {
    let cur = current();
    assert_eq!(cur.name, "web");
    assert_eq!(cur.description, "");
    assert_eq!(cur.cpus, Some(1.5));
    assert_eq!(cur.memory_bytes, Some(256 * MIB));
    // Solo las variables propias: PATH viene de la imagen.
    let keys: Vec<&str> = cur.env.iter().map(|k| k.key.as_str()).collect();
    assert_eq!(keys, ["MODE", "DB_PASSWORD"]);
    assert_eq!(cur.ports.len(), 1);
    assert_eq!(cur.ports[0].host_port, Some(8080));
    // Volúmenes y carpetas; el anónimo figura con su nombre real; tmpfs aparte.
    let m: Vec<String> = cur.mounts.iter().map(mount_text).collect();
    assert_eq!(
        m,
        [
            "data:/data",
            "/srv/www:/www:ro",
            "logs:/logs",
            "f00dfeed:/cache"
        ]
    );
    let cfg = editable_config(&inspect(), Some(&image()));
    assert_eq!(cfg.anonymous_volumes, ["f00dfeed"]);
    assert_eq!(cfg.other_mounts, ["tmpfs → /scratch"]);
    assert_eq!(cfg.image_env, [KeyValue::new("PATH", "/usr/bin")]);
    assert_eq!(cfg.networks.len(), 2);
    assert_eq!(cfg.networks[1].fixed_ipv4, "10.0.0.5");
    assert_eq!(cfg.status, "running");
}

#[test]
fn classifies_direct_and_recreate_changes() {
    let cur = current();
    let (c, kept) = diff(&cur, &cur);
    assert!(c.is_empty());
    assert_eq!(kept.len(), 9);

    let des = EditRequest {
        name: "web2".into(),
        description: "Servidor público".into(),
        restart: RestartSpec {
            name: RestartName::Always,
            max_retries: 7, // se ignora fuera de on-failure
        },
        cpus: Some(2.0),
        memory_bytes: None,
        memory_reservation_bytes: Some(64 * MIB),
        ..cur.clone()
    };
    let (c, kept) = diff(&cur, &des);
    let got: Vec<(&str, ChangeKind, Option<&str>)> = c
        .iter()
        .map(|x| (x.field.as_str(), x.kind, x.reason.as_deref()))
        .collect();
    assert_eq!(
        got,
        [
            ("name", ChangeKind::Direct, None),
            ("description", ChangeKind::Recreate, Some("labelsImmutable")),
            ("restart", ChangeKind::Direct, None),
            ("cpus", ChangeKind::Direct, None),
            ("memory", ChangeKind::Recreate, Some("limitRemoval")),
            ("memoryReservation", ChangeKind::Direct, None),
        ]
    );
    assert_eq!(kept, ["env", "ports", "mounts"]);
    assert_eq!(
        (c[2].before.as_str(), c[2].after.as_str()),
        ("no", "always")
    );
    assert_eq!((c[4].before.as_str(), c[4].after.as_str()), ("256m", ""));

    // on-failure con los mismos reintentos no es un cambio.
    let a = EditRequest {
        restart: RestartSpec {
            name: RestartName::No,
            max_retries: 3,
        },
        ..cur.clone()
    };
    assert!(diff(&cur, &a).0.is_empty());
}

#[test]
fn list_changes_are_itemised_and_secrets_masked() {
    let cur = current();
    let des = EditRequest {
        env: vec![
            KeyValue::new("MODE", "staging"),
            KeyValue::new("DB_PASSWORD", "otra"),
            KeyValue::new("API_TOKEN", "abc"),
            KeyValue::new("PLAIN", "v"),
        ],
        ports: vec![
            PortSpec {
                host_ip: "127.0.0.1".into(),
                host_port: Some(8081),
                container_port: 80,
                protocol: Protocol::Tcp,
            },
            PortSpec {
                host_ip: String::new(),
                host_port: None,
                container_port: 53,
                protocol: Protocol::Udp,
            },
        ],
        mounts: vec![
            cur.mounts[0].clone(),
            MountSpec {
                read_only: false,
                ..cur.mounts[1].clone()
            },
            MountSpec {
                kind: MountKind::Volume,
                source: "extra".into(),
                target: "/extra".into(),
                read_only: false,
            },
        ],
        ..cur.clone()
    };
    let (c, _) = diff(&cur, &des);
    assert!(c.iter().all(|x| x.kind == ChangeKind::Recreate));
    let env = &c[0];
    let rows: Vec<(&str, &str, Option<&str>, Option<&str>)> = env
        .items
        .iter()
        .map(|i| {
            (
                i.op.as_str(),
                i.label.as_str(),
                i.before.as_deref(),
                i.after.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("modified", "MODE", Some("prod"), Some("staging")),
            ("modified", "DB_PASSWORD", Some(REDACTED), Some(REDACTED)),
            ("added", "API_TOKEN", None, Some(REDACTED)),
            ("added", "PLAIN", None, Some("v")),
        ]
    );
    let text = serde_json::to_string(&c).unwrap();
    assert!(!text.contains("hunter2") && !text.contains("otra") && !text.contains("abc"));

    let ports: Vec<(&str, &str)> = c[1]
        .items
        .iter()
        .map(|i| (i.op.as_str(), i.label.as_str()))
        .collect();
    assert_eq!(ports, [("added", "53/udp"), ("modified", "80/tcp")]);
    assert_eq!(c[1].items[1].before.as_deref(), Some("8080:80/tcp"));
    assert_eq!(
        c[1].items[1].after.as_deref(),
        Some("127.0.0.1:8081:80/tcp")
    );

    let mounts: Vec<(&str, &str)> = c[2]
        .items
        .iter()
        .map(|i| (i.op.as_str(), i.label.as_str()))
        .collect();
    assert_eq!(
        mounts,
        [
            ("modified", "/www"),
            ("added", "/extra"),
            ("removed", "/logs"),
            ("removed", "/cache")
        ]
    );
}

#[test]
fn validation_only_reports_what_the_user_changed() {
    let cur = current();
    // /srv/www no existe en este equipo, pero no se toca: no bloquea un renombrado.
    let des = EditRequest {
        name: "nuevo".into(),
        ..cur.clone()
    };
    let (c, _) = diff(&cur, &des);
    assert!(validate(&cur, &des, &c, false, true).is_empty());

    let des = normalize(&EditRequest {
        name: " mal nombre ".into(),
        description: "x".repeat(MAX_DESCRIPTION + 1),
        env: vec![KeyValue::new("MAL CLAVE", "v"), KeyValue::new("OK", "v")],
        mounts: vec![
            cur.mounts[1].clone(),
            MountSpec {
                kind: MountKind::Bind,
                source: "/no/existe/dcc".into(),
                target: "/x".into(),
                read_only: false,
            },
            MountSpec {
                kind: MountKind::Volume,
                source: String::new(),
                target: "/anon".into(),
                read_only: false,
            },
        ],
        memory_bytes: Some(MIB),
        ..cur.clone()
    });
    let (c, _) = diff(&cur, &des);
    let issues = validate(&cur, &des, &c, false, true);
    let codes: Vec<(&str, &str)> = issues
        .iter()
        .map(|i| (i.field.as_str(), i.code.as_str()))
        .collect();
    assert!(codes.contains(&("name", "name.invalid")), "{codes:?}");
    assert!(
        codes.contains(&("env.0.key", "env.keyInvalid")),
        "{codes:?}"
    );
    assert!(
        codes.contains(&("mounts.1.source", "bind.missing")),
        "{codes:?}"
    );
    assert!(
        codes.contains(&("mounts.2.source", "volume.nameRequired")),
        "{codes:?}"
    );
    assert!(
        codes.contains(&("description", "description.tooLong")),
        "{codes:?}"
    );
    assert!(codes.iter().any(|(f, _)| *f == "memoryBytes"), "{codes:?}");
    // El montaje que ya existía (índice 0) no aparece.
    assert!(
        !codes.iter().any(|(f, _)| f.starts_with("mounts.0")),
        "{codes:?}"
    );

    for reserved in ["web-dcc-backup-20260101-000000", "x-dcc-replaced-1"] {
        let des = EditRequest {
            name: reserved.into(),
            ..cur.clone()
        };
        let (c, _) = diff(&cur, &des);
        assert!(validate(&cur, &des, &c, false, false)
            .iter()
            .any(|i| i.code == "name.backupReserved"));
    }
}

#[test]
fn update_body_keeps_swap_amount_and_cpu_style() {
    let c = inspect();
    let host = c.host_config.clone().unwrap();
    let cur = current();
    assert!(update_body(&host, &cur, &cur).is_none());

    let des = EditRequest {
        memory_bytes: Some(1024 * MIB),
        cpus: Some(0.5),
        restart: RestartSpec {
            name: RestartName::OnFailure,
            max_retries: 5,
        },
        ..cur.clone()
    };
    let (b, args) = update_body(&host, &cur, &des).unwrap();
    assert_eq!(b.memory, Some(1024 * MIB as i64));
    // Tenía 256 MiB de swap además de la memoria: se conservan.
    assert_eq!(b.memory_swap, Some(1280 * MIB as i64));
    assert_eq!(b.nano_cpus, Some(500_000_000));
    assert_eq!(b.cpu_quota, None);
    assert_eq!(b.restart_policy.unwrap().maximum_retry_count, Some(5));
    assert_eq!(
        args,
        [
            "--restart",
            "on-failure:5",
            "--cpus",
            "0.5",
            "--memory",
            "1g",
            "--memory-swap",
            "1280m"
        ]
    );

    // Contenedor creado con --cpu-quota: se actualiza con el mismo mecanismo.
    let quota = HostConfig {
        nano_cpus: None,
        cpu_quota: Some(50_000),
        cpu_period: Some(100_000),
        memory_swap: Some(-1),
        ..host.clone()
    };
    let mut ci = c.clone();
    ci.host_config = Some(quota.clone());
    let cur_q = current_request(&ci, Some(&image()));
    assert_eq!(cur_q.cpus, Some(0.5));
    let (b, args) = update_body(
        &quota,
        &cur_q,
        &EditRequest {
            cpus: Some(2.0),
            memory_bytes: Some(300 * MIB),
            ..cur_q.clone()
        },
    )
    .unwrap();
    assert_eq!(
        (b.nano_cpus, b.cpu_quota, b.cpu_period),
        (None, Some(200_000), Some(100_000))
    );
    assert_eq!(b.memory_swap, Some(-1), "swap ilimitado se conserva");
    assert!(args.contains(&"--cpu-quota".to_string()));

    // Quitar un límite no genera update (lo hace la recreación).
    assert!(update_body(
        &host,
        &cur,
        &EditRequest {
            cpus: None,
            memory_bytes: None,
            ..cur.clone()
        }
    )
    .is_none());
}

#[test]
fn recreate_body_without_changes_keeps_the_whole_configuration() {
    let c = inspect();
    let cur = current();
    let (body, nets) = recreate_body(&c, "nginx:1.27", &cur, &cur, &no_macs()).unwrap();
    let cfg = c.config.clone().unwrap();
    let host = body.host_config.clone().unwrap();
    assert_eq!(body.image.as_deref(), Some("nginx:1.27"));
    assert_eq!(body.env, cfg.env);
    assert_eq!(body.labels, cfg.labels);
    assert_eq!(body.cmd, cfg.cmd);
    // En la API son objetos JSON (conjuntos): el orden no importa.
    let sorted = |v: Option<Vec<String>>| {
        let mut v = v.unwrap_or_default();
        v.sort();
        v
    };
    assert_eq!(
        sorted(body.exposed_ports.clone()),
        sorted(cfg.exposed_ports.clone())
    );
    assert_eq!(sorted(body.volumes.clone()), sorted(cfg.volumes.clone()));
    // El nombre de host por defecto (ID corto) no se copia al contenedor nuevo.
    assert_eq!(body.hostname, None);
    // Montajes tal cual, con sus opciones; el anónimo, por su nombre.
    assert_eq!(
        host.binds.clone().unwrap(),
        ["data:/data", "/srv/www:/www:ro,z", "f00dfeed:/cache"]
    );
    let m = host.mounts.clone().unwrap();
    assert_eq!(m.len(), 2);
    assert!(m.iter().any(|x| x.typ == Some(MountType::TMPFS)));
    assert!(m.iter().any(|x| x.source.as_deref() == Some("logs")));
    // Lo que la aplicación no edita sigue ahí.
    let orig = c.host_config.clone().unwrap();
    assert_eq!(host.dns, orig.dns);
    assert_eq!(host.port_bindings, orig.port_bindings);
    assert_eq!(host.memory, orig.memory);
    assert_eq!(host.memory_swap, orig.memory_swap);
    assert_eq!(host.nano_cpus, orig.nano_cpus);
    assert_eq!(host.links.clone().unwrap(), ["db:database"]);
    // Red principal al crear, con IP fija y alias propios; la otra, después.
    let ep = &body.networking_config.unwrap().endpoints_config.unwrap()["front"];
    assert_eq!(
        ep.ipam_config.as_ref().unwrap().ipv4_address.as_deref(),
        Some("10.0.0.5")
    );
    assert_eq!(ep.aliases.clone().unwrap(), ["web"]);
    assert_eq!(ep.mac_address, None, "la MAC no se fija si no se pide");
    assert_eq!(ep.ip_address, None);
    assert_eq!(nets.connect.len(), 1);
    assert_eq!(nets.connect[0].0, "back");
    assert_eq!(nets.connect[0].1, EndpointSettings::default());
    assert!(nets.disconnect.is_empty());
}

#[test]
fn recreate_body_applies_only_the_edits() {
    let c = inspect();
    let cur = current();
    let des = EditRequest {
        description: "Web pública".into(),
        env: vec![
            KeyValue::new("MODE", "staging"),
            KeyValue::new("PATH", "/opt/bin"),
        ],
        ports: vec![PortSpec {
            host_ip: "127.0.0.1".into(),
            host_port: Some(9443),
            container_port: 443,
            protocol: Protocol::Tcp,
        }],
        mounts: vec![
            cur.mounts[0].clone(),
            MountSpec {
                read_only: false,
                ..cur.mounts[1].clone()
            },
            cur.mounts[3].clone(),
            MountSpec {
                kind: MountKind::Bind,
                source: "/etc/app".into(),
                target: "/conf".into(),
                read_only: true,
            },
        ],
        cpus: None,
        memory_bytes: Some(128 * MIB),
        restart: RestartSpec {
            name: RestartName::UnlessStopped,
            max_retries: 0,
        },
        ..cur.clone()
    };
    let macs = HashMap::from([("front".to_string(), "02:42:0a:00:00:05".to_string())]);
    let (body, _) = recreate_body(&c, "sha256:aaaa", &cur, &des, &macs).unwrap();
    let host = body.host_config.clone().unwrap();
    // Variables: las de la imagen se mantienen salvo la que el usuario redefine.
    assert_eq!(body.env.clone().unwrap(), ["MODE=staging", "PATH=/opt/bin"]);
    let labels = body.labels.clone().unwrap();
    assert_eq!(labels[DESCRIPTION_LABEL], "Web pública");
    assert_eq!(labels["team"], "ops");
    // Puertos: el 80 deja de publicarse y de exponerse; el 443 ya estaba expuesto.
    assert_eq!(body.exposed_ports.clone().unwrap(), ["443/tcp"]);
    let pb = host.port_bindings.clone().unwrap();
    assert_eq!(pb.len(), 1);
    assert_eq!(
        pb["443/tcp"].as_ref().unwrap()[0],
        PortBinding {
            host_ip: Some("127.0.0.1".into()),
            host_port: Some("9443".into())
        }
    );
    // Montajes: el modificado y el nuevo se declaran de nuevo; /logs desaparece.
    assert_eq!(
        host.binds.clone().unwrap(),
        [
            "data:/data",
            "/srv/www:/www",
            "f00dfeed:/cache",
            "/etc/app:/conf:ro"
        ]
    );
    let m = host.mounts.clone().unwrap();
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].typ, Some(MountType::TMPFS));
    // Recursos y reinicio.
    assert_eq!((host.nano_cpus, host.cpu_quota), (None, None));
    assert_eq!(host.memory, Some(128 * MIB as i64));
    assert_eq!(host.memory_swap, Some(384 * MIB as i64));
    assert_eq!(
        host.restart_policy.unwrap().name,
        Some(RestartPolicyNameEnum::UNLESS_STOPPED)
    );
    assert_eq!(host.dns.clone().unwrap(), ["9.9.9.9"]);
    // MAC conservada donde se pidió.
    let ep = &body.networking_config.unwrap().endpoints_config.unwrap()["front"];
    assert_eq!(ep.mac_address.as_deref(), Some("02:42:0a:00:00:05"));

    // Los secretos no salen en lo que se muestra o registra.
    let with_secret = EditRequest {
        env: vec![KeyValue::new("DB_PASSWORD", "nuevo-secreto")],
        ..cur.clone()
    };
    let (body, _) = recreate_body(&c, "x", &cur, &with_secret, &no_macs()).unwrap();
    assert!(body
        .env
        .clone()
        .unwrap()
        .contains(&"DB_PASSWORD=nuevo-secreto".to_string()));
    let shown = serde_json::to_string(&redacted_body(&body)).unwrap();
    assert!(!shown.contains("nuevo-secreto") && shown.contains("DB_PASSWORD=********"));
}

#[test]
fn network_plan_covers_special_modes() {
    let mut c = inspect();
    let cur = current();
    // La red del modo original ya no está conectada: se desconecta del nuevo.
    c.network_settings
        .as_mut()
        .unwrap()
        .networks
        .as_mut()
        .unwrap()
        .remove("front");
    let (body, nets) = recreate_body(&c, "x", &cur, &cur, &no_macs()).unwrap();
    assert!(body.networking_config.is_none());
    assert_eq!(nets.disconnect, ["front"]);
    assert_eq!(nets.connect.len(), 1);

    for mode in ["host", "none", "container:abc"] {
        let mut c = inspect();
        c.host_config.as_mut().unwrap().network_mode = Some(mode.into());
        c.config.as_mut().unwrap().hostname = Some("custom".into());
        let (body, nets) = recreate_body(&c, "x", &cur, &cur, &no_macs()).unwrap();
        assert!(body.networking_config.is_none(), "{mode}");
        assert_eq!(nets, NetworkPlan::default(), "{mode}");
        // Con la red de otro contenedor Docker no admite nombre de host propio.
        assert_eq!(
            body.hostname.is_none(),
            mode.starts_with("container:"),
            "{mode}"
        );
    }

    // En la red bridge por defecto Docker no admite alias.
    let mut c = inspect();
    c.host_config.as_mut().unwrap().network_mode = Some("default".into());
    c.network_settings.as_mut().unwrap().networks = Some(HashMap::from([(
        "bridge".to_string(),
        EndpointSettings {
            aliases: Some(vec!["x".into()]),
            ip_address: Some("172.17.0.2".into()),
            ..Default::default()
        },
    )]));
    let (body, nets) = recreate_body(&c, "x", &cur, &cur, &no_macs()).unwrap();
    assert!(body.networking_config.is_none());
    assert_eq!(nets, NetworkPlan::default());
}

#[test]
fn macs_are_kept_only_when_known_or_requested() {
    let mut c = inspect();
    assert!(macs_to_keep(&c, false).is_empty());
    assert_eq!(macs_to_keep(&c, true).len(), 2);
    c.state.as_mut().unwrap().status = Some(ContainerStateStatusEnum::CREATED);
    assert_eq!(
        macs_to_keep(&c, false).len(),
        2,
        "nunca iniciado: es la configurada"
    );
    c.state.as_mut().unwrap().status = Some(ContainerStateStatusEnum::EXITED);
    assert!(macs_to_keep(&c, true).is_empty());
}

#[test]
fn backup_names_are_unique_and_recognised() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-11-11T09:08:07Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let a = backup_name("web", BACKUP_INFIX, now, &[]);
    assert_eq!(a, "web-dcc-backup-20261111-090807");
    let b = backup_name("web", BACKUP_INFIX, now, std::slice::from_ref(&a));
    assert_eq!(b, "web-dcc-backup-20261111-090808");
    assert_eq!(backup_origin(&a), Some("web"));
    assert_eq!(
        backup_origin("a-dcc-backup-x-dcc-backup-20261111-090807"),
        Some("a-dcc-backup-x")
    );
    for bad in [
        "web",
        "web-dcc-backup-",
        "web-dcc-backup-2026",
        "-dcc-backup-20261111-090807",
        "web-dcc-backup-20261111x090807",
    ] {
        assert_eq!(backup_origin(bad), None, "{bad}");
    }
}

#[test]
fn port_conflicts_with_other_running_containers() {
    use crate::containers::{ContainerSummary, PortMapping};
    let other = |id: &str, state: &str, ip: &str, port: u16| ContainerSummary {
        id: id.into(),
        name: format!("c-{id}"),
        image: String::new(),
        image_id: String::new(),
        command: String::new(),
        created: 0,
        state: state.into(),
        status: String::new(),
        ports: vec![PortMapping {
            ip: Some(ip.into()),
            private_port: 80,
            public_port: Some(port),
            protocol: "tcp".into(),
        }],
        networks: vec![],
        mounts: 0,
        mount_points: vec![],
        compose_project: None,
        labels: Default::default(),
    };
    let list = [
        other("self", "running", "0.0.0.0", 8080),
        other("a", "running", "127.0.0.1", 9000),
        other("b", "exited", "0.0.0.0", 9001),
        other("c", "running", "10.0.0.1", 9002),
    ];
    let port = |ip: &str, p: u16, proto| PortSpec {
        host_ip: ip.into(),
        host_port: Some(p),
        container_port: 80,
        protocol: proto,
    };
    let desired = [
        port("", 8080, Protocol::Tcp), // el suyo: se libera al detenerlo
        port("", 9000, Protocol::Tcp), // choca (todas las IP frente a 127.0.0.1)
        port("", 9000, Protocol::Udp), // otro protocolo
        port("", 9001, Protocol::Tcp), // el otro está detenido
        port("10.0.0.2", 9002, Protocol::Tcp), // otra IP
        port("10.0.0.1", 9002, Protocol::Tcp), // misma IP
    ];
    let issues = port_conflicts(&desired, &list, "self");
    let fields: Vec<&str> = issues.iter().map(|i| i.field.as_str()).collect();
    assert_eq!(fields, ["ports.1.hostPort", "ports.5.hostPort"]);
    assert_eq!(issues[0].params["container"], "c-a");
}
