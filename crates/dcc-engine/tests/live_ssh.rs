// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Administración remota contra servidores SSH y Docker Engines reales (0.10.0).
//!
//! Necesita `DCC_DOCKER_TESTS=1` y `DCC_SSH_LAB=<lab.json>`: tres servidores SSH
//! (A, B y C), cada uno con su Docker Engine, preparados por
//! `scripts/dev/remote-lab.sh`. No toca nada fuera de lo que crea.
//!
//! Se ejecuta con `--test-threads=1`: varias pruebas comprueban que no queda
//! ningún proceso `ssh` hijo de la prueba, y esa cuenta es de todo el proceso.

mod common;

use std::collections::HashSet;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use common::*;
use dcc_activity::ActivityLog;
use dcc_engine::manager::{EngineManager, Timing};
use dcc_engine::ssh::connect::TestReport;
use dcc_engine::ssh::hostkey::{self, TrustError};
use dcc_engine::ssh::{
    CheckId, CheckStatus, HostKeyInfo, RemoteDocker, RemoteProblem, Secret, SecretKind,
    SecretProvider, SshAuth, SshContext, SshProfile,
};
use dcc_engine::{ConnectionState, Problem};
use serde::Deserialize;

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Server {
    name: String,
    user: String,
    host: String,
    port: u16,
    socket: String,
    host_key: String,
}

#[derive(Deserialize, Clone)]
struct Password {
    server: usize,
    user: String,
    password: String,
}

