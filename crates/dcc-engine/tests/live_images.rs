//! Fase 4 contra Docker real (`DCC_DOCKER_TESTS=1`): descarga desde un registro
//! local (con y sin autenticación, y cancelada), búsqueda, inspección, capas,
//! etiquetado, eliminación, limpieza y construcción con BuildKit y con el
//! constructor clásico. Todo lo creado lleva la etiqueta `dcc.test=true`.

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::registry::{self, Registry};
use common::*;
use dcc_activity::{ActivityLog, OpStatus};
use dcc_engine::build::{BuildSpec, BuildUpdate, Builder, StepState};
use dcc_engine::pull::PullUpdate;
use dcc_engine::registry_auth::PullAuth;
use dcc_engine::spec::KeyValue;
use dcc_engine::streams::EndReason;
use dcc_engine::{ConnectionMode, ConnectionState, Engine, EngineError, EngineManager};
use tokio_util::sync::CancellationToken;

/// Configuración de Docker vacía: las pruebas no leen las credenciales del usuario.
fn isolate_docker_config() {
    let d = std::env::temp_dir().join(format!("dcc-test-docker-config-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    std::env::set_var("DOCKER_CONFIG", &d);
}

async fn engine_via_proxy(
    dir: &tempfile::TempDir,
) -> (Engine, ActivityLog, Proxy, bollard::Docker) {
    isolate_docker_config();
    let sock = real_socket().await;
    let docker =
        bollard::Docker::connect_with_unix(&sock, 120, bollard::API_DEFAULT_VERSION).unwrap();
    fixture::ensure_image(&docker).await;
    let proxy = Proxy::start(&dir.path().join("p.sock"), sock).await;
    let log = ActivityLog::new(2000);
    let m = EngineManager::new(
        log.clone(),
        ConnectionMode::Custom {
            socket_path: proxy.path.display().to_string(),
        },
    );
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    (m.engine().await.unwrap(), log, proxy, docker)
}

/// Elimina imágenes y contenedores de prueba aunque la prueba falle.
struct Cleanup {
    docker: bollard::Docker,
    images: Vec<String>,
    containers: Vec<String>,
}

/// El constructor clásico guarda una imagen intermedia sin etiqueta por paso
/// (su caché). Esta guarda elimina las que crearon las construcciones de la
/// prueba: sin etiqueta, heredadas de la imagen de prueba y posteriores al inicio.
struct IntermediateCleanup {
    docker: bollard::Docker,
    since: i64,
}

impl Drop for IntermediateCleanup {
    fn drop(&mut self) {
        let d = self.docker.clone();
        let since = self.since;
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                let mut filters = std::collections::HashMap::new();
                filters.insert("dangling".to_string(), vec!["true".to_string()]);
                filters.insert("label".to_string(), vec!["dcc.test=true".to_string()]);
                let list = d
                    .list_images(Some(bollard::query_parameters::ListImagesOptions {
                        filters: Some(filters),
                        ..Default::default()
                    }))
                    .await
                    .unwrap_or_default();
                for i in list {
                    // Las imágenes sin etiqueta de la prueba de limpieza no son de esta prueba.
                    if i.created >= since && !i.labels.contains_key("dcc.prune") {
                        let _ = d
                            .remove_image(
                                &i.id,
                                Some(bollard::query_parameters::RemoveImageOptions {
                                    force: true,
                                    ..Default::default()
                                }),
                                None,
                            )
                            .await;
                    }
                }
            });
        })
        .join()
        .unwrap();
    }
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        let d = self.docker.clone();
        let (imgs, cs) = (
            std::mem::take(&mut self.images),
            std::mem::take(&mut self.containers),
        );
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                for c in cs {
                    let _ = d
                        .remove_container(
                            &c,
                            Some(bollard::query_parameters::RemoveContainerOptions {
                                force: true,
                                ..Default::default()
                            }),
                        )
                        .await;
                }
                for i in imgs {
                    let _ = d
                        .remove_image(
                            &i,
                            Some(bollard::query_parameters::RemoveImageOptions {
                                force: true,
                                ..Default::default()
                            }),
                            None,
                        )
                        .await;
                }
            });
        })
        .join()
        .unwrap();
    }
}

