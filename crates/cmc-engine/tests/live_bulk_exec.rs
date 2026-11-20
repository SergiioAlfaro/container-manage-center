// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Comandos masivos (1.11.0) contra Docker real (`CMC_DOCKER_TESTS=1`).
//!
//! Contenedores temporales propios, con las etiquetas `cmc.test=true` y
//! `cmc.exec-test=<ejecución>`, que se eliminan al terminar (también si la
//! prueba falla). Nunca se usan contenedores del usuario.
// Pruebas contra Docker Engine por socket Unix (Linux); no en Windows (D-114).
#![cfg(unix)]

mod common;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use bollard::models::{ContainerCreateBody, HostConfig};
use bollard::query_parameters::{
    CreateContainerOptions, RemoveContainerOptions, StartContainerOptions,
};
use cmc_activity::{ActivityLog, OpStatus};
use cmc_engine::bulk::{BulkStage, SkipReason};
use cmc_engine::bulk_exec::{BulkCommand, ExecCommand, ExecDone, ExecEnd, ExecOutput};
use cmc_engine::manager::EngineManager;
use cmc_engine::{ConnectionMode, ConnectionState, Engine};
use common::*;
use futures_util::future::join_all;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

struct Cleanup {
    docker: bollard::Docker,
    run: String,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        let d = self.docker.clone();
        let run = self.run.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                let mut filters = HashMap::new();
                filters.insert("label".to_owned(), vec![format!("cmc.exec-test={run}")]);
                let list = d
                    .list_containers(Some(bollard::query_parameters::ListContainersOptions {
                        all: true,
                        filters: Some(filters),
                        ..Default::default()
                    }))
                    .await
                    .unwrap_or_default();
                for c in list {
                    let _ = d
                        .remove_container(
                            c.id.as_deref().unwrap_or_default(),
                            Some(RemoveContainerOptions {
                                force: true,
                                ..Default::default()
                            }),
                        )
                        .await;
                }
            });
        })
        .join()
        .unwrap();
    }
}

