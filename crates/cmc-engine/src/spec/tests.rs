// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
use super::*;

fn base() -> ContainerSpec {
    ContainerSpec {
        image: "nginx:1.27".into(),
        ..Default::default()
    }
}

fn codes(spec: &ContainerSpec) -> Vec<String> {
    spec.validate(true)
        .into_iter()
        .map(|i| format!("{}:{}", i.field, i.code))
        .collect()
}

fn full() -> ContainerSpec {
    ContainerSpec {
        name: "servidor-web".into(),
        image: "nginx:1.27".into(),
        command: vec!["nginx".into(), "-g".into(), "daemon off;".into()],
        entrypoint: vec![],
        working_dir: "/srv".into(),
        user: "1000:1000".into(),
        hostname: "web".into(),
        env: vec![
            KeyValue::new("MODO", "produccion"),
            KeyValue::new("VACIA", ""),
        ],
        labels: vec![KeyValue::new("equipo", "web")],
        network: "backend".into(),
        ports: vec![
            PortSpec {
                host_ip: String::new(),
                host_port: Some(8080),
                container_port: 80,
                protocol: Protocol::Tcp,
            },
            PortSpec {
                host_ip: "127.0.0.1".into(),
                host_port: None,
                container_port: 53,
                protocol: Protocol::Udp,
            },
        ],
        publish_all: false,
        mounts: vec![
            MountSpec {
                kind: MountKind::Volume,
                source: "datos".into(),
                target: "/data".into(),
                read_only: false,
            },
            MountSpec {
                kind: MountKind::Bind,
                source: "/tmp".into(),
                target: "/host-tmp".into(),
                read_only: true,
            },
        ],
        cpus: Some(1.5),
        memory_bytes: Some(512 * MIB),
        memory_reservation_bytes: Some(256 * MIB),
        restart: RestartSpec {
            name: RestartName::OnFailure,
            max_retries: 3,
        },
        privileged: false,
        auto_remove: false,
        init: true,
        tty: true,
        interactive: true,
        read_only_rootfs: true,
        cap_add: vec!["NET_ADMIN".into()],
        cap_drop: vec!["ALL".into()],
        extra_hosts: vec![KeyValue::new("api.local", "10.0.0.5")],
        accepted_risks: vec![],
    }
}

// ------------------------------------------------------------ validación

#[test]
fn minimal_spec_is_valid() {
    assert!(codes(&base()).is_empty());
    // NET_ADMIN es una capacidad peligrosa: aviso, no error (D-086).
    assert_eq!(
        codes(&full()),
        vec![
            "capAdd:risk.capability",
            "ports.0.hostIp:risk.portAllInterfaces"
        ]
    );
}

#[test]
fn image_and_name_are_validated() {
    assert_eq!(
        codes(&ContainerSpec::default()),
        vec!["image:image.required"]
    );
    for bad in ["nginx latest", "../x", "nginx:", "-x", "a?b", "a#b"] {
        let s = ContainerSpec {
            image: bad.into(),
            ..Default::default()
        };
        assert_eq!(codes(&s), vec!["image:image.invalid"], "{bad}");
    }
    for ok in [
        "nginx",
        "library/nginx:1.27",
        "ghcr.io/org/app:v1.2.3",
        "registry:5000/a/b@sha256:abc",
    ] {
        assert!(valid_image_ref(ok), "{ok}");
    }
    let s = ContainerSpec {
        name: "mal nombre".into(),
        ..base()
    };
    assert_eq!(codes(&s), vec!["name:name.invalid"]);
}

