//! Conexión con un Docker Engine remoto en siete pasos (requisito 3.3):
//! resolución, alcance SSH, identidad, autenticación, CLI de Docker, permisos y
//! daemon, y compatibilidad de la API. Lo usan tanto «Probar conexión» como la
//! conexión real, para que el diagnóstico sea siempre el mismo.

use std::time::{Duration, Instant};

use dcc_activity::ActivityLog;
use tokio::io::AsyncReadExt;

use super::args::{effective_config_args, remote_docker};
use super::hostkey;
use super::problem::{classify_docker, classify_ssh, RemoteProblem};
use super::profile::{display_address, docker_host_uri, validate, SshProfile};
use super::{
    pending_steps, set_step, CheckId, CheckStatus, CheckStep, HostKeyInfo, SshContext, SshSession,
};
use crate::client::{self, Engine, MIN_API_VERSION};
use crate::model::{Endpoint, EndpointSource, EngineSummary};

/// Resultado de la secuencia, con cada paso.
pub struct Outcome {
    pub steps: Vec<CheckStep>,
    /// Huella presentada por el servidor (si se llegó a verla).
    pub presented: Option<HostKeyInfo>,
    pub result: Result<Connected, RemoteProblem>,
}

pub struct Connected {
    pub engine: Engine,
    pub summary: EngineSummary,
    /// Tiempo de respuesta de `GET /_ping` por SSH.
    pub latency_ms: u64,
}

/// Endpoint con el que se registran las operaciones de este servidor.
pub fn endpoint_of(p: &SshProfile) -> Endpoint {
    Endpoint {
        socket_path: docker_host_uri(p),
        source: EndpointSource::Ssh {
            server_id: p.id.clone(),
            name: p.name.clone(),
        },
    }
}

/// Configuración efectiva de `ssh -G`: nombre real, puerto y si hay un intermedio.
struct Effective {
    hostname: String,
    port: u16,
    via: Option<String>,
}

async fn effective(ctx: &SshContext, p: &SshProfile) -> Effective {
    let mut e = Effective {
        hostname: p.host.clone(),
        port: p.port,
        via: (!p.options.proxy_jump.is_empty()).then(|| p.options.proxy_jump.clone()),
    };
    if let Ok(r) = hostkey::run(
        &ctx.paths.ssh,
        &effective_config_args(p),
        Duration::from_secs(5),
    )
    .await
    {
        for line in r.stdout.lines() {
            match line.split_once(' ') {
                Some(("hostname", v)) => e.hostname = v.trim().to_owned(),
                Some(("port", v)) => e.port = v.trim().parse().unwrap_or(p.port),
                Some(("proxyjump", v)) if v.trim() != "none" => e.via = Some(v.trim().to_owned()),
                Some(("proxycommand", v)) if v.trim() != "none" => {
                    e.via = Some(v.trim().chars().take(80).collect())
                }
                _ => {}
            }
        }
    }
    e
}

/// Pasos 1 y 2: el nombre se resuelve y el puerto responde como SSH.
async fn reach(
    p: &SshProfile,
    e: &Effective,
    steps: &mut [CheckStep],
) -> Result<(), RemoteProblem> {
    if let Some(via) = &e.via {
        let d = Some(format!("a través de {via}"));
        set_step(steps, CheckId::Resolve, CheckStatus::Skipped, d.clone());
        set_step(steps, CheckId::Reach, CheckStatus::Skipped, d);
        return Ok(());
    }
    let addrs: Vec<std::net::SocketAddr> = match tokio::time::timeout(
        Duration::from_secs(5),
        tokio::net::lookup_host((e.hostname.as_str(), e.port)),
    )
    .await
    {
        Ok(Ok(a)) => a.collect(),
        _ => Vec::new(),
    };
    let Some(addr) = addrs.first().copied() else {
        set_step(steps, CheckId::Resolve, CheckStatus::Failed, None);
        return Err(RemoteProblem::HostUnresolvable {
            host: e.hostname.clone(),
        });
    };
    set_step(
        steps,
        CheckId::Resolve,
        CheckStatus::Ok,
        Some(addr.ip().to_string()),
    );
    let timeout = Duration::from_secs(u64::from(p.options.connect_timeout));
    let mut stream = match tokio::time::timeout(timeout, tokio::net::TcpStream::connect(addr)).await
    {
        Ok(Ok(s)) => s,
        Ok(Err(err)) => {
            set_step(steps, CheckId::Reach, CheckStatus::Failed, None);
            return Err(match err.kind() {
                std::io::ErrorKind::ConnectionRefused => RemoteProblem::SshRefused { port: e.port },
                _ => RemoteProblem::HostUnreachable {
                    message: err.to_string(),
                },
            });
        }
        Err(_) => {
            set_step(steps, CheckId::Reach, CheckStatus::Failed, None);
            return Err(RemoteProblem::Timeout);
        }
    };
    let mut banner = [0u8; 64];
    let n = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut banner))
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or(0);
    let text = String::from_utf8_lossy(&banner[..n]);
    let line = text.lines().next().unwrap_or("").trim();
    if !line.starts_with("SSH-") {
        set_step(steps, CheckId::Reach, CheckStatus::Failed, None);
        return Err(RemoteProblem::NotSsh);
    }
    set_step(
        steps,
        CheckId::Reach,
        CheckStatus::Ok,
        Some(line.chars().take(60).collect()),
    );
    Ok(())
}

