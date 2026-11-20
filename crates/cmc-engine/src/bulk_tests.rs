// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Reglas puras de la gestión masiva (1.10.0).

use super::*;

const STATES: [&str; 7] = [
    "created",
    "running",
    "paused",
    "restarting",
    "exited",
    "removing",
    "dead",
];

fn ok_states(op: &BulkOperation) -> Vec<&'static str> {
    STATES
        .into_iter()
        .filter(|s| applicable(op, s).is_ok())
        .collect()
}

#[test]
fn lifecycle_rules_match_the_container_list() {
    // Las mismas reglas que `allowedActions` de la interfaz.
    assert_eq!(
        ok_states(&BulkOperation::Start),
        ["created", "exited", "dead"]
    );
    assert_eq!(
        ok_states(&BulkOperation::Stop),
        ["running", "paused", "restarting"]
    );
    assert_eq!(ok_states(&BulkOperation::Restart), ["running"]);
    assert_eq!(ok_states(&BulkOperation::Pause), ["running"]);
    assert_eq!(ok_states(&BulkOperation::Unpause), ["paused"]);
}

#[test]
fn incompatible_states_are_skipped_with_the_state() {
    // Iniciar uno en marcha o pausar uno detenido no se envía a Docker.
    assert_eq!(
        applicable(&BulkOperation::Start, "running"),
        Err(SkipReason::State {
            state: "running".into()
        })
    );
    assert_eq!(
        applicable(&BulkOperation::Pause, "exited"),
        Err(SkipReason::State {
            state: "exited".into()
        })
    );
    assert!(applicable(&BulkOperation::Unpause, "running").is_err());
    assert!(
        applicable(&BulkOperation::Start, "").is_err(),
        "estado desconocido"
    );
}

#[test]
fn remove_needs_force_for_active_containers() {
    let soft = BulkOperation::Remove { force: false };
    let hard = BulkOperation::Remove { force: true };
    for s in ["running", "paused", "restarting"] {
        assert_eq!(
            applicable(&soft, s),
            Err(SkipReason::NeedsForce { state: s.into() })
        );
        assert!(applicable(&hard, s).is_ok());
    }
    for s in ["created", "exited", "dead"] {
        assert!(applicable(&soft, s).is_ok());
    }
    assert!(
        applicable(&hard, "removing").is_err(),
        "ya se está eliminando"
    );
    assert!(soft.is_destructive() && !BulkOperation::Stop.is_destructive());
}

#[test]
fn restart_policy_applies_in_any_state_but_removing() {
    let op = BulkOperation::RestartPolicy {
        policy: RestartPolicy {
            name: RestartPolicyName::Always,
            maximum_retry_count: 0,
        },
    };
    assert_eq!(ok_states(&op).len(), 6);
    assert!(applicable(&op, "removing").is_err());
}

#[test]
fn restart_policy_validation_and_cli_value() {
    let p = |name, n| RestartPolicy {
        name,
        maximum_retry_count: n,
    };
    assert!(p(RestartPolicyName::OnFailure, 5).validate().is_ok());
    assert!(p(RestartPolicyName::OnFailure, MAX_RETRY_COUNT + 1)
        .validate()
        .is_err());
    // Los reintentos solo tienen sentido con on-failure.
    assert!(p(RestartPolicyName::Always, 3).validate().is_err());
    assert_eq!(
        p(RestartPolicyName::OnFailure, 5).cli_value(),
        "on-failure:5"
    );
    assert_eq!(p(RestartPolicyName::OnFailure, 0).cli_value(), "on-failure");
    assert_eq!(
        p(RestartPolicyName::UnlessStopped, 0).cli_value(),
        "unless-stopped"
    );
    assert_eq!(p(RestartPolicyName::No, 0).cli_value(), "no");
    // La validación de la operación incluye la de la política.
    assert!(BulkOperation::RestartPolicy {
        policy: p(RestartPolicyName::No, 2)
    }
    .validate()
    .is_err());
}

#[test]
fn restart_policy_roundtrip_with_docker() {
    for name in [
        RestartPolicyName::No,
        RestartPolicyName::Always,
        RestartPolicyName::UnlessStopped,
        RestartPolicyName::OnFailure,
    ] {
        let p = RestartPolicy {
            name,
            maximum_retry_count: if name == RestartPolicyName::OnFailure {
                3
            } else {
                0
            },
        };
        assert_eq!(RestartPolicy::from_docker(Some(&p.to_docker())), p);
    }
    // Sin política o con el nombre vacío de Docker: «no».
    let none = RestartPolicy {
        name: RestartPolicyName::No,
        maximum_retry_count: 0,
    };
    assert_eq!(RestartPolicy::from_docker(None), none);
    let empty = DockerRestartPolicy {
        name: Some(RestartPolicyNameEnum::EMPTY),
        maximum_retry_count: Some(7),
    };
    assert_eq!(RestartPolicy::from_docker(Some(&empty)), none);
}