fn collect_pull() -> (
    Arc<Mutex<Vec<PullUpdate>>>,
    impl FnMut(PullUpdate) -> bool + Send,
) {
    let v: Arc<Mutex<Vec<PullUpdate>>> = Default::default();
    let v2 = v.clone();
    (v, move |u| {
        v2.lock().unwrap().push(u);
        true
    })
}

fn collect_build() -> (
    Arc<Mutex<Vec<BuildUpdate>>>,
    impl FnMut(BuildUpdate) -> bool + Send,
) {
    let v: Arc<Mutex<Vec<BuildUpdate>>> = Default::default();
    let v2 = v.clone();
    (v, move |u| {
        v2.lock().unwrap().push(u);
        true
    })
}

fn last_record(log: &ActivityLog, kind: &str) -> dcc_activity::OperationRecord {
    log.snapshot()
        .into_iter()
        .find(|r| r.kind == kind)
        .unwrap_or_else(|| panic!("sin registro {kind}"))
}

/// `true` si el daemon usa el almacén de imágenes de containerd.
async fn containerd_store(docker: &bollard::Docker) -> bool {
    let info = docker.info().await.unwrap();
    format!("{:?}", info.driver_status).contains("io.containerd.snapshotter")
}

/// Tamaño de la capa de las imágenes únicas: suficiente para que Docker
/// informe del progreso de la descarga.
const LAYER: usize = 3 * 1024 * 1024;

fn pull_records(log: &ActivityLog) -> Vec<dcc_activity::OperationRecord> {
    log.snapshot()
        .into_iter()
        .filter(|r| r.kind == "image.pull")
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn pull_from_local_registry_reports_progress_and_records_once() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (engine, log, proxy, docker) = engine_via_proxy(&dir).await;
    // Imagen que el daemon no puede tener: tiene que descargar la capa con
    // cualquier almacén de imágenes.
    let served = registry::unique(LAYER);
    let layers = served.layers();
    assert_eq!(layers.len(), 1);
    let (layer_digest, layer_size) = layers[0].clone();
    let reg = Registry::start(
        served,
        registry::Options {
            chunk_delay: Some(Duration::from_millis(15)),
            ..Default::default()
        },
    )
    .await;
    let reference = format!("{}/dcc-test/pulled:1", reg.addr);
    let _c = Cleanup {
        docker: docker.clone(),
        images: vec![reference.clone()],
        containers: vec![],
    };

    let (updates, sink) = collect_pull();
    let end = engine
        .pull_image(
            &reference,
            &PullAuth::Anonymous,
            CancellationToken::new(),
            sink,
        )
        .await
        .unwrap();
    assert_eq!(end.reason, EndReason::Finished);
    assert!(
        end.summary.contains("Downloaded newer image"),
        "{}",
        end.summary
    );
    assert!(
        end.summary.contains("1 capas descargadas, 0 ya existían"),
        "{}",
        end.summary
    );
    let ups = updates.lock().unwrap().clone();
    assert!(!ups.is_empty());
    // Progreso real: alguna actualización muestra la capa a medias, con el
    // tamaño comprimido que sirvió el registro.
    let id = &layer_digest["sha256:".len().."sha256:".len() + 12];
    let partial = ups.iter().flat_map(|u| &u.layers).find(|l| {
        l.id == id
            && l.status == "Downloading"
            && matches!((l.current, l.total), (Some(c), Some(t)) if c > 0 && c < t)
    });
    let partial =
        partial.unwrap_or_else(|| panic!("sin progreso parcial de la capa {id}: {ups:?}"));
    assert_eq!(partial.total, Some(layer_size as i64), "{partial:?}");
    let last = ups.iter().rev().find(|u| !u.layers.is_empty()).unwrap();
    assert_eq!(last.layers.len(), 1);
    assert_eq!(last.layers[0].id, id);
    assert!(
        matches!(
            last.layers[0].status.as_str(),
            "Pull complete" | "Download complete"
        ),
        "{:?}",
        last.layers
    );
    assert!(ups
        .iter()
        .flat_map(|u| &u.messages)
        .any(|m| m.starts_with("Digest: sha256:")));
    // La imagen existe de verdad.
    docker
        .inspect_image(&reference)
        .await
        .expect("imagen descargada");
    // El registro recibió el manifiesto y la capa entera una sola vez.
    let paths = reg.paths();
    assert!(
        paths
            .iter()
            .any(|p| p.contains("/v2/dcc-test/pulled/manifests/1")),
        "{paths:?}"
    );
    assert_eq!(reg.layer_gets(&layer_digest), 1, "{paths:?}");
    assert!(reg.aborted.lock().unwrap().is_empty());

    // Una sola operación, con la ruta y el equivalente exactos.
    let r = last_record(&log, "image.pull");
    assert_eq!(r.status, OpStatus::Succeeded);
    assert_eq!(
        r.equivalent_command.as_deref(),
        Some(
            format!(
                "docker --host unix://{} pull {reference}",
                proxy.path.display()
            )
            .as_str()
        )
    );
    assert_eq!(pull_records(&log).len(), 1);

    // Segunda descarga: ya está al día y no vuelve a pedir la capa.
    let (_, sink) = collect_pull();
    let end = engine
        .pull_image(
            &reference,
            &PullAuth::Anonymous,
            CancellationToken::new(),
            sink,
        )
        .await
        .unwrap();
    assert!(
        end.summary.contains("Image is up to date"),
        "{}",
        end.summary
    );
    assert_eq!(reg.layer_gets(&layer_digest), 1);
    let pulls = pull_records(&log);
    assert_eq!(pulls.len(), 2);
    assert!(pulls.iter().all(|r| r.status == OpStatus::Succeeded));
    assert_eq!(recorded(&log), proxy.requests());
}

