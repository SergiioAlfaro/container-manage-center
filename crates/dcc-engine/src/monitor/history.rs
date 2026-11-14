//! Históricos (D-103): anillo en memoria por servidor y reducción a un
//! registro por minuto para el disco.

use std::collections::{BTreeMap, VecDeque};

use super::model::*;

/// Entrada compacta del anillo: sin los datos fijos de cada contenedor (nombre,
/// imagen…), que se guardan una sola vez. Así 50 contenedores × 30 min caben en
/// pocos MB por servidor.
#[derive(Debug, Clone)]
struct Entry {
    t: i64,
    connected: bool,
    counts: Counts,
    cpu_total: Option<f64>,
    mem_total: u64,
    points: Vec<(std::sync::Arc<str>, ContainerPoint)>,
}

/// Anillo de muestras con un máximo de elementos y de antigüedad.
#[derive(Debug, Clone)]
pub struct Ring {
    items: VecDeque<Entry>,
    ids: std::collections::HashSet<std::sync::Arc<str>>,
    last: Option<ServerSample>,
    max_items: usize,
    max_age_ms: i64,
}

/// Punto del servidor en la serie (para sus gráficos).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "MonitorServerPoint")]
pub struct ServerPoint {
    #[ts(type = "number")]
    pub t: i64,
    pub connected: bool,
    pub counts: Counts,
    pub cpu_total: Option<f64>,
    #[ts(type = "number")]
    pub mem_total: u64,
}

impl Ring {
    pub fn new(max_items: usize, max_age_ms: i64) -> Self {
        Self {
            items: VecDeque::new(),
            ids: Default::default(),
            last: None,
            max_items: max_items.max(1),
            max_age_ms,
        }
    }

    fn intern(&mut self, id: &str) -> std::sync::Arc<str> {
        if let Some(a) = self.ids.get(id) {
            return a.clone();
        }
        let a: std::sync::Arc<str> = id.into();
        self.ids.insert(a.clone());
        a
    }

