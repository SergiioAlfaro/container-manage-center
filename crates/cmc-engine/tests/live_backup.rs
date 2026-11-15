// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! E4 (1.4.0) contra Docker real (`CMC_DOCKER_TESTS=1`): copias y
//! restauraciones de volúmenes, bases de datos, configuración de contenedores y
//! proyectos Compose.
//!
//! Criterios de aceptación que se comprueban aquí:
//! - copia correcta (mismo contenido, permisos y propietarios tras restaurar);
//! - una copia interrumpida nunca se da por buena;
//! - una suma alterada se detecta y la copia no se restaura;
//! - restauración completa y selectiva;
//! - datos coherentes de MariaDB y PostgreSQL con escrituras simultáneas;
//! - archivos 0600 y carpetas 0700.
//!
//! Las bases de datos usan las imágenes de prueba de
//! `scripts/dev/build-db-test-images.sh` (o `CMC_TEST_MARIADB_IMAGE`,
//! `CMC_TEST_POSTGRES_IMAGE`, `CMC_TEST_REDIS_IMAGE`). Todo lo creado lleva
//! `cmc-test-bk-` en el nombre y se elimina al terminar.

mod common;

use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use bollard::exec::{StartExecOptions, StartExecResults};
use bollard::models::{ContainerCreateBody, ExecConfig, HostConfig};
use bollard::query_parameters::{
    CreateContainerOptions, DownloadFromContainerOptions, InspectContainerOptions,
    ListContainersOptions, RemoveContainerOptions, RemoveVolumeOptions, UploadToContainerOptions,
};
use cmc_activity::ActivityLog;
use cmc_engine::backup::store::{self, BackupTarget, LocalTarget};
use cmc_engine::backup::*;
use cmc_engine::compose::project::ProjectRef;
use cmc_engine::ConnectionState;
use common::*;
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

fn img(var: &str, default: &str) -> String {
    std::env::var(var).unwrap_or_else(|_| default.to_owned())
}

/// Elimina contenedores y volúmenes de la prueba aunque falle.
struct Guard {
    docker: bollard::Docker,
    containers: Vec<String>,
    volumes: Vec<String>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        let d = self.docker.clone();
        let cs = std::mem::take(&mut self.containers);
        let vs = std::mem::take(&mut self.volumes);
        let _ = std::thread::spawn(move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async move {
                    for c in cs {
                        let _ = d
                            .remove_container(
                                &c,
                                // También los volúmenes anónimos de las imágenes de bases de datos.
                                Some(RemoveContainerOptions {
                                    force: true,
                                    v: true,
                                    ..Default::default()
                                }),
                            )
                            .await;
                    }
                    for v in vs {
                        let _ = d
                            .remove_volume(&v, Some(RemoveVolumeOptions { force: true }))
                            .await;
                    }
                })
        })
        .join();
    }
}

async fn create(docker: &bollard::Docker, name: &str, body: ContainerCreateBody, start: bool) {
    let _ = docker
        .remove_container(
            name,
            Some(RemoveContainerOptions {
                force: true,
                v: true,
                ..Default::default()
            }),
        )
        .await;
    let mut body = body;
    let mut labels = body.labels.take().unwrap_or_default();
    labels.insert("cmc.test".into(), "true".into());
    body.labels = Some(labels);
    docker
        .create_container(
            Some(CreateContainerOptions {
                name: Some(name.to_owned()),
                ..Default::default()
            }),
            body,
        )
        .await
        .unwrap();
    if start {
        docker
            .start_container(
                name,
                None::<bollard::query_parameters::StartContainerOptions>,
            )
            .await
            .unwrap();
    }
}

/// Un `tar` con archivos (ruta, contenido, modo, uid).
fn tar_of(files: &[(&str, Vec<u8>, u32, u64)]) -> Vec<u8> {
    let mut b = tar::Builder::new(Vec::new());
    for (path, data, mode, uid) in files {
        let mut h = tar::Header::new_gnu();
        h.set_size(data.len() as u64);
        h.set_mode(*mode);
        h.set_uid(*uid);
        h.set_gid(*uid);
        h.set_mtime(1_700_000_000);
        b.append_data(&mut h, path, data.as_slice()).unwrap();
    }
    b.into_inner().unwrap()
}

