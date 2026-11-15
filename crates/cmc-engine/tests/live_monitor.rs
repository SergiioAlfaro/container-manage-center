// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! E5 (1.5.0) contra Docker real (`CMC_DOCKER_TESTS=1`): el muestreo de la
//! monitorización.
//!
//! - Las métricas coinciden con `docker stats` dentro de la tolerancia
//!   documentada (D-102).
//! - Una desconexión falla enseguida (no bloquea) y el muestreo sigue al volver.
//! - Consumo con 50 contenedores y 3 servidores (D-107).
//! - Las alertas saltan con datos reales (CPU, OOM) sin repetirse.
//!
//! Todo lo creado lleva `cmc-test-mon-` en el nombre y se elimina al terminar.
// Pruebas contra Docker Engine por socket Unix (Linux); no en Windows (D-114).
#![cfg(unix)]

mod common;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use bollard::models::{ContainerCreateBody, HostConfig};
use bollard::query_parameters::{CreateContainerOptions, RemoveContainerOptions};
use cmc_activity::ActivityLog;
use cmc_engine::monitor::alerts::AlertEngine;
use cmc_engine::monitor::history::Ring;
use cmc_engine::monitor::sampler::SamplerState;
use cmc_engine::monitor::*;
use cmc_engine::ConnectionState;
use common::*;
use tokio_util::sync::CancellationToken;

struct Guard {
    docker: bollard::Docker,
    names: Vec<String>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        let d = self.docker.clone();
        let names = std::mem::take(&mut self.names);
        let _ = std::thread::spawn(move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async move {
                    for n in names {
                        let _ = d
                            .remove_container(
                                &n,
                                Some(RemoveContainerOptions {
                                    force: true,
                                    v: true,
                                    ..Default::default()
                                }),
                            )
                            .await;
                    }
                })
        })
        .join();
    }
}

