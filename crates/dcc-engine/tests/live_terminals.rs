//! 0.9.1 · Terminales múltiples contra Docker real (`DCC_DOCKER_TESTS=1`).
//!
//! Varias sesiones `exec` a la vez sobre uno o varios contenedores: cada una con
//! su proceso, su entrada y salida, su tamaño y su ciclo de vida, sin
//! interferencias; al cerrarlas no queda ningún proceso de la sesión dentro del
//! contenedor (D-063). Todo lo creado lleva la etiqueta `dcc.test=true`.

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use bollard::models::ContainerCreateBody;
use bollard::query_parameters::{
    CreateContainerOptions, RemoveContainerOptions, StartContainerOptions, StopContainerOptions,
};
use bytes::Bytes;
use common::*;
use dcc_activity::{ActivityLog, OpStatus};
use dcc_engine::streams::{
    ContainerAfter, EndReason, ShellChoice, StreamEnd, TerminalInput, PID_MARK_START,
};
use dcc_engine::{ConnectionState, Engine, EngineError};
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

async fn setup() -> (Engine, ActivityLog, bollard::Docker) {
    let sock = real_socket().await;
    let docker =
        bollard::Docker::connect_with_unix(&sock, 60, bollard::API_DEFAULT_VERSION).unwrap();
    fixture::ensure_shell_image(&docker).await;
    let log = ActivityLog::new(2000);
    // Local o, con DCC_SSH_TEST, por SSH (0.10.0).
    let m = common::manager(log.clone(), sock);
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    (m.engine().await.unwrap(), log, docker)
}

/// Contenedor en ejecución con la imagen de shell; se elimina al soltarlo.
struct Running {
    docker: bollard::Docker,
    id: String,
    name: String,
}

