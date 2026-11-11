//! Administración remota contra servidores SSH y Docker Engines reales (0.10.0).
//!
//! Necesita `DCC_DOCKER_TESTS=1` y `DCC_SSH_LAB=<lab.json>`: tres servidores SSH
//! (A, B y C), cada uno con su Docker Engine, preparados por
//! `scripts/dev/remote-lab.sh`. No toca nada fuera de lo que crea.

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