/// Escribe archivos en un volumen con un contenedor que no se arranca.
async fn put_files(docker: &bollard::Docker, volume: &str, files: &[(&str, Vec<u8>, u32, u64)]) {
    let name = format!("cmc-test-bk-put-{}", std::process::id());
    create(
        docker,
        &name,
        ContainerCreateBody {
            image: Some(fixture::IMAGE.into()),
            host_config: Some(HostConfig {
                binds: Some(vec![format!("{volume}:/data")]),
                ..Default::default()
            }),
            ..Default::default()
        },
        false,
    )
    .await;
    docker
        .upload_to_container(
            &name,
            Some(UploadToContainerOptions {
                path: "/data".into(),
                ..Default::default()
            }),
            bollard::body_full(tar_of(files).into()),
        )
        .await
        .unwrap();
    docker
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

/// Contenido de un volumen: ruta → (tamaño, modo, uid, sha256, destino del enlace).
async fn listing(
    docker: &bollard::Docker,
    volume: &str,
) -> BTreeMap<String, (u64, u32, u64, String, String)> {
    let name = format!("cmc-test-bk-ls-{}", std::process::id());
    create(
        docker,
        &name,
        ContainerCreateBody {
            image: Some(fixture::IMAGE.into()),
            host_config: Some(HostConfig {
                binds: Some(vec![format!("{volume}:/data:ro")]),
                ..Default::default()
            }),
            ..Default::default()
        },
        false,
    )
    .await;
    let mut s = docker.download_from_container(
        &name,
        Some(DownloadFromContainerOptions {
            path: "/data".into(),
        }),
    );
    let mut bytes = Vec::new();
    while let Some(c) = s.next().await {
        bytes.extend_from_slice(&c.unwrap());
    }
    docker
        .remove_container(
            &name,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await
        .unwrap();
    let mut out = BTreeMap::new();
    let mut a = tar::Archive::new(bytes.as_slice());
    for e in a.entries().unwrap() {
        let mut e = e.unwrap();
        let path = e.path().unwrap().display().to_string();
        let rel = path
            .split_once('/')
            .map_or(String::new(), |(_, r)| r.to_owned());
        let h = e.header().clone();
        let link = e
            .link_name()
            .ok()
            .flatten()
            .map(|l| l.display().to_string())
            .unwrap_or_default();
        let mut data = Vec::new();
        e.read_to_end(&mut data).unwrap();
        let sum = format!("{:x}", Sha256::digest(&data));
        out.insert(
            rel,
            (
                h.size().unwrap(),
                h.mode().unwrap() & 0o7777,
                h.uid().unwrap(),
                sum,
                link,
            ),
        );
    }
    out
}

/// Ejecuta una orden y devuelve (código, salida).
async fn exec(docker: &bollard::Docker, c: &str, cmd: &[&str], env: &[String]) -> (i64, String) {
    let ex = docker
        .create_exec(
            c,
            ExecConfig {
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                env: Some(env.to_vec()),
                cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let mut text = String::new();
    if let StartExecResults::Attached { mut output, .. } =
        docker.start_exec(&ex.id, None).await.unwrap()
    {
        while let Some(Ok(f)) = output.next().await {
            text.push_str(&f.to_string());
        }
    }
    for _ in 0..50 {
        let i = docker.inspect_exec(&ex.id).await.unwrap();
        if let (Some(false), Some(code)) = (i.running, i.exit_code) {
            return (code, text);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    (-1, text)
}

/// Lanza una orden en segundo plano.
async fn exec_detached(docker: &bollard::Docker, c: &str, cmd: &str) {
    let ex = docker
        .create_exec(
            c,
            ExecConfig {
                cmd: Some(vec!["/bin/sh".into(), "-c".into(), cmd.into()]),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    docker
        .start_exec(
            &ex.id,
            Some(StartExecOptions {
                detach: true,
                ..Default::default()
            }),
        )
        .await
        .unwrap();
}

async fn wait_for(docker: &bollard::Docker, c: &str, cmd: &[&str], env: &[String]) {
    for _ in 0..240 {
        if exec(docker, c, cmd, env).await.0 == 0 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!("{c} no llegó a estar listo");
}

/// Volúmenes anónimos sin contenedor (los que dejaría un auxiliar mal eliminado).
async fn dangling(docker: &bollard::Docker) -> usize {
    let mut f = HashMap::new();
    f.insert("dangling".to_owned(), vec!["true".to_owned()]);
    docker
        .list_volumes(Some(bollard::query_parameters::ListVolumesOptions {
            filters: Some(f),
        }))
        .await
        .unwrap()
        .volumes
        .unwrap_or_default()
        .iter()
        .filter(|v| v.name.len() == 64 && v.name.chars().all(|c| c.is_ascii_hexdigit()))
        .count()
}

async fn helpers_left(docker: &bollard::Docker) -> usize {
    let mut f = HashMap::new();
    f.insert("label".to_owned(), vec!["cmc.backup.helper=1".to_owned()]);
    docker
        .list_containers(Some(ListContainersOptions {
            all: true,
            filters: Some(f),
            ..Default::default()
        }))
        .await
        .unwrap()
        .len()
}

fn ctx(server: &str) -> BackupContext {
    BackupContext {
        server: server.into(),
        local: ssh_target().is_none(),
        app_version: "1.4.0".into(),
        ..Default::default()
    }
}

fn mode(p: &std::path::Path) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o777
}

/// Comprueba permisos de toda la copia: carpetas 0700, archivos 0600.
fn assert_private(dir: &std::path::Path) {
    assert_eq!(mode(dir), 0o700, "{}", dir.display());
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            assert_private(&p);
        } else {
            assert_eq!(mode(&p), 0o600, "{}", p.display());
        }
    }
}

async fn setup() -> Option<(
    bollard::Docker,
    cmc_engine::Engine,
    ActivityLog,
    Proxy,
    tempfile::TempDir,
)> {
    if !enabled() {
        eprintln!("omitida: define CMC_DOCKER_TESTS=1");
        return None;
    }
    let sock = real_socket().await;
    let docker =
        bollard::Docker::connect_with_unix(&sock, 120, bollard::API_DEFAULT_VERSION).unwrap();
    fixture::ensure_image(&docker).await;
    let dir = tempfile::Builder::new()
        .prefix("cmc-test")
        .tempdir()
        .unwrap();
    let proxy = Proxy::start(&dir.path().join("p.sock"), sock.clone()).await;
    let log = ActivityLog::new(20000);
    let m = manager(log.clone(), proxy.path.display().to_string());
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    let e = m.engine().await.unwrap();
    Some((docker, e, log, proxy, dir))
}

#[tokio::test(flavor = "multi_thread")]
async fn volumes_round_trip_and_bad_backups_are_never_used() {
    let Some((docker, e, log, _proxy, dir)) = setup().await else {
        return;
    };
    let pid = std::process::id();
    let vol = format!("cmc-test-bk-vol-{pid}");
    let restored = format!("cmc-test-bk-vol-{pid}-r");
    let user = format!("cmc-test-bk-user-{pid}");
    let cfg = format!("cmc-test-bk-cfg-{pid}");
    let cfg_new = format!("cmc-test-bk-cfg-{pid}-r");
    let _g = Guard {
        docker: docker.clone(),
        containers: vec![user.clone(), cfg.clone(), cfg_new.clone()],
        volumes: vec![vol.clone(), restored.clone()],
    };
    docker
        .create_volume(bollard::models::VolumeCreateRequest {
            name: Some(vol.clone()),
            ..Default::default()
        })
        .await
        .unwrap();
    let big: Vec<u8> = (0..3_000_000u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    put_files(
        &docker,
        &vol,
        &[
            ("hola.txt", b"hola mundo\n".to_vec(), 0o644, 0),
            ("privado/clave.bin", vec![7u8; 1000], 0o600, 1000),
            ("grande.bin", big, 0o640, 33),
        ],
    )
    .await;
    let before = listing(&docker, &vol).await;
    assert_eq!(before.len(), 5, "{before:?}");

    // Un contenedor en marcha usa el volumen.
    create(
        &docker,
        &user,
        ContainerCreateBody {
            image: Some(fixture::IMAGE.into()),
            host_config: Some(HostConfig {
                binds: Some(vec![format!("{vol}:/datos")]),
                ..Default::default()
            }),
            ..Default::default()
        },
        true,
    )
    .await;
    // Y otro, detenido, con un secreto en su entorno.
    create(
        &docker,
        &cfg,
        ContainerCreateBody {
            image: Some(fixture::IMAGE.into()),
            env: Some(vec![
                "MODO=prueba".into(),
                "API_TOKEN=tok-Zx81-secreto".into(),
            ]),
            ..Default::default()
        },
        false,
    )
    .await;

    let target = LocalTarget::new(dir.path()).unwrap();
    let server = "local";
    let dangling_before = dangling(&docker).await;
    let req = |mode| BackupRequest {
        volumes: vec![VolumeSelection {
            name: vol.clone(),
            mode,
        }],
        containers: vec![cfg.clone()],
        ..Default::default()
    };

    // 1. En caliente: se marca como no garantizada.
    let mut steps = Vec::new();
    let hot = e
        .backup(
            &target,
            &req(VolumeMode::Hot),
            &ctx(server),
            CancellationToken::new(),
            |p| steps.push(p),
        )
        .await
        .unwrap();
    let m = hot.manifest.clone().unwrap();
    assert_eq!(hot.state, BackupState::Complete);
    assert_eq!(m.items[0].consistency, Consistency::Hot);
    assert!(m.items[0].warnings.contains(&"backup.warn.hot".to_owned()));
    assert!(m.items[0].details["usedBy"].contains("/datos"));
    assert!(!steps.is_empty());
    let hot_dir = std::path::PathBuf::from(&hot.path);
    assert_private(&hot_dir);
    assert!(store::verify(&hot_dir).unwrap().ok);
    // La configuración del contenedor no lleva el secreto.
    let cfg_item = &m.items[1];
    assert_eq!(cfg_item.kind, ItemKind::Container);
    assert_eq!(cfg_item.details["removedEnv"], "API_TOKEN");
    let saved = std::fs::read_to_string(hot_dir.join(&cfg_item.files[0].path)).unwrap();
    assert!(
        saved.contains("MODO=prueba") && !saved.contains("tok-Zx81"),
        "{saved}"
    );

    // 2. Deteniendo: el contenedor se para durante la copia y vuelve a arrancar.
    let stop = e
        .backup(
            &target,
            &req(VolumeMode::Stop),
            &ctx(server),
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();
    let sm = stop.manifest.clone().unwrap();
    assert_eq!(sm.items[0].consistency, Consistency::Stopped);
    let st = docker
        .inspect_container(&user, None::<InspectContainerOptions>)
        .await
        .unwrap();
    assert_eq!(
        st.state.unwrap().running,
        Some(true),
        "el contenedor debe seguir en marcha"
    );
    assert_eq!(
        sm.items[0].files[0].sha256, m.items[0].files[0].sha256,
        "mismo contenido en las dos copias"
    );
    assert_eq!(
        helpers_left(&docker).await,
        0,
        "no quedan contenedores auxiliares"
    );
    // Ningún auxiliar se arrancó.
    assert!(!recorded(&log)
        .iter()
        .any(|(m, p)| m == "POST" && p.contains("cmc-backup-helper") && p.ends_with("/start")));

    // 3. Restauración completa en un volumen nuevo: idéntica.
    let sdir = std::path::PathBuf::from(&stop.path);
    let all = RestoreRequest {
        items: vec![
            RestoreItem {
                kind: ItemKind::Volume,
                name: vol.clone(),
                target: restored.clone(),
            },
            RestoreItem {
                kind: ItemKind::Container,
                name: cfg.clone(),
                target: cfg_new.clone(),
            },
        ],
        confirmed: true,
        accepted_risks: vec![],
    };
    let local = ssh_target().is_none();
    let pv = e.restore_preview(&sdir, &all, local).await.unwrap();
    assert!(pv.integrity.ok);
    assert_eq!(pv.items[0].action, "restore.action.newVolume");
    assert!(pv.items.iter().all(|i| i.blocked.is_none()), "{pv:?}");
    let mut not_confirmed = all.clone();
    not_confirmed.confirmed = false;
    assert!(matches!(
        e.restore(&sdir, &not_confirmed, local, CancellationToken::new())
            .await,
        Err(cmc_engine::EngineError::NotConfirmed)
    ));
    let out = e
        .restore(&sdir, &all, local, CancellationToken::new())
        .await
        .unwrap();
    assert!(out.items.iter().all(|i| i.ok), "{out:?}");
    assert_eq!(
        listing(&docker, &restored).await,
        before,
        "contenido, permisos y propietarios iguales"
    );
    // El contenedor restaurado existe, no se arrancó y no tiene el secreto.
    let c = docker
        .inspect_container(&cfg_new, None::<InspectContainerOptions>)
        .await
        .unwrap();
    assert_eq!(
        c.state.unwrap().status.map(|s| s.to_string()),
        Some("created".into())
    );
    let env = c.config.unwrap().env.unwrap();
    assert!(
        env.contains(&"MODO=prueba".to_owned()) && !env.iter().any(|v| v.starts_with("API_TOKEN"))
    );
    assert!(out.items[1]
        .warnings
        .iter()
        .any(|w| w.starts_with("restore.warn.secretsMissing:API_TOKEN")));

    // 4. Nunca sobre un volumen con datos.
    let over = RestoreRequest {
        items: vec![RestoreItem {
            kind: ItemKind::Volume,
            name: vol.clone(),
            target: vol.clone(),
        }],
        confirmed: true,
        accepted_risks: vec![],
    };
    let pv = e.restore_preview(&sdir, &over, local).await.unwrap();
    assert_eq!(
        pv.items[0].blocked.as_deref(),
        Some("restore.volumeNotEmpty")
    );
    let out = e
        .restore(&sdir, &over, local, CancellationToken::new())
        .await
        .unwrap();
    assert!(
        !out.items[0].ok
            && out.items[0]
                .error
                .as_deref()
                .unwrap()
                .contains("restore.volumeNotEmpty")
    );
    assert_eq!(
        listing(&docker, &vol).await,
        before,
        "el original no cambia"
    );

    // 5. Restauración selectiva: solo el volumen, en uno vacío que ya existe.
    docker
        .remove_volume(&restored, None::<RemoveVolumeOptions>)
        .await
        .unwrap();
    docker
        .create_volume(bollard::models::VolumeCreateRequest {
            name: Some(restored.clone()),
            ..Default::default()
        })
        .await
        .unwrap();
    let sel = RestoreRequest {
        items: vec![RestoreItem {
            kind: ItemKind::Volume,
            name: vol.clone(),
            target: restored.clone(),
        }],
        confirmed: true,
        accepted_risks: vec![],
    };
    assert_eq!(
        e.restore_preview(&sdir, &sel, local).await.unwrap().items[0].action,
        "restore.action.emptyVolume"
    );
    let out = e
        .restore(&sdir, &sel, local, CancellationToken::new())
        .await
        .unwrap();
    assert!(out.items[0].ok, "{out:?}");
    assert_eq!(listing(&docker, &restored).await, before);

    // 6. Suma alterada: se detecta y no se restaura.
    let tar_path = hot_dir.join(&m.items[0].files[0].path);
    let mut bytes = std::fs::read(&tar_path).unwrap();
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0xff;
    std::fs::write(&tar_path, &bytes).unwrap();
    let r = store::verify(&hot_dir).unwrap();
    assert!(!r.ok && r.altered == vec![m.items[0].files[0].path.clone()]);
    let fresh = RestoreRequest {
        items: vec![RestoreItem {
            kind: ItemKind::Volume,
            name: vol.clone(),
            target: format!("{restored}-x"),
        }],
        confirmed: true,
        accepted_risks: vec![],
    };
    let err = e
        .restore(&hot_dir, &fresh, local, CancellationToken::new())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("restore.integrityFailed"), "{err}");
    let vols = docker
        .list_volumes(None::<bollard::query_parameters::ListVolumesOptions>)
        .await
        .unwrap();
    assert!(!vols
        .volumes
        .unwrap_or_default()
        .iter()
        .any(|v| v.name == format!("{restored}-x")));

    // 7. Copia interrumpida (cancelada a mitad): no queda ninguna copia nueva.
    let before_ids: Vec<String> = target
        .list(server)
        .unwrap()
        .into_iter()
        .map(|e| e.id)
        .collect();
    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    let r = e
        .backup(
            &target,
            &req(VolumeMode::Hot),
            &ctx(server),
            cancel,
            move |_| c2.cancel(),
        )
        .await;
    assert!(r.is_err());
    let after: Vec<String> = target
        .list(server)
        .unwrap()
        .into_iter()
        .map(|e| e.id)
        .collect();
    assert_eq!(before_ids, after, "la copia cancelada no deja nada");
    assert_eq!(helpers_left(&docker).await, 0);
    // Y una que se cortó de golpe (sin manifiesto) aparece como incompleta y no se abre.
    let mut p = target.begin(server, "20260101T000000Z-cortada").unwrap();
    let mut f = p.create("volumes/x.tar").await.unwrap();
    f.write(b"medio").await.unwrap();
    drop(f);
    drop(p);
    let l = target.list(server).unwrap();
    let cut = l
        .iter()
        .find(|e| e.id == "20260101T000000Z-cortada")
        .unwrap();
    assert_eq!(cut.state, BackupState::Incomplete);
    assert!(target.open(server, &cut.id).is_err());

    // Los auxiliares no dejan volúmenes anónimos aunque su imagen declare VOLUME.
    assert_eq!(
        dangling(&docker).await,
        dangling_before,
        "volúmenes anónimos de los auxiliares"
    );
    // Ningún secreto en el registro de actividad.
    let all_log = serde_json::to_string(&log.snapshot()).unwrap();
    assert!(!all_log.contains("tok-Zx81"), "secreto en el registro");
    assert!(log.snapshot().iter().any(|r| r.kind == "backup.run"));
    assert!(log.snapshot().iter().any(|r| r.kind == "backup.restore"));
}

struct Db {
    image: String,
    env: Vec<String>,
    ready: Vec<&'static str>,
    sql: fn(&str) -> Vec<String>,
}

fn mariadb() -> Db {
    Db {
        image: img("CMC_TEST_MARIADB_IMAGE", "cmc-test/mariadb:1"),
        env: vec![
            "MARIADB_ROOT_PASSWORD=Pw-Mx91-zq\"$x".into(),
            "MARIADB_DATABASE=app".into(),
        ],
        ready: vec![
            "sh",
            "-c",
            "MYSQL_PWD=\"$MARIADB_ROOT_PASSWORD\" mariadb -uroot -e 'select 1'",
        ],
        sql: |q| {
            vec!["sh".into(), "-c".into(), "MYSQL_PWD=\"$MARIADB_ROOT_PASSWORD\" mariadb -uroot --default-character-set=utf8mb4 -N -B app -e \"$1\"".into(), "sh".into(), q.into()]
        },
    }
}

fn postgres() -> Db {
    Db {
        image: img("CMC_TEST_POSTGRES_IMAGE", "cmc-test/postgres:1"),
        env: vec![
            "POSTGRES_USER=app".into(),
            "POSTGRES_PASSWORD=Pg-Kq73-wv".into(),
            "POSTGRES_DB=app".into(),
        ],
        ready: vec!["sh", "-c", "psql -U app -d app -c 'select 1'"],
        sql: |q| {
            vec![
                "sh".into(),
                "-c".into(),
                "psql -U app -d app -At -c \"$1\"".into(),
                "sh".into(),
                q.into(),
            ]
        },
    }
}

async fn q(docker: &bollard::Docker, c: &str, db: &Db, sql: &str) -> String {
    let cmd = (db.sql)(sql);
    let refs: Vec<&str> = cmd.iter().map(String::as_str).collect();
    let (code, out) = exec(docker, c, &refs, &[]).await;
    assert_eq!(code, 0, "{sql}: {out}");
    out.trim().to_owned()
}

/// Volcado con escrituras simultáneas: cada transacción inserta una fila en
/// `a` y otra en `b`. Una copia coherente tiene siempre las mismas en las dos.
async fn db_consistency(db: Db, label: &str) {
    let Some((docker, e, log, _proxy, dir)) = setup().await else {
        return;
    };
    if docker.inspect_image(&db.image).await.is_err() {
        eprintln!(
            "omitida ({label}): falta {}; ejecuta scripts/dev/build-db-test-images.sh",
            db.image
        );
        return;
    }
    let pid = std::process::id();
    let src = format!("cmc-test-bk-{label}-{pid}");
    let dst = format!("cmc-test-bk-{label}-{pid}-r");
    let _g = Guard {
        docker: docker.clone(),
        containers: vec![src.clone(), dst.clone()],
        volumes: vec![],
    };
    for c in [&src, &dst] {
        create(
            &docker,
            c,
            ContainerCreateBody {
                image: Some(db.image.clone()),
                env: Some(db.env.clone()),
                ..Default::default()
            },
            true,
        )
        .await;
    }
    for c in [&src, &dst] {
        wait_for(&docker, c, &db.ready, &[]).await;
    }
    let charset = if label == "mariadb" {
        " DEFAULT CHARSET=utf8mb4"
    } else {
        ""
    };
    q(
        &docker,
        &src,
        &db,
        &format!("CREATE TABLE a (id SERIAL PRIMARY KEY, v INT){charset}; CREATE TABLE b (id SERIAL PRIMARY KEY, v INT){charset}; CREATE TABLE fijo (t TEXT){charset}; INSERT INTO fijo VALUES ('ñandú ✓');"),
    )
    .await;
    // Escritor: 20000 transacciones de dos filas.
    let writer = if label == "mariadb" {
        "for i in $(seq 1 20000); do echo \"START TRANSACTION; INSERT INTO a(v) VALUES ($i); INSERT INTO b(v) VALUES ($i); COMMIT;\"; done | MYSQL_PWD=\"$MARIADB_ROOT_PASSWORD\" mariadb -uroot app"
    } else {
        "for i in $(seq 1 20000); do echo \"BEGIN; INSERT INTO a(v) VALUES ($i); INSERT INTO b(v) VALUES ($i); COMMIT;\"; done | psql -q -U app -d app"
    };
    exec_detached(&docker, &src, writer).await;
    for _ in 0..200 {
        if q(&docker, &src, &db, "SELECT COUNT(*) FROM a")
            .await
            .parse::<u64>()
            .unwrap()
            > 200
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let target = LocalTarget::new(dir.path()).unwrap();
    let req = BackupRequest {
        databases: vec![src.clone()],
        ..Default::default()
    };
    let bk = e
        .backup(
            &target,
            &req,
            &ctx("local"),
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();
    let after_dump: u64 = q(&docker, &src, &db, "SELECT COUNT(*) FROM a")
        .await
        .parse()
        .unwrap();
    let m = bk.manifest.clone().unwrap();
    assert_eq!(m.items[0].consistency, Consistency::Dump);
    assert_eq!(
        m.items[0].details["engine"],
        if label == "mariadb" {
            "mariadb"
        } else {
            "postgres"
        }
    );
    let bdir = std::path::PathBuf::from(&bk.path);
    assert_private(&bdir);
    assert!(store::verify(&bdir).unwrap().ok);

    // Restauración en un contenedor nuevo del mismo motor.
    let req = RestoreRequest {
        items: vec![RestoreItem {
            kind: ItemKind::Database,
            name: src.clone(),
            target: dst.clone(),
        }],
        confirmed: true,
        accepted_risks: vec![],
    };
    let pv = e.restore_preview(&bdir, &req, true).await.unwrap();
    assert!(
        pv.items[0].data_loss && pv.items[0].blocked.is_none(),
        "{pv:?}"
    );
    let out = e
        .restore(&bdir, &req, true, CancellationToken::new())
        .await
        .unwrap();
    assert!(out.items[0].ok, "{out:?}");
    assert!(out.items[0].warnings.is_empty(), "{out:?}");
    let ca: u64 = q(&docker, &dst, &db, "SELECT COUNT(*) FROM a")
        .await
        .parse()
        .unwrap();
    let cb: u64 = q(&docker, &dst, &db, "SELECT COUNT(*) FROM b")
        .await
        .parse()
        .unwrap();
    assert_eq!(ca, cb, "{label}: copia coherente (a = b)");
    assert!(ca > 0, "{label}: la copia tiene datos");
    assert_eq!(q(&docker, &dst, &db, "SELECT t FROM fijo").await, "ñandú ✓");
    eprintln!("{label}: {ca} transacciones en la copia; {after_dump} en el origen tras el volcado");
    // Restauración completa sobre los propios datos: vuelve al estado de la copia.
    q(&docker, &dst, &db, "DROP TABLE fijo").await;
    let out = e
        .restore(&bdir, &req, true, CancellationToken::new())
        .await
        .unwrap();
    assert!(out.items[0].ok, "{out:?}");
    assert_eq!(q(&docker, &dst, &db, "SELECT t FROM fijo").await, "ñandú ✓");
    // Las contraseñas no llegan nunca al registro.
    let all_log = serde_json::to_string(&log.snapshot()).unwrap();
    assert!(
        !all_log.contains("Pw-Mx91") && !all_log.contains("Pg-Kq73"),
        "secreto en el registro"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn mariadb_dump_is_consistent_and_restores() {
    db_consistency(mariadb(), "mariadb").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn postgres_dump_is_consistent_and_restores() {
    db_consistency(postgres(), "postgres").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn redis_rdb_backup_and_restore() {
    let Some((docker, e, _log, _proxy, dir)) = setup().await else {
        return;
    };
    let image = img("CMC_TEST_REDIS_IMAGE", "cmc-test/redis:1");
    if docker.inspect_image(&image).await.is_err() {
        eprintln!("omitida: falta {image}");
        return;
    }
    let name = format!("cmc-test-bk-redis-{}", std::process::id());
    let _g = Guard {
        docker: docker.clone(),
        containers: vec![name.clone()],
        volumes: vec![],
    };
    create(
        &docker,
        &name,
        ContainerCreateBody {
            image: Some(image),
            ..Default::default()
        },
        true,
    )
    .await;
    wait_for(&docker, &name, &["redis-cli", "ping"], &[]).await;
    assert_eq!(
        exec(
            &docker,
            &name,
            &["redis-cli", "set", "clave", "valor ✓"],
            &[]
        )
        .await
        .0,
        0
    );
    let target = LocalTarget::new(dir.path()).unwrap();
    let req = BackupRequest {
        databases: vec![name.clone()],
        ..Default::default()
    };
    let bk = e
        .backup(
            &target,
            &req,
            &ctx("local"),
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();
    let f = &bk.manifest.as_ref().unwrap().items[0].files[0];
    assert!(f.path.ends_with(".rdb") && f.size > 0);
    assert_eq!(
        exec(&docker, &name, &["redis-cli", "flushall"], &[])
            .await
            .0,
        0
    );
    let bdir = std::path::PathBuf::from(&bk.path);
    let r = RestoreRequest {
        items: vec![RestoreItem {
            kind: ItemKind::Database,
            name: name.clone(),
            target: name.clone(),
        }],
        confirmed: true,
        accepted_risks: vec![],
    };
    let out = e
        .restore(&bdir, &r, true, CancellationToken::new())
        .await
        .unwrap();
    assert!(out.items[0].ok, "{out:?}");
    wait_for(&docker, &name, &["redis-cli", "ping"], &[]).await;
    let (_, v) = exec(&docker, &name, &["redis-cli", "get", "clave"], &[]).await;
    assert_eq!(v.trim(), "valor ✓");
}

#[tokio::test(flavor = "multi_thread")]
async fn compose_files_and_app_config_go_to_new_folders_only() {
    if ssh_target().is_some() {
        eprintln!("omitida por SSH: los archivos de Compose están en este equipo");
        return;
    }
    let Some((_docker, e, _log, _proxy, dir)) = setup().await else {
        return;
    };
    let proj = dir.path().join("tienda");
    std::fs::create_dir(&proj).unwrap();
    std::fs::write(
        proj.join("compose.yaml"),
        "services:\n  web:\n    image: nginx\n",
    )
    .unwrap();
    std::fs::write(proj.join(".env"), "DB_PASSWORD=env-Qw55-secreto\n").unwrap();
    let p = ProjectRef {
        name: "tienda".into(),
        files: vec![proj.join("compose.yaml").display().to_string()],
        working_dir: Some(proj.display().to_string()),
        env_files: vec![],
    };
    let dest = dir.path().join("copias");
    std::fs::create_dir(&dest).unwrap();
    let target = LocalTarget::new(&dest).unwrap();
    let mut c = ctx("local");
    c.compose_projects = vec![p];
    c.app_config = vec![("config.toml".into(), b"idioma = \"es\"\n".to_vec())];
    let mut req = BackupRequest {
        compose: vec![ComposeSelection {
            project: "tienda".into(),
            include_env: false,
        }],
        app_config: true,
        ..Default::default()
    };
    let bk = e
        .backup(&target, &req, &c, CancellationToken::new(), |_| {})
        .await
        .unwrap();
    let m = bk.manifest.clone().unwrap();
    assert_eq!(m.items[0].files.len(), 1, "sin .env");
    assert!(m.items[0]
        .warnings
        .contains(&"backup.warn.envExcluded".to_owned()));
    let bdir = std::path::PathBuf::from(&bk.path);
    assert!(!std::fs::read_dir(bdir.join("compose/tienda"))
        .unwrap()
        .any(|e| e.unwrap().file_name().to_string_lossy().contains("env")));
    // Con aceptación expresa, el .env entra.
    req.compose[0].include_env = true;
    let bk2 = e
        .backup(&target, &req, &c, CancellationToken::new(), |_| {})
        .await
        .unwrap();
    assert_eq!(bk2.manifest.as_ref().unwrap().items[0].files.len(), 2);
    assert_private(std::path::Path::new(&bk2.path));

    // Restaurar en carpetas nuevas.
    let out_dir = dir.path().join("restaurado");
    let app_dir = dir.path().join("config-restaurada");
    let r = RestoreRequest {
        items: vec![
            RestoreItem {
                kind: ItemKind::Compose,
                name: "tienda".into(),
                target: out_dir.display().to_string(),
            },
            RestoreItem {
                kind: ItemKind::AppConfig,
                name: "app".into(),
                target: app_dir.display().to_string(),
            },
        ],
        confirmed: true,
        accepted_risks: vec![],
    };
    let b2 = std::path::PathBuf::from(&bk2.path);
    let out = e
        .restore(&b2, &r, true, CancellationToken::new())
        .await
        .unwrap();
    assert!(out.items.iter().all(|i| i.ok), "{out:?}");
    assert_eq!(
        std::fs::read_to_string(out_dir.join("compose.yaml")).unwrap(),
        "services:\n  web:\n    image: nginx\n"
    );
    assert_eq!(
        std::fs::read_to_string(out_dir.join("env/.env")).unwrap(),
        "DB_PASSWORD=env-Qw55-secreto\n"
    );
    assert_eq!(mode(&out_dir), 0o700);
    assert_eq!(mode(&out_dir.join("compose.yaml")), 0o600);
    assert_eq!(
        std::fs::read_to_string(app_dir.join("config.toml")).unwrap(),
        "idioma = \"es\"\n"
    );
    // Una segunda vez: las carpetas ya existen y nada se sobrescribe.
    let pv = e.restore_preview(&b2, &r, true).await.unwrap();
    assert!(
        pv.items
            .iter()
            .all(|i| i.blocked.as_deref() == Some("restore.folderExists")),
        "{pv:?}"
    );
    let out = e
        .restore(&b2, &r, true, CancellationToken::new())
        .await
        .unwrap();
    assert!(out.items.iter().all(|i| !i.ok));
}
