// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Muestreo de un servidor (D-102).
//!
//! En cada muestra: la lista de contenedores y, de cada uno en marcha, una
//! lectura `stats` **one-shot** (Docker responde enseguida, sin esperar un
//! segundo). La CPU se calcula entre dos muestras seguidas con la fórmula de
//! `docker stats` (`calculateCPUPercentUnix`):
//! `Δ uso del contenedor / Δ uso del sistema × CPUs × 100`. La memoria, con
//! la de la CLI (uso menos `inactive_file`). Red y disco, como velocidades a
//! partir de sus contadores. Nada de esto se registra petición a petición.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use bollard::models::ContainerStatsResponse;
use bollard::query_parameters::{InspectContainerOptions, StatsOptions};
use futures_util::{stream, StreamExt};

use super::model::*;
use crate::client::Engine;
use crate::containers::ContainerSummary;
use crate::error::EngineError;
use crate::streams::sample_from;

/// Lecturas simultáneas como mucho (para no saturar Docker ni el SSH).
pub const CONCURRENCY: usize = 8;
const READ_TIMEOUT: Duration = Duration::from_secs(10);

/// Contadores acumulados de una lectura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Raw {
    pub t_ms: i64,
    pub cpu_total: u64,
    pub system: u64,
    pub online: u32,
    pub net_rx: u64,
    pub net_tx: u64,
    pub blk_read: u64,
    pub blk_write: u64,
}

pub fn raw_from(s: &ContainerStatsResponse, t_ms: i64) -> Raw {
    let cpu = s.cpu_stats.clone().unwrap_or_default();
    let x = sample_from(s);
    Raw {
        t_ms,
        cpu_total: cpu
            .cpu_usage
            .as_ref()
            .and_then(|u| u.total_usage)
            .unwrap_or(0),
        system: cpu.system_cpu_usage.unwrap_or(0),
        online: x.online_cpus.unwrap_or(1).max(1),
        net_rx: x.net_rx,
        net_tx: x.net_tx,
        blk_read: x.block_read,
        blk_write: x.block_write,
    }
}

fn rate(prev: u64, cur: u64, dt: f64) -> Option<f64> {
    // Un contador que baja es un contenedor que ha vuelto a empezar.
    (cur >= prev && dt > 0.0).then(|| (cur - prev) as f64 / dt)
}

/// Punto de un contenedor a partir de la lectura actual y de la anterior.
pub fn point(prev: Option<&Raw>, cur: &Raw, s: &ContainerStatsResponse) -> ContainerPoint {
    let x = sample_from(s);
    let (cpu, net_rx, net_tx, blk_read, blk_write) = match prev {
        Some(p) if cur.t_ms > p.t_ms => {
            let dt = (cur.t_ms - p.t_ms) as f64 / 1000.0;
            let cpu = (cur.system > p.system && cur.cpu_total >= p.cpu_total).then(|| {
                (cur.cpu_total - p.cpu_total) as f64 / (cur.system - p.system) as f64
                    * f64::from(cur.online)
                    * 100.0
            });
            (
                cpu,
                rate(p.net_rx, cur.net_rx, dt),
                rate(p.net_tx, cur.net_tx, dt),
                rate(p.blk_read, cur.blk_read, dt),
                rate(p.blk_write, cur.blk_write, dt),
            )
        }
        _ => (None, None, None, None, None),
    };
    ContainerPoint {
        t: cur.t_ms,
        cpu,
        mem: x.memory_used,
        mem_limit: x.memory_limit,
        mem_percent: x.memory_percent,
        net_rx,
        net_tx,
        blk_read,
        blk_write,
        pids: x.pids,
    }
}

/// Lo que el muestreador recuerda entre una muestra y la siguiente.
#[derive(Debug, Default)]
pub struct SamplerState {
    prev: HashMap<String, Raw>,
    meta: HashMap<String, ContainerMeta>,
    dirty: HashSet<String>,
}

impl SamplerState {
    /// Fuerza a releer `inspect` de un contenedor (tras un evento de Docker).
    pub fn mark_dirty(&mut self, id: &str) {
        self.dirty.insert(id.to_owned());
    }