/// Lo que ocurrió en Debian 13 (almacén clásico): la imagen ya estaba en el
/// equipo con otro nombre. Docker no descarga nada y la operación termina bien,
/// con un resumen que lo dice.
#[tokio::test(flavor = "multi_thread")]
async fn pull_of_content_already_present_is_a_real_finished_pull() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (engine, log, proxy, docker) = engine_via_proxy(&dir).await;
    let containerd = containerd_store(&docker).await;
    let served = registry::export(&docker, fixture::IMAGE).await;
    let layer_digests: Vec<String> = served.layers().into_iter().map(|(d, _)| d).collect();
    let reg = Registry::start(served, Default::default()).await;
    let reference = format!("{}/dcc-test/presente:1", reg.addr);
    let _c = Cleanup {
        docker: docker.clone(),
        images: vec![reference.clone()],
        containers: vec![],
    };
    let (updates, sink) = collect_pull();
    let end = engine
        .pull_image(
            &reference,
            &PullAuth::Anonymous,
            CancellationToken::new(),
            sink,
        )
        .await
        .unwrap();
    assert_eq!(end.reason, EndReason::Finished);
    assert!(
        end.summary.contains("Downloaded newer image"),
        "{}",
        end.summary
    );
    let layers: Vec<_> = updates
        .lock()
        .unwrap()
        .iter()
        .flat_map(|u| u.layers.clone())
        .collect();
    if containerd {
        // containerd compara capas comprimidas: la recomprimida es "nueva".
        assert!(
            end.summary.contains("1 capas descargadas"),
            "{}",
            end.summary
        );
    } else {
        // El almacén clásico reconoce la imagen y no pide ninguna capa.
        assert!(layers.is_empty(), "{layers:?}");
        assert!(
            end.summary
                .contains("Docker ya tenía todo su contenido, no ha descargado capas"),
            "{}",
            end.summary
        );
        for d in &layer_digests {
            assert_eq!(reg.layer_gets(d), 0, "{:?}", reg.paths());
        }
    }
    // Con cualquier almacén la imagen queda disponible con el nuevo nombre.
    let new = docker.inspect_image(&reference).await.unwrap();
    let orig = docker.inspect_image(fixture::IMAGE).await.unwrap();
    if !containerd {
        assert_eq!(new.id, orig.id, "misma imagen, otro nombre");
    }
    let pulls = pull_records(&log);
    assert_eq!(pulls.len(), 1);
    assert_eq!(pulls[0].status, OpStatus::Succeeded);
    assert_eq!(recorded(&log), proxy.requests());
}

