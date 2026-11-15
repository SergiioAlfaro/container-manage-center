// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Utilidades compartidas por las pruebas contra Docker real.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use cmc_activity::{ActivityLog, Transport};
use cmc_engine::endpoint::HostEnv;
use cmc_engine::manager::{attempt, Attempt};
use cmc_engine::ConnectionMode;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

pub fn enabled() -> bool {
    let on = std::env::var("CMC_DOCKER_TESTS").as_deref() == Ok("1");
    if !on {
        eprintln!("omitida: define CMC_DOCKER_TESTS=1 para probar contra Docker real");
    }
    on
}

/// Socket real con el que trabajar.
pub async fn real_socket() -> String {
    if let Ok(p) = std::env::var("CMC_DOCKER_SOCKET") {
        return p;
    }
    match attempt(
        &ActivityLog::new(10),
        &ConnectionMode::Auto,
        &HostEnv::from_process(),
        false,
    )
    .await
    {
        Attempt::Connected(e, _) => e.endpoint().socket_path.clone(),
        Attempt::Failed(d) => panic!("no se encontró un Docker accesible: {:?}", d.problem),
    }
}

/// Proxy de socket Unix que registra las líneas de petición HTTP.
pub struct Proxy {
    pub path: PathBuf,
    requests: Arc<Mutex<Vec<(String, String)>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Proxy {
    pub async fn start(path: &Path, upstream: String) -> Self {
        let _ = std::fs::remove_file(path);
        let listener = UnixListener::bind(path).unwrap();
        let requests: Arc<Mutex<Vec<(String, String)>>> = Default::default();
        let reqs = requests.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((client, _)) = listener.accept().await else {
                    break;
                };
                let upstream = upstream.clone();
                let reqs = reqs.clone();
                tokio::spawn(async move {
                    let Ok(server) = UnixStream::connect(&upstream).await else {
                        return;
                    };
                    let (mut cr, mut cw) = client.into_split();
                    let (mut sr, mut sw) = server.into_split();
                    let up = tokio::spawn(async move {
                        let mut buf = vec![0u8; 16 * 1024];
                        let mut pending = String::new();
                        loop {
                            let n = match cr.read(&mut buf).await {
                                Ok(0) | Err(_) => break,
                                Ok(n) => n,
                            };
                            pending.push_str(&String::from_utf8_lossy(&buf[..n]));
                            // La línea de petición puede llegar pegada al cuerpo de la
                            // anterior (sin salto de línea entre ambos), según cómo se
                            // agrupen los bytes: se busca en cualquier punto del flujo.
                            let mut consumed = 0;
                            for c in request_line().captures_iter(&pending) {
                                reqs.lock()
                                    .unwrap()
                                    .push((c[1].to_owned(), c[2].to_owned()));
                                consumed = c.get(0).unwrap().end();
                            }
                            pending.drain(..consumed);
                            if pending.len() > 64 * 1024 {
                                let keep = pending.len() - 1024;
                                let cut = (keep..pending.len())
                                    .find(|i| pending.is_char_boundary(*i))
                                    .unwrap_or(0);
                                pending.drain(..cut);
                            }
                            if sw.write_all(&buf[..n]).await.is_err() {
                                break;
                            }
                        }
                    });
                    let _ = tokio::io::copy(&mut sr, &mut cw).await;
                    up.abort();
                });
            }
        });
        Self {
            path: path.to_owned(),
            requests,
            task,
        }
    }

    pub fn stop(self) -> PathBuf {
        self.task.abort();
        let _ = std::fs::remove_file(&self.path);
        self.path
    }

    /// Peticiones que pasaron por el proxy. En modo SSH se excluye exactamente
    /// `HEAD /_ping`: lo envía el CLI `docker system dial-stdio` del servidor
    /// al abrir cada canal para negociar la versión; el motor nunca lo usa
    /// (lo comprueba `remote_cli_negotiates_with_head_ping_only` en live_ssh).
    pub fn requests(&self) -> Vec<(String, String)> {
        let v = self.requests.lock().unwrap().clone();
        if ssh_target().is_some() {
            v.into_iter()
                .filter(|(m, p)| !(m == "HEAD" && p == "/_ping"))
                .collect()
        } else {
            v
        }
    }

    /// Todas las peticiones, también las del CLI remoto.
    pub fn all_requests(&self) -> Vec<(String, String)> {
        self.requests.lock().unwrap().clone()
    }
}

fn request_line() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(GET|POST|PUT|DELETE|HEAD) (\S+) HTTP/1\.[01]\r\n").unwrap()
    })
}