async fn create(
    docker: &bollard::Docker,
    run: &str,
    name: &str,
    image: &str,
    start: bool,
) -> String {
    let labels = HashMap::from([
        ("cmc.test".to_owned(), "true".to_owned()),
        ("cmc.exec-test".to_owned(), run.to_owned()),
    ]);
    let id = docker
        .create_container(
            Some(CreateContainerOptions {
                name: Some(format!("cmc-exec-{run}-{name}")),
                ..Default::default()
            }),
            ContainerCreateBody {
                image: Some(image.into()),
                labels: Some(labels),
                host_config: Some(HostConfig::default()),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .id;
    if start {
        docker
            .start_container(&id, None::<StartContainerOptions>)
            .await
            .unwrap();
    }
    id
}

fn script(interpreter: &str, text: &str) -> BulkCommand {
    BulkCommand {
        command: ExecCommand::Script {
            interpreter: interpreter.into(),
            flag: "-c".into(),
            script: text.into(),
        },
        user: None,
        workdir: None,
        timeout_secs: 30,
    }
}

fn direct(program: &str, args: &[&str]) -> BulkCommand {
    BulkCommand {
        command: ExecCommand::Direct {
            program: program.into(),
            args: args.iter().map(|s| (*s).to_owned()).collect(),
        },
        user: None,
        workdir: None,
        timeout_secs: 30,
    }
}

fn ran(r: Result<ExecDone, cmc_engine::bulk::ItemError>) -> ExecOutput {
    match r {
        Ok(ExecDone::Ran(o)) => o,
        other => panic!("se esperaba una ejecución: {other:?}"),
    }
}

/// Reenvía un socket Unix a otro; al abortar la tarea se corta la conexión
/// (simula la pérdida de conexión con Docker a mitad de una ejecución).
async fn proxy(real: String, path: std::path::PathBuf) -> tokio::task::JoinHandle<()> {
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    tokio::spawn(async move {
        let mut conns = tokio::task::JoinSet::new();
        loop {
            let (mut a, _) = listener.accept().await.unwrap();
            let real = real.clone();
            conns.spawn(async move {
                let mut b = tokio::net::UnixStream::connect(real).await.unwrap();
                let _ = tokio::io::copy_bidirectional(&mut a, &mut b).await;
            });
        }
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn bulk_commands_on_real_temporary_containers() {
    if !enabled() {
        return;
    }
    let sock = real_socket().await;
    let docker =
        bollard::Docker::connect_with_unix(&sock, 60, bollard::API_DEFAULT_VERSION).unwrap();
    fixture::ensure_image(&docker).await;
    fixture::ensure_shell_image(&docker).await;
    fixture::ensure_noshell_image(&docker).await;
    let run = format!(
        "{}-{}",
        std::process::id(),
        &Uuid::new_v4().to_string()[..8]
    );
    let _cleanup = Cleanup {
        docker: docker.clone(),
        run: run.clone(),
    };

    let log = ActivityLog::new(2000);
    let m = common::manager(log.clone(), sock.clone());
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    let batch = Uuid::new_v4();
    let base = m.engine().await.unwrap();
    let e: Engine = base.with_activity(base.activity().with_batch(batch));
    let none = CancellationToken::new;

    // Shell con sleep y cat; otra solo con /bin/sh; otra sin ninguna shell;
    // y una detenida.
    let shell = create(&docker, &run, "shell", fixture::SHELL_IMAGE, true).await;
    let sh_only = create(&docker, &run, "sh", fixture::IMAGE, true).await;
    let noshell = create(&docker, &run, "noshell", fixture::NOSHELL_IMAGE, true).await;
    let stopped = create(&docker, &run, "parado", fixture::SHELL_IMAGE, false).await;

    // 1. El mismo comando en dos contenedores a la vez: stdout, stderr y código.
    let cmd = script("/bin/sh", "echo salida; echo error >&2; exit 3");
    let r = join_all([&shell, &sh_only].map(|id| e.bulk_exec(&cmd, id, "x", none()))).await;
    for x in r {
        let o = ran(x);
        assert_eq!(o.end, ExecEnd::Exited);
        assert_eq!(o.exit_code, Some(3));
        assert_eq!(o.stdout, "salida\n");
        assert_eq!(o.stderr, "error\n");
        assert!(!o.launch_failed);
    }

    // 2. Argumentos: llegan tal cual, sin ninguna shell del equipo.
    let args = direct(
        "/bin/sh",
        &[
            "-c",
            r#"for a in "$@"; do printf '[%s]' "$a"; done"#,
            "x",
            "a b",
            "$(id)",
            "; rm -rf /",
        ],
    );
    let o = ran(e.bulk_exec(&args, &shell, "x", none()).await);
    assert_eq!(o.stdout, "[a b][$(id)][; rm -rf /]");
    assert_eq!(o.exit_code, Some(0));

    // 3. Secuencia de varias líneas.
    let multi = script("/bin/sh", "set -e\na=uno\nb=dos\necho \"$a\"\necho \"$b\"");
    let o = ran(e.bulk_exec(&multi, &sh_only, "x", none()).await);
    assert_eq!(o.stdout, "uno\ndos\n");

    // 4. Intérprete que no existe en la imagen: Docker lo dice, no se inventa otro.
    let o = ran(e
        .bulk_exec(&script("/bin/bash", "echo hola"), &sh_only, "x", none())
        .await);
    assert_eq!(o.exit_code, Some(127));
    assert!(o.launch_failed, "{o:?}");
    assert!(o.stdout.contains("bash") || o.stderr.contains("bash"));

    // 5. Contenedor sin shell: con intérprete falla; un ejecutable directo funciona.
    let o = ran(e
        .bulk_exec(&script("/bin/sh", "echo"), &noshell, "x", none())
        .await);
    assert!(o.launch_failed);
    let o = ran(e
        .bulk_exec(&direct("/sleeper", &["exit", "4"]), &noshell, "x", none())
        .await);
    assert_eq!(o.exit_code, Some(4));
    assert!(!o.launch_failed);

    // 6. Usuario y directorio de trabajo.
    let mut who = direct("/bin/cat", &["/proc/self/status"]);
    who.user = Some("65534:65534".into());
    let o = ran(e.bulk_exec(&who, &shell, "x", none()).await);
    assert!(
        o.stdout
            .lines()
            .any(|l| l.starts_with("Uid:") && l.contains("65534")),
        "{}",
        o.stdout
    );
    let mut pwd = script("/bin/sh", "pwd");
    pwd.workdir = Some("/tmp".into());
    assert_eq!(
        ran(e.bulk_exec(&pwd, &shell, "x", none()).await).stdout,
        "/tmp\n"
    );
    let mut nobody = direct("/bin/cat", &["/proc/self/status"]);
    nobody.user = Some("no-existe".into());
    let err = e.bulk_exec(&nobody, &shell, "x", none()).await.unwrap_err();
    assert_eq!(
        err.stage,
        BulkStage::Check,
        "Docker lo rechaza al crear: {err:?}"
    );
    assert!(!err.outcome_unknown());

    // 7. Tiempo máximo: se deja de esperar y se informa de que sigue vivo.
    let mut slow = direct("/bin/sleep", &["30"]);
    slow.timeout_secs = 2;
    let t0 = Instant::now();
    let o = ran(e.bulk_exec(&slow, &shell, "x", none()).await);
    assert_eq!(o.end, ExecEnd::TimedOut);
    assert_eq!(o.still_running, Some(true));
    assert_eq!(o.exit_code, None);
    assert!(t0.elapsed() < Duration::from_secs(10));

    // 8. Interrupción pedida: igual, sin matar ni reiniciar el contenedor.
    let stop = CancellationToken::new();
    let s2 = stop.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(800)).await;
        s2.cancel();
    });
    let o = ran(e
        .bulk_exec(&direct("/bin/sleep", &["30"]), &shell, "x", stop)
        .await);
    assert_eq!(o.end, ExecEnd::Interrupted);
    let st = docker.inspect_container(&shell, None).await.unwrap();
    assert_eq!(
        st.state.unwrap().running,
        Some(true),
        "el contenedor sigue en marcha"
    );

    // 9. Detenido o eliminado: no se ejecuta nada.
    let r = e.bulk_exec(&cmd, &stopped, "x", none()).await.unwrap();
    assert!(
        matches!(
            r,
            ExecDone::NotRun {
                reason: SkipReason::State { .. }
            }
        ),
        "{r:?}"
    );
    docker
        .remove_container(&stopped, None::<RemoveContainerOptions>)
        .await
        .unwrap();
    let r = e.bulk_exec(&cmd, &stopped, "x", none()).await.unwrap();
    assert_eq!(
        r,
        ExecDone::NotRun {
            reason: SkipReason::Gone
        }
    );

    // 10. Fallo parcial: uno falla y los demás terminan bien.
    let ok = direct("/bin/sh", &["-c", "echo ok"]);
    let r =
        join_all([&shell, &noshell, &sh_only].map(|id| e.bulk_exec(&ok, id, "x", none()))).await;
    let outs: Vec<_> = r.into_iter().map(ran).collect();
    assert_eq!(outs[0].exit_code, Some(0));
    assert!(outs[1].launch_failed);
    assert_eq!(outs[2].stdout, "ok\n");

    // 11. Pérdida de conexión a mitad de una ejecución: error, nunca correcto,
    //     y sin reintentos (no hay ninguna otra ejecución de ese comando).
    let dir = tempfile::tempdir().unwrap();
    let psock = dir.path().join("docker.sock");
    let p = proxy(sock.clone(), psock.clone()).await;
    let plog = ActivityLog::new(100);
    let pm = EngineManager::new(
        plog.clone(),
        ConnectionMode::Custom {
            socket_path: psock.to_string_lossy().into_owned(),
        },
    );
    assert!(matches!(
        pm.connect().await,
        ConnectionState::Connected { .. }
    ));
    let pe = pm.engine().await.unwrap();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1500)).await;
        p.abort();
    });
    let err = pe
        .bulk_exec(&direct("/bin/sleep", &["20"]), &shell, "x", none())
        .await
        .unwrap_err();
    assert_eq!(err.stage, BulkStage::Execute, "{err:?}");
    assert!(
        err.outcome_unknown(),
        "se envió: el resultado es desconocido"
    );
    let runs: Vec<_> = plog
        .snapshot()
        .into_iter()
        .filter(|r| r.kind == "container.exec.run")
        .collect();
    assert_eq!(runs.len(), 1, "sin reintentos");
    assert_eq!(runs[0].status, OpStatus::Failed);

    // 12. Registro: cada ejecución con el lote, el usuario y el comando
    //     equivalente; nunca la salida.
    let recs: Vec<_> = log
        .snapshot()
        .into_iter()
        .filter(|r| r.kind == "container.exec.run")
        .collect();
    assert!(recs.len() >= 10);
    assert!(recs.iter().all(|r| r.batch == Some(batch)));
    assert!(recs.iter().any(|r| r
        .equivalent_command
        .as_deref()
        .is_some_and(|c| c.contains("exec --user 65534:65534"))));
    assert!(recs
        .iter()
        .all(|r| r.stdout.is_none() && r.stderr.is_none()));
    assert!(recs
        .iter()
        .any(|r| r.status == OpStatus::Failed && r.exit_code == Some(3)));
    assert!(recs
        .iter()
        .any(|r| r.status == OpStatus::Succeeded && r.exit_code == Some(0)));
}
