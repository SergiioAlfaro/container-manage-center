// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Coordinación de la gestión masiva (1.10.0): validación, reservas,
//! concurrencia, cancelación, resultados y registro.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use cmc_activity::OpStatus;
use cmc_engine::bulk::{ItemError, RestartPolicy, RestartPolicyName};

use super::*;

fn target(server: &str, c: char, name: &str) -> BulkTarget {
    BulkTarget {
        server: server.into(),
        id: c.to_string().repeat(64),
        name: name.into(),
    }
}

fn req(op: BulkOperation, targets: Vec<BulkTarget>) -> BulkRequest {
    BulkRequest {
        operation: op,
        targets,
        confirmed: true,
        confirm_count: None,
    }
}

fn known(s: &str) -> bool {
    s == "local" || s == "prod"
}

fn ok(index: u32, target: BulkTarget) -> BulkItemResult {
    BulkItemResult {
        index,
        target,
        status: BulkItemStatus::Succeeded,
        state: Some("running".into()),
        skip: None,
        error: None,
        stage: None,
        details: None,
        duration_ms: 1,
    }
}

#[test]
fn validation_rejects_unconfirmed_empty_duplicated_and_unknown() {
    let a = target("local", 'a', "web");
    assert!(validate(&req(BulkOperation::Stop, vec![a.clone()]), known).is_ok());

    let mut r = req(BulkOperation::Stop, vec![a.clone()]);
    r.confirmed = false;
    assert_eq!(validate(&r, known).unwrap_err().code, "engine.notConfirmed");

    let code = |r: BulkRequest| validate(&r, known).unwrap_err().code;
    assert_eq!(code(req(BulkOperation::Stop, vec![])), "bulk.invalid");
    assert_eq!(
        code(req(BulkOperation::Stop, vec![a.clone(), a.clone()])),
        "bulk.invalid",
        "el mismo contenedor dos veces"
    );
    assert_eq!(
        code(req(BulkOperation::Stop, vec![target("otro", 'a', "web")])),
        "server.unknown"
    );
    let mut short = a.clone();
    short.id.truncate(12);
    assert_eq!(code(req(BulkOperation::Stop, vec![short])), "bulk.invalid");
    let many: Vec<_> = (0..=MAX_TARGETS)
        .map(|i| BulkTarget {
            server: "local".into(),
            id: format!("{i:064x}"),
            name: format!("c{i}"),
        })
        .collect();
    assert_eq!(code(req(BulkOperation::Stop, many)), "bulk.invalid");
    let bad_policy = BulkOperation::RestartPolicy {
        policy: RestartPolicy {
            name: RestartPolicyName::Always,
            maximum_retry_count: 3,
        },
    };
    assert_eq!(code(req(bad_policy, vec![a])), "engine.invalidInput");
}

#[test]
fn same_id_on_two_servers_are_different_containers() {
    // El mismo ID en dos servidores no es un duplicado: son contenedores distintos.
    let r = req(
        BulkOperation::Stop,
        vec![target("local", 'a', "web"), target("prod", 'a', "web")],
    );
    assert!(validate(&r, known).is_ok());
}

#[test]
fn remove_requires_typing_the_number_of_containers() {
    let ts = vec![target("local", 'a', "a"), target("local", 'b', "b")];
    let mut r = req(BulkOperation::Remove { force: false }, ts);
    assert_eq!(validate(&r, known).unwrap_err().code, "bulk.notConfirmed");
    r.confirm_count = Some(1);
    assert_eq!(validate(&r, known).unwrap_err().code, "bulk.notConfirmed");
    r.confirm_count = Some(2);
    assert!(validate(&r, known).is_ok());
}

