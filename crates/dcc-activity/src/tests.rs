use super::*;

fn api_spec(kind: &str) -> OperationSpec {
    OperationSpec {
        category: Category::Container,
        kind: kind.into(),
        target: Some("web".into()),
        transport: Transport::Api {
            method: "POST".into(),
            path: "/v1.47/containers/web/start".into(),
            endpoint: "/var/run/docker.sock".into(),
        },
        request_body: None,
        equivalent_command: Some(quote::render_argv("docker", &["start", "web"])),
    }
}

#[test]
fn begin_records_running_operation() {
    let log = ActivityLog::new(10);
    let handle = log.begin(api_spec("container.start"));
    let snap = log.snapshot();
    assert_eq!(snap.len(), 1);
    assert_eq!(snap[0].id, handle.id());
    assert_eq!(snap[0].status, OpStatus::Running);
    assert!(snap[0].finished_at.is_none());
    handle.succeed(Outcome::default());
}

#[test]
fn succeed_sets_status_duration_and_outputs() {
    let log = ActivityLog::new(10);
    let h = log.begin(api_spec("container.start"));
    h.succeed(Outcome {
        summary: Some("ok".into()),
        http_status: Some(204),
        ..Default::default()
    });
    let r = &log.snapshot()[0];
    assert_eq!(r.status, OpStatus::Succeeded);
    assert_eq!(r.http_status, Some(204));
    assert_eq!(r.summary.as_deref(), Some("ok"));
    assert!(r.finished_at.is_some());
    assert!(r.duration_ms.is_some());
    assert!(r.error.is_none());
}

#[test]
fn fail_records_error() {
    let log = ActivityLog::new(10);
    let h = log.begin(api_spec("container.start"));
    h.fail(Failure {
        error: "No such container: web".into(),
        http_status: Some(404),
        ..Default::default()
    });
    let r = &log.snapshot()[0];
    assert_eq!(r.status, OpStatus::Failed);
    assert_eq!(r.error.as_deref(), Some("No such container: web"));
    assert_eq!(r.http_status, Some(404));
}

#[test]
fn dropped_handle_marks_cancelled() {
    let log = ActivityLog::new(10);
    {
        let _h = log.begin(api_spec("image.pull"));
    }
    let r = &log.snapshot()[0];
    assert_eq!(r.status, OpStatus::Cancelled);
    assert!(r.finished_at.is_some());
}

#[test]
fn capacity_discards_oldest() {
    let log = ActivityLog::new(3);
    for i in 0..5 {
        log.begin(api_spec(&format!("op.{i}")))
            .succeed(Outcome::default());
    }
    let kinds: Vec<_> = log.snapshot().into_iter().map(|r| r.kind).collect();
    assert_eq!(kinds, vec!["op.4", "op.3", "op.2"]);
}

#[test]
fn set_capacity_shrinks() {
    let log = ActivityLog::new(10);
    for i in 0..6 {
        log.begin(api_spec(&format!("op.{i}")))
            .succeed(Outcome::default());
    }
    log.set_capacity(2);
    assert_eq!(log.len(), 2);
    assert_eq!(log.snapshot()[0].kind, "op.5");
}

#[test]
fn clear_keeps_running_operations() {
    let log = ActivityLog::new(10);
    log.begin(api_spec("done")).succeed(Outcome::default());
    let running = log.begin(api_spec("running"));
    log.clear();
    let snap = log.snapshot();
    assert_eq!(snap.len(), 1);
    assert_eq!(snap[0].kind, "running");
    running.succeed(Outcome::default());
    assert_eq!(log.snapshot()[0].status, OpStatus::Succeeded);
}

#[test]
fn capture_truncates_on_char_boundary() {
    let c = capture("ñññ", 3); // cada ñ ocupa 2 bytes
    assert!(c.truncated);
    assert_eq!(c.text, "ñ");
    assert_eq!(c.original_bytes, 6);
    let c = capture("abc", 10);
    assert!(!c.truncated);
}

