// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Fase 7 contra Docker real (`CMC_DOCKER_TESTS=1`): proyectos Docker Compose
//! con el CLI oficial (sin shell). Se usa el Compose detectado en el equipo o,
//! con `CMC_TEST_COMPOSE=/ruta/docker-compose`, ese binario independiente (para
//! probar otras versiones). Si no hay Compose, las pruebas se omiten y lo dicen.
//!
//! Cada proyecto tiene un nombre único, todos sus recursos llevan la etiqueta
//! `cmc.test=true` y se eliminan al terminar aunque la prueba falle.

mod common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use cmc_activity::{ActivityLog, OpStatus, Transport};
use cmc_engine::cli::{OutputStream, RunEnd};
use cmc_engine::compose::project::{action_args, ProjectState};
use cmc_engine::{
    ComposeAction, ComposeCli, ConnectionMode, ConnectionState, DownImages, Engine, EngineError,
    EngineManager, ProjectRef, PullPolicy,
};
use common::*;
use tokio_util::sync::CancellationToken;

const SECRET: &str = "valor-secreto-cmc-7";

struct Ctx {
    engine: Engine,
    log: ActivityLog,
    cli: ComposeCli,
    docker: bollard::Docker,
}

async fn ctx() -> Option<Ctx> {
    if !enabled() {
        return None;
    }
    let sock = real_socket().await;
    let docker =
        bollard::Docker::connect_with_unix(&sock, 120, bollard::API_DEFAULT_VERSION).unwrap();
    fixture::ensure_image(&docker).await;
    let log = ActivityLog::new(2000);
    let m = EngineManager::new(log.clone(), ConnectionMode::Custom { socket_path: sock });
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    let cli = match std::env::var("CMC_TEST_COMPOSE") {
        Ok(p) if !p.is_empty() => ComposeCli {
            program: p,
            prefix: vec![],
            version: "CMC_TEST_COMPOSE".into(),
        },
        _ => match m.compose_cli().await {
            Ok(c) => c,
            Err(_) => {
                eprintln!("omitida: Docker Compose no está instalado en este equipo");
                return None;
            }
        },
    };
    eprintln!("Compose: {} {}", cli.display(), cli.version);
    Some(Ctx {
        engine: m.engine().await.unwrap(),
        log,
        cli,
        docker,
    })
}

/// Elimina el proyecto con el mismo Compose (y por etiquetas) aunque la prueba falle.
struct Project {
    dir: tempfile::TempDir,
    name: String,
    cli: ComposeCli,
    sock: String,
}

impl Project {
    fn new(ctx: &Ctx, tag: &str, compose: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let name = format!("cmc-test-c7-{tag}-{}", std::process::id());
        std::fs::write(dir.path().join("compose.yaml"), compose).unwrap();
        std::fs::write(dir.path().join(".env"), format!("SECRET={SECRET}\n")).unwrap();
        Project {
            dir,
            name,
            cli: ctx.cli.clone(),
            sock: ctx.engine.endpoint().host_uri(),
        }
    }

    fn file(&self) -> PathBuf {
        self.dir.path().join("compose.yaml")
    }

    fn reference(&self) -> ProjectRef {
        ProjectRef {
            name: self.name.clone(),
            files: vec![self.file().display().to_string()],
            working_dir: None,
            env_files: vec![],
        }
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::process::Command::new(&self.cli.program)
            .args(&self.cli.prefix)
            .args([
                "-p",
                &self.name,
                "down",
                "--volumes",
                "--rmi",
                "local",
                "--remove-orphans",
                "--timeout",
                "1",
            ])
            .env("DOCKER_HOST", &self.sock)
            .current_dir("/")
            .output();
    }
}