#[test]
fn reservations_block_overlapping_bulk_operations() {
    let jobs = BulkJobs::default();
    let a = target("local", 'a', "web");
    let b = target("local", 'b', "db");
    let first = jobs.reserve(&[a.clone(), b.clone()]).unwrap();
    let e = jobs.reserve(std::slice::from_ref(&b)).err().unwrap();
    assert_eq!(e.code, "bulk.busy");
    assert!(e.message.contains("db"));
    // Todo o nada: un conflicto no deja reservado el resto.
    let c = target("local", 'c', "cache");
    assert!(jobs.reserve(&[c.clone(), a.clone()]).is_err());
    assert!(jobs.reserve(std::slice::from_ref(&c)).is_ok());
    // Otro servidor con el mismo ID no choca.
    assert!(jobs.reserve(&[target("prod", 'a', "web")]).is_ok());
    drop(first);
    assert!(jobs.reserve(&[a, b]).is_ok(), "se liberan al terminar");
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrency_limit_is_respected_and_results_keep_their_order() {
    let running = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let targets: Vec<_> = "abcdefghij"
        .chars()
        .map(|c| target("local", c, &c.to_string()))
        .collect();
    let (r2, p2) = (running.clone(), peak.clone());
    let events = Arc::new(Mutex::new(Vec::new()));
    let ev2 = events.clone();
    let results = run_items(
        targets,
        3,
        CancellationToken::new(),
        move |i, t| {
            let (r, p) = (r2.clone(), p2.clone());
            async move {
                let now = r.fetch_add(1, Ordering::SeqCst) + 1;
                p.fetch_max(now, Ordering::SeqCst);
                // Los primeros tardan más: terminan en otro orden.
                tokio::time::sleep(Duration::from_millis(30 - u64::from(i) * 2)).await;
                r.fetch_sub(1, Ordering::SeqCst);
                ok(i, t)
            }
        },
        move |e| ev2.lock().push(e),
    )
    .await;
    assert_eq!(peak.load(Ordering::SeqCst), 3, "nunca más de 3 a la vez");
    assert_eq!(results.len(), 10);
    assert!(results.iter().enumerate().all(|(i, r)| r.index == i as u32));
    let ev = events.lock();
    let items = ev
        .iter()
        .filter(|e| matches!(e, BulkEvent::Item { .. }))
        .count();
    let runs = ev
        .iter()
        .filter(|e| matches!(e, BulkEvent::Running { .. }))
        .count();
    assert_eq!((items, runs), (10, 10), "un evento por contenedor");
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_stops_pending_items_but_lets_running_ones_finish() {
    let cancel = CancellationToken::new();
    let started = Arc::new(AtomicUsize::new(0));
    let targets: Vec<_> = "abcdef".chars().map(|c| target("local", c, "x")).collect();
    let (c2, s2) = (cancel.clone(), started.clone());
    let results = run_items(
        targets,
        2,
        cancel.clone(),
        move |i, t| {
            let (c, s) = (c2.clone(), s2.clone());
            async move {
                s.fetch_add(1, Ordering::SeqCst);
                // El primero pide cancelar cuando los dos huecos están ocupados.
                if i == 0 {
                    while s.load(Ordering::SeqCst) < 2 {
                        tokio::time::sleep(Duration::from_millis(1)).await;
                    }
                    c.cancel();
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
                ok(i, t)
            }
        },
        |_| {},
    )
    .await;
    let counts = BulkCounts::of(&results);
    assert_eq!(
        started.load(Ordering::SeqCst),
        2,
        "solo los que ya estaban en marcha"
    );
    assert_eq!(counts.succeeded, 2, "los que estaban en marcha terminan");
    assert_eq!(counts.cancelled, 4);
    assert!(results[2..]
        .iter()
        .all(|r| r.status == BulkItemStatus::Cancelled && r.duration_ms == 0));
    assert!(!counts.clean());
}

#[test]
fn engine_outcomes_map_to_item_status() {
    let t = target("local", 'a', "web");
    let now = Instant::now();
    let skipped = item_result(
        0,
        t.clone(),
        now,
        Ok(ItemDone::Skipped {
            reason: SkipReason::State {
                state: "running".into(),
            },
            state: "running".into(),
        }),
    );
    assert_eq!(skipped.status, BulkItemStatus::Skipped);
    assert_eq!(skipped.state.as_deref(), Some("running"));

    let removed = item_result(
        0,
        t.clone(),
        now,
        Ok(ItemDone::Applied {
            state: None,
            details: None,
        }),
    );
    assert_eq!(removed.status, BulkItemStatus::Succeeded);
    assert_eq!(removed.state, None);

    let rejected = item_result(
        0,
        t.clone(),
        now,
        Err(ItemError {
            stage: BulkStage::Execute,
            error: EngineError::Docker {
                status: 409,
                message: "conflict".into(),
            },
        }),
    );
    assert_eq!(rejected.status, BulkItemStatus::Failed);
    assert_eq!(rejected.error.as_ref().unwrap().code, "engine.docker");
    assert_eq!(rejected.error.as_ref().unwrap().http_status, Some(409));

    let cut = item_result(
        0,
        t,
        now,
        Err(ItemError {
            stage: BulkStage::Execute,
            error: EngineError::Io {
                message: "broken pipe".into(),
            },
        }),
    );
    assert_eq!(cut.status, BulkItemStatus::Unknown, "nunca se da por buena");
    assert_eq!(cut.stage, Some(BulkStage::Execute));
}

#[test]
fn errors_shown_to_the_interface_hide_secrets() {
    let r = item_result(
        0,
        target("local", 'a', "web"),
        Instant::now(),
        Err(ItemError {
            stage: BulkStage::Execute,
            error: EngineError::Docker {
                status: 500,
                message: "fallo al conectar con https://admin:S3creta@registro.example/v2/".into(),
            },
        }),
    );
    let json = serde_json::to_string(&r).unwrap();
    assert!(!json.contains("S3creta"), "{json}");
}

#[test]
fn counts_text_and_summary_record() {
    let c = BulkCounts {
        succeeded: 3,
        failed: 1,
        skipped: 2,
        cancelled: 0,
        unknown: 1,
    };
    assert_eq!(
        c.text(),
        "3 correctas, 1 fallidas, 1 con resultado desconocido, 2 omitidas"
    );
    assert!(!c.clean());
    assert!(BulkCounts {
        succeeded: 1,
        skipped: 5,
        ..Default::default()
    }
    .clean());

    let log = ActivityLog::new(10);
    let batch = Uuid::new_v4();
    let r = req(
        BulkOperation::RestartPolicy {
            policy: RestartPolicy {
                name: RestartPolicyName::UnlessStopped,
                maximum_retry_count: 0,
            },
        },
        vec![target("local", 'a', "web"), target("prod", 'b', "db")],
    );
    let h = begin_summary(&log, batch, &r, 4);
    finish_summary(h, &c);
    let rec = &log.snapshot()[0];
    assert_eq!(rec.kind, "container.bulk");
    assert_eq!(rec.batch, Some(batch));
    assert_eq!(
        rec.status,
        OpStatus::Failed,
        "hay fallos: el resumen lo refleja"
    );
    assert_eq!(rec.error.as_deref(), Some(c.text().as_str()));
    let body = &rec.request_body.as_ref().unwrap().text;
    assert!(body.contains("\"server\": \"prod\"") && body.contains("\"concurrency\": 4"));
    match &rec.transport {
        Transport::App { action } => assert_eq!(
            action,
            "Operación masiva: cambiar la política de reinicio a unless-stopped 2 contenedor(es)"
        ),
        t => panic!("{t:?}"),
    }
}

#[test]
fn details_cache_is_per_server_and_pruned() {
    let c = DetailsCache::default();
    let d = BulkDetails {
        restart_policy: RestartPolicy {
            name: RestartPolicyName::Always,
            maximum_retry_count: 0,
        },
        auto_remove: false,
    };
    c.set("local", "a", d);
    c.set("prod", "a", d);
    c.set("local", "b", d);
    c.retain("local", &HashSet::from(["a"]));
    assert!(c.get("local", "a").is_some());
    assert!(c.get("local", "b").is_none(), "ya no existe");
    assert!(c.get("prod", "a").is_some(), "otro servidor no se toca");
}