#[test]
fn ports_are_validated() {
    let p = |ip: &str, h: Option<u16>, c: u16| PortSpec {
        host_ip: ip.into(),
        host_port: h,
        container_port: c,
        protocol: Protocol::Tcp,
    };
    let s = ContainerSpec {
        ports: vec![
            p("", Some(8080), 80),
            p("", Some(8080), 81),
            p("999.1.1.1", None, 80),
            p("", Some(0), 0),
            p("", Some(80), 80),
        ],
        ..base()
    };
    let c = codes(&s);
    assert!(
        c.contains(&"ports.1.hostPort:port.duplicate".to_string()),
        "{c:?}"
    );
    assert!(c.contains(&"ports.2.hostIp:ip.invalid".to_string()));
    assert!(c.contains(&"ports.3.containerPort:port.invalid".to_string()));
    assert!(c.contains(&"ports.3.hostPort:port.invalid".to_string()));
    assert!(c.contains(&"ports.4.hostPort:port.privileged".to_string()));
    // Mismo puerto en distinta IP o protocolo no es duplicado.
    let s = ContainerSpec {
        ports: vec![p("127.0.0.1", Some(8080), 80), p("", Some(8080), 80)],
        ..base()
    };
    // E2 (D-088): sin IP, el puerto queda en todas las interfaces (aviso bajo).
    assert_eq!(codes(&s), vec!["ports.1.hostIp:risk.portAllInterfaces"]);
}

#[test]
fn mounts_are_validated() {
    let m = |k, s: &str, t: &str, ro| MountSpec {
        kind: k,
        source: s.into(),
        target: t.into(),
        read_only: ro,
    };
    let s = ContainerSpec {
        mounts: vec![
            m(MountKind::Bind, "/no/existe/cmc", "/a", false),
            m(MountKind::Bind, "relativa", "/b", false),
            m(MountKind::Bind, "", "/c", false),
            m(MountKind::Volume, "mal nombre", "/d", false),
            m(MountKind::Volume, "", "relativo", false),
            m(MountKind::Volume, "", "/", false),
            m(MountKind::Volume, "v", "/data", false),
            m(MountKind::Volume, "w", "/data/", false),
            m(
                MountKind::Bind,
                "/var/run/docker.sock",
                "/var/run/docker.sock",
                false,
            ),
        ],
        ..base()
    };
    let c = codes(&s);
    for want in [
        "mounts.0.source:bind.missing",
        "mounts.1.source:path.notAbsolute",
        "mounts.2.source:bind.required",
        "mounts.3.source:volume.invalid",
        "mounts.4.target:mount.targetInvalid",
        "mounts.5.target:mount.targetInvalid",
        "mounts.7.target:mount.targetDuplicate",
        "mounts.8.source:risk.runtimeSocket",
    ] {
        assert!(c.contains(&want.to_string()), "falta {want} en {c:?}");
    }
    // Sin comprobar el disco, una ruta inexistente no es error (vista previa).
    let s = ContainerSpec {
        mounts: vec![m(MountKind::Bind, "/no/existe/cmc", "/a", false)],
        ..base()
    };
    assert!(s.validate(false).is_empty());
    // E1 (D-086): /etc en solo lectura sigue exponiendo credenciales (aviso),
    // y el socket de Docker en solo lectura sigue dando control total.
    let s = ContainerSpec {
        mounts: vec![
            m(MountKind::Bind, "/etc", "/host-etc", true),
            m(
                MountKind::Bind,
                "/var/run/docker.sock",
                "/var/run/docker.sock",
                true,
            ),
            m(MountKind::Bind, "/etc/nginx", "/etc/nginx", true),
        ],
        ..base()
    };
    let c: Vec<String> = s
        .validate(false)
        .into_iter()
        .map(|i| format!("{}:{}", i.field, i.code))
        .collect();
    assert_eq!(
        c,
        vec![
            "mounts.0.source:risk.credentials",
            "mounts.1.source:risk.runtimeSocket"
        ]
    );
}