#[tokio::test(flavor = "multi_thread")]
async fn pull_with_credentials_never_records_them() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (engine, log, proxy, docker) = engine_via_proxy(&dir).await;
    let secret = "clave-dcc-no-debe-aparecer";
    let served = registry::unique(64 * 1024);
    let layer_digest = served.layers()[0].0.clone();
    let reg = Registry::start(
        served,
        registry::Options {
            auth: Some(("ana".into(), secret.into())),
            ..Default::default()
        },
    )
    .await;
    let reference = format!("{}/dcc-test/privada:1", reg.addr);
    let _c = Cleanup {
        docker: docker.clone(),
        images: vec![reference.clone()],
        containers: vec![],
    };

    // Sin credenciales: Docker devuelve el error del registro.
    let (_, sink) = collect_pull();
    let e = engine
        .pull_image(
            &reference,
            &PullAuth::Anonymous,
            CancellationToken::new(),
            sink,
        )
        .await
        .unwrap_err();
    assert!(matches!(e, EngineError::Docker { .. }), "{e:?}");
    // Contraseña incorrecta: también falla.
    let (_, sink) = collect_pull();
    let bad = PullAuth::Manual {
        username: "ana".into(),
        password: "otra".into(),
    };
    assert!(engine
        .pull_image(&reference, &bad, CancellationToken::new(), sink)
        .await
        .is_err());
    // Correcta: descarga (la capa se pide con las credenciales).
    let (_, sink) = collect_pull();
    let good = PullAuth::Manual {
        username: "ana".into(),
        password: secret.into(),
    };
    let end = engine
        .pull_image(&reference, &good, CancellationToken::new(), sink)
        .await
        .unwrap();
    assert_eq!(end.reason, EndReason::Finished);
    assert!(
        end.summary
            .contains("credenciales introducidas para esta operación"),
        "{}",
        end.summary
    );
    assert!(
        end.summary.contains("1 capas descargadas"),
        "{}",
        end.summary
    );
    docker
        .inspect_image(&reference)
        .await
        .expect("imagen privada descargada");
    // El daemon envió las credenciales al registro al pedir la capa…
    assert!(reg
        .requests
        .lock()
        .unwrap()
        .iter()
        .any(|(m, p, a)| m == "GET" && p.ends_with(&layer_digest) && a.is_some()));
    // …pero no aparecen en ningún registro de actividad ni en los errores.
    for r in log.snapshot() {
        let all = format!("{r:?}");
        assert!(!all.contains(secret), "secreto en el registro: {all}");
    }
    let pulls = pull_records(&log);
    assert_eq!(pulls.len(), 3);
    assert_eq!(
        pulls
            .iter()
            .filter(|r| r.status == OpStatus::Failed)
            .count(),
        2
    );
    assert_eq!(recorded(&log), proxy.requests());
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelled_pull_stops_and_is_recorded_as_cancelled() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (engine, log, proxy, docker) = engine_via_proxy(&dir).await;
    let containerd = containerd_store(&docker).await;
    // Imagen nueva: Docker tiene que pedir la capa. El registro la retiene 12 s
    // y después la envía despacio (~0,7 s), así que si Docker siguiera
    // descargando tras la cancelación llegaría a recibirla entera.
    let delay = Duration::from_secs(12);
    let served = registry::unique(LAYER);
    let layer_digest = served.layers()[0].0.clone();
    let reg = Registry::start(
        served,
        registry::Options {
            blob_delay: Some(delay),
            chunk_delay: Some(Duration::from_millis(15)),
            ..Default::default()
        },
    )
    .await;
    let reference = format!("{}/dcc-test/lenta:1", reg.addr);
    let _c = Cleanup {
        docker: docker.clone(),
        images: vec![reference.clone()],
        containers: vec![],
    };
    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    let canceller = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        c2.cancel();
    });
    let t0 = std::time::Instant::now();
    let (_, sink) = collect_pull();
    let end = engine
        .pull_image(&reference, &PullAuth::Anonymous, cancel, sink)
        .await
        .unwrap();
    canceller.await.unwrap();
    assert_eq!(end.reason, EndReason::Closed, "{}", end.summary);
    let cancelled_at = t0.elapsed();
    assert!(
        cancelled_at < Duration::from_secs(6),
        "la cancelación esperó al blob: {cancelled_at:?}"
    );
    // Docker pidió la capa antes de la cancelación…
    assert_eq!(reg.layer_gets(&layer_digest), 1, "{:?}", reg.paths());
    // …y no llegó a recibirla: cerró la conexión con el registro. Con el almacén
    // de containerd lo hace al momento; con el clásico, Docker mantiene la
    // conexión hasta que el registro empieza a responder y entonces la corta
    // (igual que "docker pull" interrumpido con Ctrl+C; ver D-045).
    let deadline = t0 + delay + Duration::from_secs(10);
    while reg.aborted.lock().unwrap().is_empty() && std::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let closed_after = t0.elapsed();
    assert_eq!(
        *reg.aborted.lock().unwrap(),
        vec![layer_digest.clone()],
        "Docker recibió la capa entera tras la cancelación"
    );
    if containerd {
        assert!(
            closed_after < Duration::from_secs(6),
            "containerd debería cortar al momento: {closed_after:?}"
        );
    }
    let r = last_record(&log, "image.pull");
    assert_eq!(r.status, OpStatus::Failed);
    assert!(
        r.error
            .as_deref()
            .unwrap_or("")
            .contains("cancelada por el usuario"),
        "{:?}",
        r.error
    );
    assert!(
        docker.inspect_image(&reference).await.is_err(),
        "no debe quedar la imagen"
    );

    // Sin restos: una nueva descarga baja la capa entera desde el principio.
    reg.set_blob_delay(None);
    let (_, sink) = collect_pull();
    let end = engine
        .pull_image(
            &reference,
            &PullAuth::Anonymous,
            CancellationToken::new(),
            sink,
        )
        .await
        .unwrap();
    assert_eq!(end.reason, EndReason::Finished);
    assert!(
        end.summary.contains("1 capas descargadas, 0 ya existían"),
        "{}",
        end.summary
    );
    assert_eq!(reg.layer_gets(&layer_digest), 2);
    docker.inspect_image(&reference).await.unwrap();
    let pulls = pull_records(&log);
    assert_eq!(pulls.len(), 2, "un registro por descarga");
    assert_eq!(
        pulls[0].status,
        OpStatus::Succeeded,
        "la más reciente primero"
    );
    assert_eq!(pulls[1].status, OpStatus::Failed);
    assert_eq!(recorded(&log), proxy.requests());
}