impl Running {
    async fn start(docker: &bollard::Docker, tag: &str) -> Self {
        let name = format!("dcc-test-term-{tag}-{}", std::process::id());
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
        let r = docker
            .create_container(
                Some(CreateContainerOptions {
                    name: Some(name.clone()),
                    ..Default::default()
                }),
                ContainerCreateBody {
                    image: Some(fixture::SHELL_IMAGE.into()),
                    labels: Some(labels),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        docker
            .start_container(&r.id, None::<StartContainerOptions>)
            .await
            .unwrap();
        Self {
            docker: docker.clone(),
            id: r.id,
            name,
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let d = self.docker.clone();
        let id = self.id.clone();
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

struct Session {
    handle: tokio::task::JoinHandle<Result<StreamEnd, EngineError>>,
    input: mpsc::Sender<TerminalInput>,
    out: Arc<Mutex<Vec<u8>>>,
    cancel: CancellationToken,
}

impl Session {
    fn open(e: &Engine, c: &Running, size: (u16, u16)) -> Self {
        let (tx, rx) = mpsc::channel(32);
        let out: Arc<Mutex<Vec<u8>>> = Default::default();
        let cancel = CancellationToken::new();
        let (e2, id, name, o2, c2) = (
            e.clone(),
            c.id.clone(),
            c.name.clone(),
            out.clone(),
            cancel.clone(),
        );
        let handle = tokio::spawn(async move {
            e2.run_terminal(
                &id,
                Some(&name),
                ShellChoice::Sh.argv(),
                size,
                c2,
                rx,
                move |b: Bytes| {
                    o2.lock().unwrap().extend_from_slice(&b);
                    true
                },
            )
            .await
        });
        Self {
            handle,
            input: tx,
            out,
            cancel,
        }
    }

    async fn send(&self, s: &str) {
        self.input
            .send(TerminalInput::Data(s.as_bytes().to_vec()))
            .await
            .unwrap();
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.out.lock().unwrap()).into_owned()
    }

    /// Espera a que aparezca `needle` en la salida (hasta 10 s).
    async fn wait(&self, needle: &str) -> bool {
        for _ in 0..200 {
            if self.text().contains(needle) {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        false
    }

    async fn close(self) -> StreamEnd {
        self.cancel.cancel();
        tokio::time::timeout(Duration::from_secs(30), self.handle)
            .await
            .expect("la sesión no terminó")
            .unwrap()
            .unwrap()
    }
}

fn count(ps: &[(String, String)], comm: &str) -> usize {
    ps.iter().filter(|(c, _)| c == comm).count()
}

/// Procesos vivos del contenedor (`comm` y sesión), leídos con un exec propio.
/// El proceso principal es `sleep infinity` y el que lista es un `sh`.
async fn processes(docker: &bollard::Docker, id: &str) -> Vec<(String, String)> {
    let script =
        r#"for d in /proc/[0-9]*; do st=; read -r st < "$d/stat" 2>/dev/null && echo "$st"; done"#;
    let ex = docker
        .create_exec(
            id,
            bollard::models::ExecConfig {
                attach_stdout: Some(true),
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
    text.lines()
        .filter_map(|l| {
            let comm = l.split_once('(')?.1.rsplit_once(')')?.0.to_owned();
            let rest = l.rsplit_once(") ")?.1;
            let mut f = rest.split_whitespace();
            // Los zombis ya han terminado: el proceso principal de la imagen de
            // prueba (`sleep infinity`) no los recoge, como haría un init.
            if f.next()? == "Z" {
                return None;
            }
            let session = f.nth(2)?.to_owned();
            Some((comm, session))
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn several_terminals_run_at_once_without_interfering() {
    if !enabled() {
        return;
    }
    let (e, log, docker) = setup().await;
    let a = Running::start(&docker, "a").await;
    let b = Running::start(&docker, "b").await;
    // Dos sesiones en el mismo contenedor y una en otro, abiertas a la vez.
    let s1 = Session::open(&e, &a, (120, 40));
    let s2 = Session::open(&e, &a, (90, 30));
    let s3 = Session::open(&e, &b, (100, 25));

    // La misma orden a la vez en las tres: cada una ve solo lo suyo.
    let (r1, r2, r3) = tokio::join!(
        s1.send("echo uno-$((6*7)); stty size\n"),
        s2.send("echo dos-$((6*7)); stty size\n"),
        s3.send("echo tres-$((6*7)); stty size\n"),
    );
    let _ = (r1, r2, r3);
    assert!(s1.wait("uno-42").await, "{}", s1.text());
    assert!(s2.wait("dos-42").await, "{}", s2.text());
    assert!(s3.wait("tres-42").await, "{}", s3.text());
    // Su propio tamaño de TTY.
    assert!(s1.wait("40 120").await, "{}", s1.text());
    assert!(s2.wait("30 90").await, "{}", s2.text());
    assert!(s3.wait("25 100").await, "{}", s3.text());
    for (s, own, others) in [
        (&s1, "uno", ["dos-42", "tres-42"]),
        (&s2, "dos", ["uno-42", "tres-42"]),
        (&s3, "tres", ["uno-42", "dos-42"]),
    ] {
        let t = s.text();
        assert!(t.contains(&format!("{own}-42")));
        for o in others {
            assert!(!t.contains(o), "salida mezclada en «{own}»: {t}");
        }
        // La marca del PID no llega a la terminal.
        let raw = s.out.lock().unwrap().clone();
        assert!(
            !raw.windows(PID_MARK_START.len())
                .any(|w| w == PID_MARK_START),
            "{t}"
        );
    }

    // Redimensionado individual: solo cambia la sesión 1.
    s1.input
        .send(TerminalInput::Resize { cols: 70, rows: 20 })
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    s1.send("stty size\n").await;
    s2.send("stty size\n").await;
    assert!(s1.wait("20 70").await, "{}", s1.text());
    assert!(s2.wait("30 90").await);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !s2.text().contains("20 70"),
        "el tamaño de una sesión cambió el de otra"
    );

    // Cada sesión es un proceso distinto: tres shells en marcha.
    let pa = processes(&docker, &a.id).await;
    let shells_a: std::collections::BTreeSet<_> = pa
        .iter()
        .filter(|(c, _)| c == "sh" || c == "dash")
        .map(|(_, s)| s.clone())
        .collect();
    assert!(shells_a.len() >= 2, "dos sesiones en A: {pa:?}");

    // Las tres se registran por separado.
    for s in [s1, s2, s3] {
        let end = s.close().await;
        assert_eq!(end.reason, EndReason::Closed);
    }
    let execs = log
        .snapshot()
        .into_iter()
        .filter(|r| r.kind == "container.exec" && r.status == OpStatus::Succeeded)
        .count();
    assert_eq!(execs, 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn closing_one_terminal_keeps_the_others_and_leaves_no_processes() {
    if !enabled() {
        return;
    }
    let (e, log, docker) = setup().await;
    let a = Running::start(&docker, "close").await;
    let s1 = Session::open(&e, &a, (100, 30));
    let s2 = Session::open(&e, &a, (100, 30));
    // En la 1: algo en segundo plano y la shell a la espera.
    s1.send("sleep 1001 & echo listo-1\n").await;
    assert!(s1.wait("listo-1").await, "{}", s1.text());
    s2.send("echo listo-2\n").await;
    assert!(s2.wait("listo-2").await);
    assert_eq!(
        count(&processes(&docker, &a.id).await, "sleep"),
        2,
        "principal + segundo plano"
    );

    let end = s1.close().await;
    for r in log.snapshot() {
        eprintln!(
            "DBG {} {:?} {:?} {:?}",
            r.kind, r.status, r.summary, r.error
        );
    }
    assert_eq!(end.reason, EndReason::Closed);
    assert!(
        end.summary
            .contains("se terminaron 1 procesos que seguían en segundo plano"),
        "{}",
        end.summary
    );
    // No queda nada de la sesión 1 (ni la shell ni el sleep): solo el
    // principal, la shell de la sesión 2 y la que lista.
    let left = processes(&docker, &a.id).await;
    assert_eq!(count(&left, "sleep"), 1, "{left:?}");
    assert_eq!(count(&left, "sh"), 2, "{left:?}");
    // La 2 sigue funcionando.
    s2.send("echo sigue-$((1+1))\n").await;
    assert!(s2.wait("sigue-2").await, "{}", s2.text());
    let cleanup = log
        .snapshot()
        .into_iter()
        .find(|r| r.kind == "container.exec.cleanup")
        .expect("limpieza registrada");
    assert_eq!(cleanup.status, OpStatus::Succeeded);
    s2.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_program_that_ignores_ctrl_c_is_terminated_on_close() {
    if !enabled() {
        return;
    }
    let (e, _log, docker) = setup().await;
    let a = Running::start(&docker, "stubborn").await;
    let s = Session::open(&e, &a, (100, 30));
    // Ignora Ctrl+C y ocupa la terminal: Ctrl+D tampoco llega a la shell.
    s.send("trap '' INT; echo terco; sleep 1002\n").await;
    assert!(s.wait("terco").await, "{}", s.text());
    tokio::time::sleep(Duration::from_millis(300)).await;
    let end = s.close().await;
    // La shell no sale por las buenas y la aplicación termina sus procesos. Al
    // cerrar la conexión, Docker cierra el terminal y el núcleo envía SIGHUP al
    // programa en primer plano, que a veces muere solo: el número de procesos
    // que termina la aplicación (1 o 2) depende de esa carrera; lo que importa
    // es que no quede ninguno (comprobado abajo).
    // A veces ese SIGHUP llega antes y la shell sale sola (código 130): no
    // hace falta terminar nada (visto una vez con Docker 26 en la 1.2.0).
    assert!(
        end.summary
            .contains("la shell no salió con Ctrl+C y Ctrl+D y se terminaron sus")
            || end.summary.contains("la shell terminó con código"),
        "{}",
        end.summary
    );
    let left = processes(&docker, &a.id).await;
    assert_eq!(count(&left, "sleep"), 1, "solo el principal: {left:?}");
    assert_eq!(count(&left, "sh"), 1, "solo la que lista: {left:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn stopping_or_removing_the_container_ends_its_terminals_only() {
    if !enabled() {
        return;
    }
    let (e, _log, docker) = setup().await;
    let keep = Running::start(&docker, "keep").await;
    let stop = Running::start(&docker, "stop").await;
    let gone = Running::start(&docker, "gone").await;
    let sk = Session::open(&e, &keep, (100, 30));
    let ss = Session::open(&e, &stop, (100, 30));
    let sg = Session::open(&e, &gone, (100, 30));
    for (s, w) in [(&sk, "k"), (&ss, "s"), (&sg, "g")] {
        s.send(&format!("echo hola-{w}\n")).await;
        assert!(s.wait(&format!("hola-{w}")).await);
    }

    docker
        .stop_container(
            &stop.id,
            Some(StopContainerOptions {
                t: Some(1),
                ..Default::default()
            }),
        )
        .await
        .unwrap();
    let end = tokio::time::timeout(Duration::from_secs(30), ss.handle)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(end.reason, EndReason::Finished);
    assert_eq!(
        end.container,
        Some(ContainerAfter::Stopped),
        "{}",
        end.summary
    );
    assert!(
        end.summary.starts_with("El contenedor se detuvo"),
        "{}",
        end.summary
    );

    docker
        .remove_container(
            &gone.id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await
        .unwrap();
    let end = tokio::time::timeout(Duration::from_secs(30), sg.handle)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(end.reason, EndReason::Finished);
    assert_eq!(
        end.container,
        Some(ContainerAfter::Removed),
        "{}",
        end.summary
    );

    // La del contenedor que sigue en marcha no se ha enterado.
    sk.send("echo aqui-sigo\n").await;
    assert!(sk.wait("aqui-sigo").await, "{}", sk.text());
    assert_eq!(sk.close().await.reason, EndReason::Closed);
}

#[tokio::test(flavor = "multi_thread")]
async fn closing_all_terminals_ends_every_session() {
    if !enabled() {
        return;
    }
    let (e, _log, docker) = setup().await;
    let a = Running::start(&docker, "all-a").await;
    let b = Running::start(&docker, "all-b").await;
    let sessions: Vec<Session> = vec![
        Session::open(&e, &a, (100, 30)),
        Session::open(&e, &a, (100, 30)),
        Session::open(&e, &b, (100, 30)),
        Session::open(&e, &b, (100, 30)),
    ];
    for (n, s) in sessions.iter().enumerate() {
        s.send(&format!("sleep 100{n} & echo en-marcha-{n}\n"))
            .await;
        assert!(s.wait(&format!("en-marcha-{n}")).await, "{}", s.text());
    }
    // Todas a la vez (como al cerrar la ventana).
    for s in &sessions {
        s.cancel.cancel();
    }
    for s in sessions {
        let end = tokio::time::timeout(Duration::from_secs(30), s.handle)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(end.reason, EndReason::Closed, "{}", end.summary);
    }
    for c in [&a, &b] {
        // Solo quedan el proceso principal (sleep infinity) y el que lista.
        let left = processes(&docker, &c.id).await;
        assert_eq!(count(&left, "sleep"), 1, "{left:?}");
        assert_eq!(count(&left, "sh"), 1, "{left:?}");
        assert_eq!(left.len(), 2, "{left:?}");
    }
}