/// Clasifica por qué no responde Docker, preguntando al CLI remoto.
async fn docker_problem(session: &SshSession, fallback: String) -> RemoteProblem {
    let mut argv = remote_docker(&session.profile().docker);
    argv.extend([
        "version".into(),
        "--format".into(),
        "{{.Server.Version}}".into(),
    ]);
    match session.run(&argv, Duration::from_secs(20)).await {
        Ok(o) => {
            classify_docker(&o.stderr).unwrap_or(RemoteProblem::NotDocker { message: fallback })
        }
        Err(p) => p,
    }
}

/// Ejecuta la secuencia completa. `activity`: dónde se registran las
/// peticiones a la API (las de comprobación se registran siempre, como en la
/// conexión local: fundamentan lo que se muestra después).
pub async fn connect(ctx: &SshContext, p: &SshProfile, activity: &ActivityLog) -> Outcome {
    let mut steps = pending_steps();
    let mut presented = None;
    let result = run_steps(ctx, p, activity, &mut steps, &mut presented).await;
    Outcome {
        steps,
        presented,
        result,
    }
}

async fn run_steps(
    ctx: &SshContext,
    p: &SshProfile,
    activity: &ActivityLog,
    steps: &mut [CheckStep],
    presented: &mut Option<HostKeyInfo>,
) -> Result<Connected, RemoteProblem> {
    let issues = validate(p);
    if !issues.is_empty() {
        return Err(RemoteProblem::InvalidProfile {
            codes: issues.into_iter().map(|i| i.code).collect(),
        });
    }
    if !ctx.paths.ssh.exists() {
        return Err(RemoteProblem::SshMissing);
    }
    let eff = effective(ctx, p).await;
    reach(p, &eff, steps).await?;

    // 3. Identidad.
    match hostkey::check(&ctx.paths, p).await {
        Ok(k) => {
            set_step(
                steps,
                CheckId::Identity,
                CheckStatus::Ok,
                k.as_ref().map(|k| k.fingerprint.clone()),
            );
            *presented = k;
        }
        Err(mut problem) => {
            if let RemoteProblem::HostKeyUnknown {
                presented: pk @ None,
            } = &mut problem
            {
                *pk = hostkey::presented(&ctx.paths, p).await.ok();
            }
            // ¿La clave anterior está en el known_hosts del usuario (que no se toca)?
            if let RemoteProblem::HostKeyChanged {
                file: Some(f),
                in_user_file,
                ..
            } = &mut problem
            {
                *in_user_file = std::path::Path::new(f.as_str()) == ctx.paths.user_known_hosts;
            }
            if let RemoteProblem::HostKeyUnknown { presented: pk }
            | RemoteProblem::HostKeyChanged { presented: pk, .. } = &problem
            {
                *presented = pk.clone();
            }
            let failed_here = matches!(
                problem,
                RemoteProblem::HostKeyUnknown { .. } | RemoteProblem::HostKeyChanged { .. }
            );
            if failed_here {
                set_step(
                    steps,
                    CheckId::Identity,
                    CheckStatus::Failed,
                    presented.as_ref().map(|k| k.fingerprint.clone()),
                );
            } else {
                // Problema de red antes de llegar a la identidad.
                set_step(steps, CheckId::Reach, CheckStatus::Failed, None);
            }
            return Err(problem);
        }
    }

    // 4. Autenticación.
    let session = match SshSession::open(ctx, p).await {
        Ok(s) => s,
        Err(problem) => {
            set_step(steps, CheckId::Auth, CheckStatus::Failed, None);
            return Err(problem);
        }
    };
    set_step(
        steps,
        CheckId::Auth,
        CheckStatus::Ok,
        Some(display_address(p)),
    );

    // 5. CLI de Docker en el servidor.
    match session
        .run(
            &["docker".to_owned(), "--version".into()],
            Duration::from_secs(20),
        )
        .await
    {
        Ok(o) if o.code == Some(0) => {
            set_step(
                steps,
                CheckId::Docker,
                CheckStatus::Ok,
                Some(o.stdout.trim().chars().take(80).collect()),
            );
        }
        Ok(o) => {
            set_step(steps, CheckId::Docker, CheckStatus::Failed, None);
            session.close().await;
            return Err(classify_ssh(o.code, &o.stderr, &p.host, p.port));
        }
        Err(problem) => {
            set_step(steps, CheckId::Docker, CheckStatus::Failed, None);
            session.close().await;
            return Err(problem);
        }
    }

    // 6. Permisos sobre el socket y daemon en marcha: un ping real por dial-stdio.
    let engine = match Engine::new_ssh(endpoint_of(p), session.clone(), activity.clone()) {
        Ok(e) => e,
        Err(e) => {
            session.close().await;
            return Err(RemoteProblem::Other {
                message: e.to_string(),
            });
        }
    };
    let started = Instant::now();
    if let Err(e) = engine.ping().await {
        set_step(steps, CheckId::Permission, CheckStatus::Failed, None);
        let problem = docker_problem(&session, e.to_string()).await;
        drop(engine);
        session.close().await;
        return Err(problem);
    }
    let latency_ms = started.elapsed().as_millis() as u64;
    set_step(
        steps,
        CheckId::Permission,
        CheckStatus::Ok,
        Some(format!("{latency_ms} ms")),
    );

    // 7. API compatible.
    let checked = async {
        let version = engine.version().await.map_err(|e| e.to_string())?;
        let api = version.api_version.clone().unwrap_or_default();
        if !client::api_at_least(&api, MIN_API_VERSION) {
            return Ok(Err(RemoteProblem::ApiTooOld {
                api_version: api,
                minimum: format!("{}.{}", MIN_API_VERSION.0, MIN_API_VERSION.1),
            }));
        }
        let info = engine.info().await.map_err(|e| e.to_string())?;
        Ok::<_, String>(Ok(client::summary(&version, &info)))
    }
    .await;
    match checked {
        Ok(Ok(summary)) => {
            set_step(
                steps,
                CheckId::Api,
                CheckStatus::Ok,
                Some(format!(
                    "Docker {} · API {}",
                    summary.version, summary.api_version
                )),
            );
            Ok(Connected {
                engine,
                summary,
                latency_ms,
            })
        }
        Ok(Err(problem)) => {
            set_step(steps, CheckId::Api, CheckStatus::Failed, None);
            drop(engine);
            session.close().await;
            Err(problem)
        }
        Err(message) => {
            set_step(steps, CheckId::Api, CheckStatus::Failed, None);
            drop(engine);
            session.close().await;
            Err(RemoteProblem::NotDocker { message })
        }
    }
}

/// Resultado de «Probar conexión» (la sesión no se conserva).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TestReport {
    pub steps: Vec<CheckStep>,
    pub presented: Option<HostKeyInfo>,
    pub problem: Option<RemoteProblem>,
    pub engine: Option<EngineSummary>,
    #[ts(type = "number | null")]
    pub latency_ms: Option<u64>,
}

/// Prueba la conexión completa y la cierra.
pub async fn test(ctx: &SshContext, p: &SshProfile, activity: &ActivityLog) -> TestReport {
    let out = connect(ctx, p, activity).await;
    let (problem, engine, latency_ms) = match out.result {
        Ok(c) => {
            if let Some(s) = c.engine.ssh_session().cloned() {
                drop(c.engine);
                s.close().await;
            }
            (None, Some(c.summary), Some(c.latency_ms))
        }
        Err(p) => (Some(p), None, None),
    };
    TestReport {
        steps: out.steps,
        presented: out.presented,
        problem,
        engine,
        latency_ms,
    }
}