/// Servicios: `web` (imagen de prueba, volumen con nombre, escribe registros),
/// `db` (se construye), `extra` (solo con el perfil `debug`).
fn compose_yaml(dockerfile_sleep: u32) -> String {
    format!(
        r#"services:
  web:
    image: {img}
    command: ["/bin/sh", "-c", "i=0; while true; do echo linea-$$i; i=$$((i+1)); sleep 0.2; done"]
    environment:
      SECRET: ${{SECRET}}
      PLAIN: visible
    volumes:
      - datos:/data
    labels: {{cmc.test: "true"}}
    depends_on: [db]
  db:
    build:
      context: .
      labels: {{cmc.test: "true"}}
    labels: {{cmc.test: "true"}}
  extra:
    image: {img}
    profiles: [debug]
    labels: {{cmc.test: "true"}}
volumes:
  datos:
    labels: {{cmc.test: "true"}}
networks:
  default:
    labels: {{cmc.test: "true"}}
"#,
        img = fixture::IMAGE,
    ) + &format!("# espera en la construcción: {dockerfile_sleep}\n")
}

fn dockerfile(dir: &Path, sleep: u32) {
    // La imagen de prueba no tiene `sleep`: el paso lento de verdad es el propio
    // `/sleeper`, que espera hasta recibir una señal (la construcción solo
    // termina si se cancela).
    let run = if sleep > 0 {
        "RUN echo construyendo && /sleeper\n".to_string()
    } else {
        String::new()
    };
    std::fs::write(
        dir.join("Dockerfile"),
        format!("FROM {}\n{run}LABEL cmc.test=true\n", fixture::IMAGE),
    )
    .unwrap();
}

async fn run(
    ctx: &Ctx,
    p: &ProjectRef,
    a: ComposeAction,
    confirmed: bool,
) -> (cmc_engine::cli::StreamedRun, Vec<String>) {
    let mut lines = Vec::new();
    let r = ctx
        .engine
        .compose_run(&ctx.cli, p, &a, confirmed, CancellationToken::new(), |l| {
            lines.extend(l.into_iter().map(|x| x.text));
            true
        })
        .await
        .unwrap_or_else(|e| panic!("{a:?}: {e}"));
    assert!(
        r.success(),
        "{a:?} falló: {:?}\n{}",
        r.end,
        lines.join("\n")
    );
    (r, lines)
}

async fn project_state(ctx: &Ctx, p: &ProjectRef) -> Option<cmc_engine::ComposeProject> {
    ctx.engine
        .compose_projects(&[])
        .await
        .unwrap()
        .into_iter()
        .find(|x| x.reference.name == p.name)
}

