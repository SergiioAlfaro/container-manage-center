// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Comandos masivos (1.11.0): validación, riesgo, resultados, registro y
//! comandos guardados.

use cmc_activity::{OpStatus, Transport};
use cmc_engine::bulk_exec::ExecCommand;

use super::*;

fn target(server: &str, c: char, name: &str) -> BulkTarget {
    BulkTarget {
        server: server.into(),
        id: c.to_string().repeat(64),
        name: name.into(),
    }
}

fn cmd(program: &str, args: &[&str]) -> BulkCommand {
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

fn req(c: BulkCommand, targets: Vec<BulkTarget>) -> BulkExecRequest {
    BulkExecRequest {
        command: c,
        targets,
        concurrency: 4,
        confirmed: true,
        confirm_count: None,
    }
}

fn known(s: &str) -> bool {
    s == "local" || s == "prod"
}

fn output(end: ExecEnd, exit: Option<i64>, launch_failed: bool) -> ExecOutput {
    ExecOutput {
        end,
        exit_code: exit,
        still_running: if end == ExecEnd::Exited {
            Some(false)
        } else {
            Some(true)
        },
        stdout: "o".into(),
        stderr: "e".into(),
        stdout_truncated: false,
        stderr_truncated: false,
        launch_failed,
        started_at: Utc::now(),
    }
}

#[test]
fn validation_requires_confirmation_limits_and_a_valid_command() {
    let ts = vec![target("local", 'a', "web"), target("prod", 'a', "web")];
    assert!(
        validate(&req(cmd("df", &["-h"]), ts.clone()), known).is_ok(),
        "mismo ID en dos servidores"
    );
    let mut r = req(cmd("df", &[]), ts.clone());
    r.confirmed = false;
    assert_eq!(validate(&r, known).unwrap_err().code, "engine.notConfirmed");
    for c in [0, 17] {
        let mut r = req(cmd("df", &[]), ts.clone());
        r.concurrency = c;
        assert_eq!(
            validate(&r, known).unwrap_err().code,
            "bulk.invalid",
            "concurrencia {c}"
        );
    }
    assert_eq!(
        validate(&req(cmd("", &[]), ts.clone()), known)
            .unwrap_err()
            .code,
        "engine.invalidInput"
    );
    assert_eq!(
        validate(&req(cmd("df", &[]), vec![]), known)
            .unwrap_err()
            .code,
        "bulk.invalid"
    );
    let dup = vec![target("local", 'a', "web"), target("local", 'a', "web")];
    assert_eq!(
        validate(&req(cmd("df", &[]), dup), known).unwrap_err().code,
        "bulk.invalid"
    );
    assert_eq!(
        validate(&req(cmd("df", &[]), vec![target("otro", 'a', "x")]), known)
            .unwrap_err()
            .code,
        "server.unknown"
    );
}

#[test]
fn risky_commands_need_the_number_of_containers_typed() {
    let ts = vec![target("local", 'a', "a"), target("local", 'b', "b")];
    let mut r = req(cmd("apt-get", &["update"]), ts.clone());
    assert_eq!(validate(&r, known).unwrap_err().code, "bulk.notConfirmed");
    r.confirm_count = Some(1);
    assert_eq!(validate(&r, known).unwrap_err().code, "bulk.notConfirmed");
    r.confirm_count = Some(2);
    assert!(validate(&r, known).is_ok());
    // Informativo o de modificación: basta la confirmación.
    assert!(validate(&req(cmd("touch", &["/tmp/x"]), ts), known).is_ok());
}

#[test]
fn preview_shows_exact_argv_equivalent_and_risk() {
    let mut c = cmd("rm", &["-rf", "/var/cache/x y"]);
    c.user = Some("root".into());
    let p = preview(&c).unwrap();
    assert_eq!(p.argv, vec!["rm", "-rf", "/var/cache/x y"]);
    assert_eq!(
        p.equivalent,
        "docker exec --user root '<contenedor>' rm -rf '/var/cache/x y'"
    );
    assert_eq!(p.risk.level, CommandRiskLevel::Destructive);
    assert!(p.needs_count);
    assert!(preview(&cmd("", &[])).is_err());
    assert!(!preview(&cmd("df", &["-h"])).unwrap().needs_count);
}

#[test]
fn engine_outcomes_map_to_the_states_shown() {
    let t = target("local", 'a', "web");
    let now = Instant::now();
    let st = |r| item_result(0, t.clone(), now, r).status;
    assert_eq!(
        st(Ok(ExecDone::Ran(output(ExecEnd::Exited, Some(0), false)))),
        ExecItemStatus::Succeeded
    );
    assert_eq!(
        st(Ok(ExecDone::Ran(output(ExecEnd::Exited, Some(2), false)))),
        ExecItemStatus::Failed
    );
    assert_eq!(
        st(Ok(ExecDone::Ran(output(ExecEnd::Exited, Some(127), true)))),
        ExecItemStatus::Failed
    );
    assert_eq!(
        st(Ok(ExecDone::Ran(output(ExecEnd::Exited, None, false)))),
        ExecItemStatus::Failed
    );
    assert_eq!(
        st(Ok(ExecDone::Ran(output(ExecEnd::TimedOut, None, false)))),
        ExecItemStatus::TimedOut
    );
    assert_eq!(
        st(Ok(ExecDone::Ran(output(ExecEnd::Interrupted, None, false)))),
        ExecItemStatus::Interrupted
    );
    let nr = item_result(
        0,
        t.clone(),
        now,
        Ok(ExecDone::NotRun {
            reason: SkipReason::State {
                state: "exited".into(),
            },
        }),
    );
    assert_eq!(nr.status, ExecItemStatus::NotRun);
    assert!(nr.skip.is_some() && !nr.cancelled);
    let rejected = item_result(
        0,
        t.clone(),
        now,
        Err(ItemError {
            stage: BulkStage::Check,
            error: EngineError::Docker {
                status: 409,
                message: "is paused".into(),
            },
        }),
    );
    assert_eq!(rejected.status, ExecItemStatus::Failed);
    let cut = item_result(
        0,
        t.clone(),
        now,
        Err(ItemError {
            stage: BulkStage::Execute,
            error: EngineError::Io {
                message: "conexión perdida".into(),
            },
        }),
    );
    assert_eq!(
        cut.status,
        ExecItemStatus::Unknown,
        "nunca se da por bueno ni se repite"
    );
    let c = cancelled(3, t);
    assert_eq!(
        (c.status, c.cancelled, c.index),
        (ExecItemStatus::NotRun, true, 3)
    );
}

#[test]
fn counts_and_text() {
    let c = ExecCounts {
        succeeded: 3,
        failed: 1,
        timed_out: 1,
        interrupted: 0,
        not_run: 2,
        unknown: 1,
    };
    assert_eq!(c.text(), "3 correctos, 1 con error, 1 con tiempo agotado, 1 con resultado desconocido, 2 no ejecutados");
    assert!(!c.clean());
    assert!(ExecCounts {
        succeeded: 2,
        ..Default::default()
    }
    .clean());
}

#[test]
fn summary_record_has_command_targets_and_hides_secrets() {
    let log = ActivityLog::new(10);
    let batch = Uuid::new_v4();
    let mut c = cmd("mysql", &["--password=S3creta", "-e", "select 1"]);
    c.user = Some("app".into());
    let r = req(
        c,
        vec![target("local", 'a', "db"), target("prod", 'b', "db2")],
    );
    let h = begin_summary(&log, batch, &r);
    finish_summary(
        h,
        &ExecCounts {
            succeeded: 1,
            failed: 1,
            ..Default::default()
        },
    );
    let rec = &log.snapshot()[0];
    assert_eq!(rec.kind, "container.exec.bulk");
    assert_eq!(rec.batch, Some(batch));
    assert_eq!(rec.status, OpStatus::Failed);
    let body = &rec.request_body.as_ref().unwrap().text;
    assert!(
        body.contains("\"server\": \"prod\"") && body.contains("\"user\": \"app\""),
        "{body}"
    );
    assert!(!body.contains("S3creta"), "{body}");
    match &rec.transport {
        Transport::App { action } => {
            assert!(
                action.starts_with("Comando masivo en 2 contenedor(es): mysql"),
                "{action}"
            );
            assert!(!action.contains("S3creta"), "{action}");
        }
        t => panic!("{t:?}"),
    }
}

#[test]
fn interrupts_are_per_stream() {
    let i = ExecInterrupts::default();
    let t = CancellationToken::new();
    i.insert(7, t.clone());
    assert!(!i.interrupt(8));
    assert!(!t.is_cancelled());
    assert!(i.interrupt(7));
    assert!(t.is_cancelled());
    i.remove(7);
    assert!(!i.interrupt(7));
}

fn template(id: &str, name: &str) -> ExecTemplate {
    ExecTemplate {
        id: id.into(),
        name: name.into(),
        program: "df".into(),
        args: vec!["-h".into()],
        ..ExecTemplate::default()
    }
}

#[test]
fn saved_commands_are_validated_and_never_keep_secrets() {
    assert!(validate_templates(&[template("a", "Disco"), template("b", "Otro")]).is_ok());
    assert_eq!(
        validate_templates(&[template("a", " ")]).unwrap_err().code,
        "exec.templateInvalid"
    );
    assert_eq!(
        validate_templates(&[template("a", "x"), template("a", "y")])
            .unwrap_err()
            .code,
        "exec.templateInvalid"
    );
    let mut bad = template("a", "Malo");
    bad.program = String::new();
    assert_eq!(
        validate_templates(&[bad]).unwrap_err().code,
        "exec.templateInvalid"
    );
    for t in [
        ExecTemplate {
            args: vec!["--password".into(), "S3creta".into()],
            ..template("a", "x")
        },
        ExecTemplate {
            args: vec!["DB_PASSWORD=S3creta".into()],
            ..template("a", "x")
        },
        ExecTemplate {
            mode: ExecTemplateMode::Script,
            interpreter: "/bin/sh".into(),
            flag: "-c".into(),
            script: "curl https://admin:S3creta@ejemplo.org/".into(),
            ..template("a", "x")
        },
    ] {
        assert_eq!(
            validate_templates(&[t]).unwrap_err().code,
            "exec.templateSecret"
        );
    }
    let script = ExecTemplate {
        mode: ExecTemplateMode::Script,
        interpreter: "/bin/sh".into(),
        flag: "-c".into(),
        script: "df -h\nfree -m".into(),
        ..template("s", "Recursos")
    };
    assert!(validate_templates(std::slice::from_ref(&script)).is_ok());
    assert_eq!(
        template_command(&script).argv(),
        vec!["/bin/sh", "-c", "df -h\nfree -m"]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn pool_runs_exec_items_with_cancel_of_pending() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let cancel = CancellationToken::new();
    let started = Arc::new(AtomicUsize::new(0));
    let targets: Vec<_> = "abcde".chars().map(|c| target("local", c, "x")).collect();
    let (c2, s2) = (cancel.clone(), started.clone());
    let results = crate::bulk::run_pool(
        targets,
        1,
        cancel,
        move |i, t| {
            let (c, s) = (c2.clone(), s2.clone());
            async move {
                // El primero que empieza cancela el resto.
                if s.fetch_add(1, Ordering::SeqCst) == 0 {
                    c.cancel();
                }
                item_result(
                    i,
                    t,
                    Instant::now(),
                    Ok(ExecDone::Ran(output(ExecEnd::Exited, Some(0), false))),
                )
            }
        },
        |_| {},
        |_| {},
    )
    .await;
    assert_eq!(started.load(Ordering::SeqCst), 1);
    let c = ExecCounts::of(&results);
    assert_eq!((c.succeeded, c.not_run), (1, 4));
    assert_eq!(results.iter().filter(|r| r.cancelled).count(), 4);
    assert!(results.iter().enumerate().all(|(i, r)| r.index == i as u32));
}