#[test]
fn resources_restart_and_security_are_validated() {
    let s = ContainerSpec {
        cpus: Some(0.001),
        memory_bytes: Some(MIB),
        memory_reservation_bytes: Some(2 * GIB),
        auto_remove: true,
        restart: RestartSpec {
            name: RestartName::Always,
            max_retries: 0,
        },
        privileged: true,
        cap_add: vec!["net_admin".into()],
        extra_hosts: vec![KeyValue::new("a:b", "no-ip")],
        env: vec![
            KeyValue::new("", "x"),
            KeyValue::new("A=B", "x"),
            KeyValue::new("X", "1"),
            KeyValue::new("X", "2"),
        ],
        ..base()
    };
    let c = codes(&s);
    for want in [
        "cpus:cpus.range",
        "memoryBytes:memory.min",
        "memoryReservationBytes:memory.reservationAboveLimit",
        "autoRemove:autoRemove.restart",
        "privileged:risk.privileged",
        "capAdd:risk.capability",
        "capAdd:cap.invalid",
        "extraHosts.0.key:host.invalid",
        "extraHosts.0.value:ip.invalid",
        "env.0.key:env.keyRequired",
        "env.1.key:env.keyInvalid",
        "env.3.key:env.duplicate",
    ] {
        assert!(c.contains(&want.to_string()), "falta {want} en {c:?}");
    }
    let sev = s.validate(true);
    assert!(sev
        .iter()
        .any(|i| i.code == "risk.privileged" && i.severity == Severity::Warning));
    assert!(sev
        .iter()
        .any(|i| i.code == "env.duplicate" && i.severity == Severity::Warning));
}

// ------------------------------------------------------------ API y CLI

#[test]
fn create_body_maps_every_field() {
    let (name, b) = full().to_create_body();
    assert_eq!(name.as_deref(), Some("servidor-web"));
    let v = serde_json::to_value(&b).unwrap();
    assert_eq!(v["Image"], "nginx:1.27");
    assert_eq!(v["Cmd"], serde_json::json!(["nginx", "-g", "daemon off;"]));
    assert!(v.get("Entrypoint").is_none());
    assert_eq!(v["Env"], serde_json::json!(["MODO=produccion", "VACIA="]));
    assert_eq!(v["Labels"]["equipo"], "web");
    assert_eq!(v["WorkingDir"], "/srv");
    assert_eq!(v["User"], "1000:1000");
    assert_eq!(v["Hostname"], "web");
    assert_eq!(v["Tty"], true);
    assert_eq!(v["OpenStdin"], true);
    assert!(v["ExposedPorts"].get("80/tcp").is_some());
    assert!(v["ExposedPorts"].get("53/udp").is_some());
    let h = &v["HostConfig"];
    assert_eq!(
        h["PortBindings"]["80/tcp"],
        serde_json::json!([{ "HostPort": "8080" }])
    );
    assert_eq!(
        h["PortBindings"]["53/udp"],
        serde_json::json!([{ "HostIp": "127.0.0.1", "HostPort": "" }])
    );
    assert_eq!(h["NetworkMode"], "backend");
    assert_eq!(
        h["Mounts"][0],
        serde_json::json!({ "Target": "/data", "Source": "datos", "Type": "volume" })
    );
    assert_eq!(
        h["Mounts"][1],
        serde_json::json!({ "Target": "/host-tmp", "Source": "/tmp", "Type": "bind", "ReadOnly": true })
    );
    assert_eq!(h["NanoCpus"], 1_500_000_000i64);
    assert_eq!(h["Memory"], 536_870_912i64);
    assert_eq!(h["MemoryReservation"], 268_435_456i64);
    assert_eq!(
        h["RestartPolicy"],
        serde_json::json!({ "Name": "on-failure", "MaximumRetryCount": 3 })
    );
    assert_eq!(h["Init"], true);
    assert_eq!(h["ReadonlyRootfs"], true);
    assert_eq!(h["CapAdd"], serde_json::json!(["NET_ADMIN"]));
    assert_eq!(h["CapDrop"], serde_json::json!(["ALL"]));
    assert_eq!(h["ExtraHosts"], serde_json::json!(["api.local:10.0.0.5"]));
    assert!(h.get("Privileged").is_none());
    assert!(h.get("AutoRemove").is_none());
}

#[test]
fn minimal_body_sends_only_what_is_needed() {
    let (name, b) = base().to_create_body();
    assert!(name.is_none());
    let v = serde_json::to_value(&b).unwrap();
    let mut keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, vec!["HostConfig", "Image"], "{v}");
    assert_eq!(
        v["HostConfig"],
        serde_json::json!({ "RestartPolicy": { "Name": "no" } })
    );
}