#[test]
fn full_ids_only() {
    let id = "a".repeat(64);
    assert!(is_full_id(&id));
    assert!(!is_full_id(&"a".repeat(12)), "prefijo");
    assert!(!is_full_id("web"), "nombre");
    assert!(!is_full_id(&"A".repeat(64)), "mayúsculas");
    assert!(!is_full_id(&format!("{}g", "a".repeat(63))));
}

#[test]
fn health_exit_code_and_error_from_docker_status() {
    assert_eq!(
        health_of("Up 3 minutes (unhealthy)").as_deref(),
        Some("unhealthy")
    );
    assert_eq!(
        health_of("Up 3 minutes (healthy)").as_deref(),
        Some("healthy")
    );
    assert_eq!(
        health_of("Up 2 seconds (health: starting)").as_deref(),
        Some("starting")
    );
    assert_eq!(health_of("Up 3 minutes"), None);
    assert_eq!(exit_code_of("Exited (137) 2 hours ago"), Some(137));
    assert_eq!(exit_code_of("Exited (0) 1 minute ago"), Some(0));
    assert_eq!(exit_code_of("Up 1 minute"), None);

    assert!(has_error("exited", "Exited (1) 1 minute ago"));
    assert!(!has_error("exited", "Exited (0) 1 minute ago"));
    // Detenido con docker stop: no es un error.
    assert!(!has_error("exited", "Exited (143) 1 minute ago"));
    assert!(!has_error("exited", "Exited (137) 1 minute ago"));
    assert!(has_error("exited", "Exited (3) 1 minute ago"));
    assert!(has_error("dead", ""));
    assert!(has_error("restarting", "Restarting (1) 3 seconds ago"));
    assert!(has_error("running", "Up 1 minute (unhealthy)"));
    assert!(!has_error("running", "Up 1 minute (healthy)"));
    assert!(!has_error("created", "Created"));
}

#[test]
fn operation_wire_format() {
    let op: BulkOperation = serde_json::from_str(
        r#"{"kind":"restartPolicy","policy":{"name":"on-failure","maximumRetryCount":3}}"#,
    )
    .unwrap();
    assert_eq!(
        op,
        BulkOperation::RestartPolicy {
            policy: RestartPolicy {
                name: RestartPolicyName::OnFailure,
                maximum_retry_count: 3
            }
        }
    );
    let rm: BulkOperation = serde_json::from_str(r#"{"kind":"remove","force":true}"#).unwrap();
    assert_eq!(rm, BulkOperation::Remove { force: true });
    assert_eq!(rm.key(), "remove");
    // No existe una operación para cambiar etiquetas: Docker no lo permite sin recrear.
    assert!(serde_json::from_str::<BulkOperation>(r#"{"kind":"labels"}"#).is_err());
}

#[test]
fn unknown_outcome_only_after_sending_the_order() {
    let conn = EngineError::Io {
        message: "corte".into(),
    };
    let docker = EngineError::Docker {
        status: 500,
        message: "x".into(),
    };
    let e = |stage, error| ItemError { stage, error };
    assert!(
        !e(BulkStage::Check, conn.clone()).outcome_unknown(),
        "no se envió nada"
    );
    assert!(e(BulkStage::Execute, conn.clone()).outcome_unknown());
    assert!(e(BulkStage::Execute, EngineError::Timeout).outcome_unknown());
    // bollard informa de algunos cortes como un error genérico del cliente HTTP.
    assert!(e(
        BulkStage::Execute,
        EngineError::Other {
            message: "Error in the hyper legacy client: client error (SendRequest)".into()
        }
    )
    .outcome_unknown());
    assert!(
        !e(BulkStage::Execute, EngineError::NotConnected).outcome_unknown(),
        "sin conexión no se envió"
    );
    assert!(
        !e(BulkStage::Execute, docker.clone()).outcome_unknown(),
        "Docker la rechazó"
    );
    assert!(e(BulkStage::Confirm, docker).outcome_unknown());
}