async fn run(
    docker: &bollard::Docker,
    name: &str,
    image: &str,
    cmd: Option<Vec<&str>>,
    memory: Option<i64>,
) {
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
    docker
        .create_container(
            Some(CreateContainerOptions {
                name: Some(name.to_owned()),
                ..Default::default()
            }),
            ContainerCreateBody {
                image: Some(image.to_owned()),
                cmd: cmd.map(|c| c.into_iter().map(str::to_owned).collect()),
                labels: Some(HashMap::from([("cmc.test".to_owned(), "true".to_owned())])),
                host_config: Some(HostConfig {
                    memory,
                    memory_swap: memory,
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    docker
        .start_container(
            name,
            None::<bollard::query_parameters::StartContainerOptions>,
        )
        .await
        .unwrap();
}

async fn setup(
    log: &ActivityLog,
) -> Option<(
    bollard::Docker,
    String,
    Proxy,
    tempfile::TempDir,
    cmc_engine::EngineManager,
)> {
    if !enabled() {
        eprintln!("omitida: define CMC_DOCKER_TESTS=1");
        return None;
    }
    let sock = real_socket().await;
    let docker =
        bollard::Docker::connect_with_unix(&sock, 120, bollard::API_DEFAULT_VERSION).unwrap();
    fixture::ensure_image(&docker).await;
    fixture::ensure_shell_image(&docker).await;
    let dir = tempfile::Builder::new()
        .prefix("cmc-test")
        .tempdir()
        .unwrap();
    let proxy = Proxy::start(&dir.path().join("p.sock"), sock.clone()).await;
    let m = manager(log.clone(), proxy.path.display().to_string());
    assert!(matches!(
        m.connect().await,
        ConnectionState::Connected { .. }
    ));
    Some((docker, sock, proxy, dir, m))
}

/// Parsea «12.5MiB / 256MiB» de `docker stats`.
fn parse_size(s: &str) -> f64 {
    let s = s.trim();
    let units = [
        ("KiB", 1024.0),
        ("MiB", 1024.0 * 1024.0),
        ("GiB", 1024.0 * 1024.0 * 1024.0),
        ("kB", 1000.0),
        ("MB", 1e6),
        ("GB", 1e9),
        ("B", 1.0),
    ];
    for (u, f) in units {
        if let Some(n) = s.strip_suffix(u) {
            return n.trim().parse::<f64>().unwrap() * f;
        }
    }
    panic!("tamaño no reconocido: {s}");
}

/// D-102: CPU, memoria, límite y PIDs iguales a `docker stats` dentro de la
/// tolerancia documentada: CPU ±15 puntos con una carga constante (la CLI mide
/// ~1 s y el muestreo, el intervalo), memoria ±5 % (la CLI redondea), límite y
/// PIDs exactos.
#[tokio::test(flavor = "multi_thread")]
async fn metrics_match_docker_stats() {
    let log = ActivityLog::new(1000);
    let Some((docker, sock, proxy, _dir, m)) = setup(&log).await else {
        return;
    };
    if ssh_target().is_some() {
        eprintln!("omitida por SSH: la CLI de comparación habla con el Docker de este equipo");
        return;
    }
    let name = format!("cmc-test-mon-cpu-{}", std::process::id());
    let _g = Guard {
        docker: docker.clone(),
        names: vec![name.clone()],
    };
    run(
        &docker,
        &name,
        fixture::SHELL_IMAGE,
        Some(vec!["sh", "-c", "while :; do :; done"]),
        Some(256 * 1024 * 1024),
    )
    .await;
    let e = m.engine().await.unwrap();
    let before = log.snapshot().len();
    let reqs_before = proxy.requests().len();
    let mut st = SamplerState::default();
    e.monitor_sample(&mut st).await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    let s = e.monitor_sample(&mut st).await.unwrap();
    let c = s
        .containers
        .iter()
        .find(|c| c.meta.name == name)
        .expect("contenedor en la muestra");
    let p = c.point.clone().expect("punto");
    assert_eq!(c.meta.state, "running");
    assert!(
        c.meta.started_at.is_some() && c.meta.restart_count == Some(0),
        "{:?}",
        c.meta
    );
    let out = std::process::Command::new("docker")
        .args([
            "stats",
            "--no-stream",
            "--format",
            "{{.CPUPerc}}|{{.MemUsage}}|{{.PIDs}}",
            &name,
        ])
        .env("DOCKER_HOST", format!("unix://{sock}"))
        .output()
        .expect("docker CLI");
    let text = String::from_utf8_lossy(&out.stdout);
    let parts: Vec<&str> = text.trim().split('|').collect();
    let cli_cpu: f64 = parts[0].trim_end_matches('%').parse().unwrap();
    let (used, limit) = parts[1].split_once('/').unwrap();
    let cli_pids: u64 = parts[2].trim().parse().unwrap();
    eprintln!(
        "monitorización: cpu={:.1}% mem={} límite={} pids={:?} · docker stats: {text}",
        p.cpu.unwrap(),
        p.mem,
        p.mem_limit,
        p.pids
    );
    assert!(
        (p.cpu.unwrap() - cli_cpu).abs() <= 15.0,
        "CPU {} frente a {cli_cpu}",
        p.cpu.unwrap()
    );
    assert!(p.cpu.unwrap() > 50.0, "el bucle ocupa una CPU");
    let cli_used = parse_size(used);
    assert!(
        (p.mem as f64 - cli_used).abs() <= (cli_used * 0.05).max(256.0 * 1024.0),
        "memoria {} frente a {cli_used}",
        p.mem
    );
    assert_eq!(p.mem_limit as f64, parse_size(limit));
    assert_eq!(p.mem_limit, 256 * 1024 * 1024);
    assert_eq!(p.pids, Some(cli_pids));
    assert!(p.net_rx.is_some(), "velocidad de red con dos muestras");
    // El muestreo no registra cada petición en el panel de actividad.
    assert_eq!(
        log.snapshot().len(),
        before,
        "el muestreo no llena el registro"
    );
    // Sin agentes ni cambios: el muestreo solo hace lecturas.
    let sampled = proxy.requests()[reqs_before..].to_vec();
    assert!(!sampled.is_empty());
    assert!(
        sampled.iter().all(|(m, _)| m == "GET"),
        "solo lecturas: {sampled:?}"
    );
    // Equipo según Docker.
    let h = e.monitor_host().await.unwrap();
    assert!(h.ncpu.unwrap_or(0) > 0 && h.mem_total.unwrap_or(0) > 0);
}

/// Sin Docker, la muestra falla enseguida (no bloquea); al volver, sigue.
#[tokio::test(flavor = "multi_thread")]
async fn disconnection_fails_fast_and_sampling_resumes() {
    let log = ActivityLog::new(1000);
    let Some((_docker, sock, proxy, _dir, m)) = setup(&log).await else {
        return;
    };
    if ssh_target().is_some() {
        // Por SSH el motor no pasa por el proxy local: cortarlo no corta nada.
        // La pérdida de la conexión SSH la prueba
        // `live_ssh::a_lost_connection_is_detected_and_restored_and_nothing_is_left_behind`.
        eprintln!("omitida por SSH: el corte se simula con el proxy local");
        return;
    }
    let e = m.engine().await.unwrap();
    let mut st = SamplerState::default();
    assert!(e.monitor_sample(&mut st).await.unwrap().connected);
    let path = proxy.stop();
    let t = Instant::now();
    let r = e.monitor_sample(&mut st).await;
    assert!(r.is_err(), "sin conexión no hay muestra");
    assert!(
        t.elapsed() < Duration::from_secs(16),
        "falla sin quedarse colgado: {:?}",
        t.elapsed()
    );
    st.reset_counters();
    let _proxy = Proxy::start(&path, sock).await;
    let mut ok = false;
    for _ in 0..20 {
        if let Ok(s) = e.monitor_sample(&mut st).await {
            ok = s.connected;
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert!(ok, "el muestreo sigue al volver la conexión");
}

fn proc_cpu_secs() -> f64 {
    let s = std::fs::read_to_string("/proc/self/stat").unwrap();
    let rest = &s[s.rfind(')').unwrap() + 2..];
    let f: Vec<&str> = rest.split_whitespace().collect();
    let ticks = 100.0; // CLK_TCK en Linux
    (f[11].parse::<f64>().unwrap() + f[12].parse::<f64>().unwrap()) / ticks
}

fn proc_rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap()
        .lines()
        .find_map(|l| l.strip_prefix("VmRSS:"))
        .and_then(|v| v.split_whitespace().next()?.parse().ok())
        .unwrap()
}

/// D-107: consumo del muestreo con 50 contenedores en marcha y 3 servidores, a
/// 2 s (el intervalo más exigente) durante 60 s, con el anillo de 30 min de
/// cada servidor. Límites: CPU media ≤ 10 % de una CPU y memoria adicional
/// ≤ 64 MiB; cada muestra de un servidor, < 2 s.
#[tokio::test(flavor = "multi_thread")]
async fn consumption_with_50_containers_and_3_servers() {
    let log = ActivityLog::new(1000);
    let Some((docker, sock, _proxy, dir, m1)) = setup(&log).await else {
        return;
    };
    let pid = std::process::id();
    let names: Vec<String> = (0..50)
        .map(|i| format!("cmc-test-mon-load-{pid}-{i:02}"))
        .collect();
    let _g = Guard {
        docker: docker.clone(),
        names: names.clone(),
    };
    for n in &names {
        run(&docker, n, fixture::IMAGE, None, None).await;
    }
    // Tres servidores: tres conexiones independientes (por SSH, si hay laboratorio).
    let p2 = Proxy::start(&dir.path().join("p2.sock"), sock.clone()).await;
    let p3 = Proxy::start(&dir.path().join("p3.sock"), sock.clone()).await;
    let m2 = manager(log.clone(), p2.path.display().to_string());
    let m3 = manager(log.clone(), p3.path.display().to_string());
    for m in [&m2, &m3] {
        assert!(matches!(
            m.connect().await,
            ConnectionState::Connected { .. }
        ));
    }
    let interval = Duration::from_secs(2);
    let run_for = Duration::from_secs(60);
    let cpu0 = proc_cpu_secs();
    let rss0 = proc_rss_kib();
    let start = Instant::now();
    let cancel = CancellationToken::new();
    let mut tasks = Vec::new();
    for m in [m1.clone(), m2.clone(), m3.clone()] {
        let cancel = cancel.clone();
        tasks.push(tokio::spawn(async move {
            let e = m.engine().await.unwrap();
            let mut st = SamplerState::default();
            let mut ring = Ring::new(900, 30 * 60_000);
            let mut alerts = AlertEngine::new(AlertRules::default());
            let mut agg = cmc_engine::monitor::history::MinuteAggregator::default();
            let mut worst = 0u64;
            let mut n = 0;
            let mut seen = 0usize;
            let mut tick = tokio::time::interval(interval);
            loop {
                tokio::select! {
                    _ = tick.tick() => {}
                    _ = cancel.cancelled() => break,
                }
                let s = e.monitor_sample(&mut st).await.unwrap();
                worst = worst.max(s.took_ms);
                seen = seen.max(s.containers.iter().filter(|c| c.point.is_some()).count());
                alerts.on_sample("x", &s);
                agg.push(&s);
                ring.push(s);
                n += 1;
            }
            (n, worst, seen, ring.approx_bytes())
        }));
    }
    tokio::time::sleep(run_for).await;
    cancel.cancel();
    let mut total = 0;
    let mut worst = 0;
    let mut ring_bytes = 0;
    for t in tasks {
        let (n, w, seen, b) = t.await.unwrap();
        assert!(seen >= 50, "cada servidor ve los 50 contenedores ({seen})");
        total += n;
        worst = worst.max(w);
        ring_bytes += b;
    }
    let secs = start.elapsed().as_secs_f64();
    let cpu = (proc_cpu_secs() - cpu0) / secs * 100.0;
    let rss = proc_rss_kib().saturating_sub(rss0) / 1024;
    let mode = if ssh_target().is_some() {
        "SSH"
    } else {
        "local"
    };
    eprintln!(
        "consumo ({mode}): {total} muestras de servidor en {secs:.0} s · CPU media {cpu:.1} % de una CPU · memoria adicional {rss} MiB · anillos {} KiB · muestra más lenta {worst} ms",
        ring_bytes / 1024
    );
    assert!(cpu <= 10.0, "CPU media {cpu:.1} %");
    assert!(rss <= 64, "memoria adicional {rss} MiB");
    assert!(
        worst < 2000,
        "una muestra de 50 contenedores tarda {worst} ms"
    );
    drop((p2, p3));
}

/// Alertas con datos reales: un bucle que ocupa una CPU hace saltar la de CPU
/// una sola vez; un proceso que se queda sin memoria, la de OOM (por eventos).
#[tokio::test(flavor = "multi_thread")]
async fn real_cpu_and_oom_alerts_fire_once() {
    let log = ActivityLog::new(1000);
    let Some((docker, _sock, _proxy, _dir, m)) = setup(&log).await else {
        return;
    };
    let pid = std::process::id();
    let cpu = format!("cmc-test-mon-alert-{pid}");
    let oom = format!("cmc-test-mon-oom-{pid}");
    let _g = Guard {
        docker: docker.clone(),
        names: vec![cpu.clone(), oom.clone()],
    };
    let e = m.engine().await.unwrap();
    let mut rules = AlertRules::default();
    rules.cpu.above = 50.0;
    rules.cpu.clear_below = 20.0;
    rules.cpu.samples = 2;
    let mut alerts = AlertEngine::new(rules);
    // Eventos de Docker, como los recibe la aplicación.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let cancel = CancellationToken::new();
    let e2 = e.clone();
    let c2 = cancel.clone();
    let watcher = tokio::spawn(async move {
        let _ = e2
            .events_watch(c2, move |ev| {
                let _ = tx.send(ev);
                true
            })
            .await;
    });
    tokio::time::sleep(Duration::from_millis(500)).await;
    run(
        &docker,
        &cpu,
        fixture::SHELL_IMAGE,
        Some(vec!["sh", "-c", "while :; do :; done"]),
        None,
    )
    .await;
    let mut st = SamplerState::default();
    let mut fired = Vec::new();
    for _ in 0..6 {
        let s = e.monitor_sample(&mut st).await.unwrap();
        fired.extend(
            alerts
                .on_sample("local", &s)
                .into_iter()
                .filter(|a| a.container.as_deref() == Some(cpu.as_str())),
        );
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    assert_eq!(fired.len(), 1, "una sola alerta de CPU: {fired:?}");
    assert_eq!(
        (fired[0].kind, fired[0].state, fired[0].notified),
        (AlertKind::Cpu, AlertState::Fired, true)
    );
    // Sin memoria: la cadena se duplica hasta superar 16 MiB.
    run(
        &docker,
        &oom,
        fixture::SHELL_IMAGE,
        Some(vec!["sh", "-c", "x=a; while :; do x=\"$x$x\"; done"]),
        Some(16 * 1024 * 1024),
    )
    .await;
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut kinds = Vec::new();
    while Instant::now() < deadline && !kinds.contains(&AlertKind::Oom) {
        if let Ok(Some(ev)) = tokio::time::timeout(Duration::from_secs(1), rx.recv()).await {
            for a in alerts.on_event("local", &ev) {
                if a.container.as_deref() == Some(oom.as_str()) {
                    kinds.push(a.kind);
                }
            }
        }
    }
    cancel.cancel();
    let _ = watcher.await;
    assert!(kinds.contains(&AlertKind::Oom), "alerta de OOM: {kinds:?}");
}