/// Valor de `--host` en las órdenes equivalentes: el socket del proxy en local
/// o `ssh://usuario@host:puerto` en modo SSH.
pub fn docker_host(proxy_path: &Path) -> String {
    match ssh_target() {
        Some(t) => format!("ssh://{}@{}:{}", t.user, t.host, t.port),
        None => format!("unix://{}", proxy_path.display()),
    }
}

pub fn recorded(log: &ActivityLog) -> Vec<(String, String)> {
    let mut v: Vec<_> = log
        .snapshot()
        .into_iter()
        .filter_map(|r| match r.transport {
            Transport::Api { method, path, .. } => Some((method, path)),
            Transport::Cli { .. } | Transport::App { .. } => None,
        })
        .collect();
    v.reverse(); // snapshot es del más reciente al más antiguo
    v
}

// ------------------------------------------------------------- SSH (0.10.0)

/// Servidor SSH de pruebas: `CMC_SSH_TEST=usuario@host:puerto`, con
/// `CMC_SSH_TEST_KEY` (clave sin frase) y `CMC_SSH_TEST_KNOWN_HOSTS` (huella ya
/// confirmada). Si está definido, las pruebas reales usan el motor por SSH.
#[derive(Debug, Clone)]
pub struct SshTarget {
    pub user: String,
    pub host: String,
    pub port: u16,
    pub key: String,
    pub known_hosts: PathBuf,
}

pub fn ssh_target() -> Option<SshTarget> {
    let spec = std::env::var("CMC_SSH_TEST").ok()?;
    let (user, rest) = spec.split_once('@')?;
    let (host, port) = rest.rsplit_once(':')?;
    Some(SshTarget {
        user: user.into(),
        host: host.into(),
        port: port.parse().ok()?,
        key: std::env::var("CMC_SSH_TEST_KEY").expect("falta CMC_SSH_TEST_KEY"),
        known_hosts: std::env::var("CMC_SSH_TEST_KNOWN_HOSTS")
            .expect("falta CMC_SSH_TEST_KNOWN_HOSTS")
            .into(),
    })
}

/// Contexto SSH de pruebas: askpass = el binario `cmc-askpass` del motor.
pub fn ssh_context(
    runtime: &Path,
    known_hosts: PathBuf,
    secrets: Arc<dyn cmc_engine::ssh::SecretProvider>,
) -> cmc_engine::ssh::SshContext {
    cmc_engine::ssh::SshContext {
        paths: cmc_engine::ssh::SshPaths {
            ssh: "/usr/bin/ssh".into(),
            ssh_keygen: "/usr/bin/ssh-keygen".into(),
            runtime_dir: runtime.to_owned(),
            app_known_hosts: known_hosts,
            user_known_hosts: "/nonexistent/known_hosts".into(),
            askpass: env!("CARGO_BIN_EXE_cmc-askpass").into(),
        },
        secrets,
    }
}

pub fn ssh_profile(
    t: &SshTarget,
    docker: cmc_engine::ssh::RemoteDocker,
) -> cmc_engine::ssh::SshProfile {
    cmc_engine::ssh::SshProfile {
        id: format!("srv-{:08x}", std::process::id()),
        name: "Pruebas".into(),
        host: t.host.clone(),
        port: t.port,
        user: t.user.clone(),
        description: String::new(),
        auth: cmc_engine::ssh::SshAuth::Key {
            path: t.key.clone(),
        },
        docker,
        options: Default::default(),
    }
}

/// Deja que el usuario remoto (otro usuario del mismo equipo en las pruebas)
/// llegue al socket: directorio 0755 y socket 0666. Solo en modo SSH.
pub fn expose_for_remote(socket: &Path) {
    use std::os::unix::fs::PermissionsExt;
    // Solo sockets creados por la prueba en un directorio temporal (el proxy):
    // nunca el socket real de Docker ni su directorio.
    if !socket.starts_with(std::env::temp_dir())
        || socket.starts_with("/var/run")
        || socket.starts_with("/run")
    {
        return;
    }
    if let Some(dir) = socket.parent() {
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
    }
    let _ = std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o666));
}

/// Gestor para las pruebas reales: local con el socket indicado o, con
/// `CMC_SSH_TEST`, por SSH con el Docker remoto apuntando a ese mismo socket
/// (el proxy de registro queda detrás de SSH y sus comprobaciones siguen valiendo).
pub fn manager(log: ActivityLog, socket: String) -> cmc_engine::manager::EngineManager {
    match ssh_target() {
        None => cmc_engine::manager::EngineManager::new(
            log,
            ConnectionMode::Custom {
                socket_path: socket,
            },
        ),
        Some(t) => {
            expose_for_remote(Path::new(&socket));
            let runtime = std::env::temp_dir().join(format!("cmc-ssh-test-{}", std::process::id()));
            let ctx = ssh_context(
                &runtime,
                t.known_hosts.clone(),
                Arc::new(cmc_engine::ssh::NoSecrets),
            );
            let profile = ssh_profile(&t, cmc_engine::ssh::RemoteDocker::Socket { path: socket });
            cmc_engine::manager::EngineManager::remote(log, profile, ctx, Default::default())
        }
    }
}