    /// Olvida las lecturas anteriores (tras una desconexión): la siguiente
    /// muestra no calcula velocidades sobre un intervalo con hueco.
    pub fn reset_counters(&mut self) {
        self.prev.clear();
    }
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn counts(list: &[ContainerSummary]) -> Counts {
    let mut c = Counts::default();
    for x in list {
        c.total += 1;
        match x.state.as_str() {
            "running" | "restarting" => c.running += 1,
            "paused" => c.paused += 1,
            _ => c.stopped += 1,
        }
    }
    c
}

impl Engine {
    async fn stats_one_shot(&self, id: &str) -> Result<ContainerStatsResponse, EngineError> {
        let d = self.docker().clone();
        let id = id.to_owned();
        self.call_silent(Some(READ_TIMEOUT), async move {
            let mut s = d.stats(
                &id,
                Some(StatsOptions {
                    stream: false,
                    one_shot: true,
                }),
            );
            match s.next().await {
                Some(r) => r,
                None => Err(bollard::errors::Error::IOError {
                    err: std::io::Error::other("sin respuesta de stats"),
                }),
            }
        })
        .await
    }

    async fn meta_of(&self, c: &ContainerSummary) -> ContainerMeta {
        let mut m = ContainerMeta {
            id: c.id.clone(),
            name: c.name.clone(),
            image: c.image.clone(),
            state: c.state.clone(),
            compose_project: c.compose_project.clone(),
            started_at: None,
            restart_count: None,
        };
        let d = self.docker().clone();
        let id = c.id.clone();
        if let Ok(i) = self
            .call_silent(Some(READ_TIMEOUT), async move {
                d.inspect_container(&id, None::<InspectContainerOptions>)
                    .await
            })
            .await
        {
            m.started_at = i.state.and_then(|s| s.started_at);
            m.restart_count = i.restart_count;
        }
        m
    }

    /// Una muestra de todo el servidor. Solo lectura y sin registrar cada
    /// petición. Un error de una lectura no hace fallar la muestra.
    pub async fn monitor_sample(&self, st: &mut SamplerState) -> Result<ServerSample, EngineError> {
        let started = Instant::now();
        let list = self.list_containers_silent().await?;
        let t = now_ms();
        // Datos de cada contenedor: se releen al aparecer, al cambiar de estado
        // o tras un evento.
        let mut metas = Vec::with_capacity(list.len());
        for c in &list {
            let known = st.meta.get(&c.id);
            let stale = known.is_none_or_changed(&c.state) || st.dirty.contains(&c.id);
            let m = if stale {
                self.meta_of(c).await
            } else {
                let mut m = known.cloned().unwrap_or_else(|| unreachable_meta(c));
                m.name.clone_from(&c.name);
                m
            };
            st.meta.insert(c.id.clone(), m.clone());
            metas.push(m);
        }
        st.dirty.clear();
        let live: HashSet<&str> = list.iter().map(|c| c.id.as_str()).collect();
        st.meta.retain(|id, _| live.contains(id.as_str()));
        st.prev.retain(|id, _| live.contains(id.as_str()));

        let running: Vec<String> = list
            .iter()
            .filter(|c| c.state == "running")
            .map(|c| c.id.clone())
            .collect();
        let reads: Vec<(String, Result<ContainerStatsResponse, EngineError>, i64)> =
            stream::iter(running)
                .map(|id| async move {
                    let r = self.stats_one_shot(&id).await;
                    (id, r, now_ms())
                })
                .buffer_unordered(CONCURRENCY)
                .collect()
                .await;
        let mut errors = 0;
        let mut points: HashMap<String, ContainerPoint> = HashMap::new();
        for (id, r, at) in reads {
            match r {
                Ok(s) => {
                    let raw = raw_from(&s, at);
                    let p = point(st.prev.get(&id), &raw, &s);
                    st.prev.insert(id.clone(), raw);
                    points.insert(id, p);
                }
                Err(e) => {
                    // Un contenedor que se acaba de parar no es un error.
                    if !matches!(
                        e,
                        EngineError::Docker {
                            status: 404 | 409,
                            ..
                        }
                    ) {
                        errors += 1;
                    }
                    st.prev.remove(&id);
                }
            }
        }
        let mut containers: Vec<ContainerSample> = metas
            .into_iter()
            .map(|meta| {
                let point = points.remove(&meta.id);
                ContainerSample { meta, point }
            })
            .collect();
        containers.sort_by(|a, b| a.meta.name.cmp(&b.meta.name));
        let cpus: Vec<f64> = containers
            .iter()
            .filter_map(|c| c.point.as_ref()?.cpu)
            .collect();
        Ok(ServerSample {
            t,
            connected: true,
            counts: counts(&list),
            cpu_total: (!cpus.is_empty()).then(|| cpus.iter().sum()),
            mem_total: containers
                .iter()
                .filter_map(|c| c.point.as_ref())
                .map(|p| p.mem)
                .sum(),
            containers,
            errors,
            took_ms: started.elapsed().as_millis() as u64,
        })
    }