#[derive(Deserialize, Clone)]
struct NoDocker {
    server: usize,
    user: String,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Lab {
    key: String,
    pass_key: String,
    passphrase: String,
    known_hosts: String,
    servers: Vec<Server>,
    password: Password,
    no_docker: NoDocker,
}

fn lab() -> Option<Lab> {
    if !enabled() {
        return None;
    }
    let Ok(path) = std::env::var("DCC_SSH_LAB") else {
        eprintln!("omitida: define DCC_SSH_LAB=<lab.json> (scripts/dev/remote-lab.sh)");
        return None;
    };
    Some(serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap())
}

/// Secretos fijos de una prueba; cuenta cuántas veces se piden.
struct Fixed {
    value: Option<String>,
    asked: std::sync::atomic::AtomicUsize,
}

impl Fixed {
    fn asked(&self) -> usize {
        self.asked.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl SecretProvider for Fixed {
    fn secret(&self, _: &str, _: SecretKind) -> Option<Secret> {
        self.asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.value.clone().map(Secret::new)
    }
}

fn fixed(v: Option<&str>) -> Arc<Fixed> {
    Arc::new(Fixed {
        value: v.map(str::to_owned),
        asked: Default::default(),
    })
}

/// Directorio de trabajo de una prueba: runtime privado y known_hosts propios.
struct Env {
    dir: tempfile::TempDir,
}

impl Env {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }
    fn known(&self) -> PathBuf {
        self.dir.path().join("known_hosts")
    }
    fn ctx(&self, secrets: Arc<dyn SecretProvider>) -> SshContext {
        ssh_context(&self.dir.path().join("rt"), self.known(), secrets)
    }
    /// known_hosts con las huellas ya confirmadas de los tres servidores.
    fn trusted(&self, lab: &Lab) -> &Self {
        std::fs::copy(&lab.known_hosts, self.known()).unwrap();
        self
    }
}

fn profile(
    lab: &Lab,
    i: usize,
    user: Option<&str>,
    auth: SshAuth,
    docker: RemoteDocker,
) -> SshProfile {
    let s = &lab.servers[i];
    SshProfile {
        id: format!("srv-{:08x}{:02x}", std::process::id(), i),
        name: format!("Servidor {}", s.name),
        host: s.host.clone(),
        port: s.port,
        user: user.unwrap_or(&s.user).to_owned(),
        description: String::new(),
        auth,
        docker,
        options: Default::default(),
    }
}

fn key(lab: &Lab) -> SshAuth {
    SshAuth::Key {
        path: lab.key.clone(),
    }
}

/// Huella real del servidor (de su clave pública, en el propio servidor).
fn real_fingerprint(s: &Server) -> HostKeyInfo {
    let out = std::process::Command::new("ssh-keygen")
        .args(["-l", "-f", &s.host_key])
        .output()
        .unwrap();
    hostkey::parse_fingerprints(&String::from_utf8_lossy(&out.stdout))
        .pop()
        .unwrap()
}

fn status(r: &TestReport, id: CheckId) -> CheckStatus {
    r.steps.iter().find(|s| s.id == id).unwrap().status
}

fn statuses(r: &TestReport) -> Vec<CheckStatus> {
    r.steps.iter().map(|s| s.status).collect()
}

/// Procesos `ssh` hijos de esta prueba.
fn ssh_children() -> usize {
    let me = std::process::id().to_string();
    std::fs::read_dir("/proc")
        .unwrap()
        .filter_map(|e| {
            let pid = e.ok()?.file_name().into_string().ok()?;
            let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
            let comm = stat.split('(').nth(1)?.split(')').next()?.to_owned();
            let ppid = stat
                .rsplit(')')
                .next()?
                .split_whitespace()
                .nth(1)?
                .to_owned();
            (comm == "ssh" && ppid == me).then_some(())
        })
        .count()
}

async fn wait_for(what: &str, secs: u64, mut f: impl FnMut() -> bool) {
    for _ in 0..secs * 10 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("no se cumplió a tiempo: {what}");
}

fn local(socket: &str) -> bollard::Docker {
    bollard::Docker::connect_with_unix(socket, 30, bollard::API_DEFAULT_VERSION).unwrap()
}

// ------------------------------------------------------------------ identidad

#[tokio::test(flavor = "multi_thread")]
async fn first_contact_shows_the_fingerprint_and_only_the_confirmed_key_is_trusted() {
    let Some(lab) = lab() else { return };
    let env = Env::new();
    let ctx = env.ctx(fixed(None));
    let p = profile(&lab, 0, None, key(&lab), RemoteDocker::Default);
    let log = ActivityLog::new(200);
    let real = real_fingerprint(&lab.servers[0]);

    let r = dcc_engine::ssh::connect::test(&ctx, &p, &log).await;
    assert_eq!(
        r.problem,
        Some(RemoteProblem::HostKeyUnknown {
            presented: Some(real.clone())
        })
    );
    assert_eq!(r.presented.as_ref(), Some(&real));
    assert_eq!(
        statuses(&r),
        [
            CheckStatus::Ok,
            CheckStatus::Ok,
            CheckStatus::Failed,
            CheckStatus::NotRun,
            CheckStatus::NotRun,
            CheckStatus::NotRun,
            CheckStatus::NotRun
        ]
    );
    assert!(r.steps[1]
        .detail
        .as_deref()
        .unwrap()
        .starts_with("SSH-2.0-"));
    assert!(!env.known().exists(), "no se guarda nada sin confirmar");

    // Una huella distinta de la que presenta el servidor no se guarda.
    let other = real_fingerprint(&lab.servers[1]);
    match hostkey::trust(&ctx.paths, &p, &other, false).await {
        Err(TrustError::Mismatch { presented }) => assert_eq!(presented, vec![real.clone()]),
        other => panic!("{other:?}"),
    }
    assert!(std::fs::read_to_string(env.known())
        .unwrap_or_default()
        .trim()
        .is_empty());

    // La confirmada sí, con permisos 600, y ya conecta en los siete pasos.
    assert_eq!(
        hostkey::trust(&ctx.paths, &p, &real, false).await.unwrap(),
        real
    );
    assert_eq!(
        std::fs::metadata(env.known()).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let trusted = hostkey::trusted(&ctx.paths, &p).await;
    assert_eq!(trusted.len(), 1);
    assert!(trusted[0].in_app_file && trusted[0].key == real);
    let r = dcc_engine::ssh::connect::test(&ctx, &p, &log).await;
    assert_eq!(r.problem, None, "{:?}", r.steps);
    assert!(
        statuses(&r).iter().all(|s| *s == CheckStatus::Ok),
        "{:?}",
        r.steps
    );
    let version = local(&lab.servers[0].socket)
        .version()
        .await
        .unwrap()
        .version
        .unwrap();
    assert_eq!(r.engine.unwrap().version, version);
    assert!(r.latency_ms.is_some());
    assert_eq!(ssh_children(), 0, "la prueba no deja procesos");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_changed_identity_is_refused_and_never_retried_on_its_own() {
    let Some(lab) = lab() else { return };
    let env = Env::new();
    // known_hosts con la clave de B en el lugar de A: identidad cambiada.
    let b_line = std::fs::read_to_string(&lab.known_hosts)
        .unwrap()
        .lines()
        .find(|l| l.starts_with("[127.0.0.1]:2202"))
        .unwrap()
        .replace("[127.0.0.1]:2202", "[127.0.0.1]:2201");
    std::fs::write(env.known(), format!("{b_line}\n")).unwrap();
    let ctx = env.ctx(fixed(None));
    let p = profile(&lab, 0, None, key(&lab), RemoteDocker::Default);
    let real = real_fingerprint(&lab.servers[0]);
    let log = ActivityLog::new(200);

    let r = dcc_engine::ssh::connect::test(&ctx, &p, &log).await;
    match &r.problem {
        Some(RemoteProblem::HostKeyChanged {
            presented,
            file,
            in_user_file,
        }) => {
            assert_eq!(presented.as_ref(), Some(&real));
            assert_eq!(file.as_deref(), Some(env.known().to_str().unwrap()));
            assert!(!in_user_file);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(status(&r, CheckId::Identity), CheckStatus::Failed);
    assert_eq!(status(&r, CheckId::Auth), CheckStatus::NotRun);

    // El gestor no reintenta solo, aunque el monitor sea rápido.
    let m = EngineManager::remote(
        log.clone(),
        p.clone(),
        ctx.clone(),
        Timing {
            heartbeat: Duration::from_millis(300),
            retry_min: Duration::from_millis(200),
            retry_max: Duration::from_millis(400),
        },
    );
    let monitor = m.spawn_monitor();
    let state = m.connect().await;
    assert!(matches!(
        &state,
        ConnectionState::Disconnected { diagnosis, .. }
            if matches!(&diagnosis.problem, Problem::Remote { remote: RemoteProblem::HostKeyChanged { .. }, .. })
    ));
    let probes = log.snapshot().len();
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert_eq!(log.snapshot().len(), probes, "no hubo reintentos");
    assert!(matches!(m.state(), ConnectionState::Disconnected { .. }));

    // Revisión explícita: se sustituye solo tras confirmar la huella nueva.
    let other = real_fingerprint(&lab.servers[1]);
    assert!(matches!(
        hostkey::trust(&ctx.paths, &p, &other, true).await,
        Err(TrustError::Mismatch { .. })
    ));
    assert_eq!(
        std::fs::read_to_string(env.known()).unwrap().trim(),
        b_line.trim(),
        "nada cambió"
    );
    hostkey::trust(&ctx.paths, &p, &real, true).await.unwrap();
    assert!(!Path::new(&format!("{}.old", env.known().display())).exists());
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    m.disconnect().await;
    monitor.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn an_old_key_in_the_users_known_hosts_is_never_modified() {
    let Some(lab) = lab() else { return };
    let env = Env::new();
    let mut ctx = env.ctx(fixed(None));
    let user_file = env.dir.path().join("user_known_hosts");
    let b_line = std::fs::read_to_string(&lab.known_hosts)
        .unwrap()
        .lines()
        .find(|l| l.starts_with("[127.0.0.1]:2202"))
        .unwrap()
        .replace("[127.0.0.1]:2202", "[127.0.0.1]:2201");
    std::fs::write(&user_file, format!("{b_line}\n")).unwrap();
    ctx.paths.user_known_hosts = user_file.clone();
    let p = profile(&lab, 0, None, key(&lab), RemoteDocker::Default);
    let r = dcc_engine::ssh::connect::test(&ctx, &p, &ActivityLog::new(50)).await;
    assert!(
        matches!(
            r.problem,
            Some(RemoteProblem::HostKeyChanged {
                in_user_file: true,
                ..
            })
        ),
        "{:?}",
        r.problem
    );
    let real = real_fingerprint(&lab.servers[0]);
    match hostkey::trust(&ctx.paths, &p, &real, true).await {
        Err(TrustError::InUserFile { pattern, .. }) => assert_eq!(pattern, "[127.0.0.1]:2201"),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        std::fs::read_to_string(&user_file).unwrap().trim(),
        b_line.trim()
    );
}

// ----------------------------------------------------------- autenticación

#[tokio::test(flavor = "multi_thread")]
async fn authentication_methods_and_failures() {
    let Some(lab) = lab() else { return };
    let env = Env::new();
    env.trusted(&lab);
    let log = ActivityLog::new(500);

    // Usuario no admitido en ese servidor.
    let ctx = env.ctx(fixed(None));
    let p = profile(&lab, 0, Some("dccc"), key(&lab), RemoteDocker::Default);
    let r = dcc_engine::ssh::connect::test(&ctx, &p, &log).await;
    assert!(
        matches!(r.problem, Some(RemoteProblem::AuthFailed { .. })),
        "{:?}",
        r.problem
    );
    assert_eq!(status(&r, CheckId::Identity), CheckStatus::Ok);
    assert_eq!(status(&r, CheckId::Auth), CheckStatus::Failed);

    // Clave que no existe: no se intenta conectar.
    let p = profile(
        &lab,
        0,
        None,
        SshAuth::Key {
            path: "/nonexistent/id".into(),
        },
        RemoteDocker::Default,
    );
    let r = dcc_engine::ssh::connect::test(&ctx, &p, &log).await;
    assert_eq!(
        r.problem,
        Some(RemoteProblem::KeyUnreadable {
            path: "/nonexistent/id".into()
        })
    );

    // Clave con frase de paso: sin frase, frase incorrecta y correcta.
    let pk = SshAuth::KeyPassphrase {
        path: lab.pass_key.clone(),
    };
    let p = profile(&lab, 0, None, pk.clone(), RemoteDocker::Default);
    let none = fixed(None);
    let r = dcc_engine::ssh::connect::test(&env.ctx(none.clone()), &p, &log).await;
    assert_eq!(
        r.problem,
        Some(RemoteProblem::SecretRequired {
            secret: SecretKind::Passphrase
        })
    );
    assert_eq!(none.asked(), 1);
    let r = dcc_engine::ssh::connect::test(&env.ctx(fixed(Some("frase mala"))), &p, &log).await;
    assert!(
        matches!(r.problem, Some(RemoteProblem::AuthFailed { .. })),
        "{:?}",
        r.problem
    );
    let r = dcc_engine::ssh::connect::test(&env.ctx(fixed(Some(&lab.passphrase))), &p, &log).await;
    assert_eq!(r.problem, None, "{:?}", r.steps);

    // Contraseña: incorrecta (un solo intento) y correcta.
    let pw = &lab.password;
    let p = profile(
        &lab,
        pw.server,
        Some(&pw.user),
        SshAuth::Password,
        RemoteDocker::Default,
    );
    let r = dcc_engine::ssh::connect::test(&env.ctx(fixed(Some("no es esta"))), &p, &log).await;
    assert!(
        matches!(r.problem, Some(RemoteProblem::AuthFailed { .. })),
        "{:?}",
        r.problem
    );
    let r = dcc_engine::ssh::connect::test(&env.ctx(fixed(Some(&pw.password))), &p, &log).await;
    assert_eq!(r.problem, None, "{:?}", r.steps);

    // Ningún secreto aparece en la actividad, en los pasos ni en los problemas.
    let dump = serde_json::to_string(&log.snapshot()).unwrap();
    for secret in [
        lab.passphrase.as_str(),
        "frase mala",
        pw.password.as_str(),
        "no es esta",
    ] {
        assert!(!dump.contains(secret), "secreto en la actividad: {secret}");
    }
    assert!(!format!("{r:?}").contains(&pw.password));
    // El socket de askpass no sobrevive a la sesión.
    let rt = env.dir.path().join("rt");
    let left: Vec<_> = std::fs::read_dir(&rt)
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert!(left.is_empty(), "{left:?}");
    assert_eq!(ssh_children(), 0);
}

// ---------------------------------------------------- Docker en el servidor

#[tokio::test(flavor = "multi_thread")]
async fn problems_with_docker_on_the_server_are_told_apart() {
    let Some(lab) = lab() else { return };
    let env = Env::new();
    env.trusted(&lab);
    let ctx = env.ctx(fixed(None));
    let log = ActivityLog::new(500);
    let t = |p: SshProfile| {
        let ctx = ctx.clone();
        let log = log.clone();
        async move { dcc_engine::ssh::connect::test(&ctx, &p, &log).await }
    };

    let nd = &lab.no_docker;
    let r = t(profile(
        &lab,
        nd.server,
        Some(&nd.user),
        key(&lab),
        RemoteDocker::Default,
    ))
    .await;
    assert_eq!(r.problem, Some(RemoteProblem::DockerMissing));
    assert_eq!(status(&r, CheckId::Auth), CheckStatus::Ok);
    assert_eq!(status(&r, CheckId::Docker), CheckStatus::Failed);

    // B no tiene permiso sobre el socket de A.
    let r = t(profile(
        &lab,
        1,
        None,
        key(&lab),
        RemoteDocker::Socket {
            path: lab.servers[0].socket.clone(),
        },
    ))
    .await;
    assert!(
        matches!(r.problem, Some(RemoteProblem::SocketDenied { .. })),
        "{:?}",
        r.problem
    );
    assert_eq!(status(&r, CheckId::Docker), CheckStatus::Ok);
    assert_eq!(status(&r, CheckId::Permission), CheckStatus::Failed);

    let r = t(profile(
        &lab,
        1,
        None,
        key(&lab),
        RemoteDocker::Socket {
            path: "/run/dcc-nope/docker.sock".into(),
        },
    ))
    .await;
    assert!(
        matches!(r.problem, Some(RemoteProblem::DockerNotRunning { .. })),
        "{:?}",
        r.problem
    );

    let r = t(profile(
        &lab,
        1,
        None,
        key(&lab),
        RemoteDocker::Context {
            name: "no-existe".into(),
        },
    ))
    .await;
    assert_eq!(
        r.problem,
        Some(RemoteProblem::ContextMissing {
            name: "no-existe".into()
        })
    );

    let mut p = profile(&lab, 1, None, key(&lab), RemoteDocker::Default);
    p.port = 2299;
    let r = t(p).await;
    assert_eq!(r.problem, Some(RemoteProblem::SshRefused { port: 2299 }));
    assert_eq!(status(&r, CheckId::Reach), CheckStatus::Failed);

    let mut p = profile(&lab, 1, None, key(&lab), RemoteDocker::Default);
    p.host = "servidor-que-no-existe.invalid".into();
    let r = t(p).await;
    assert!(
        matches!(r.problem, Some(RemoteProblem::HostUnresolvable { .. })),
        "{:?}",
        r.problem
    );
    assert_eq!(status(&r, CheckId::Resolve), CheckStatus::Failed);

    // El socket rootless/alternativo correcto sí funciona.
    let r = t(profile(
        &lab,
        1,
        None,
        key(&lab),
        RemoteDocker::Socket {
            path: lab.servers[1].socket.clone(),
        },
    ))
    .await;
    assert_eq!(r.problem, None, "{:?}", r.steps);
    assert_eq!(ssh_children(), 0);
}

// ------------------------------------------------------------ multiservidor

#[tokio::test(flavor = "multi_thread")]
async fn three_servers_at_once_stay_isolated() {
    let Some(lab) = lab() else { return };
    let env = Env::new();
    env.trusted(&lab);
    let ctx = env.ctx(fixed(None));
    let log = ActivityLog::new(2000);
    let managers: Vec<EngineManager> = (0..3)
        .map(|i| {
            EngineManager::remote(
                log.clone(),
                profile(&lab, i, None, key(&lab), RemoteDocker::Default),
                ctx.clone(),
                Timing::default(),
            )
        })
        .collect();
    for m in &managers {
        assert!(
            matches!(m.state(), ConnectionState::Idle),
            "empieza sin conectar"
        );
        assert!(matches!(
            m.connect().await,
            ConnectionState::Connected { .. }
        ));
    }
    // Cada uno habla con su propio Docker Engine.
    for (i, m) in managers.iter().enumerate() {
        let expected = local(&lab.servers[i].socket)
            .version()
            .await
            .unwrap()
            .version
            .unwrap();
        let ConnectionState::Connected {
            engine,
            endpoint,
            latency_ms,
            ..
        } = m.state()
        else {
            panic!()
        };
        assert_eq!(engine.version, expected, "servidor {}", lab.servers[i].name);
        assert_eq!(
            endpoint.socket_path,
            format!(
                "ssh://{}@127.0.0.1:{}",
                lab.servers[i].user, lab.servers[i].port
            )
        );
        assert!(latency_ms.is_some());
    }
    // Un contenedor creado en B solo existe en B.
    fixture::ensure_image(&local(&lab.servers[1].socket)).await;
    let name = format!("dcc-test-iso-{}", std::process::id());
    let b = managers[1].engine().await.unwrap();
    let spec = dcc_engine::spec::ContainerSpec {
        image: fixture::IMAGE.into(),
        name: name.clone(),
        labels: vec![dcc_engine::spec::KeyValue::new("dcc.test", "true")],
        ..Default::default()
    };
    let created = b.create_container(&spec).await.expect("crear en B");
    let exists = |socket: String, id: String| async move {
        local(&socket).inspect_container(&id, None).await.is_ok()
    };
    assert!(exists(lab.servers[1].socket.clone(), created.id.clone()).await);
    assert!(!exists(lab.servers[0].socket.clone(), name.clone()).await);
    assert!(!exists(lab.servers[2].socket.clone(), name.clone()).await);
    let names = |m: &EngineManager| {
        let m = m.clone();
        async move {
            m.engine()
                .await
                .unwrap()
                .list_containers()
                .await
                .unwrap()
                .into_iter()
                .map(|c| c.name)
                .collect::<HashSet<_>>()
        }
    };
    assert!(names(&managers[1]).await.contains(&name));
    assert!(!names(&managers[0]).await.contains(&name));
    assert!(!names(&managers[2]).await.contains(&name));
    // La actividad identifica el servidor de cada operación.
    let rec = log
        .snapshot()
        .into_iter()
        .find(|r| r.kind == "container.create")
        .unwrap();
    match &rec.transport {
        dcc_activity::Transport::Api { endpoint, .. } => {
            assert_eq!(endpoint, "ssh://dccb@127.0.0.1:2202")
        }
        other => panic!("{other:?}"),
    }
    assert!(rec
        .equivalent_command
        .as_deref()
        .unwrap()
        .starts_with("docker --host ssh://dccb@127.0.0.1:2202 create"));

    // Desconectar B no afecta a A ni a C; B vuelve a conectar.
    managers[1].disconnect().await;
    assert!(matches!(managers[1].state(), ConnectionState::Idle));
    assert!(managers[1].engine().await.is_err());
    assert!(!names(&managers[0]).await.contains(&name));
    assert!(managers[2].engine().await.unwrap().ping().await.is_ok());
    assert!(matches!(
        managers[1].connect().await,
        ConnectionState::Connected { .. }
    ));
    let b = managers[1].engine().await.unwrap();
    b.remove_container(&created.id, Some(&name), true, true)
        .await
        .unwrap();
    assert!(!exists(lab.servers[1].socket.clone(), created.id.clone()).await);
    for m in &managers {
        m.disconnect().await;
    }
    drop(b);
    wait_for("cerrar todos los procesos ssh", 10, || ssh_children() == 0).await;
}

// ---------------------------------------------- canales, pérdida y limpieza

#[tokio::test(flavor = "multi_thread")]
async fn a_lost_connection_is_detected_and_restored_and_nothing_is_left_behind() {
    let Some(lab) = lab() else { return };
    let env = Env::new();
    env.trusted(&lab);
    let log = ActivityLog::new(500);
    let m = EngineManager::remote(
        log.clone(),
        profile(&lab, 2, None, key(&lab), RemoteDocker::Default),
        env.ctx(fixed(None)),
        Timing {
            heartbeat: Duration::from_millis(500),
            retry_min: Duration::from_millis(300),
            retry_max: Duration::from_secs(1),
        },
    );
    let monitor = m.spawn_monitor();
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    let first = m.engine().await.unwrap();
    let control = first.ssh_session().unwrap().control_path().to_owned();
    // Se pierde la conexión maestra (como si se cayera la red).
    let pid = std::process::Command::new("pgrep")
        .args(["-f", &format!("ControlPath={}", control.display())])
        .output()
        .unwrap();
    let pid = String::from_utf8_lossy(&pid.stdout)
        .lines()
        .next()
        .unwrap()
        .to_owned();
    std::process::Command::new("kill")
        .arg(&pid)
        .status()
        .unwrap();
    drop(first);
    // El monitor lo detecta y restablece la conexión con una sesión nueva.
    let mut restored = false;
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if let Ok(e) = m.engine().await {
            if e.ssh_session()
                .map(|s| s.control_path() != control)
                .unwrap_or(false)
                && e.ping().await.is_ok()
            {
                restored = true;
                break;
            }
        }
    }
    assert!(restored, "no se restableció: {:?}", m.state());
    m.disconnect().await;
    monitor.abort();
    wait_for("cerrar todos los procesos ssh", 10, || ssh_children() == 0).await;
    let rt = env.dir.path().join("rt");
    let left: Vec<_> = std::fs::read_dir(&rt)
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert!(left.is_empty(), "{left:?}");
}

/// Justifica el único filtro del modo SSH de las demás pruebas reales.
#[tokio::test(flavor = "multi_thread")]
async fn remote_cli_negotiates_with_head_ping_only() {
    let Some(lab) = lab() else { return };
    let env = Env::new();
    env.trusted(&lab);
    let dir = tempfile::tempdir().unwrap();
    let proxy = Proxy::start(&dir.path().join("p.sock"), lab.servers[0].socket.clone()).await;
    expose_for_remote(&proxy.path);
    let log = ActivityLog::new(200);
    let m = EngineManager::remote(
        log.clone(),
        profile(
            &lab,
            0,
            None,
            key(&lab),
            RemoteDocker::Socket {
                path: proxy.path.display().to_string(),
            },
        ),
        env.ctx(fixed(None)),
        Timing::default(),
    );
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    let e = m.engine().await.unwrap();
    e.ping().await.unwrap();
    let all = proxy.all_requests();
    let ours = recorded(&log);
    assert!(
        all.iter().any(|(m, p)| m == "HEAD" && p == "/_ping"),
        "{all:?}"
    );
    assert!(
        !ours.iter().any(|(m, _)| m == "HEAD"),
        "el motor nunca envía HEAD"
    );
    let without: Vec<_> = all
        .into_iter()
        .filter(|(m, p)| !(m == "HEAD" && p == "/_ping"))
        .collect();
    // `docker --version` y la sonda de identidad no pasan por la API.
    assert_eq!(without, ours);
    drop(e);
    m.disconnect().await;
}

/// Si la aplicación terminó sin cerrar sus sesiones (cierre forzado), al
/// arrancar se cierran solo las de instancias muertas; las de una instancia
/// viva no se tocan.
#[tokio::test(flavor = "multi_thread")]
async fn orphaned_sessions_of_dead_instances_are_closed_at_startup() {
    let Some(lab) = lab() else { return };
    let env = Env::new();
    env.trusted(&lab);
    let ctx = env.ctx(fixed(None));
    let p = profile(&lab, 2, None, key(&lab), RemoteDocker::Default);
    dcc_engine::ssh::askpass::private_dir(&ctx.paths.runtime_dir).unwrap();
    // PID de un proceso que ya terminó.
    let mut gone = std::process::Command::new("true").spawn().unwrap();
    let dead = gone.id();
    gone.wait().unwrap();
    // Maestra «huérfana»: desligada de esta prueba (como si su instancia
    // hubiera muerto), con su socket de control a nombre del PID muerto.
    let orphan_sock = ctx.paths.runtime_dir.join(format!("c-{dead}-orphan-aa"));
    let st = std::process::Command::new("setsid")
        .arg("-f")
        .arg(&ctx.paths.ssh)
        .args(dcc_engine::ssh::args::master_args(
            &p,
            &ctx.paths,
            &orphan_sock,
        ))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(st.success());
    let running = || {
        std::process::Command::new("pgrep")
            .args(["-f", &orphan_sock.display().to_string()])
            .output()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false)
    };
    wait_for("socket de la huérfana", 15, || orphan_sock.exists()).await;
    // Sesión de una instancia viva (esta).
    let live = dcc_engine::ssh::SshSession::open(&ctx, &p).await.unwrap();
    assert!(live
        .control_path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with(&format!("c-{}-", std::process::id())));

    assert_eq!(dcc_engine::ssh::session::close_orphans(&ctx.paths).await, 1);
    wait_for("la sesión huérfana termina", 5, || !running()).await;
    assert!(!orphan_sock.exists());
    assert!(live.alive().await, "la sesión viva sigue");
    assert_eq!(dcc_engine::ssh::session::close_orphans(&ctx.paths).await, 0);
    live.close().await;
}

// ------------------------------------------------------------------ Compose remoto

/// Docker Compose en el servidor (D-080): se detecta allí, los archivos son
/// los del servidor (uno del equipo no cuenta), las acciones afectan solo a
/// ese servidor, cancelar los registros no deja procesos en el servidor y
/// el guardado se niega si el archivo cambió.
#[tokio::test(flavor = "multi_thread")]
async fn compose_runs_on_the_server_with_server_files() {
    use dcc_engine::compose::project::{ComposeAction, ComposeCli, ProjectRef};
    use dcc_engine::compose::remote_file;
    use dcc_engine::compose::ComposeStatus;
    let Some(lab) = lab() else { return };
    let env = Env::new();
    env.trusted(&lab);
    let ctx = env.ctx(fixed(None));
    let log = ActivityLog::new(2000);
    let b = &lab.servers[1];
    fixture::ensure_image(&local(&b.socket)).await;
    let m = EngineManager::remote(
        log.clone(),
        profile(&lab, 1, None, key(&lab), RemoteDocker::Default),
        ctx.clone(),
        Timing::default(),
    );
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    let e = m.engine().await.unwrap();
    let session = e.ssh_session().unwrap().clone();

    // Compose del servidor.
    let status = m.compose_status().await;
    let ComposeStatus::Available { program, .. } = &status else {
        panic!("{status:?}")
    };
    assert_eq!(
        program, "docker",
        "el programa del servidor, no una ruta local"
    );
    let cli: ComposeCli = m.compose_cli().await.unwrap();

    // Carpeta de trabajo en el servidor (preparación de la prueba).
    let pid = std::process::id();
    let dir = format!("/home/{}/dcc-compose-{pid}/con ' comilla", b.user);
    session
        .run(
            &["mkdir".into(), "-p".into(), "--".into(), dir.clone()],
            Duration::from_secs(10),
        )
        .await
        .unwrap();
    let project = format!("dccremote{pid}");
    let content = format!(
        "name: {project}\nservices:\n  web:\n    image: {}\n    labels:\n      dcc.test: \"true\"\n",
        fixture::IMAGE
    );
    let created = remote_file::create(&e, &dir, &content).await.unwrap();
    let path = format!("{dir}/compose.yaml");
    assert_eq!(created.path, path);
    assert_eq!(created.content, content);
    assert!(created.writable);
    // No se pisa un archivo existente.
    assert!(remote_file::create(&e, &dir, "x").await.is_err());
    assert_eq!(
        remote_file::existing_default(&e, &dir)
            .await
            .unwrap()
            .as_deref(),
        Some(path.as_str())
    );

    // Un archivo que existe en ESTE equipo no cuenta para el servidor: se
    // comprueba allí. En el laboratorio comparten disco, así que el archivo
    // va en una carpeta privada (0700) que el usuario remoto no puede ver.
    let private = tempfile::tempdir().unwrap();
    std::fs::set_permissions(private.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        std::fs::metadata(private.path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let here_path = private.path().join("compose.yaml").display().to_string();
    std::fs::write(&here_path, "services: {}\n").unwrap();
    assert!(std::path::Path::new(&here_path).is_file());
    let missing = e
        .files_missing(&[path.clone(), here_path.clone()])
        .await
        .unwrap();
    assert_eq!(missing, vec![here_path]);

    // config y up en el servidor.
    let p = ProjectRef {
        name: project.clone(),
        files: vec![path.clone()],
        working_dir: None,
        env_files: vec![],
    };
    let cfg = e.compose_config(&cli, &p, None).await.unwrap();
    assert!(cfg.error.is_none(), "{:?}", cfg.error);
    assert!(cfg.model.unwrap().services.iter().any(|s| s.name == "web"));
    let up = e
        .compose_run(
            &cli,
            &p,
            &ComposeAction::Up {
                services: vec![],
                build: false,
                pull: dcc_engine::compose::project::PullPolicy::Missing,
                force_recreate: false,
                remove_orphans: false,
                profiles: vec![],
            },
            false,
            tokio_util::sync::CancellationToken::new(),
            |_| true,
        )
        .await
        .unwrap();
    assert!(up.success(), "{up:?}");
    let label = format!("com.docker.compose.project={project}");
    let on = |socket: &str| {
        let out = std::process::Command::new("docker")
            .args([
                "-H",
                &format!("unix://{socket}"),
                "ps",
                "-aq",
                "--filter",
                &format!("label={label}"),
            ])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).lines().count()
    };
    assert_eq!(on(&b.socket), 1, "en B");
    assert_eq!(on(&lab.servers[0].socket), 0, "nada en A");
    assert_eq!(on(&lab.servers[2].socket), 0, "nada en C");
    // La lista de proyectos del servidor lo encuentra por sus etiquetas y
    // comprueba sus archivos en el servidor.
    let ps = e.compose_projects(&[]).await.unwrap();
    let mine = ps
        .iter()
        .find(|x| x.reference.name == project)
        .expect("proyecto desplegado");
    assert_eq!(mine.reference.files, vec![path.clone()]);
    assert!(mine.missing_files.is_empty(), "{:?}", mine.missing_files);

    // Registros: cancelar cierra el canal y no deja `docker compose logs` en el servidor.
    let cancel = tokio_util::sync::CancellationToken::new();
    let c2 = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        c2.cancel();
    });
    let logs = e
        .compose_logs(&cli, &p, &[], 10, false, cancel, |_| true)
        .await
        .unwrap();
    assert!(
        !matches!(logs.end, dcc_engine::cli::RunEnd::Exited { code: Some(c) } if c != 0),
        "{logs:?}"
    );
    let user = b.user.clone();
    wait_for("sin compose logs en el servidor", 10, || {
        std::process::Command::new("pgrep")
            .args(["-u", &user, "-f", "compose.*logs"])
            .output()
            .map(|o| o.stdout.is_empty())
            .unwrap_or(false)
    })
    .await;

    // Guardar: con la versión leída sí; con una antigua, no.
    let edited = content.replace(
        "dcc.test: \"true\"",
        "dcc.test: \"true\"\n      dcc.edit: \"1\"",
    );
    let saved = remote_file::save(&e, &path, &edited, &created.version)
        .await
        .unwrap();
    assert_eq!(saved.content, edited);
    let stale = remote_file::save(&e, &path, "otra", &created.version).await;
    assert!(
        matches!(stale, Err(dcc_engine::EngineError::FileChanged { .. })),
        "{stale:?}"
    );

    // Reglas de rutas (E2, D-089) en el servidor: archivos del sistema, carpetas
    // ocultas y enlaces que apuntan a ellos se rechazan; un enlace legítimo se
    // guarda en su destino y sigue siendo un enlace.
    let invalid = |r: &Result<_, dcc_engine::EngineError>| {
        matches!(r, Err(dcc_engine::EngineError::PathNotAllowed { .. }))
    };
    assert!(invalid(&remote_file::read(&e, "/etc/passwd").await));
    let ssh_cfg = format!("/home/{}/.ssh/authorized_keys", b.user);
    assert!(invalid(&remote_file::read(&e, &ssh_cfg).await));
    let trap = format!("{dir}/trampa.yaml");
    let linked = format!("{dir}/enlace.yaml");
    session
        .run(
            &[
                "ln".into(),
                "-s".into(),
                "--".into(),
                "/etc/hostname".into(),
                trap.clone(),
            ],
            Duration::from_secs(10),
        )
        .await
        .unwrap();
    session
        .run(
            &[
                "ln".into(),
                "-s".into(),
                "--".into(),
                path.clone(),
                linked.clone(),
            ],
            Duration::from_secs(10),
        )
        .await
        .unwrap();
    assert!(invalid(&remote_file::read(&e, &trap).await));
    assert!(invalid(
        &remote_file::save(&e, &trap, "x", &saved.version).await
    ));
    let via_link = remote_file::read(&e, &linked).await.unwrap();
    assert_eq!(via_link.content, edited);
    let relinked = remote_file::save(&e, &linked, &content, &via_link.version)
        .await
        .unwrap();
    assert_eq!(relinked.path, linked);
    assert_eq!(remote_file::read(&e, &path).await.unwrap().content, content);
    let kind = session
        .run(
            &[
                "stat".into(),
                "-c".into(),
                "%F".into(),
                "--".into(),
                linked.clone(),
            ],
            Duration::from_secs(10),
        )
        .await
        .unwrap();
    assert_eq!(kind.stdout.trim(), "symbolic link", "el enlace se conserva");

    // down (destructiva: exige confirmación).
    let down = ComposeAction::Down {
        volumes: false,
        images: dcc_engine::compose::project::DownImages::None,
        remove_orphans: false,
    };
    assert!(matches!(
        e.compose_run(
            &cli,
            &p,
            &down,
            false,
            tokio_util::sync::CancellationToken::new(),
            |_| true
        )
        .await,
        Err(dcc_engine::EngineError::NotConfirmed)
    ));
    let r = e
        .compose_run(
            &cli,
            &p,
            &down,
            true,
            tokio_util::sync::CancellationToken::new(),
            |_| true,
        )
        .await
        .unwrap();
    assert!(r.success(), "{r:?}");
    assert_eq!(on(&b.socket), 0);

    // En actividad: la orden ssh equivalente, nunca el contenido del archivo.
    let recs = log.snapshot();
    let compose: Vec<_> = recs
        .iter()
        .filter(|r| r.kind.starts_with("compose."))
        .collect();
    assert!(!compose.is_empty());
    for r in &compose {
        let dcc_activity::Transport::Cli { program, args } = &r.transport else {
            panic!()
        };
        assert_eq!(program, "ssh");
        assert!(args.contains(&format!("{}@127.0.0.1", b.user)), "{args:?}");
        assert!(r.server.as_ref().is_some_and(|s| s.remote));
    }
    let all = serde_json::to_string(&recs).unwrap();
    assert!(!all.contains("dcc.edit"), "el contenido no se registra");

    // Limpieza en el servidor.
    session
        .run(
            &[
                "rm".into(),
                "-rf".into(),
                "--".into(),
                format!("/home/{}/dcc-compose-{pid}", b.user),
            ],
            Duration::from_secs(10),
        )
        .await
        .unwrap();
    m.disconnect().await;
}

// ------------------------------------------------------------------ terminal SSH del servidor

/// La shell del servidor: tamaño inicial y cambios de tamaño llegan al
/// servidor, `exit` la termina, y cerrarla desde la aplicación no deja la
/// shell en el servidor ni procesos `ssh` locales.
#[tokio::test(flavor = "multi_thread")]
async fn server_shell_session_resizes_and_closes_cleanly() {
    use dcc_engine::ssh::shell::run_server_shell;
    use dcc_engine::streams::{EndReason, TerminalInput};
    let Some(lab) = lab() else { return };
    let env = Env::new();
    env.trusted(&lab);
    let ctx = env.ctx(fixed(None));
    let log = ActivityLog::new(200);
    let c = &lab.servers[2];
    let m = EngineManager::remote(
        log.clone(),
        profile(&lab, 2, None, key(&lab), RemoteDocker::Default),
        ctx.clone(),
        Timing::default(),
    );
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    let engine = m.engine().await.unwrap();
    let shells = |user: &str| {
        std::process::Command::new("pgrep")
            .args(["-u", user, "-x", "bash"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).lines().count())
            .unwrap_or(0)
    };
    let before = shells(&c.user);

    // 1) Tamaño, cambio de tamaño y salida con `exit`.
    let (tx, rx) = tokio::sync::mpsc::channel(16);
    let out = Arc::new(std::sync::Mutex::new(String::new()));
    let o2 = out.clone();
    let e2 = engine.clone();
    let task = tokio::spawn(async move {
        run_server_shell(
            &e2,
            (100, 24),
            tokio_util::sync::CancellationToken::new(),
            rx,
            move |b| {
                o2.lock().unwrap().push_str(&String::from_utf8_lossy(&b));
                true
            },
        )
        .await
    });
    let has = |t: &str| out.lock().unwrap().contains(t);
    tx.send(TerminalInput::Data(
        b"stty size; echo FIN-$((6*7))\n".to_vec(),
    ))
    .await
    .unwrap();
    wait_for("primera salida", 15, || has("FIN-42")).await;
    assert!(has("24 100"), "{}", out.lock().unwrap());
    tx.send(TerminalInput::Resize {
        cols: 132,
        rows: 40,
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    tx.send(TerminalInput::Data(
        b"stty size; echo OTRA-$((6*8))\n".to_vec(),
    ))
    .await
    .unwrap();
    wait_for("tras cambiar el tamaño", 15, || has("OTRA-48")).await;
    assert!(
        has("40 132"),
        "el servidor ve el nuevo tamaño: {}",
        out.lock().unwrap()
    );
    tx.send(TerminalInput::Data(b"exit 0\n".to_vec()))
        .await
        .unwrap();
    let end = tokio::time::timeout(Duration::from_secs(15), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(end.reason, EndReason::Finished);
    assert_eq!(end.exit_code, Some(0));

    // 2) Cerrada desde la aplicación: la shell del servidor termina.
    let (tx, rx) = tokio::sync::mpsc::channel(16);
    let cancel = tokio_util::sync::CancellationToken::new();
    let out = Arc::new(std::sync::Mutex::new(String::new()));
    let o2 = out.clone();
    let (e2, c2) = (engine.clone(), cancel.clone());
    let task = tokio::spawn(async move {
        run_server_shell(&e2, (80, 24), c2, rx, move |b| {
            o2.lock().unwrap().push_str(&String::from_utf8_lossy(&b));
            true
        })
        .await
    });
    tx.send(TerminalInput::Data(b"echo LISTO-$((2+3))\n".to_vec()))
        .await
        .unwrap();
    wait_for("shell abierta", 15, || {
        out.lock().unwrap().contains("LISTO-5")
    })
    .await;
    assert!(shells(&c.user) > before, "hay una shell en el servidor");
    cancel.cancel();
    let end = tokio::time::timeout(Duration::from_secs(15), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(end.reason, EndReason::Closed);
    let user = c.user.clone();
    wait_for("sin shell en el servidor", 10, || shells(&user) == before).await;
    // Ningún `ssh -tt` (canal de shell) sigue vivo: quedan la maestra y los
    // canales de la API que el cliente HTTP conserva abiertos.
    let shell_channels = || {
        std::fs::read_dir("/proc")
            .unwrap()
            .filter_map(|e| {
                let pid = e.ok()?.file_name().into_string().ok()?;
                let cmd = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
                let args: Vec<String> = cmd
                    .split(|b| *b == 0)
                    .map(|a| String::from_utf8_lossy(a).into_owned())
                    .collect();
                (args.first().is_some_and(|a| a.ends_with("ssh"))
                    && args.iter().any(|a| a == "-tt"))
                .then_some(())
            })
            .count()
    };
    wait_for("sin canales de shell locales", 10, || shell_channels() == 0).await;

    // En actividad: la orden ssh equivalente y cómo terminó.
    let recs: Vec<_> = log
        .snapshot()
        .into_iter()
        .filter(|r| r.kind == "server.shell")
        .collect();
    assert_eq!(recs.len(), 2);
    assert!(recs.iter().all(|r| r
        .server
        .as_ref()
        .is_some_and(|s| s.remote && s.address.contains(":2203"))));
    drop(engine);
    m.disconnect().await;
    wait_for("sin procesos ssh", 10, || ssh_children() == 0).await;
}

// ------------------------------------------------------------------ carpetas del servidor

/// Los bind mounts se comprueban en el servidor: una carpeta que solo existe
/// en este equipo falta allí; una del servidor se describe como la ve el
/// usuario SSH (tipo, propietario, lectura).
/// E3 (1.3.0): la auditoría de un servidor remoto lee sus contenedores por la
/// sesión SSH, encuentra lo peligroso, no analiza vulnerabilidades (las
/// herramientas solo ven el Docker de este equipo) y no cambia nada.
#[tokio::test(flavor = "multi_thread")]
async fn audit_of_a_remote_server_reads_its_containers_only() {
    use bollard::models::{ContainerCreateBody, HostConfig};
    use bollard::query_parameters::{CreateContainerOptions, RemoveContainerOptions};
    use dcc_engine::audit::AuditRequest;
    let Some(lab) = lab() else { return };
    let env = Env::new();
    env.trusted(&lab);
    let ctx = env.ctx(fixed(None));
    let log = ActivityLog::new(2000);
    let b = &lab.servers[1];
    let docker_b = local(&b.socket);
    fixture::ensure_image(&docker_b).await;
    let name = format!("dcc-test-audit-remote-{}", std::process::id());
    let _ = docker_b
        .remove_container(
            &name,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await;
    docker_b
        .create_container(
            Some(CreateContainerOptions {
                name: Some(name.clone()),
                ..Default::default()
            }),
            ContainerCreateBody {
                image: Some(fixture::IMAGE.into()),
                labels: Some([("dcc.test".to_owned(), "true".to_owned())].into()),
                host_config: Some(HostConfig {
                    cap_add: Some(vec!["SYS_ADMIN".into()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let m = EngineManager::remote(
        log.clone(),
        profile(&lab, 1, None, key(&lab), RemoteDocker::Default),
        ctx.clone(),
        Timing::default(),
    );
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    let e = m.engine().await.unwrap();
    let before = docker_b
        .inspect_container(
            &name,
            None::<bollard::query_parameters::InspectContainerOptions>,
        )
        .await
        .unwrap();
    let req = AuditRequest {
        container: Some(name.clone()),
        include_compose: false,
        include_images: false,
        scan_vulnerabilities: true,
        tool: None,
    };
    let r = e
        .audit(
            &b.name,
            &req,
            None,
            tokio_util::sync::CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();
    assert_eq!(r.targets.len(), 1);
    assert!(
        r.findings
            .iter()
            .any(|f| f.code == "risk.capability" && f.target.name == name),
        "{:#?}",
        r.findings
    );
    assert_eq!(
        r.vulnerabilities.not_analyzed.as_deref(),
        Some("audit.vuln.remote")
    );
    assert_eq!(r.server, b.name);
    let after = docker_b
        .inspect_container(
            &name,
            None::<bollard::query_parameters::InspectContainerOptions>,
        )
        .await
        .unwrap();
    assert_eq!(before.host_config, after.host_config);
    assert_eq!(
        before.state.map(|s| s.status),
        after.state.map(|s| s.status)
    );
    docker_b
        .remove_container(
            &name,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await
        .unwrap();
}

/// E2 (DCC-SEC-07, D-090): en un servidor remoto, la configuración de Docker
/// de este equipo no se consulta (y sus credenciales no se envían) sin permiso
/// explícito; con permiso, sí. Se usa un `credsStore` inexistente: su aviso
/// solo aparece si se intentó obtener las credenciales locales.
#[tokio::test(flavor = "multi_thread")]
async fn local_registry_credentials_reach_a_server_only_with_consent() {
    use dcc_engine::registry_auth::PullAuth;
    let Some(lab) = lab() else { return };
    let env = Env::new();
    env.trusted(&lab);
    let ctx = env.ctx(fixed(None));
    let log = ActivityLog::new(500);
    let m = EngineManager::remote(
        log.clone(),
        profile(&lab, 1, None, key(&lab), RemoteDocker::Default),
        ctx.clone(),
        Timing::default(),
    );
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    let e = m.engine().await.unwrap();
    let cfg = tempfile::Builder::new()
        .prefix("dcc-test-cfg")
        .tempdir()
        .unwrap();
    std::fs::write(
        cfg.path().join("config.json"),
        r#"{"credsStore":"dcc-e2-inexistente"}"#,
    )
    .unwrap();
    let previous = std::env::var_os("DOCKER_CONFIG");
    std::env::set_var("DOCKER_CONFIG", cfg.path());
    let pull = |share: bool| {
        let e = e.clone();
        async move {
            let mut messages = Vec::new();
            let _ = e
                .pull_image_with(
                    "127.0.0.1:9/dcc-e2/nada:1",
                    &PullAuth::DockerConfig,
                    share,
                    tokio_util::sync::CancellationToken::new(),
                    |u| {
                        messages.extend(u.messages);
                        true
                    },
                )
                .await;
            messages.join("\n")
        }
    };
    let without = pull(false).await;
    let with = pull(true).await;
    match previous {
        Some(v) => std::env::set_var("DOCKER_CONFIG", v),
        None => std::env::remove_var("DOCKER_CONFIG"),
    }
    assert!(
        !without.contains("dcc-e2-inexistente"),
        "sin permiso no se consultan las credenciales locales: {without}"
    );
    assert!(
        with.contains("docker-credential-dcc-e2-inexistente"),
        "con permiso sí: {with}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn bind_folders_are_checked_on_the_server() {
    use dcc_engine::binds::PathKind;
    use dcc_engine::spec::{ContainerSpec, MountKind, MountSpec};
    let Some(lab) = lab() else { return };
    let env = Env::new();
    env.trusted(&lab);
    let ctx = env.ctx(fixed(None));
    let log = ActivityLog::new(500);
    let b = &lab.servers[1];
    fixture::ensure_image(&local(&b.socket)).await;
    let m = EngineManager::remote(
        log.clone(),
        profile(&lab, 1, None, key(&lab), RemoteDocker::Default),
        ctx.clone(),
        Timing::default(),
    );
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    let e = m.engine().await.unwrap();
    let session = e.ssh_session().unwrap().clone();
    let pid = std::process::id();
    let on_server = format!("/home/{}/dcc-bind-{pid}", b.user);
    session
        .run(
            &["mkdir".into(), "-p".into(), "--".into(), on_server.clone()],
            Duration::from_secs(10),
        )
        .await
        .unwrap();
    // Solo en este equipo (carpeta privada que el usuario remoto no ve).
    let private = tempfile::tempdir().unwrap();
    std::fs::set_permissions(private.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let only_here = private.path().join("datos");
    std::fs::create_dir(&only_here).unwrap();
    let only_here = only_here.display().to_string();

    let spec = |source: &str| ContainerSpec {
        image: fixture::IMAGE.into(),
        name: format!("dcc-test-bind-{pid}"),
        mounts: vec![MountSpec {
            kind: MountKind::Bind,
            source: source.into(),
            target: "/datos".into(),
            read_only: false,
        }],
        labels: vec![dcc_engine::spec::KeyValue {
            key: "dcc.test".into(),
            value: "true".into(),
        }],
        ..Default::default()
    };
    let issues = e.spec_issues(&spec(&only_here)).await.unwrap();
    assert!(
        issues
            .iter()
            .any(|i| i.code == "bind.missing" && i.field == "mounts.0.source"),
        "{issues:?}"
    );
    // Crear con esa carpeta se rechaza antes de llamar a Docker.
    assert!(matches!(
        e.create_container(&spec(&only_here)).await,
        Err(dcc_engine::EngineError::InvalidSpec { .. })
    ));
    let issues = e.spec_issues(&spec(&on_server)).await.unwrap();
    assert!(
        !issues.iter().any(|i| i.code == "bind.missing"),
        "{issues:?}"
    );

    // Un contenedor del servidor con esa carpeta: la vista la describe como la ve el servidor.
    let created = e.create_container(&spec(&on_server)).await.unwrap();
    let binds = e.list_binds(None).await.unwrap();
    let mine = binds
        .iter()
        .find(|x| x.source == on_server)
        .expect("vinculación del servidor");
    assert_eq!(mine.host.kind, PathKind::Directory);
    assert!(mine.host.readable);
    let uid: u32 = String::from_utf8(
        std::process::Command::new("id")
            .args(["-u", &b.user])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .parse()
    .unwrap();
    assert_eq!(mine.host.uid, Some(uid));
    // En actividad, las consultas al servidor llevan su orden ssh.
    assert!(log.snapshot().iter().any(|r| r.kind == "bind.inspect" && matches!(&r.transport, dcc_activity::Transport::Cli { program, .. } if program == "ssh")));

    e.remove_container(&created.id, None, true, false)
        .await
        .unwrap();
    session
        .run(
            &["rm".into(), "-rf".into(), "--".into(), on_server],
            Duration::from_secs(10),
        )
        .await
        .unwrap();
    drop(e);
    m.disconnect().await;
}

// ------------------------------------------------------- E1: servidor intermedio

/// Líneas nuevas del registro de un sshd del laboratorio desde `from` bytes.
fn sshd_log_since(s: &Server, from: u64) -> String {
    let log = Path::new(&s.host_key).with_file_name("sshd.log");
    let all = std::fs::read(&log).unwrap_or_default();
    String::from_utf8_lossy(&all[(from as usize).min(all.len())..]).into_owned()
}

fn sshd_log_len(s: &Server) -> u64 {
    std::fs::metadata(Path::new(&s.host_key).with_file_name("sshd.log"))
        .map(|m| m.len())
        .unwrap_or(0)
}

/// E1 (DCC-SEC-02, D-087): el servidor intermedio se comprueba con las mismas
/// reglas que el destino (huella confirmada en el `known_hosts` de la
/// aplicación), solo se autentica con clave o agente y nunca recibe el secreto
/// del destino.
#[tokio::test(flavor = "multi_thread")]
async fn proxy_jump_checks_both_identities_and_never_shares_the_target_secret() {
    let Some(lab) = lab() else { return };
    let log = ActivityLog::new(500);
    let a = lab.servers[0].clone();
    let jump = format!("{}@{}:{}", a.user, a.host, a.port);

    // 1. Destino C con clave a través de A (clave): primero la huella del
    //    salto, después la del destino, y conecta.
    let env = Env::new();
    let ctx = env.ctx(fixed(None));
    let mut p = profile(
        &lab,
        2,
        None,
        key(&lab),
        RemoteDocker::Socket {
            path: lab.servers[2].socket.clone(),
        },
    );
    p.options.proxy_jump = jump.clone();
    let r = dcc_engine::ssh::connect::test(&ctx, &p, &log).await;
    let fa = real_fingerprint(&a);
    match &r.problem {
        Some(RemoteProblem::JumpHostKeyUnknown { jump: j, presented }) => {
            assert_eq!(j, &jump);
            assert_eq!(
                presented.as_ref().map(|k| &k.fingerprint),
                Some(&fa.fingerprint)
            );
        }
        other => panic!("se esperaba la huella del salto: {other:?}"),
    }
    assert_eq!(status(&r, CheckId::Identity), CheckStatus::Failed);
    assert!(
        !env.known().exists()
            || std::fs::read_to_string(env.known())
                .unwrap()
                .trim()
                .is_empty()
    );
    let jp = dcc_engine::ssh::profile::jump_profile(&p).unwrap();
    hostkey::trust(&ctx.paths, &jp, &fa, false)
        .await
        .expect("confiar en el salto");
    let r = dcc_engine::ssh::connect::test(&ctx, &p, &log).await;
    let fc = real_fingerprint(&lab.servers[2]);
    match &r.problem {
        Some(RemoteProblem::HostKeyUnknown { presented }) => {
            assert_eq!(
                presented.as_ref().map(|k| &k.fingerprint),
                Some(&fc.fingerprint)
            );
        }
        other => panic!("se esperaba la huella del destino (por el salto): {other:?}"),
    }
    hostkey::trust(&ctx.paths, &p, &fc, false)
        .await
        .expect("confiar en el destino por el salto");
    let r = dcc_engine::ssh::connect::test(&ctx, &p, &log).await;
    assert!(r.problem.is_none(), "{:?}", r.problem);
    assert!(r.engine.is_some());

    // 2. Una identidad del salto distinta de la confirmada: se rechaza.
    let env2 = Env::new();
    let other = env2.dir.path().join("otra");
    let st = std::process::Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-f"])
        .arg(&other)
        .status()
        .unwrap();
    assert!(st.success());
    let pubkey = std::fs::read_to_string(other.with_extension("pub")).unwrap();
    let mut it = pubkey.split_whitespace();
    let (t, k) = (it.next().unwrap(), it.next().unwrap());
    std::fs::write(env2.known(), format!("[{}]:{} {t} {k}\n", a.host, a.port)).unwrap();
    let ctx2 = env2.ctx(fixed(None));
    let r = dcc_engine::ssh::connect::test(&ctx2, &p, &log).await;
    assert!(
        matches!(r.problem, Some(RemoteProblem::JumpHostKeyChanged { .. })),
        "{:?}",
        r.problem
    );

    // 3. Destino B con contraseña a través de A sin clave para el salto: el
    //    salto no admite contraseña, la del destino no se pide al llavero y A
    //    no recibe ningún intento de contraseña.
    let env3 = Env::new();
    env3.trusted(&lab);
    let secrets = fixed(Some(&lab.password.password));
    let ctx3 = env3.ctx(secrets.clone());
    let mut pb = profile(
        &lab,
        lab.password.server,
        Some(&lab.password.user),
        SshAuth::Password,
        RemoteDocker::Socket {
            path: lab.servers[lab.password.server].socket.clone(),
        },
    );
    pb.options.proxy_jump = jump.clone();
    let before = sshd_log_len(&a);
    let r = dcc_engine::ssh::connect::test(&ctx3, &pb, &log).await;
    assert!(
        matches!(&r.problem, Some(RemoteProblem::JumpAuthFailed { jump: j, .. }) if j == &jump),
        "{:?}",
        r.problem
    );
    assert_eq!(status(&r, CheckId::Auth), CheckStatus::Failed);
    assert_eq!(
        secrets.asked(),
        0,
        "el secreto del destino ni siquiera se lee"
    );
    let seen = sshd_log_since(&a, before);
    assert!(
        !seen.contains("password for"),
        "el salto no ha recibido ninguna contraseña:\n{seen}"
    );

    // 4. La sesión real (sin la comprobación previa) tampoco entrega la
    //    contraseña al salto: falla en el salto y A no ve contraseñas.
    let before = sshd_log_len(&a);
    let s = dcc_engine::ssh::SshSession::open(&ctx3, &pb).await;
    assert!(s.is_err(), "sin clave para el salto no hay sesión");
    let seen = sshd_log_since(&a, before);
    assert!(!seen.contains("password for"), "{seen}");
    wait_for("sin procesos ssh", 10, || ssh_children() == 0).await;
}