pub mod registry;

pub mod fixture {
    use bollard::models::ContainerCreateBody;
    use bollard::query_parameters::{
        CreateContainerOptions, CreateImageOptions, RemoveContainerOptions,
    };
    use futures_util::StreamExt;

    pub const IMAGE: &str = "cmc-test/sleeper:2";

    /// Programa estático mínimo:
    /// - sin argumentos: espera a SIGTERM;
    /// - `talk N`: escribe N líneas en stdout y N en stderr, y espera;
    /// - `exit C`: termina con el código C.
    ///
    /// Permite probar ciclo de vida y logs sin descargar imágenes de Internet.
    const SLEEPER_C: &str = r#"
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
static void bye(int s) { (void)s; _exit(0); }
int main(int argc, char **argv) {
  signal(SIGTERM, bye); signal(SIGINT, bye);
  if (argc > 2 && strcmp(argv[1], "exit") == 0) return atoi(argv[2]);
  if (argc > 2 && strcmp(argv[1], "talk") == 0) {
    int n = atoi(argv[2]);
    for (int i = 1; i <= n; i++) {
      printf("linea %d\n", i); fflush(stdout);
      fprintf(stderr, "aviso %d\n", i); fflush(stderr);
    }
  }
  for (;;) pause();
}
"#;

    fn add_file(tar: &mut tar::Builder<Vec<u8>>, path: &str, data: &[u8], mode: u32) {
        let mut h = tar::Header::new_gnu();
        h.set_size(data.len() as u64);
        h.set_mode(mode);
        h.set_cksum();
        tar.append_data(&mut h, path.trim_start_matches('/'), data)
            .unwrap();
    }

