// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! E3 (1.3.0) contra Docker real (`DCC_DOCKER_TESTS=1`): la auditoría detecta
//! las comprobaciones en un contenedor peligroso, no encuentra nada en uno
//! legítimo y endurecido, y **solo lee**: todas las peticiones que llegan a
//! Docker son de lectura y los contenedores quedan igual.
//!
//! Todo lo creado lleva `dcc-test-audit-` en el nombre y se elimina al terminar.

mod common;

use std::collections::HashMap;

use bollard::models::{ContainerCreateBody, HostConfig, PortBinding};
use bollard::query_parameters::{
    CreateContainerOptions, InspectContainerOptions, RemoveContainerOptions,
};
use common::*;
use dcc_activity::ActivityLog;
use dcc_engine::audit::{AuditRequest, ImageVulns, TargetKind};
use dcc_engine::ConnectionState;
use tokio_util::sync::CancellationToken;

async fn create(
    docker: &bollard::Docker,
    name: &str,
    hc: HostConfig,
    user: Option<&str>,
    port: &str,
) {
    let _ = docker
        .remove_container(
            name,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;
    docker
        .create_container(
            Some(CreateContainerOptions {
                name: Some(name.to_owned()),
                ..Default::default()
            }),
            ContainerCreateBody {
                image: Some(fixture::IMAGE.into()),
                user: user.map(str::to_owned),
                exposed_ports: Some(vec![port.to_owned()]),
                labels: Some([("dcc.test".to_owned(), "true".to_owned())].into()),
                host_config: Some(hc),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    docker
        .start_container(
            name,
            None::<bollard::query_parameters::StartContainerOptions>,
        )
        .await
        .unwrap();
}

/// Elimina los contenedores de la prueba aunque falle.
struct Guard {
    docker: bollard::Docker,
    names: Vec<String>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        let d = self.docker.clone();
        let names = std::mem::take(&mut self.names);
        let _ = std::thread::spawn(move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async move {
                    for n in names {
                        let _ = d
                            .remove_container(
                                &n,
                                Some(RemoveContainerOptions {
                                    force: true,
                                    ..Default::default()
                                }),
                            )
                            .await;
                    }
                })
        })
        .join();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn audit_detects_dangerous_settings_ignores_hardened_ones_and_only_reads() {
    if !enabled() {
        eprintln!("omitida: define DCC_DOCKER_TESTS=1");
        return;
    }
    let sock = real_socket().await;
    let docker =
        bollard::Docker::connect_with_unix(&sock, 60, bollard::API_DEFAULT_VERSION).unwrap();
    fixture::ensure_image(&docker).await;
    let dir = tempfile::Builder::new()
        .prefix("dcc-test")
        .tempdir()
        .unwrap();
    let proxy = Proxy::start(&dir.path().join("p.sock"), sock.clone()).await;
    let log = ActivityLog::new(5000);
    let m = manager(log.clone(), proxy.path.display().to_string());
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    let e = m.engine().await.unwrap();
    let pid = std::process::id();
    let bad = format!("dcc-test-audit-bad-{pid}");
    let good = format!("dcc-test-audit-ok-{pid}");
    let _guard = Guard {
        docker: docker.clone(),
        names: vec![bad.clone(), good.clone()],
    };

    let mut ports = HashMap::new();
    ports.insert(
        "80/tcp".to_owned(),
        Some(vec![PortBinding {
            host_ip: Some(String::new()),
            host_port: Some(String::new()),
        }]),
    );
    create(
        &docker,
        &bad,
        HostConfig {
            cap_add: Some(vec!["SYS_ADMIN".into()]),
            pid_mode: Some("host".into()),
            security_opt: Some(vec!["seccomp=unconfined".into()]),
            binds: Some(vec![format!("{sock}:/var/run/docker.sock:ro")]),
            port_bindings: Some(ports),
            ..Default::default()
        },
        None,
        "80/tcp",
    )
    .await;
    let mut ports = HashMap::new();
    ports.insert(
        "80/tcp".to_owned(),
        Some(vec![PortBinding {
            host_ip: Some("127.0.0.1".into()),
            host_port: Some(String::new()),
        }]),
    );
    create(
        &docker,
        &good,
        HostConfig {
            cap_drop: Some(vec!["ALL".into()]),
            security_opt: Some(vec!["no-new-privileges:true".into()]),
            readonly_rootfs: Some(true),
            memory: Some(64 * 1024 * 1024),
            nano_cpus: Some(500_000_000),
            pids_limit: Some(50),
            port_bindings: Some(ports),
            ..Default::default()
        },
        Some("65534:65534"),
        "80/tcp",
    )
    .await;
    let before_bad = docker
        .inspect_container(&bad, None::<InspectContainerOptions>)
        .await
        .unwrap();
    let before_good = docker
        .inspect_container(&good, None::<InspectContainerOptions>)
        .await
        .unwrap();
    let requests_before = proxy.requests().len();

    // Peligroso.
    let req = |c: Option<&str>| AuditRequest {
        container: c.map(str::to_owned),
        include_compose: true,
        include_images: true,
        scan_vulnerabilities: true,
        tool: None,
    };
    let r = e
        .audit(
            "local",
            &req(Some(&bad)),
            None,
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();
    let codes: Vec<String> = r.findings.iter().map(|f| f.code.clone()).collect();
    for expected in [
        "risk.runtimeSocket",
        "risk.capability",
        "risk.hostNamespace",
        "risk.securityOpt",
        "risk.portAllInterfaces",
        "audit.rootUser",
        "audit.writableRootfs",
        "audit.noMemoryLimit",
        "audit.noCpuLimit",
        "audit.noPidsLimit",
        "audit.noNewPrivileges",
    ] {
        assert!(
            codes.iter().any(|c| c == expected),
            "falta {expected}: {codes:?}"
        );
    }
    assert_eq!(r.targets.len(), 1, "solo el contenedor pedido");
    assert!(r.counts["critical"] >= 2, "{:?}", r.counts);
    // Sin Trivy ni Grype en el entorno de pruebas: «no analizado», nunca 0.
    if e.ssh_session().is_some() {
        assert_eq!(
            r.vulnerabilities.not_analyzed.as_deref(),
            Some("audit.vuln.remote")
        );
    } else if dcc_engine::sys::find_executable("trivy").is_none()
        && dcc_engine::sys::find_executable("grype").is_none()
    {
        assert_eq!(
            r.vulnerabilities.not_analyzed.as_deref(),
            Some("audit.vuln.noTool")
        );
        assert!(r.vulnerabilities.images.is_empty());
    } else {
        assert!(r
            .vulnerabilities
            .images
            .iter()
            .all(|i| !matches!(i, ImageVulns::Done { .. })
                || r.vulnerabilities.tools.iter().any(|t| t.db_ready)));
    }

    // Legítimo y endurecido: ningún hallazgo.
    let r = e
        .audit(
            "local",
            &req(Some(&good)),
            None,
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();
    assert!(r.findings.is_empty(), "{:#?}", r.findings);

    // Todo: los dos y las imágenes, con progreso.
    let mut steps = Vec::new();
    let r = e
        .audit("local", &req(None), None, CancellationToken::new(), |p| {
            steps.push(p.step)
        })
        .await
        .unwrap();
    assert!(r.targets.iter().any(|t| t.target.name == bad));
    assert!(r.targets.iter().any(|t| t.target.name == good));
    assert!(r.targets.iter().any(|t| t.target.kind == TargetKind::Image));
    assert!(steps.contains(&"containers".to_owned()) && steps.contains(&"images".to_owned()));
    // Compose pedido sin CLI: se dice que se omitió.
    assert!(r
        .skipped
        .iter()
        .any(|s| s.reason == "audit.skipped.composeUnavailable"));
    assert!(!r.findings.iter().any(|f| f.target.name == good));

    // Solo lectura: ninguna petición que no sea GET o HEAD, y nada ha cambiado.
    let reqs = proxy.requests();
    let writes: Vec<_> = reqs[requests_before..]
        .iter()
        .filter(|(m, _)| m != "GET" && m != "HEAD")
        .collect();
    assert!(
        writes.is_empty(),
        "la auditoría hizo peticiones de escritura: {writes:?}"
    );
    let after_bad = docker
        .inspect_container(&bad, None::<InspectContainerOptions>)
        .await
        .unwrap();
    let after_good = docker
        .inspect_container(&good, None::<InspectContainerOptions>)
        .await
        .unwrap();
    assert_eq!(before_bad.host_config, after_bad.host_config);
    assert_eq!(before_good.host_config, after_good.host_config);
    assert_eq!(
        before_bad.state.as_ref().and_then(|s| s.started_at.clone()),
        after_bad.state.as_ref().and_then(|s| s.started_at.clone())
    );
    // Queda en el panel de actividad como una operación de solo lectura.
    assert!(log.snapshot().iter().any(|o| o.kind == "audit.run"));

    // Un contenedor que no existe.
    assert!(e
        .audit(
            "local",
            &req(Some("dcc-test-audit-no-existe")),
            None,
            CancellationToken::new(),
            |_| {}
        )
        .await
        .is_err());
    // Cancelar.
    let c = CancellationToken::new();
    c.cancel();
    assert!(e.audit("local", &req(None), None, c, |_| {}).await.is_err());

    proxy.stop();
}

/// Proyectos Compose: la auditoría lee `docker compose config` y revisa cada
/// servicio (sin desplegar nada). Un proyecto con archivos que no existen se
/// lista como no auditado.
#[tokio::test(flavor = "multi_thread")]
async fn audit_reviews_compose_services_without_deploying() {
    if !enabled() {
        eprintln!("omitida: define DCC_DOCKER_TESTS=1");
        return;
    }
    let sock = real_socket().await;
    let log = ActivityLog::new(2000);
    let m = manager(log.clone(), sock);
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    let Ok(cli) = m.compose_cli().await else {
        eprintln!("omitida: Docker Compose no está instalado");
        return;
    };
    let e = m.engine().await.unwrap();
    if e.ssh_session().is_some() {
        // Los archivos de la prueba están en este equipo; la auditoría remota
        // de Compose se comprueba con los proyectos del servidor (live_ssh).
        eprintln!("omitida por SSH: el archivo Compose de la prueba está en este equipo");
        return;
    }
    let dir = tempfile::Builder::new()
        .prefix("dcc-test-audit")
        .tempdir()
        .unwrap();
    let file = dir.path().join("compose.yaml");
    std::fs::write(
        &file,
        "name: dcctestaudit\nservices:\n  peligroso:\n    image: dcc-test/sleeper:2\n    privileged: true\n    pid: host\n  seguro:\n    image: dcc-test/sleeper:2\n    user: \"1000\"\n    read_only: true\n    mem_limit: 64m\n    cpus: 0.5\n    pids_limit: 50\n    security_opt: [\"no-new-privileges:true\"]\n",
    )
    .unwrap();
    let project = dcc_engine::ProjectRef {
        name: "dcctestaudit".into(),
        files: vec![file.display().to_string()],
        working_dir: None,
        env_files: vec![],
    };
    let missing = dcc_engine::ProjectRef {
        name: "dcctestfalta".into(),
        files: vec![dir.path().join("no-existe.yaml").display().to_string()],
        working_dir: None,
        env_files: vec![],
    };
    let req = AuditRequest {
        container: None,
        include_compose: true,
        include_images: false,
        scan_vulnerabilities: false,
        tool: None,
    };
    let r = e
        .audit(
            "local",
            &req,
            Some((cli, vec![project, missing])),
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();
    let svc = |n: &str| -> Vec<String> {
        r.findings
            .iter()
            .filter(|f| {
                f.target.kind == TargetKind::Service && f.target.id == format!("dcctestaudit/{n}")
            })
            .map(|f| f.code.clone())
            .collect()
    };
    let bad = svc("peligroso");
    assert!(bad.contains(&"risk.privileged".to_owned()), "{bad:?}");
    assert!(bad.contains(&"risk.hostNamespace".to_owned()), "{bad:?}");
    assert!(bad.contains(&"audit.rootUser".to_owned()), "{bad:?}");
    assert!(svc("seguro").is_empty(), "{:?}", svc("seguro"));
    assert!(r
        .targets
        .iter()
        .any(|t| t.target.id == "dcctestaudit/seguro"));
    assert!(r
        .skipped
        .iter()
        .any(|s| s.target.project.as_deref() == Some("dcctestfalta")
            && s.reason == "audit.skipped.composeConfig"));
    // Nada desplegado.
    let ps = e.list_containers().await.unwrap();
    assert!(!ps
        .iter()
        .any(|c| c.compose_project.as_deref() == Some("dcctestaudit")));
}