#[tokio::test(flavor = "multi_thread")]
async fn inspect_history_tag_remove_and_prune() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (engine, log, proxy, docker) = engine_via_proxy(&dir).await;
    let pid = std::process::id();
    let tag_repo = format!("dcc-test/etiquetada-{pid}");
    let tagged = format!("{tag_repo}:v1");
    let mut cleanup = Cleanup {
        docker: docker.clone(),
        images: vec![tagged.clone()],
        containers: vec![],
    };

    let d = engine.image_details(fixture::IMAGE).await.unwrap();
    assert!(d.repo_tags.contains(&fixture::IMAGE.to_string()));
    assert_eq!(d.command, vec!["/sleeper"]);
    assert!(d
        .env
        .iter()
        .any(|e| e.key == "DCC_FIXTURE" && e.value == "2"));
    assert_eq!(d.labels.get("dcc.test").map(String::as_str), Some("true"));
    assert_eq!(d.layers, 1);
    assert!(d.raw.contains("\"Architecture\""));

    let h = engine.image_history(fixture::IMAGE).await.unwrap();
    assert!(!h.is_empty());
    assert!(h.iter().any(|l| l.size > 0));

    // Etiquetar.
    assert_eq!(
        engine
            .tag_image(fixture::IMAGE, &tag_repo, "v1")
            .await
            .unwrap(),
        tagged
    );
    let d2 = engine.image_details(&tagged).await.unwrap();
    assert_eq!(d2.id, d.id);
    assert!(matches!(
        engine.tag_image(fixture::IMAGE, "Mal", "v1").await,
        Err(EngineError::InvalidInput { .. })
    ));

    // Un contenedor la usa: quitar esa etiqueta exige forzar (error real de Docker).
    let name = format!("dcc-test-usa-imagen-{pid}");
    let body = bollard::models::ContainerCreateBody {
        image: Some(tagged.clone()),
        labels: Some([("dcc.test".to_string(), "true".to_string())].into()),
        ..Default::default()
    };
    docker
        .create_container(
            Some(bollard::query_parameters::CreateContainerOptions {
                name: Some(name.clone()),
                ..Default::default()
            }),
            body,
        )
        .await
        .unwrap();
    cleanup.containers.push(name.clone());
    assert!(matches!(
        engine.remove_image(&tagged, false, false).await,
        Err(EngineError::NotConfirmed)
    ));
    // Según la versión, Docker exige forzar (409) o quita la etiqueta sin más
    // porque la imagen conserva otra etiqueta (Docker 29 con containerd).
    let r = match engine.remove_image(&tagged, false, true).await {
        Err(EngineError::Docker {
            status: 409,
            message,
        }) => {
            assert!(
                message.contains("container") || message.contains("force"),
                "{message}"
            );
            engine.remove_image(&tagged, true, true).await.unwrap()
        }
        Ok(r) => r,
        Err(e) => panic!("{e:?}"),
    };
    assert!(
        r.untagged
            .iter()
            .any(|u| u == &tagged || u.ends_with(&format!("/{tagged}"))),
        "{r:?}"
    );
    assert!(
        docker.inspect_image(&tagged).await.is_err(),
        "la etiqueta debe desaparecer"
    );
    assert!(
        r.deleted.is_empty(),
        "la imagen sigue con otra etiqueta: no se borra nada"
    );
    assert!(docker.inspect_image(fixture::IMAGE).await.is_ok());

    // Limpieza: una imagen sin etiqueta propia de esta prueba (etiqueta única)
    // y prune filtrado por esa etiqueta, para no tocar nada más.
    let mark = format!("dcc.prune={pid}");
    let ctx = dir.path().join("prune");
    std::fs::create_dir_all(&ctx).unwrap();
    std::fs::write(
        ctx.join("Dockerfile"),
        format!("FROM {}\nLABEL dcc.prune={pid}\n", fixture::IMAGE),
    )
    .unwrap();
    let spec = BuildSpec {
        context: ctx.to_string_lossy().into_owned(),
        ..Default::default()
    };
    let built = engine
        .build_image(&spec, CancellationToken::new(), |_| true)
        .await
        .unwrap();
    let dangling = built.image_id.unwrap();
    let p = engine
        .prune_images_filtered(false, Some(&mark), true)
        .await
        .unwrap();
    assert!(p.deleted.contains(&dangling), "{p:?} no incluye {dangling}");
    assert!(docker.inspect_image(&dangling).await.is_err());
    let rec = last_record(&log, "image.prune");
    assert!(rec
        .equivalent_command
        .unwrap()
        .ends_with(&format!("image prune --force --filter label={mark}")));
    assert!(matches!(
        engine.prune_images(false, false).await,
        Err(EngineError::NotConfirmed)
    ));
    assert_eq!(recorded(&log), proxy.requests());
}