#[test]
fn cli_equivalent_is_exact_and_quoted() {
    assert_eq!(
        base().cli_command(true),
        "docker run --detach --restart no nginx:1.27"
    );
    assert_eq!(
        base().cli_command(false),
        "docker create --restart no nginx:1.27"
    );
    assert_eq!(
        full().cli_command(false),
        "docker create --name servidor-web --hostname web --user 1000:1000 --workdir /srv \
--env MODO=produccion --env VACIA= --label equipo=web --network backend \
--publish 8080:80/tcp --publish 127.0.0.1::53/udp \
--mount type=volume,source=datos,target=/data --mount type=bind,source=/tmp,target=/host-tmp,readonly \
--cpus 1.5 --memory 512m --memory-reservation 256m --restart on-failure:3 \
--cap-add NET_ADMIN --cap-drop ALL --add-host api.local:10.0.0.5 \
--init --tty --interactive --read-only nginx:1.27 nginx -g 'daemon off;'"
    );
    let s = ContainerSpec {
        env: vec![KeyValue::new("MSG", "hola mundo; $(id)")],
        ..base()
    };
    assert!(s
        .cli_command(true)
        .contains("--env 'MSG=hola mundo; $(id)'"));
    let s = ContainerSpec {
        ports: vec![PortSpec {
            host_ip: "::1".into(),
            host_port: Some(80),
            container_port: 80,
            protocol: Protocol::Tcp,
        }],
        ..base()
    };
    assert!(s.cli_command(true).contains("--publish '[::1]:80:80/tcp'"));
}

#[test]
fn multi_part_entrypoint_is_normalized_identically_in_api_and_cli() {
    let s = ContainerSpec {
        entrypoint: vec!["/bin/sh".into(), "-c".into()],
        command: vec!["echo hola".into()],
        ..base()
    };
    let (_, b) = s.to_create_body();
    assert_eq!(b.entrypoint, Some(vec!["/bin/sh".to_string()]));
    assert_eq!(b.cmd, Some(vec!["-c".to_string(), "echo hola".to_string()]));
    assert!(s
        .cli_command(true)
        .ends_with("--entrypoint /bin/sh nginx:1.27 -c 'echo hola'"));
}

#[test]
fn byte_and_cpu_formatting() {
    assert_eq!(format_bytes_cli(GIB), "1g");
    assert_eq!(format_bytes_cli(1536 * MIB), "1536m");
    assert_eq!(format_bytes_cli(7 * 1024), "7k");
    assert_eq!(format_bytes_cli(6_291_457), "6291457b");
    assert_eq!(format_cpus(1.0), "1");
    assert_eq!(format_cpus(0.25), "0.25");
    assert_eq!(format_cpus(2.0 / 3.0), "0.667");
}

// ------------------------------------------------------------ duplicado

/// Simula lo que devolvería inspect para el contenedor creado con `spec`
/// sobre una imagen con valores por defecto.
fn inspect_for(spec: &ContainerSpec, img: &ImageConfig) -> ContainerInspectResponse {
    let (name, body) = spec.to_create_body();
    let mut env = img.env.clone().unwrap_or_default();
    env.extend(body.env.clone().unwrap_or_default());
    let mut labels = img.labels.clone().unwrap_or_default();
    labels.extend(body.labels.clone().unwrap_or_default());
    let entry_set = body.entrypoint.is_some();
    let cfg = bollard::models::ContainerConfig {
        image: body.image.clone(),
        cmd: body
            .cmd
            .clone()
            .or_else(|| if entry_set { None } else { img.cmd.clone() }),
        entrypoint: body.entrypoint.clone().or_else(|| img.entrypoint.clone()),
        env: Some(env),
        labels: Some(labels),
        working_dir: body.working_dir.clone().or_else(|| img.working_dir.clone()),
        user: body.user.clone().or_else(|| img.user.clone()),
        hostname: body
            .hostname
            .clone()
            .or_else(|| Some("0123456789ab".into())),
        tty: body.tty,
        open_stdin: body.open_stdin,
        ..Default::default()
    };
    ContainerInspectResponse {
        id: Some("0123456789abcdef".into()),
        name: name.map(|n| format!("/{n}")),
        config: Some(cfg),
        host_config: body.host_config.clone(),
        ..Default::default()
    }
}