    /// La shell del equipo (`/bin/sh`) y sus bibliotecas, según `ldd`.
    fn host_shell() -> Vec<(String, Vec<u8>)> {
        let sh = std::fs::canonicalize("/bin/sh").expect("/bin/sh");
        let mut files = vec![("/bin/sh".to_string(), std::fs::read(&sh).unwrap())];
        let out = std::process::Command::new("ldd")
            .arg(&sh)
            .output()
            .expect("ldd");
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            // "libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 (0x...)" o "/lib64/ld-linux-x86-64.so.2 (0x...)"
            let path = match line.split_once("=>") {
                Some((_, r)) => r.split_whitespace().next(),
                None => line.split_whitespace().next(),
            };
            if let Some(p) = path.filter(|p| p.starts_with('/')) {
                let real = std::fs::canonicalize(p).unwrap();
                files.push((p.to_string(), std::fs::read(real).unwrap()));
            }
        }
        files
    }

    /// Crea (una sola vez) la imagen de prueba importando un rootfs local.
    ///
    /// Las pruebas de un mismo binario se ejecutan en paralelo: sin el cerrojo,
    /// con un daemon que aún no tiene la imagen, varias la importaban a la vez
    /// y las perdedoras quedaban como imágenes sin etiqueta.
    pub async fn ensure_image(docker: &bollard::Docker) {
        static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
        let _guard = LOCK.lock().await;
        if docker.inspect_image(IMAGE).await.is_ok() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("sleeper.c");
        let bin = dir.path().join("sleeper");
        std::fs::write(&src, SLEEPER_C).unwrap();
        let st = std::process::Command::new("cc")
            .args(["-static", "-Os", "-s", "-o"])
            .arg(&bin)
            .arg(&src)
            .status()
            .expect("hace falta un compilador C (build-essential)");
        assert!(st.success(), "no se pudo compilar el binario de prueba");
        let mut tar = tar::Builder::new(Vec::new());
        add_file(&mut tar, "/sleeper", &std::fs::read(&bin).unwrap(), 0o755);
        for (path, data) in host_shell() {
            add_file(&mut tar, &path, &data, 0o755);
        }
        let body = tar.into_inner().unwrap();
        let opts = CreateImageOptions {
            from_src: Some("-".into()),
            repo: Some("cmc-test/sleeper".into()),
            tag: Some("2".into()),
            changes: vec![
                r#"CMD ["/sleeper"]"#.into(),
                "LABEL cmc.test=true".into(),
                "ENV CMC_FIXTURE=2".into(),
            ],
            ..Default::default()
        };
        let mut s = docker.create_image(
            Some(opts),
            Some(bollard::body_full(bytes::Bytes::from(body))),
            None,
        );
        while let Some(r) = s.next().await {
            r.expect("importación de la imagen de prueba");
        }
    }

    /// Imagen con shell y algunas órdenes del equipo (0.9.1, terminales
    /// múltiples): `sleep` para un programa en primer plano, `stty` para leer el
    /// tamaño de la TTY y `cat`. Sin descargar nada.
    pub const SHELL_IMAGE: &str = "cmc-test/shell:1";

    /// Programas del equipo y sus bibliotecas (según `ldd`), sin repetir.
    fn host_programs(names: &[&str]) -> Vec<(String, Vec<u8>)> {
        let mut seen = std::collections::BTreeSet::new();
        let mut files = Vec::new();
        for name in names {
            let path = ["/bin", "/usr/bin"]
                .iter()
                .map(|d| format!("{d}/{name}"))
                .find(|p| std::path::Path::new(p).exists())
                .unwrap_or_else(|| panic!("falta {name} en el equipo"));
            let real = std::fs::canonicalize(&path).unwrap();
            if seen.insert(format!("/bin/{name}")) {
                files.push((format!("/bin/{name}"), std::fs::read(&real).unwrap()));
            }
            let out = std::process::Command::new("ldd")
                .arg(&real)
                .output()
                .expect("ldd");
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let lib = match line.split_once("=>") {
                    Some((_, r)) => r.split_whitespace().next(),
                    None => line.split_whitespace().next(),
                };
                if let Some(l) = lib.filter(|p| p.starts_with('/')) {
                    if seen.insert(l.to_string()) {
                        let real = std::fs::canonicalize(l).unwrap();
                        files.push((l.to_string(), std::fs::read(real).unwrap()));
                    }
                }
            }
        }
        files
    }

    pub async fn ensure_shell_image(docker: &bollard::Docker) {
        static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
        let _guard = LOCK.lock().await;
        if docker.inspect_image(SHELL_IMAGE).await.is_ok() {
            return;
        }
        let mut tar = tar::Builder::new(Vec::new());
        for (path, data) in host_programs(&["sh", "sleep", "stty", "cat"]) {
            add_file(&mut tar, &path, &data, 0o755);
        }
        // /proc existe al arrancar el contenedor; /tmp para las pruebas.
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(tar::EntryType::Directory);
        h.set_mode(0o1777);
        h.set_size(0);
        h.set_cksum();
        tar.append_data(&mut h, "tmp", std::io::empty()).unwrap();
        let body = tar.into_inner().unwrap();
        let opts = CreateImageOptions {
            from_src: Some("-".into()),
            repo: Some("cmc-test/shell".into()),
            tag: Some("1".into()),
            changes: vec![
                r#"CMD ["/bin/sleep", "infinity"]"#.into(),
                "LABEL cmc.test=true".into(),
            ],
            ..Default::default()
        };
        let mut s = docker.create_image(
            Some(opts),
            Some(bollard::body_full(bytes::Bytes::from(body))),
            None,
        );
        while let Some(r) = s.next().await {
            r.expect("importación de la imagen de prueba con shell");
        }
    }

    /// Contenedor de prueba detenido. Se elimina al soltar el guardián.
    pub struct TestContainer {
        pub docker: bollard::Docker,
        pub id: String,
        pub name: String,
    }

    impl TestContainer {
        pub async fn create(docker: &bollard::Docker, tag: &str) -> Self {
            ensure_image(docker).await;
            let name = format!("cmc-test-{tag}-{}", std::process::id());
            let _ = docker
                .remove_container(
                    &name,
                    Some(RemoveContainerOptions {
                        force: true,
                        ..Default::default()
                    }),
                )
                .await;
            let mut labels = std::collections::HashMap::new();
            labels.insert("cmc.test".to_string(), "true".to_string());
            let body = ContainerCreateBody {
                image: Some(IMAGE.into()),
                labels: Some(labels),
                ..Default::default()
            };
            let r = docker
                .create_container(
                    Some(CreateContainerOptions {
                        name: Some(name.clone()),
                        ..Default::default()
                    }),
                    body,
                )
                .await
                .expect("crear contenedor de prueba");
            Self {
                docker: docker.clone(),
                id: r.id,
                name,
            }
        }
    }

    impl Drop for TestContainer {
        fn drop(&mut self) {
            let d = self.docker.clone();
            let id = self.id.clone();
            // Limpieza síncrona fuera del runtime de la prueba.
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(async {
                    let _ = d
                        .remove_container(
                            &id,
                            Some(RemoveContainerOptions {
                                force: true,
                                ..Default::default()
                            }),
                        )
                        .await;
                });
            })
            .join()
            .ok();
        }
    }
}