fn no_secret_in_log(log: &ActivityLog) {
    let json = serde_json::to_string(&log.snapshot()).unwrap();
    assert!(
        !json.contains(SECRET),
        "el valor de una variable llegó al registro de actividad"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn full_lifecycle_with_build_profiles_and_confirmation() {
    let Some(ctx) = ctx().await else { return };
    let proj = Project::new(&ctx, "ciclo", &compose_yaml(0));
    dockerfile(proj.dir.path(), 0);
    let p = proj.reference();

    // Modelo: todos los servicios (también los de perfiles), sin valores de variables.
    let cfg = ctx.engine.compose_config(&ctx.cli, &p, None).await.unwrap();
    assert!(cfg.error.is_none(), "{:?}", cfg.error);
    let model = cfg.model.unwrap();
    assert_eq!(
        model
            .services
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["db", "extra", "web"]
    );
    assert_eq!(model.profiles, ["debug"]);
    let web = model.services.iter().find(|s| s.name == "web").unwrap();
    assert_eq!(web.env_keys, ["PLAIN", "SECRET"]);
    assert!(model
        .services
        .iter()
        .find(|s| s.name == "db")
        .unwrap()
        .build
        .is_some());
    assert!(
        cfg.resolved.unwrap().contains(SECRET),
        "la configuración resuelta sí trae el valor (solo para la interfaz)"
    );
    no_secret_in_log(&ctx.log);

    // Desplegar: construye db, crea red y volumen, y no arranca el servicio del perfil.
    let up = ComposeAction::Up {
        services: vec![],
        build: false,
        pull: PullPolicy::Missing,
        force_recreate: false,
        remove_orphans: false,
        profiles: vec![],
    };
    let (_, lines) = run(&ctx, &p, up.clone(), false).await;
    assert!(!lines.is_empty(), "se recibe la salida en vivo");
    let st = project_state(&ctx, &p)
        .await
        .expect("proyecto descubierto por sus etiquetas");
    assert_eq!(st.state, ProjectState::Running);
    assert_eq!(st.services, ["db", "web"]);
    assert_eq!(st.reference.files, [proj.file().display().to_string()]);
    assert_eq!(
        st.reference.working_dir.as_deref(),
        Some(proj.dir.path().to_str().unwrap())
    );
    let rec = ctx
        .log
        .snapshot()
        .into_iter()
        .rev()
        .find(|r| r.kind == "compose.up")
        .unwrap();
    assert_eq!(rec.status, OpStatus::Succeeded);
    assert_eq!(rec.exit_code, Some(0));
    match &rec.transport {
        Transport::Cli { program, args } => {
            assert_eq!(program, &ctx.cli.program);
            assert_eq!(args, &action_args(&ctx.cli, &p, &up));
        }
        other => panic!("{other:?}"),
    }
    let vol = format!("{}_datos", p.name);
    ctx.docker
        .inspect_volume(&vol)
        .await
        .expect("volumen del proyecto");

    // Parar y arrancar.
    run(&ctx, &p, ComposeAction::Stop { services: vec![] }, false).await;
    assert_eq!(
        project_state(&ctx, &p).await.unwrap().state,
        ProjectState::Stopped
    );
    run(
        &ctx,
        &p,
        ComposeAction::Start {
            services: vec!["db".into()],
        },
        false,
    )
    .await;
    let st = project_state(&ctx, &p).await.unwrap();
    assert_eq!(
        (st.state, st.running, st.total),
        (ProjectState::Partial, 1, 2)
    );
    run(&ctx, &p, ComposeAction::Restart { services: vec![] }, false).await;
    assert_eq!(
        project_state(&ctx, &p).await.unwrap().state,
        ProjectState::Running
    );

    // Con el perfil se añade el servicio extra.
    run(
        &ctx,
        &p,
        ComposeAction::Up {
            services: vec!["extra".into()],
            build: false,
            pull: PullPolicy::Never,
            force_recreate: false,
            remove_orphans: false,
            profiles: vec!["debug".into()],
        },
        false,
    )
    .await;
    assert_eq!(
        project_state(&ctx, &p).await.unwrap().services,
        ["db", "extra", "web"]
    );

    // Una sola acción a la vez por proyecto.
    {
        let _held = ctx
            .engine
            .locks()
            .acquire(&format!("compose:{}", p.name))
            .unwrap();
        let before = ctx.log.len();
        let e = ctx
            .engine
            .compose_run(
                &ctx.cli,
                &p,
                &ComposeAction::Stop { services: vec![] },
                false,
                CancellationToken::new(),
                |_| true,
            )
            .await
            .unwrap_err();
        assert!(matches!(e, EngineError::Busy { .. }), "{e:?}");
        assert_eq!(ctx.log.len(), before, "no se ejecutó nada");
    }

    // Eliminar exige confirmación y, sin ella, no se ejecuta nada.
    let down = ComposeAction::Down {
        volumes: true,
        images: DownImages::Local,
        remove_orphans: true,
    };
    let before = ctx.log.len();
    let e = ctx
        .engine
        .compose_run(&ctx.cli, &p, &down, false, CancellationToken::new(), |_| {
            true
        })
        .await
        .unwrap_err();
    assert_eq!(e, EngineError::NotConfirmed);
    assert_eq!(ctx.log.len(), before);
    let (_, out) = run(&ctx, &p, down, true).await;
    let left = project_state(&ctx, &p).await;
    assert!(
        left.is_none(),
        "sin contenedores: {left:?}\n{}",
        out.join("\n")
    );
    assert!(
        ctx.docker.inspect_volume(&vol).await.is_err(),
        "--volumes eliminó el volumen"
    );
    assert!(
        ctx.docker
            .inspect_image(&format!("{}-db", p.name))
            .await
            .is_err(),
        "--rmi local eliminó la imagen construida"
    );
    no_secret_in_log(&ctx.log);
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_without_files_and_refusal_of_actions_that_need_them() {
    let Some(ctx) = ctx().await else { return };
    let proj = Project::new(&ctx, "sinarchivos", &compose_yaml(0));
    dockerfile(proj.dir.path(), 0);
    let p = proj.reference();
    run(
        &ctx,
        &p,
        ComposeAction::Up {
            services: vec!["web".into()],
            build: false,
            pull: PullPolicy::Missing,
            force_recreate: false,
            remove_orphans: false,
            profiles: vec![],
        },
        false,
    )
    .await;
    // El archivo desaparece (carpeta borrada o movida).
    let moved = proj.dir.path().join("movido.yaml");
    std::fs::rename(proj.file(), &moved).unwrap();
    let st = project_state(&ctx, &p).await.unwrap();
    assert_eq!(st.missing_files, [proj.file().display().to_string()]);

    // Parar funciona solo con las etiquetas, sin -f.
    run(&ctx, &p, ComposeAction::Stop { services: vec![] }, false).await;
    let rec = ctx
        .log
        .snapshot()
        .into_iter()
        .rev()
        .find(|r| r.kind == "compose.stop")
        .unwrap();
    match &rec.transport {
        Transport::Cli { args, .. } => assert!(!args.contains(&"-f".to_string()), "{args:?}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        project_state(&ctx, &p).await.unwrap().state,
        ProjectState::Stopped
    );

    // Desplegar necesita el archivo: se rechaza sin ejecutar nada.
    let before = ctx.log.len();
    let e = ctx
        .engine
        .compose_run(
            &ctx.cli,
            &p,
            &ComposeAction::Up {
                services: vec![],
                build: false,
                pull: PullPolicy::Missing,
                force_recreate: false,
                remove_orphans: false,
                profiles: vec![],
            },
            false,
            CancellationToken::new(),
            |_| true,
        )
        .await
        .unwrap_err();
    assert!(matches!(e, EngineError::InvalidInput { .. }), "{e:?}");
    assert_eq!(ctx.log.len(), before);

    run(
        &ctx,
        &p,
        ComposeAction::Down {
            volumes: true,
            images: DownImages::None,
            remove_orphans: false,
        },
        true,
    )
    .await;
    assert!(project_state(&ctx, &p).await.is_none());
    assert!(
        ctx.docker
            .inspect_volume(&format!("{}_datos", p.name))
            .await
            .is_err(),
        "sin archivos, --volumes también elimina el volumen del proyecto"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn unsaved_content_is_validated_without_touching_the_file() {
    let Some(ctx) = ctx().await else { return };
    let proj = Project::new(&ctx, "validar", &compose_yaml(0));
    dockerfile(proj.dir.path(), 0);
    let p = proj.reference();
    let original = std::fs::read_to_string(proj.file()).unwrap();

    let bad = "services:\n  web:\n    image: x\n    ports: 12\n";
    let cfg = ctx
        .engine
        .compose_config(&ctx.cli, &p, Some((0, bad.into())))
        .await
        .unwrap();
    assert!(cfg.model.is_none());
    let err = cfg.error.expect("error de Compose");
    assert!(err.contains("ports"), "{err}");
    let rec = ctx
        .log
        .snapshot()
        .into_iter()
        .rev()
        .find(|r| r.kind == "compose.config")
        .unwrap();
    assert_eq!(rec.status, OpStatus::Failed);
    match &rec.transport {
        Transport::Cli { args, .. } => {
            assert!(args.windows(2).any(|w| w == ["-f", "-"]), "{args:?}")
        }
        other => panic!("{other:?}"),
    }

    let undefined = "services:\n  web:\n    image: x\n    depends_on: [nope]\n";
    let cfg = ctx
        .engine
        .compose_config(&ctx.cli, &p, Some((0, undefined.into())))
        .await
        .unwrap();
    assert!(cfg.error.unwrap().contains("nope"));

    let good = format!("services:\n  solo:\n    image: {}\n", fixture::IMAGE);
    let cfg = ctx
        .engine
        .compose_config(&ctx.cli, &p, Some((0, good)))
        .await
        .unwrap();
    assert_eq!(cfg.model.unwrap().services[0].name, "solo");
    assert_eq!(
        std::fs::read_to_string(proj.file()).unwrap(),
        original,
        "el archivo no se modificó"
    );

    // Entradas peligrosas: se rechazan antes de ejecutar nada.
    let before = ctx.log.len();
    let e = ctx
        .engine
        .compose_run(
            &ctx.cli,
            &p,
            &ComposeAction::Stop {
                services: vec!["--rmi".into()],
            },
            false,
            CancellationToken::new(),
            |_| true,
        )
        .await
        .unwrap_err();
    assert!(matches!(e, EngineError::InvalidInput { .. }));
    let mut evil = p.clone();
    evil.name = "x; rm -rf /".into();
    assert!(ctx
        .engine
        .compose_config(&ctx.cli, &evil, None)
        .await
        .is_err());
    assert_eq!(ctx.log.len(), before);
}

#[tokio::test(flavor = "multi_thread")]
async fn logs_follow_in_real_time_and_close_cleanly() {
    let Some(ctx) = ctx().await else { return };
    let proj = Project::new(&ctx, "logs", &compose_yaml(0));
    dockerfile(proj.dir.path(), 0);
    let p = proj.reference();
    run(
        &ctx,
        &p,
        ComposeAction::Up {
            services: vec!["web".into()],
            build: false,
            pull: PullPolicy::Missing,
            force_recreate: false,
            remove_orphans: false,
            profiles: vec![],
        },
        false,
    )
    .await;
    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    let mut seen = Vec::new();
    let r = tokio::time::timeout(
        Duration::from_secs(60),
        ctx.engine
            .compose_logs(&ctx.cli, &p, &["web".into()], 5, true, cancel, |l| {
                for x in l {
                    if x.stream == OutputStream::Stdout && x.text.contains("linea-") {
                        seen.push(x.text);
                    }
                }
                if seen.len() >= 12 {
                    c2.cancel();
                }
                true
            }),
    )
    .await
    .expect("los registros llegan en vivo")
    .unwrap();
    assert_eq!(r.end, RunEnd::Stopped);
    assert!(seen.len() >= 12);
    assert!(
        seen[0].contains("web-1"),
        "prefijo del servicio: {}",
        seen[0]
    );
    let rec = ctx
        .log
        .snapshot()
        .into_iter()
        .rev()
        .find(|r| r.kind == "compose.logs")
        .unwrap();
    assert_eq!(rec.status, OpStatus::Succeeded);
    assert!(rec.summary.unwrap().starts_with("Cerrado por el usuario"));
    assert!(
        rec.stdout.is_none(),
        "las líneas no se guardan en el registro"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelling_a_slow_up_stops_compose_and_is_recorded() {
    let Some(ctx) = ctx().await else { return };
    let proj = Project::new(&ctx, "cancelar", &compose_yaml(60));
    dockerfile(proj.dir.path(), 60);
    let p = proj.reference();
    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    let started = std::time::Instant::now();
    // Se cancela en cuanto Compose empieza a construir (o a los 8 s como muy tarde).
    let killer = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(8)).await;
        c2.cancel();
    });
    let c3 = cancel.clone();
    let mut lines = Vec::new();
    let r = tokio::time::timeout(
        Duration::from_secs(90),
        ctx.engine.compose_run(
            &ctx.cli,
            &p,
            &ComposeAction::Up {
                services: vec![],
                build: true,
                pull: PullPolicy::Never,
                force_recreate: false,
                remove_orphans: false,
                profiles: vec![],
            },
            false,
            cancel,
            |l| {
                if l.iter().any(|x| x.text.contains("construyendo")) {
                    c3.cancel();
                }
                lines.extend(l.into_iter().map(|x| x.text));
                true
            },
        ),
    )
    .await
    .expect("la cancelación detuvo Compose (la construcción no termina sola)")
    .unwrap();
    killer.abort();
    assert_eq!(
        r.end,
        RunEnd::Stopped,
        "salida de Compose:\n{}",
        lines.join("\n")
    );
    assert!(
        started.elapsed() < Duration::from_secs(40),
        "se detuvo sin esperar a la construcción"
    );
    let rec = ctx
        .log
        .snapshot()
        .into_iter()
        .rev()
        .find(|r| r.kind == "compose.up")
        .unwrap();
    assert_eq!(rec.status, OpStatus::Cancelled);
    // No queda ningún proceso de Compose de este proyecto.
    tokio::time::sleep(Duration::from_secs(1)).await;
    let ps = std::process::Command::new("pgrep")
        .args(["-f", &p.name])
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&ps.stdout).trim().is_empty(),
        "procesos de Compose vivos"
    );
    // El proyecto no quedó en marcha.
    assert!(
        project_state(&ctx, &p)
            .await
            .map(|s| s.running)
            .unwrap_or(0)
            == 0
    );
}

/// Estado real según Docker: `docker compose ps -a --format json` (servicio →
/// estado) e inspect de cada uno de esos contenedores.
fn docker_truth(ctx: &Ctx, project: &str) -> Vec<(String, String, String)> {
    let out = std::process::Command::new(&ctx.cli.program)
        .args(&ctx.cli.prefix)
        .args(["-p", project, "ps", "-a", "--format", "json"])
        .env("DOCKER_HOST", ctx.engine.endpoint().host_uri())
        .current_dir("/")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    // Compose 2.21+ escribe un objeto por línea; versiones anteriores, un array.
    let items: Vec<serde_json::Value> = if text.starts_with('[') {
        serde_json::from_str(&text).unwrap()
    } else {
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    };
    let mut v: Vec<(String, String, String)> = items
        .iter()
        .map(|i| {
            let name = i["Name"].as_str().unwrap().to_owned();
            let inspect = std::process::Command::new("docker")
                .args(["inspect", "--format", "{{.State.Status}}", &name])
                .env("DOCKER_HOST", ctx.engine.endpoint().host_uri())
                .output()
                .unwrap();
            (
                i["Service"].as_str().unwrap().to_owned(),
                i["State"].as_str().unwrap().to_owned(),
                String::from_utf8_lossy(&inspect.stdout).trim().to_owned(),
            )
        })
        .collect();
    v.sort();
    v
}

/// El estado de la aplicación justo después de la acción coincide con Docker:
/// mismo número de contenedores, mismos en marcha, y el estado global correcto.
async fn assert_state(ctx: &Ctx, p: &ProjectRef, expected: ProjectState, running: u32, total: u32) {
    let st = project_state(ctx, p).await;
    let truth = docker_truth(ctx, &p.name);
    for (svc, ps, inspect) in &truth {
        assert_eq!(
            ps, inspect,
            "compose ps e inspect discrepan para {svc}: {truth:?}"
        );
    }
    let real_running = truth.iter().filter(|t| t.2 == "running").count() as u32;
    match st {
        None => {
            assert_eq!(expected, ProjectState::NotDeployed);
            assert!(
                truth.is_empty(),
                "la aplicación no ve contenedores y Docker sí: {truth:?}"
            );
        }
        Some(s) => {
            assert_eq!(
                (s.state, s.running, s.total),
                (expected, running, total),
                "estado de la aplicación; Docker dice {truth:?}; contenedores vistos {:?}",
                s.containers
                    .iter()
                    .map(|c| (&c.service, &c.state))
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                (real_running, truth.len() as u32),
                (running, total),
                "Docker: {truth:?}"
            );
            let mut seen: Vec<_> = s
                .containers
                .iter()
                .map(|c| (c.service.clone(), c.state.clone()))
                .collect();
            seen.sort();
            let real: Vec<_> = truth.iter().map(|t| (t.0.clone(), t.2.clone())).collect();
            assert_eq!(seen, real, "cada contenedor con su estado real");
        }
    }
}

/// Regresión de D-056 (fallo en Debian 13, Docker 26.1.5): tras cada operación
/// individual o global el estado coincide con Docker, sin "Parcial" falsos.
/// Cubre: sin desplegar, todos en marcha, parada de un servicio (parcial),
/// parada de todos, arranque, servicio de perfil, servicio del archivo aún sin
/// crear (no altera el estado) y proyecto sin archivo.
#[tokio::test(flavor = "multi_thread")]
async fn project_state_matches_docker_after_every_operation() {
    let Some(ctx) = ctx().await else { return };
    let proj = Project::new(&ctx, "estados", &compose_yaml(0));
    dockerfile(proj.dir.path(), 0);
    let p = proj.reference();
    let up = |services: Vec<&str>, profiles: Vec<&str>| ComposeAction::Up {
        services: services.into_iter().map(String::from).collect(),
        build: false,
        pull: PullPolicy::Missing,
        force_recreate: false,
        remove_orphans: false,
        profiles: profiles.into_iter().map(String::from).collect(),
    };
    let stop = |s: Vec<&str>| ComposeAction::Stop {
        services: s.into_iter().map(String::from).collect(),
    };
    let start = |s: Vec<&str>| ComposeAction::Start {
        services: s.into_iter().map(String::from).collect(),
    };

    assert_state(&ctx, &p, ProjectState::NotDeployed, 0, 0).await;
    // web y db en marcha; extra (perfil) aún sin crear: no altera el estado.
    run(&ctx, &p, up(vec![], vec![]), false).await;
    assert_state(&ctx, &p, ProjectState::Running, 2, 2).await;
    for _ in 0..3 {
        // Parada individual: parcial, nunca "detenido" con un servicio activo.
        run(&ctx, &p, stop(vec!["web"]), false).await;
        assert_state(&ctx, &p, ProjectState::Partial, 1, 2).await;
        // Parada de todos: detenido, nunca "parcial" sin servicios activos.
        run(&ctx, &p, stop(vec![]), false).await;
        assert_state(&ctx, &p, ProjectState::Stopped, 0, 2).await;
        run(&ctx, &p, start(vec!["db"]), false).await;
        assert_state(&ctx, &p, ProjectState::Partial, 1, 2).await;
        run(&ctx, &p, start(vec![]), false).await;
        assert_state(&ctx, &p, ProjectState::Running, 2, 2).await;
    }
    // Servicio de perfil desplegado: cuenta como los demás; parar y reiniciar
    // el proyecto lo incluyen (D-054).
    run(&ctx, &p, up(vec!["extra"], vec!["debug"]), false).await;
    assert_state(&ctx, &p, ProjectState::Running, 3, 3).await;
    run(&ctx, &p, stop(vec![]), false).await;
    assert_state(&ctx, &p, ProjectState::Stopped, 0, 3).await;
    run(&ctx, &p, ComposeAction::Restart { services: vec![] }, false).await;
    assert_state(&ctx, &p, ProjectState::Running, 3, 3).await;
    // Sin archivo: las operaciones van por etiquetas y el estado sigue siendo exacto.
    std::fs::rename(proj.file(), proj.dir.path().join("fuera.yaml")).unwrap();
    run(&ctx, &p, stop(vec!["extra"]), false).await;
    assert_state(&ctx, &p, ProjectState::Partial, 2, 3).await;
    run(&ctx, &p, stop(vec![]), false).await;
    assert_state(&ctx, &p, ProjectState::Stopped, 0, 3).await;
    assert_eq!(
        project_state(&ctx, &p).await.unwrap().missing_files,
        [proj.file().display().to_string()]
    );
    run(&ctx, &p, start(vec![]), false).await;
    assert_state(&ctx, &p, ProjectState::Running, 3, 3).await;
    run(
        &ctx,
        &p,
        ComposeAction::Down {
            volumes: true,
            images: DownImages::Local,
            remove_orphans: false,
        },
        true,
    )
    .await;
    assert_state(&ctx, &p, ProjectState::NotDeployed, 0, 0).await;
    // Cada acción que cambia contenedores confirmó su estado con inspect.
    let checks = ctx
        .log
        .snapshot()
        .into_iter()
        .filter(|r| r.kind == "container.state")
        .count();
    assert!(checks >= 30, "confirmaciones con inspect: {checks}");
}