fn image_cfg() -> ImageConfig {
    ImageConfig {
        cmd: Some(vec!["nginx".into(), "-g".into(), "daemon off;".into()]),
        entrypoint: Some(vec!["/docker-entrypoint.sh".into()]),
        env: Some(vec!["PATH=/usr/bin".into(), "NGINX_VERSION=1.27".into()]),
        labels: Some([("maintainer".to_string(), "nginx".to_string())].into()),
        working_dir: Some(String::new()),
        user: Some(String::new()),
        ..Default::default()
    }
}

#[test]
fn duplicate_roundtrip_recovers_the_spec() {
    let mut original = full();
    original.command = vec!["nginx".into(), "-T".into()];
    let d = from_inspect(&inspect_for(&original, &image_cfg()), Some(&image_cfg()));
    assert_eq!(d.source_name, "servidor-web");
    let mut expected = original.clone();
    expected.name = String::new();
    expected.ports.sort_by_key(|p| {
        (
            p.container_port,
            p.protocol.as_str(),
            p.host_ip.clone(),
            p.host_port,
        )
    });
    assert_eq!(d.spec, expected);
    assert!(d.notes.contains(&"portsConflict".to_string()));
    assert!(d.notes.contains(&"namedVolumesShared".to_string()));
}

#[test]
fn duplicate_drops_image_defaults() {
    let d = from_inspect(&inspect_for(&base(), &image_cfg()), Some(&image_cfg()));
    assert!(d.spec.command.is_empty(), "{:?}", d.spec.command);
    assert!(d.spec.entrypoint.is_empty());
    assert!(d.spec.env.is_empty(), "{:?}", d.spec.env);
    assert!(d.spec.labels.is_empty());
    assert!(d.spec.hostname.is_empty());
    assert!(d.notes.is_empty(), "{:?}", d.notes);
}

#[test]
fn duplicate_keeps_command_when_entrypoint_changed() {
    let s = ContainerSpec {
        entrypoint: vec!["/bin/sh".into()],
        command: vec![],
        ..base()
    };
    let d = from_inspect(&inspect_for(&s, &image_cfg()), Some(&image_cfg()));
    assert_eq!(d.spec.entrypoint, vec!["/bin/sh"]);
    assert!(d.spec.command.is_empty());
}

#[test]
fn duplicate_notes_unsupported_settings_and_compose() {
    let mut i = inspect_for(&base(), &image_cfg());
    let h = i.host_config.as_mut().unwrap();
    h.dns = Some(vec!["1.1.1.1".into()]);
    h.binds = Some(vec!["/srv:/srv:ro".into(), "cache:/cache".into()]);
    h.network_mode = Some("bridge".into());
    i.config
        .as_mut()
        .unwrap()
        .labels
        .as_mut()
        .unwrap()
        .insert("com.docker.compose.project".into(), "blog".into());
    let d = from_inspect(&i, Some(&image_cfg()));
    assert!(d.notes.contains(&"dns".to_string()));
    assert!(d.notes.contains(&"composeLabels".to_string()));
    assert!(d
        .spec
        .labels
        .iter()
        .all(|l| !l.key.starts_with("com.docker.compose.")));
    assert_eq!(d.spec.network, "");
    assert_eq!(
        d.spec.mounts,
        vec![
            MountSpec {
                kind: MountKind::Bind,
                source: "/srv".into(),
                target: "/srv".into(),
                read_only: true
            },
            MountSpec {
                kind: MountKind::Volume,
                source: "cache".into(),
                target: "/cache".into(),
                read_only: false
            },
        ]
    );
    let d = from_inspect(&i, None);
    assert!(d.notes.contains(&"imageMissing".to_string()));
}