    pub fn push(&mut self, s: ServerSample) {
        let t = s.t;
        let points = s
            .containers
            .iter()
            .filter_map(|c| Some((c.meta.id.as_str(), c.point.clone()?)))
            .map(|(id, p)| (self.intern(id), p))
            .collect();
        self.items.push_back(Entry {
            t,
            connected: s.connected,
            counts: s.counts,
            cpu_total: s.cpu_total,
            mem_total: s.mem_total,
            points,
        });
        // La última muestra completa con conexión (datos de los contenedores).
        if s.connected {
            self.last = Some(s);
        }
        while self.items.len() > self.max_items {
            self.items.pop_front();
        }
        while self
            .items
            .front()
            .is_some_and(|f| t - f.t > self.max_age_ms)
        {
            self.items.pop_front();
        }
        // Ids que ya no aparecen.
        if self.ids.len() > 4 * self.items.back().map_or(0, |e| e.points.len()).max(64) {
            let used: std::collections::HashSet<&str> = self
                .items
                .iter()
                .flat_map(|e| e.points.iter().map(|(id, _)| &**id))
                .collect();
            let keep: std::collections::HashSet<std::sync::Arc<str>> = self
                .ids
                .iter()
                .filter(|a| used.contains(&***a))
                .cloned()
                .collect();
            self.ids = keep;
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// La última muestra completa (con los datos de cada contenedor).
    pub fn last(&self) -> Option<&ServerSample> {
        self.last.as_ref()
    }

    /// ¿La muestra más reciente es un hueco?
    pub fn last_is_gap(&self) -> bool {
        self.items.back().is_some_and(|e| !e.connected)
    }

    /// Serie del servidor desde `since` (ms).
    pub fn server_series(&self, since: i64) -> Vec<ServerPoint> {
        self.items
            .iter()
            .filter(|e| e.t >= since)
            .map(|e| ServerPoint {
                t: e.t,
                connected: e.connected,
                counts: e.counts,
                cpu_total: e.cpu_total,
                mem_total: e.mem_total,
            })
            .collect()
    }

    /// Serie de un contenedor: un elemento por muestra del servidor; `None`
    /// donde no hay dato (hueco, contenedor parado o lectura fallida).
    pub fn series(&self, id: &str, since: i64) -> Vec<(i64, Option<ContainerPoint>)> {
        self.items
            .iter()
            .filter(|e| e.t >= since)
            .map(|e| {
                (
                    e.t,
                    e.points
                        .iter()
                        .find(|(x, _)| &**x == id)
                        .map(|(_, p)| p.clone()),
                )
            })
            .collect()
    }

    /// Memoria aproximada que ocupa (bytes), para medirla.
    pub fn approx_bytes(&self) -> usize {
        let per_point = std::mem::size_of::<(std::sync::Arc<str>, ContainerPoint)>();
        self.items
            .iter()
            .map(|e| std::mem::size_of::<Entry>() + e.points.capacity() * per_point)
            .sum::<usize>()
            + self.ids.iter().map(|a| a.len() + 16).sum::<usize>()
    }
}

#[derive(Debug, Default, Clone)]
struct Acc {
    name: String,
    samples: u32,
    cpu_sum: f64,
    cpu_n: u32,
    cpu_max: Option<f64>,
    mem_sum: u128,
    mem_max: u64,
    mem_limit: u64,
    net_rx: f64,
    net_tx: f64,
    blk_read: f64,
    blk_write: f64,
    pids_max: Option<u64>,
    last_t: Option<i64>,
}

/// Agrupa las muestras de un servidor por minuto.
#[derive(Debug, Default)]
pub struct MinuteAggregator {
    minute: Option<i64>,
    samples: u32,
    gaps: u32,
    cpu_sum: f64,
    cpu_n: u32,
    mem_sum: u128,
    acc: BTreeMap<String, Acc>,
}

pub fn minute_of(t: i64) -> i64 {
    t - t.rem_euclid(60_000)
}

impl MinuteAggregator {
    /// Añade una muestra. Si empieza un minuto nuevo, devuelve el anterior ya
    /// cerrado.
    pub fn push(&mut self, s: &ServerSample) -> Option<MinuteRecord> {
        let m = minute_of(s.t);
        let done = match self.minute {
            Some(cur) if cur != m => self.flush(),
            _ => None,
        };
        self.minute = Some(m);
        if !s.connected {
            self.gaps += 1;
            return done;
        }
        self.samples += 1;
        if let Some(c) = s.cpu_total {
            self.cpu_sum += c;
            self.cpu_n += 1;
        }
        self.mem_sum += u128::from(s.mem_total);
        for c in &s.containers {
            let Some(p) = &c.point else { continue };
            let a = self.acc.entry(c.meta.id.clone()).or_default();
            a.name.clone_from(&c.meta.name);
            a.samples += 1;
            if let Some(v) = p.cpu {
                a.cpu_sum += v;
                a.cpu_n += 1;
                a.cpu_max = Some(a.cpu_max.map_or(v, |m| m.max(v)));
            }
            a.mem_sum += u128::from(p.mem);
            a.mem_max = a.mem_max.max(p.mem);
            a.mem_limit = p.mem_limit;
            // Bytes = velocidad × tiempo desde la muestra anterior del contenedor.
            if let Some(prev) = a.last_t {
                let dt = (p.t - prev).max(0) as f64 / 1000.0;
                a.net_rx += p.net_rx.unwrap_or(0.0) * dt;
                a.net_tx += p.net_tx.unwrap_or(0.0) * dt;
                a.blk_read += p.blk_read.unwrap_or(0.0) * dt;
                a.blk_write += p.blk_write.unwrap_or(0.0) * dt;
            }
            a.last_t = Some(p.t);
            a.pids_max = match (a.pids_max, p.pids) {
                (Some(x), Some(y)) => Some(x.max(y)),
                (x, y) => x.or(y),
            };
        }
        done
    }

    /// Cierra el minuto en curso (al terminar o al cambiar de minuto).
    pub fn flush(&mut self) -> Option<MinuteRecord> {
        let t = self.minute.take()?;
        if self.samples == 0 && self.gaps == 0 {
            return None;
        }
        let containers = std::mem::take(&mut self.acc)
            .into_iter()
            .map(|(id, a)| MinuteContainer {
                id,
                name: a.name,
                samples: a.samples,
                cpu_avg: (a.cpu_n > 0).then(|| a.cpu_sum / f64::from(a.cpu_n)),
                cpu_max: a.cpu_max,
                mem_avg: (a.mem_sum / u128::from(a.samples.max(1))) as u64,
                mem_max: a.mem_max,
                mem_limit: a.mem_limit,
                net_rx: a.net_rx.round() as u64,
                net_tx: a.net_tx.round() as u64,
                blk_read: a.blk_read.round() as u64,
                blk_write: a.blk_write.round() as u64,
                pids_max: a.pids_max,
            })
            .collect();
        let r = MinuteRecord {
            t,
            samples: self.samples,
            gaps: self.gaps,
            cpu_total_avg: (self.cpu_n > 0).then(|| self.cpu_sum / f64::from(self.cpu_n)),
            mem_total_avg: (self.mem_sum / u128::from(self.samples.max(1))) as u64,
            containers,
        };
        *self = Self::default();
        Some(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(t: i64, cpu: Option<f64>, mem: u64, rx: Option<f64>) -> ServerSample {
        ServerSample {
            t,
            connected: true,
            counts: Counts::default(),
            cpu_total: cpu,
            mem_total: mem,
            containers: vec![ContainerSample {
                meta: ContainerMeta {
                    id: "c1".into(),
                    name: "web".into(),
                    image: "nginx".into(),
                    state: "running".into(),
                    compose_project: None,
                    started_at: None,
                    restart_count: None,
                },
                point: Some(ContainerPoint {
                    t,
                    cpu,
                    mem,
                    mem_limit: 1000,
                    mem_percent: None,
                    net_rx: rx,
                    net_tx: None,
                    blk_read: None,
                    blk_write: None,
                    pids: Some(3),
                }),
            }],
            errors: 0,
            took_ms: 1,
        }
    }

    #[test]
    fn ring_keeps_size_and_age_limits_and_gaps() {
        let mut r = Ring::new(3, 10_000);
        for i in 0..5 {
            r.push(sample(i * 1000, Some(1.0), 1, None));
        }
        assert_eq!(r.len(), 3, "máximo de elementos");
        assert_eq!(r.server_series(0)[0].t, 2000);
        assert_eq!(r.series("c1", 0).len(), 3);
        assert_eq!(r.series("c1", 0)[0].1.as_ref().unwrap().mem, 1);
        r.push(ServerSample::gap(20_000));
        assert_eq!(r.len(), 1, "antigüedad: solo queda el hueco");
        assert!(r.last_is_gap());
        // La última muestra completa sigue siendo la que tenía conexión.
        assert!(r.last().unwrap().connected);
        let s = r.series("c1", 0);
        assert_eq!(s, vec![(20_000, None)], "un hueco es None, no un cero");
        assert!(!r.server_series(0)[0].connected);
        assert!(r.approx_bytes() > 0);
    }

    #[test]
    fn minutes_are_reduced_with_averages_maxima_bytes_and_gaps() {
        let mut a = MinuteAggregator::default();
        let base = 1_800_000_000_000; // múltiplo de 60 s
        assert!(a.push(&sample(base, None, 100, None)).is_none());
        assert!(a
            .push(&sample(base + 10_000, Some(10.0), 200, Some(100.0)))
            .is_none());
        assert!(a
            .push(&sample(base + 20_000, Some(30.0), 300, Some(100.0)))
            .is_none());
        assert!(a.push(&ServerSample::gap(base + 30_000)).is_none());
        // Primera muestra del minuto siguiente: cierra el anterior.
        let m = a
            .push(&sample(base + 60_000, Some(50.0), 400, Some(1.0)))
            .unwrap();
        assert_eq!(m.t, base);
        assert_eq!((m.samples, m.gaps), (3, 1));
        assert_eq!(m.cpu_total_avg, Some(20.0));
        let c = &m.containers[0];
        assert_eq!(c.name, "web");
        assert_eq!(c.cpu_avg, Some(20.0));
        assert_eq!(c.cpu_max, Some(30.0));
        assert_eq!((c.mem_avg, c.mem_max, c.mem_limit), (200, 300, 1000));
        assert_eq!(c.net_rx, 2000, "100 B/s × 10 s, dos veces");
        assert_eq!(c.pids_max, Some(3));
        let m2 = a.flush().unwrap();
        assert_eq!(m2.t, base + 60_000);
        assert_eq!(m2.samples, 1);
        assert!(a.flush().is_none());
        assert_eq!(minute_of(base + 59_999), base);
    }
}
