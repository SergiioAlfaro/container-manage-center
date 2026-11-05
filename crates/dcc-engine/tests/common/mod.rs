//! Utilidades compartidas por las pruebas contra Docker real.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use dcc_activity::{ActivityLog, Transport};
use dcc_engine::endpoint::HostEnv;
use dcc_engine::manager::{attempt, Attempt};
use dcc_engine::ConnectionMode;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

pub fn enabled() -> bool {
    let on = std::env::var("DCC_DOCKER_TESTS").as_deref() == Ok("1");
    if !on {
        eprintln!("omitida: define DCC_DOCKER_TESTS=1 para probar contra Docker real");
    }
    on
}

/// Socket real con el que trabajar.
pub async fn real_socket() -> String {
    if let Ok(p) = std::env::var("DCC_DOCKER_SOCKET") {
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
                            while let Some(i) = pending.find("\r\n") {
                                let line: String = pending.drain(..i + 2).collect();
                                let mut parts = line.trim_end().splitn(3, ' ');
                                if let (Some(m), Some(p), Some(v)) =
                                    (parts.next(), parts.next(), parts.next())
                                {
                                    if v.starts_with("HTTP/")
                                        && ["GET", "POST", "PUT", "DELETE", "HEAD"].contains(&m)
                                    {
                                        reqs.lock().unwrap().push((m.to_owned(), p.to_owned()));
                                    }
                                }
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

    pub fn requests(&self) -> Vec<(String, String)> {
        self.requests.lock().unwrap().clone()
    }
}

pub fn recorded(log: &ActivityLog) -> Vec<(String, String)> {
    let mut v: Vec<_> = log
        .snapshot()
        .into_iter()
        .filter_map(|r| match r.transport {
            Transport::Api { method, path, .. } => Some((method, path)),
            Transport::Cli { .. } => None,
        })
        .collect();
    v.reverse(); // snapshot es del más reciente al más antiguo
    v
}

pub mod fixture {
    use bollard::models::ContainerCreateBody;
    use bollard::query_parameters::{
        CreateContainerOptions, CreateImageOptions, RemoveContainerOptions,
    };
    use futures_util::StreamExt;

    pub const IMAGE: &str = "dcc-test/sleeper:2";

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
    pub async fn ensure_image(docker: &bollard::Docker) {
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
            repo: Some("dcc-test/sleeper".into()),
            tag: Some("2".into()),
            changes: vec![
                r#"CMD ["/sleeper"]"#.into(),
                "LABEL dcc.test=true".into(),
                "ENV DCC_FIXTURE=2".into(),
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

    /// Contenedor de prueba detenido. Se elimina al soltar el guardián.
    pub struct TestContainer {
        pub docker: bollard::Docker,
        pub id: String,
        pub name: String,
    }

    impl TestContainer {
        pub async fn create(docker: &bollard::Docker, tag: &str) -> Self {
            ensure_image(docker).await;
            let name = format!("dcc-test-{tag}-{}", std::process::id());
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
            labels.insert("dcc.test".to_string(), "true".to_string());
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