#[test]
fn redacted_hides_secret_values_only() {
    let spec = ContainerSpec {
        image: "nginx".into(),
        env: vec![
            KeyValue::new("POSTGRES_PASSWORD", "s3creto"),
            KeyValue::new("api_token", "abc"),
            KeyValue::new("EMPTY_SECRET", ""),
            KeyValue::new("MODO", "prod"),
        ],
        ..Default::default()
    };
    let (shown, n) = spec.redacted();
    assert_eq!(n, 2);
    assert_eq!(shown.env[0].value, REDACTED);
    assert_eq!(shown.env[1].value, REDACTED);
    assert_eq!(shown.env[2].value, "", "un valor vacío no oculta nada");
    assert_eq!(shown.env[3].value, "prod");
    let cmd = shown.cli_command(false);
    assert!(!cmd.contains("s3creto") && !cmd.contains("abc"), "{cmd}");
    assert!(cmd.contains("--env 'POSTGRES_PASSWORD=********'"), "{cmd}");
    // La especificación original no cambia.
    assert_eq!(spec.env[0].value, "s3creto");
    assert!(is_secret_key("my_private_key") && !is_secret_key("PATH"));
}

/// E1 (D-086, PROJECT_EVOLUTION_PLAN §9.5): configuraciones legítimas
/// habituales sin riesgos altos ni críticos (sin falsos positivos), y las
/// peligrosas equivalentes sí los tienen.
#[test]
fn legitimate_configurations_have_no_host_risks() {
    let bind = |s: &str, t: &str, ro: bool| MountSpec {
        kind: MountKind::Bind,
        source: s.into(),
        target: t.into(),
        read_only: ro,
    };
    let vol = |s: &str, t: &str| MountSpec {
        kind: MountKind::Volume,
        source: s.into(),
        target: t.into(),
        read_only: false,
    };
    let legit = [
        // nginx con su contenido en solo lectura.
        ContainerSpec {
            mounts: vec![bind("/home/ana/web/html", "/usr/share/nginx/html", true)],
            ports: vec![PortSpec {
                host_ip: "127.0.0.1".into(),
                host_port: Some(8080),
                container_port: 80,
                protocol: Protocol::Tcp,
            }],
            ..base()
        },
        // PostgreSQL con un volumen con nombre.
        ContainerSpec {
            image: "postgres:16".into(),
            mounts: vec![vol("pgdata", "/var/lib/postgresql/data")],
            ..base()
        },
        // Una aplicación con la carpeta del proyecto con escritura.
        ContainerSpec {
            mounts: vec![
                bind("/home/ana/proyecto", "/app", false),
                bind("/etc/localtime", "/etc/localtime", true),
            ],
            ..base()
        },
        // Mínimas capacidades.
        ContainerSpec {
            cap_drop: vec!["ALL".into()],
            cap_add: vec!["NET_BIND_SERVICE".into()],
            read_only_rootfs: true,
            ..base()
        },
        // Red propia.
        ContainerSpec {
            network: "backend".into(),
            ..base()
        },
    ];
    for s in &legit {
        let risks = s.risks(false);
        assert!(
            risks.iter().all(|r| !r.level.needs_acceptance()),
            "falso positivo: {risks:?}"
        );
        assert!(risks.is_empty(), "sin ningún aviso: {risks:?}");
    }
    let dangerous = [
        ContainerSpec {
            mounts: vec![bind("/var/run/docker.sock", "/var/run/docker.sock", true)],
            ..base()
        },
        ContainerSpec {
            privileged: true,
            ..base()
        },
        ContainerSpec {
            cap_drop: vec!["ALL".into()],
            cap_add: vec!["SYS_ADMIN".into()],
            ..base()
        },
        ContainerSpec {
            network: "host".into(),
            ..base()
        },
        ContainerSpec {
            mounts: vec![bind("/home/ana/.ssh", "/root/.ssh", true)],
            ..base()
        },
    ];
    for s in &dangerous {
        assert!(
            s.risks(false).iter().any(|r| r.level.needs_acceptance()),
            "debería exigir aceptación: {s:?}"
        );
    }
}