#[test]
fn record_serializes_with_explicit_transport_tag() {
    let log = ActivityLog::new(10);
    log.begin(api_spec("container.start"))
        .succeed(Outcome::default());
    let json = serde_json::to_value(&log.snapshot()[0]).unwrap();
    assert_eq!(json["transport"]["via"], "api");
    assert_eq!(json["status"], "succeeded");
    assert_eq!(json["category"], "container");
    assert!(json.get("equivalentCommand").is_some());

    let cli = Transport::Cli {
        program: "docker".into(),
        args: vec!["compose".into(), "up".into()],
    };
    assert_eq!(serde_json::to_value(cli).unwrap()["via"], "cli");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn subscribers_receive_start_and_finish() {
    let log = ActivityLog::new(10);
    let mut rx = log.subscribe();
    let h = log.begin(api_spec("container.start"));
    h.succeed(Outcome::default());
    let first = rx.recv().await.unwrap();
    let second = rx.recv().await.unwrap();
    match (first, second) {
        (ActivityEvent::Upsert { record: a }, ActivityEvent::Upsert { record: b }) => {
            assert_eq!(a.status, OpStatus::Running);
            assert_eq!(b.status, OpStatus::Succeeded);
            assert_eq!(a.id, b.id);
        }
        other => panic!("eventos inesperados: {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_operations_are_all_recorded() {
    let log = ActivityLog::new(1000);
    let mut tasks = Vec::new();
    for i in 0..200 {
        let log = log.clone();
        tasks.push(tokio::spawn(async move {
            let h = log.begin(api_spec(&format!("op.{i}")));
            tokio::task::yield_now().await;
            if i % 2 == 0 {
                h.succeed(Outcome::default());
            } else {
                h.fail(Failure {
                    error: "x".into(),
                    ..Default::default()
                });
            }
        }));
    }
    for t in tasks {
        t.await.unwrap();
    }
    let snap = log.snapshot();
    assert_eq!(snap.len(), 200);
    assert!(snap.iter().all(|r| r.status != OpStatus::Running));
    assert_eq!(
        snap.iter().filter(|r| r.status == OpStatus::Failed).count(),
        100
    );
}

#[test]
fn a_server_view_tags_its_operations_and_shares_the_log() {
    let log = ActivityLog::new(10);
    let tag = ServerTag {
        id: "srv-0123abcd".into(),
        name: "Producción A".into(),
        address: "deploy@a.example.com:22".into(),
        remote: true,
    };
    let remote = log.with_server(tag.clone());
    let local = log.with_server(ServerTag::local());
    let mut rx = log.subscribe();
    let spec = |kind: &str| OperationSpec {
        category: Category::Container,
        kind: kind.into(),
        target: None,
        transport: Transport::App {
            action: "prueba".into(),
        },
        request_body: None,
        equivalent_command: None,
    };
    remote
        .begin(spec("container.start"))
        .succeed(Outcome::default());
    local
        .begin(spec("container.stop"))
        .succeed(Outcome::default());
    log.begin(spec("ui.language")).succeed(Outcome::default());
    let snap = log.snapshot();
    assert_eq!(snap.len(), 3, "un solo registro compartido");
    let by_kind = |k: &str| snap.iter().find(|r| r.kind == k).unwrap().server.clone();
    assert_eq!(by_kind("container.start"), Some(tag.clone()));
    assert_eq!(
        by_kind("container.stop").map(|s| s.id),
        Some("local".to_owned())
    );
    assert_eq!(by_kind("ui.language"), None);
    assert!(
        rx.try_recv().is_ok(),
        "los suscriptores del original reciben todo"
    );
    let h = crate::HistoryEntry::from_record(
        snap.iter().find(|r| r.kind == "container.start").unwrap(),
    )
    .unwrap();
    assert_eq!(h.server, Some(tag));
    // Una entrada antigua (sin servidor) se sigue leyendo.
    let mut v = serde_json::to_value(&h).unwrap();
    v.as_object_mut().unwrap().remove("server");
    let old: crate::HistoryEntry = serde_json::from_value(v).unwrap();
    assert_eq!(old.server, None);
}