#[tokio::test(flavor = "multi_thread")]
async fn search_is_recorded_whether_or_not_the_daemon_has_internet() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (engine, log, proxy, _docker) = engine_via_proxy(&dir).await;
    match engine.search_images("alpine", 5, true).await {
        Ok(v) => {
            eprintln!("búsqueda con Internet: {} resultados", v.len());
            assert!(!v.is_empty() && v.len() <= 5);
            assert!(v.iter().any(|r| r.name.contains("alpine")));
            assert!(v.iter().all(|r| r.official));
        }
        Err(e) => {
            eprintln!("búsqueda sin Internet en el daemon: {e}");
            assert!(
                matches!(e, EngineError::Docker { .. } | EngineError::Timeout),
                "{e:?}"
            );
        }
    }
    let r = last_record(&log, "image.search");
    assert_eq!(
        r.equivalent_command.as_deref(),
        Some(
            format!(
                "docker --host unix://{} search --limit 5 --filter is-official=true alpine",
                proxy.path.display()
            )
            .as_str()
        )
    );
    assert!(matches!(
        engine.search_images("  ", 5, false).await,
        Err(EngineError::InvalidInput { .. })
    ));
    assert_eq!(recorded(&log), proxy.requests());
}

fn context(dir: &std::path::Path, run: &str) -> String {
    let ctx = dir.join("ctx");
    std::fs::create_dir_all(ctx.join("sub")).unwrap();
    std::fs::write(
        ctx.join("Dockerfile"),
        format!("FROM {}\nARG VERSION=0\nLABEL dcc.version=$VERSION\nCOPY . /ctx/\nRUN [\"/bin/sh\", \"-c\", \"{run}\"]\n", fixture::IMAGE),
    )
    .unwrap();
    std::fs::write(ctx.join(".dockerignore"), "secreto.txt\n*.log\n").unwrap();
    std::fs::write(ctx.join("incluido.txt"), "hola").unwrap();
    std::fs::write(ctx.join("secreto.txt"), "no debe enviarse").unwrap();
    std::fs::write(ctx.join("ruido.log"), "x").unwrap();
    std::fs::write(ctx.join("sub/dentro.txt"), "y").unwrap();
    ctx.to_string_lossy().into_owned()
}

