// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Gestión masiva (1.10.0) contra Docker real (`CMC_DOCKER_TESTS=1`).
//!
//! Contenedores temporales propios, con las etiquetas `cmc.test=true` y
//! `cmc.bulk-test=<ejecución>`, que se eliminan al terminar (también si la
//! prueba falla). Nunca se usan contenedores del usuario.
// Pruebas contra Docker Engine por socket Unix (Linux); no en Windows (D-114).
#![cfg(unix)]

mod common;

use std::collections::HashMap;

use bollard::models::{ContainerCreateBody, HostConfig};
use bollard::query_parameters::{
    CreateContainerOptions, InspectContainerOptions, RemoveContainerOptions, RemoveVolumeOptions,
};
use cmc_activity::{ActivityLog, OpStatus};
use cmc_engine::bulk::{BulkOperation, ItemDone, RestartPolicy, RestartPolicyName, SkipReason};
use cmc_engine::{ConnectionState, Engine};
use common::*;
use futures_util::future::join_all;
use uuid::Uuid;

struct Cleanup {
    docker: bollard::Docker,
    run: String,
    volume: String,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        let d = self.docker.clone();
        let (run, volume) = (self.run.clone(), self.volume.clone());
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                let mut filters = HashMap::new();
                filters.insert("label".to_owned(), vec![format!("cmc.bulk-test={run}")]);
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
                                v: true,
                                ..Default::default()
                            }),
                        )
                        .await;
                }
                let _ = d.remove_volume(&volume, None::<RemoveVolumeOptions>).await;
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
    cmd: Option<Vec<&str>>,
    host: HostConfig,
) -> String {
    let labels = HashMap::from([
        ("cmc.test".to_owned(), "true".to_owned()),
        ("cmc.bulk-test".to_owned(), run.to_owned()),
    ]);
    docker
        .create_container(
            Some(CreateContainerOptions {
                name: Some(format!("cmc-bulk-{run}-{name}")),
                ..Default::default()
            }),
            ContainerCreateBody {
                image: Some(fixture::IMAGE.into()),
                cmd: cmd.map(|c| c.into_iter().map(str::to_owned).collect()),
                labels: Some(labels),
                host_config: Some(host),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .id
}

async fn status(docker: &bollard::Docker, id: &str) -> Option<String> {
    let r = docker
        .inspect_container(id, Some(InspectContainerOptions { size: false }))
        .await
        .ok()?;
    serde_json::to_value(r.state?.status?)
        .ok()?
        .as_str()
        .map(str::to_owned)
}

/// Aplica `op` a varios contenedores a la vez (como la coordinación de la app).
async fn apply_all(
    e: &Engine,
    op: &BulkOperation,
    ids: &[(String, String)],
) -> Vec<Result<ItemDone, cmc_engine::bulk::ItemError>> {
    join_all(ids.iter().map(|(id, name)| e.bulk_apply(op, id, name))).await
}

fn applied_state(r: &Result<ItemDone, cmc_engine::bulk::ItemError>) -> Option<String> {
    match r {
        Ok(ItemDone::Applied { state, .. }) => state.as_ref().map(|s| s.status.clone()),
        _ => None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn bulk_operations_on_real_temporary_containers() {
    if !enabled() {
        return;
    }
    let sock = real_socket().await;
    let docker =
        bollard::Docker::connect_with_unix(&sock, 60, bollard::API_DEFAULT_VERSION).unwrap();
    fixture::ensure_image(&docker).await;
    let run = format!(
        "{}-{}",
        std::process::id(),
        &Uuid::new_v4().to_string()[..8]
    );
    let volume = format!("cmc-bulk-vol-{run}");
    let _cleanup = Cleanup {
        docker: docker.clone(),
        run: run.clone(),
        volume: volume.clone(),
    };

    let log = ActivityLog::new(2000);
    let m = common::manager(log.clone(), sock.clone());
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    let batch = Uuid::new_v4();
    let base = m.engine().await.unwrap();
    let e = base.with_activity(base.activity().with_batch(batch));

    // Tres contenedores normales (uno con un volumen con nombre), uno cuyo
    // comando no existe (fallo parcial) y uno creado con --rm.
    let a = create(
        &docker,
        &run,
        "a",
        None,
        HostConfig {
            binds: Some(vec![format!("{volume}:/datos")]),
            ..Default::default()
        },
    )
    .await;
    let b = create(&docker, &run, "b", None, HostConfig::default()).await;
    let c = create(&docker, &run, "c", None, HostConfig::default()).await;
    let bad = create(
        &docker,
        &run,
        "roto",
        Some(vec!["/no-existe"]),
        HostConfig::default(),
    )
    .await;
    let rm = create(
        &docker,
        &run,
        "rm",
        None,
        HostConfig {
            auto_remove: Some(true),
            ..Default::default()
        },
    )
    .await;
    let named = |id: &str, n: &str| (id.to_owned(), format!("cmc-bulk-{run}-{n}"));
    let trio = vec![named(&a, "a"), named(&b, "b"), named(&c, "c")];

    // Inicio simultáneo con un fallo parcial: los demás se inician igual.
    let mut four = trio.clone();
    four.push(named(&bad, "roto"));
    let r = apply_all(&e, &BulkOperation::Start, &four).await;
    for x in &r[..3] {
        assert_eq!(applied_state(x).as_deref(), Some("running"), "{x:?}");
    }
    let err = r[3].as_ref().unwrap_err();
    assert!(
        matches!(err.error, cmc_engine::EngineError::Docker { .. }),
        "Docker rechaza iniciar el roto: {err:?}"
    );
    assert!(!err.outcome_unknown());
    assert_ne!(status(&docker, &bad).await.as_deref(), Some("running"));

    // Iniciar otra vez uno en marcha: se omite sin enviar nada.
    let again = e.bulk_apply(&BulkOperation::Start, &a, "a").await.unwrap();
    assert!(matches!(
        again,
        ItemDone::Skipped {
            reason: SkipReason::State { .. },
            ..
        }
    ));

    // Pausa y reanudación.
    let r = apply_all(&e, &BulkOperation::Pause, &trio[..2]).await;
    assert!(r
        .iter()
        .all(|x| applied_state(x).as_deref() == Some("paused")));
    assert_eq!(status(&docker, &a).await.as_deref(), Some("paused"));
    let r = apply_all(&e, &BulkOperation::Restart, &trio[..1]).await;
    assert!(
        matches!(&r[0], Ok(ItemDone::Skipped { .. })),
        "reiniciar uno pausado no se intenta"
    );
    let r = apply_all(&e, &BulkOperation::Unpause, &trio[..2]).await;
    assert!(r
        .iter()
        .all(|x| applied_state(x).as_deref() == Some("running")));

    // Reinicio simultáneo.
    let r = apply_all(&e, &BulkOperation::Restart, &trio).await;
    assert!(
        r.iter()
            .all(|x| applied_state(x).as_deref() == Some("running")),
        "{r:?}"
    );

    // Política de reinicio, comprobada en Docker.
    let policy = BulkOperation::RestartPolicy {
        policy: RestartPolicy {
            name: RestartPolicyName::OnFailure,
            maximum_retry_count: 2,
        },
    };
    let r = apply_all(&e, &policy, &trio).await;
    assert!(
        r.iter().all(|x| matches!(x, Ok(ItemDone::Applied { .. }))),
        "{r:?}"
    );
    for (id, _) in &trio {
        let hc = docker
            .inspect_container(id, Some(InspectContainerOptions { size: false }))
            .await
            .unwrap()
            .host_config
            .unwrap();
        let p = RestartPolicy::from_docker(hc.restart_policy.as_ref());
        assert_eq!(p.cli_value(), "on-failure:2");
    }
    let r = apply_all(&e, &policy, &trio[..1]).await;
    assert!(matches!(
        r[0],
        Ok(ItemDone::Skipped {
            reason: SkipReason::Unchanged,
            ..
        })
    ));
    // Contenedor con --rm: Docker no admite otra política; se omite.
    let r = e.bulk_apply(&policy, &rm, "rm").await.unwrap();
    assert!(matches!(
        r,
        ItemDone::Skipped {
            reason: SkipReason::AutoRemove,
            ..
        }
    ));

    // Detención simultánea.
    let r = apply_all(&e, &BulkOperation::Stop, &trio).await;
    assert!(
        r.iter()
            .all(|x| applied_state(x).as_deref() == Some("exited")),
        "{r:?}"
    );

    // Eliminar sin forzar uno parado funciona; uno en marcha se omite.
    let soft = BulkOperation::Remove { force: false };
    let started = e.bulk_apply(&BulkOperation::Start, &b, "b").await.unwrap();
    assert!(matches!(started, ItemDone::Applied { .. }));
    let r = apply_all(
        &e,
        &soft,
        &[named(&a, "a"), named(&b, "b"), named(&bad, "roto")],
    )
    .await;
    assert!(matches!(r[0], Ok(ItemDone::Applied { state: None, .. })));
    assert!(matches!(
        r[1],
        Ok(ItemDone::Skipped {
            reason: SkipReason::NeedsForce { .. },
            ..
        })
    ));
    assert!(matches!(r[2], Ok(ItemDone::Applied { state: None, .. })));
    assert_eq!(status(&docker, &a).await, None, "eliminado");
    assert!(status(&docker, &b).await.is_some(), "en marcha: no se toca");
    // El volumen del contenedor eliminado sigue existiendo.
    assert!(
        docker.inspect_volume(&volume).await.is_ok(),
        "la eliminación masiva nunca borra volúmenes"
    );
    let hard = BulkOperation::Remove { force: true };
    let r = apply_all(&e, &hard, &[named(&b, "b"), named(&c, "c")]).await;
    assert!(r
        .iter()
        .all(|x| matches!(x, Ok(ItemDone::Applied { state: None, .. }))));

    // Todo lo que se ejecutó quedó registrado con el lote; nada de otros.
    let recs: Vec<_> = log
        .snapshot()
        .into_iter()
        .filter(|r| r.kind.starts_with("container."))
        .collect();
    assert!(!recs.is_empty());
    assert!(
        recs.iter().all(|r| r.batch == Some(batch)),
        "lote en cada registro"
    );
    assert!(recs
        .iter()
        .any(|r| r.kind == "container.start" && r.status == OpStatus::Failed));
    assert!(recs.iter().any(|r| r.kind == "container.update"));
    let removes: Vec<_> = recs
        .iter()
        .filter(|r| r.kind == "container.remove")
        .collect();
    assert_eq!(removes.len(), 4);
    assert!(removes.iter().all(|r| r
        .equivalent_command
        .as_deref()
        .is_some_and(|c| !c.contains("--volumes"))));
}