    /// Datos del equipo según Docker (`info`), sin registrar.
    pub async fn monitor_host(&self) -> Result<HostInfo, EngineError> {
        let d = self.docker().clone();
        let i = self
            .call_silent(Some(Duration::from_secs(15)), async move { d.info().await })
            .await?;
        Ok(HostInfo {
            ncpu: i.ncpu,
            mem_total: i.mem_total,
            server_version: i.server_version,
            operating_system: i.operating_system,
            kernel_version: i.kernel_version,
        })
    }
}

trait StaleExt {
    fn is_none_or_changed(&self, state: &str) -> bool;
}

impl StaleExt for Option<&ContainerMeta> {
    fn is_none_or_changed(&self, state: &str) -> bool {
        self.map_or(true, |m| m.state != state)
    }
}

fn unreachable_meta(c: &ContainerSummary) -> ContainerMeta {
    ContainerMeta {
        id: c.id.clone(),
        name: c.name.clone(),
        image: c.image.clone(),
        state: c.state.clone(),
        compose_project: c.compose_project.clone(),
        started_at: None,
        restart_count: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(total: u64, system: u64, online: u32, rx: u64, mem: u64) -> ContainerStatsResponse {
        serde_json::from_value(serde_json::json!({
            "cpu_stats": { "cpu_usage": { "total_usage": total }, "system_cpu_usage": system, "online_cpus": online },
            "precpu_stats": { "cpu_usage": { "total_usage": 0 } },
            "memory_stats": { "usage": mem, "limit": 1_000_000, "stats": { "inactive_file": 1000 } },
            "networks": { "eth0": { "rx_bytes": rx, "tx_bytes": rx / 2 } },
            "blkio_stats": { "io_service_bytes_recursive": [ { "op": "read", "value": rx }, { "op": "write", "value": 10 } ] },
            "pids_stats": { "current": 7 }
        }))
        .unwrap()
    }

    /// Misma fórmula que `docker stats`: Δcpu / Δsistema × CPUs × 100.
    #[test]
    fn cpu_and_rates_between_two_samples_like_the_cli() {
        let s1 = stats(1_000_000_000, 100_000_000_000, 4, 1000, 101_000);
        let r1 = raw_from(&s1, 10_000);
        let p1 = point(None, &r1, &s1);
        assert_eq!(p1.cpu, None, "la primera muestra no tiene CPU");
        assert_eq!(p1.net_rx, None);
        assert_eq!(p1.mem, 100_000, "uso menos inactive_file");
        assert_eq!(p1.mem_percent, Some(10.0));
        assert_eq!(p1.pids, Some(7));
        // 2 s después: el contenedor usó 1 s de CPU de los 8 s de sistema (4 CPU × 2 s).
        let s2 = stats(2_000_000_000, 108_000_000_000, 4, 5000, 101_000);
        let r2 = raw_from(&s2, 12_000);
        let p2 = point(Some(&r1), &r2, &s2);
        assert!((p2.cpu.unwrap() - 50.0).abs() < 1e-9, "{:?}", p2.cpu);
        assert_eq!(p2.net_rx, Some(2000.0));
        assert_eq!(p2.net_tx, Some(1000.0));
        assert_eq!(p2.blk_read, Some(2000.0));
        assert_eq!(p2.blk_write, Some(0.0));
        // Contenedor reiniciado: los contadores bajan y no se inventa nada.
        let s3 = stats(10, 109_000_000_000, 4, 3, 101_000);
        let r3 = raw_from(&s3, 14_000);
        let p3 = point(Some(&r2), &r3, &s3);
        assert_eq!(p3.cpu, None);
        assert_eq!(p3.net_rx, None);
        // Mismo instante: sin velocidad.
        assert_eq!(point(Some(&r3), &r3, &s3).cpu, None);
    }

    #[test]
    fn counts_by_state() {
        let c = |state: &str| ContainerSummary {
            id: state.into(),
            name: state.into(),
            image: String::new(),
            image_id: String::new(),
            command: String::new(),
            created: 0,
            state: state.into(),
            status: String::new(),
            ports: vec![],
            networks: vec![],
            mounts: 0,
            mount_points: vec![],
            compose_project: None,
            labels: Default::default(),
        };
        let x = counts(&[
            c("running"),
            c("restarting"),
            c("paused"),
            c("exited"),
            c("created"),
        ]);
        assert_eq!(
            x,
            Counts {
                running: 2,
                paused: 1,
                stopped: 2,
                total: 5
            }
        );
    }
}