const LIST: &str = "echo construyendo; for f in /ctx/* /ctx/sub/*; do echo contenido: $f; done";

#[tokio::test(flavor = "multi_thread")]
async fn buildkit_build_applies_dockerignore_and_matches_wire() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (engine, log, proxy, docker) = engine_via_proxy(&dir).await;
    let pid = std::process::id();
    let tags = vec![
        format!("dcc-test/construida-{pid}:1"),
        format!("dcc-test/construida-{pid}:latest"),
    ];
    let _c = Cleanup {
        docker: docker.clone(),
        images: tags.clone(),
        containers: vec![],
    };
    let spec = BuildSpec {
        context: context(dir.path(), LIST),
        tags: tags.clone(),
        build_args: vec![KeyValue::new("VERSION", "4.0")],
        labels: vec![KeyValue::new("dcc.origen", "prueba")],
        no_cache: true,
        builder: Builder::BuildKit,
        ..Default::default()
    };
    let (updates, sink) = collect_build();
    let end = engine
        .build_image(&spec, CancellationToken::new(), sink)
        .await
        .unwrap();
    assert_eq!(end.end.reason, EndReason::Finished);
    let id = end.image_id.clone().expect("ID de la imagen");
    let ups = updates.lock().unwrap().clone();
    let lines: Vec<String> = ups.iter().flat_map(|u| u.lines.clone()).collect();
    let text = lines.join("\n");
    assert!(text.contains("construyendo"), "{text}");
    assert!(text.contains("contenido: /ctx/incluido.txt"), "{text}");
    assert!(text.contains("contenido: /ctx/sub/dentro.txt"), "{text}");
    assert!(
        !text.contains("secreto.txt") && !text.contains("ruido.log"),
        ".dockerignore no se aplicó:\n{text}"
    );
    assert!(ups
        .iter()
        .flat_map(|u| &u.steps)
        .any(|s| s.state == StepState::Done));
    // La imagen existe con las dos etiquetas, la etiqueta y el argumento.
    let d = engine.image_details(&tags[0]).await.unwrap();
    assert_eq!(d.id, id);
    assert!(d.repo_tags.contains(&tags[1]));
    assert_eq!(d.labels.get("dcc.version").map(String::as_str), Some("4.0"));
    assert_eq!(
        d.labels.get("dcc.origen").map(String::as_str),
        Some("prueba")
    );
    let r = last_record(&log, "image.build");
    assert_eq!(r.status, OpStatus::Succeeded);
    assert!(
        format!("{:?}", r.request_body).contains("2 excluidos"),
        "{:?}",
        r.request_body
    );
    assert!(r.equivalent_command.as_deref().unwrap().contains(&format!(
        "build --tag {} --tag {} --build-arg VERSION=4.0",
        tags[0], tags[1]
    )));
    assert_eq!(recorded(&log), proxy.requests());
}

#[tokio::test(flavor = "multi_thread")]
async fn classic_builder_also_works_and_failures_are_reported() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (engine, log, proxy, docker) = engine_via_proxy(&dir).await;
    let pid = std::process::id();
    let tag = format!("dcc-test/clasica-{pid}:1");
    let _intermediates = IntermediateCleanup {
        docker: docker.clone(),
        since: chrono::Utc::now().timestamp() - 1,
    };
    let _c = Cleanup {
        docker: docker.clone(),
        images: vec![tag.clone()],
        containers: vec![],
    };
    let spec = BuildSpec {
        context: context(dir.path(), LIST),
        tags: vec![tag.clone()],
        no_cache: true,
        builder: Builder::Classic,
        ..Default::default()
    };
    let (updates, sink) = collect_build();
    let end = engine
        .build_image(&spec, CancellationToken::new(), sink)
        .await
        .unwrap();
    assert!(end.image_id.is_some());
    let text: String = updates
        .lock()
        .unwrap()
        .iter()
        .flat_map(|u| u.lines.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        text.contains("contenido: /ctx/incluido.txt") && !text.contains("secreto.txt"),
        "{text}"
    );
    assert!(last_record(&log, "image.build")
        .equivalent_command
        .unwrap()
        .starts_with("DOCKER_BUILDKIT=0 docker"));
    docker.inspect_image(&tag).await.expect("imagen clásica");

    // Un RUN que falla: error real, paso en error y registro fallido (con ambos constructores).
    for builder in [Builder::BuildKit, Builder::Classic] {
        let d2 = tempfile::tempdir().unwrap();
        let spec = BuildSpec {
            context: context(d2.path(), "echo antes de fallar; exit 3"),
            tags: vec![format!("dcc-test/rota-{pid}:1")],
            builder,
            ..Default::default()
        };
        let (updates, sink) = collect_build();
        let e = engine
            .build_image(&spec, CancellationToken::new(), sink)
            .await
            .unwrap_err();
        assert!(matches!(e, EngineError::Docker { .. }), "{e:?}");
        let text: String = updates
            .lock()
            .unwrap()
            .iter()
            .flat_map(|u| u.lines.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("antes de fallar"), "{builder:?}: {text}");
        assert_eq!(last_record(&log, "image.build").status, OpStatus::Failed);
        // Ni el constructor clásico deja el contenedor del paso fallido (forcerm).
        let left = docker
            .list_containers(Some(bollard::query_parameters::ListContainersOptions {
                all: true,
                ..Default::default()
            }))
            .await
            .unwrap()
            .into_iter()
            .filter(|c| {
                c.image
                    .as_deref()
                    .is_some_and(|i| i.starts_with("sha256:") || i.len() == 12)
                    && c.state
                        .as_ref()
                        .map(|s| format!("{s:?}").to_lowercase().contains("exited"))
                        .unwrap_or(false)
                    && c.labels
                        .as_ref()
                        .is_some_and(|l| l.contains_key("dcc.test"))
            })
            .count();
        assert_eq!(left, 0, "{builder:?} dejó contenedores intermedios");
        if builder == Builder::BuildKit {
            assert!(updates
                .lock()
                .unwrap()
                .iter()
                .flat_map(|u| &u.steps)
                .any(|s| s.state == StepState::Error));
        }
    }
    // Especificación inválida: no llega a Docker.
    let before = proxy.requests().len();
    let bad = BuildSpec {
        context: "/no/existe/dcc".into(),
        tags: vec!["x".into()],
        ..Default::default()
    };
    let (_, sink) = collect_build();
    assert!(matches!(
        engine
            .build_image(&bad, CancellationToken::new(), sink)
            .await,
        Err(EngineError::InvalidSpec { .. })
    ));
    assert_eq!(proxy.requests().len(), before);
    assert_eq!(recorded(&log), proxy.requests());
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelled_build_is_recorded_and_leaves_no_image() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (engine, log, _proxy, docker) = engine_via_proxy(&dir).await;
    let tag = format!("dcc-test/cancelada-{}:1", std::process::id());
    let _c = Cleanup {
        docker: docker.clone(),
        images: vec![tag.clone()],
        containers: vec![],
    };
    let ctx = dir.path().join("ctx");
    std::fs::create_dir_all(&ctx).unwrap();
    // /sleeper sin argumentos espera indefinidamente.
    std::fs::write(
        ctx.join("Dockerfile"),
        format!("FROM {}\nRUN [\"/sleeper\"]\n", fixture::IMAGE),
    )
    .unwrap();
    let spec = BuildSpec {
        context: ctx.to_string_lossy().into_owned(),
        tags: vec![tag.clone()],
        no_cache: true,
        ..Default::default()
    };
    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(4)).await;
        c2.cancel();
    });
    let t0 = std::time::Instant::now();
    let (_, sink) = collect_build();
    let end = engine.build_image(&spec, cancel, sink).await.unwrap();
    assert_eq!(end.end.reason, EndReason::Closed);
    assert!(t0.elapsed() < Duration::from_secs(15));
    let r = last_record(&log, "image.build");
    assert_eq!(r.status, OpStatus::Failed);
    assert!(r.error.unwrap().contains("cancelada por el usuario"));
    assert!(docker.inspect_image(&tag).await.is_err());
}
