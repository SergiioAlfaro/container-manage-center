//! Alertas (D-104): umbrales con histéresis, eventos de Docker y conexión,
//! con un periodo de silencio para no repetir el aviso.
//!
//! Lógica pura: recibe muestras, eventos y cambios de conexión con su
//! momento, y devuelve las alertas que saltan o se resuelven. No lee el reloj
//! ni hace E/S, así que se prueba de forma determinista.

use std::collections::HashMap;

use super::model::*;
use crate::events::DockerEvent;

/// Un `die` dentro de este tiempo tras un `kill` o `stop` es una parada pedida,
/// no una caída.
const KILL_WINDOW_MS: i64 = 30_000;
/// Un `start` tras una caída inesperada dentro de este tiempo es un reinicio
/// de la política de reinicio.
const RESTART_WINDOW_MS: i64 = 120_000;

#[derive(Debug, Default, Clone, Copy)]
struct Threshold {
    firing: bool,
    above: u32,
    below: u32,
}

#[derive(Debug, Default)]
pub struct AlertEngine {
    rules: AlertRules,
    thresholds: HashMap<(String, AlertKind), Threshold>,
    /// Última notificación por clave de alerta.
    notified: HashMap<String, i64>,
    /// Último `kill`/`stop` por contenedor.
    killed: HashMap<String, i64>,
    /// Última caída inesperada por contenedor.
    crashed: HashMap<String, i64>,
    disconnected: bool,
}

impl AlertEngine {
    pub fn new(rules: AlertRules) -> Self {
        Self {
            rules,
            ..Default::default()
        }
    }

    pub fn rules(&self) -> &AlertRules {
        &self.rules
    }

    /// Cambia las reglas. Las alertas activas de umbrales desactivados se olvidan.
    pub fn set_rules(&mut self, rules: AlertRules) {
        if !rules.cpu.enabled {
            self.thresholds.retain(|(_, k), _| *k != AlertKind::Cpu);
        }
        if !rules.memory.enabled {
            self.thresholds.retain(|(_, k), _| *k != AlertKind::Memory);
        }
        self.rules = rules;
    }

    /// Recuerda los avisos ya dados (del historial en disco), para que el
    /// periodo de silencio siga valiendo tras reiniciar la aplicación.
    pub fn remember_notified(&mut self, past: &[Alert]) {
        for a in past.iter().filter(|a| a.notified) {
            let e = self.notified.entry(a.key()).or_insert(a.at);
            *e = (*e).max(a.at);
        }
    }

    /// ¿Se avisa? No, si ya se avisó de la misma dentro del silencio.
    fn notify(&mut self, a: &mut Alert) {
        if a.state != AlertState::Fired {
            return;
        }
        let key = a.key();
        let silence = i64::from(self.rules.silence_minutes) * 60_000;
        let recent = self.notified.get(&key).is_some_and(|t| a.at - *t < silence);
        if !recent {
            a.notified = true;
            self.notified.insert(key, a.at);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn alert(
        &mut self,
        server: &str,
        kind: AlertKind,
        state: AlertState,
        id: Option<&str>,
        name: Option<&str>,
        value: Option<f64>,
        at: i64,
    ) -> Alert {
        let mut a = Alert {
            server: server.to_owned(),
            kind,
            state,
            container_id: id.map(str::to_owned),
            container: name.map(str::to_owned),
            value,
            at,
            notified: false,
        };
        self.notify(&mut a);
        a
    }

    #[allow(clippy::too_many_arguments)]
    fn threshold(
        &mut self,
        server: &str,
        kind: AlertKind,
        id: &str,
        name: &str,
        v: Option<f64>,
        at: i64,
        out: &mut Vec<Alert>,
    ) {
        let rule = match kind {
            AlertKind::Cpu => self.rules.cpu,
            _ => self.rules.memory,
        };
        let key = (id.to_owned(), kind);
        let Some(v) = v else {
            // Sin dato no se cambia nada: ni salta ni se resuelve.
            return;
        };
        let st = self.thresholds.entry(key.clone()).or_default();
        if !st.firing {
            st.below = 0;
            st.above = if v > rule.above { st.above + 1 } else { 0 };
            if st.above >= rule.samples {
                st.firing = true;
                st.above = 0;
                out.push(self.alert(
                    server,
                    kind,
                    AlertState::Fired,
                    Some(id),
                    Some(name),
                    Some(v),
                    at,
                ));
            }
        } else {
            // Histéresis: sigue activa hasta bajar de `clear_below`.
            st.above = 0;
            st.below = if v < rule.clear_below {
                st.below + 1
            } else {
                0
            };
            if st.below >= rule.samples {
                st.firing = false;
                st.below = 0;
                out.push(self.alert(
                    server,
                    kind,
                    AlertState::Resolved,
                    Some(id),
                    Some(name),
                    Some(v),
                    at,
                ));
            }
        }
    }

    /// Evalúa una muestra del servidor.
    pub fn on_sample(&mut self, server: &str, s: &ServerSample) -> Vec<Alert> {
        let mut out = Vec::new();
        if !s.connected {
            return out;
        }
        for c in &s.containers {
            let Some(p) = &c.point else { continue };
            if self.rules.cpu.enabled {
                self.threshold(
                    server,
                    AlertKind::Cpu,
                    &c.meta.id,
                    &c.meta.name,
                    p.cpu,
                    s.t,
                    &mut out,
                );
            }
            if self.rules.memory.enabled {
                self.threshold(
                    server,
                    AlertKind::Memory,
                    &c.meta.id,
                    &c.meta.name,
                    p.mem_percent,
                    s.t,
                    &mut out,
                );
            }
        }
        // Contenedores que ya no están: sus alertas de umbral se cierran sin aviso.
        let live: std::collections::HashSet<&str> = s
            .containers
            .iter()
            .filter(|c| c.point.is_some())
            .map(|c| c.meta.id.as_str())
            .collect();
        let gone: Vec<(String, AlertKind)> = self
            .thresholds
            .iter()
            .filter(|((id, _), st)| st.firing && !live.contains(id.as_str()))
            .map(|(k, _)| k.clone())
            .collect();
        for (id, kind) in gone {
            self.thresholds.remove(&(id.clone(), kind));
            let name = s
                .containers
                .iter()
                .find(|c| c.meta.id == id)
                .map(|c| c.meta.name.clone());
            out.push(self.alert(
                server,
                kind,
                AlertState::Resolved,
                Some(&id),
                name.as_deref(),
                None,
                s.t,
            ));
        }
        out
    }

    /// Evalúa un evento de Docker (`oom`, `die`, `kill`, `stop`, `start`).
    pub fn on_event(&mut self, server: &str, e: &DockerEvent) -> Vec<Alert> {
        let mut out = Vec::new();
        if e.kind != "container" {
            return out;
        }
        let at = e.time_nano / 1_000_000;
        let id = e.actor_id.as_str();
        let name = e
            .name
            .as_deref()
            .or_else(|| e.attributes.get("name").map(String::as_str));
        match e.action.as_str() {
            "kill" | "stop" => {
                self.killed.insert(id.to_owned(), at);
            }
            "oom" if self.rules.oom => {
                out.push(self.alert(
                    server,
                    AlertKind::Oom,
                    AlertState::Fired,
                    Some(id),
                    name,
                    None,
                    at,
                ));
            }
            "die" => {
                let asked = self
                    .killed
                    .get(id)
                    .is_some_and(|t| at - *t <= KILL_WINDOW_MS);
                let code: Option<i64> = e.attributes.get("exitCode").and_then(|c| c.parse().ok());
                if !asked {
                    self.crashed.insert(id.to_owned(), at);
                    if self.rules.exit_error && code.is_some_and(|c| c != 0) {
                        out.push(self.alert(
                            server,
                            AlertKind::ExitError,
                            AlertState::Fired,
                            Some(id),
                            name,
                            code.map(|c| c as f64),
                            at,
                        ));
                    }
                }
            }
            "start" => {
                let crash = self
                    .crashed
                    .remove(id)
                    .filter(|t| at - *t <= RESTART_WINDOW_MS);
                if crash.is_some() && self.rules.restart {
                    out.push(self.alert(
                        server,
                        AlertKind::Restart,
                        AlertState::Fired,
                        Some(id),
                        name,
                        None,
                        at,
                    ));
                }
                self.killed.remove(id);
            }
            "destroy" => {
                self.killed.remove(id);
                self.crashed.remove(id);
            }
            _ => {}
        }
        out
    }

    /// Cambio de la conexión con el servidor.
    pub fn on_connection(&mut self, server: &str, connected: bool, at: i64) -> Vec<Alert> {
        let mut out = Vec::new();
        if !connected && !self.disconnected {
            self.disconnected = true;
            if self.rules.disconnected {
                out.push(self.alert(
                    server,
                    AlertKind::Disconnected,
                    AlertState::Fired,
                    None,
                    None,
                    None,
                    at,
                ));
            }
        } else if connected && self.disconnected {
            self.disconnected = false;
            if self.rules.disconnected {
                out.push(self.alert(
                    server,
                    AlertKind::Disconnected,
                    AlertState::Resolved,
                    None,
                    None,
                    None,
                    at,
                ));
            }
        }
        out
    }

    /// Alertas de umbral activas ahora mismo: (contenedor, tipo).
    pub fn firing(&self) -> Vec<(String, AlertKind)> {
        let mut v: Vec<_> = self
            .thresholds
            .iter()
            .filter(|(_, s)| s.firing)
            .map(|(k, _)| k.clone())
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn sample(t: i64, cpu: f64) -> ServerSample {
        ServerSample {
            t,
            connected: true,
            counts: Counts::default(),
            cpu_total: Some(cpu),
            mem_total: 0,
            containers: vec![ContainerSample {
                meta: ContainerMeta {
                    id: "c1".into(),
                    name: "web".into(),
                    image: String::new(),
                    state: "running".into(),
                    compose_project: None,
                    started_at: None,
                    restart_count: None,
                },
                point: Some(ContainerPoint {
                    t,
                    cpu: Some(cpu),
                    mem: 10,
                    mem_limit: 100,
                    mem_percent: Some(10.0),
                    net_rx: None,
                    net_tx: None,
                    blk_read: None,
                    blk_write: None,
                    pids: None,
                }),
            }],
            errors: 0,
            took_ms: 0,
        }
    }

    fn feed(e: &mut AlertEngine, values: &[(i64, f64)]) -> Vec<Alert> {
        values
            .iter()
            .flat_map(|(t, v)| e.on_sample("local", &sample(*t, *v)))
            .collect()
    }

    /// CPU por encima del umbral durante 3 muestras: salta una vez; mientras
    /// siga alta (aunque baje un poco, histéresis) no se repite; se resuelve
    /// tras 3 muestras por debajo de 70.
    #[test]
    fn thresholds_fire_once_with_hysteresis() {
        let mut e = AlertEngine::new(AlertRules::default());
        let s = 5_000;
        let a = feed(
            &mut e,
            &[
                (0, 95.0),
                (s, 95.0),
                (2 * s, 50.0),
                (3 * s, 95.0),
                (4 * s, 95.0),
            ],
        );
        assert!(a.is_empty(), "una muestra baja reinicia la cuenta: {a:?}");
        let a = feed(&mut e, &[(5 * s, 99.0)]);
        assert_eq!(a.len(), 1);
        assert_eq!(
            (a[0].kind, a[0].state, a[0].notified),
            (AlertKind::Cpu, AlertState::Fired, true)
        );
        assert_eq!(a[0].container.as_deref(), Some("web"));
        assert_eq!(a[0].value, Some(99.0));
        // Oscila entre 75 y 95: sigue activa, sin más alertas.
        let a = feed(
            &mut e,
            &[
                (6 * s, 75.0),
                (7 * s, 95.0),
                (8 * s, 80.0),
                (9 * s, 71.0),
                (10 * s, 69.0),
                (11 * s, 72.0),
            ],
        );
        assert!(a.is_empty(), "{a:?}");
        assert_eq!(e.firing(), vec![("c1".to_owned(), AlertKind::Cpu)]);
        let a = feed(&mut e, &[(12 * s, 10.0), (13 * s, 10.0), (14 * s, 10.0)]);
        assert_eq!(a.len(), 1);
        assert_eq!((a[0].state, a[0].notified), (AlertState::Resolved, false));
        assert!(e.firing().is_empty());
    }

    /// Una alerta que vuelve a saltar dentro del silencio se apunta, pero no se
    /// avisa otra vez.
    #[test]
    fn silence_survives_a_restart_from_the_history() {
        let mut first = AlertEngine::new(AlertRules::default());
        first.on_connection("s", true, 0);
        let fired = first.on_connection("s", false, 1_000);
        assert!(fired[0].notified);
        // Otra instancia (la aplicación se reinicia) con el historial en disco.
        let mut again = AlertEngine::new(AlertRules::default());
        again.remember_notified(&fired);
        again.on_connection("s", true, 2_000);
        let a = again.on_connection("s", false, 60_000);
        assert!(
            !a[0].notified,
            "dentro del silencio, aunque se haya reiniciado"
        );
        let a = {
            again.on_connection("s", true, 61_000);
            again.on_connection("s", false, 1_000 + 16 * 60_000)
        };
        assert!(a[0].notified, "pasado el silencio, se avisa");
    }

    #[test]
    fn silence_period_prevents_repeated_notifications() {
        let mut rules = AlertRules::default();
        rules.cpu.samples = 1;
        rules.silence_minutes = 10;
        let mut e = AlertEngine::new(rules);
        let min = 60_000;
        let fired = |a: &Vec<Alert>| {
            a.iter()
                .filter(|x| x.state == AlertState::Fired)
                .cloned()
                .collect::<Vec<_>>()
        };
        let a = fired(&feed(&mut e, &[(0, 99.0), (min, 10.0), (2 * min, 99.0)]));
        assert_eq!(a.len(), 2);
        assert!(
            a[0].notified && !a[1].notified,
            "segunda vez dentro de 10 min: sin aviso"
        );
        let a = fired(&feed(&mut e, &[(3 * min, 10.0), (13 * min, 99.0)]));
        assert!(a[0].notified, "pasado el silencio, se avisa de nuevo");
    }

    fn ev(t_ms: i64, action: &str, attrs: &[(&str, &str)]) -> DockerEvent {
        DockerEvent {
            time_nano: t_ms * 1_000_000,
            kind: "container".into(),
            action: action.into(),
            actor_id: "c1".into(),
            name: Some("web".into()),
            attributes: attrs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect::<BTreeMap<_, _>>(),
            scope: None,
            server: None,
        }
    }

    #[test]
    fn events_distinguish_crashes_from_requested_stops() {
        let mut e = AlertEngine::new(AlertRules::default());
        // Parada pedida (docker stop): kill → die 143 → stop → nada.
        let mut a = e.on_event("local", &ev(1000, "kill", &[("signal", "15")]));
        a.extend(e.on_event("local", &ev(1500, "die", &[("exitCode", "143")])));
        a.extend(e.on_event("local", &ev(1600, "stop", &[])));
        a.extend(e.on_event("local", &ev(5000, "start", &[])));
        assert!(a.is_empty(), "{a:?}");
        // Caída: die 1 sin kill → alerta de salida; la política lo reinicia → reinicio.
        let a = e.on_event("local", &ev(100_000, "die", &[("exitCode", "1")]));
        assert_eq!(a.len(), 1);
        assert_eq!((a[0].kind, a[0].value), (AlertKind::ExitError, Some(1.0)));
        let a = e.on_event("local", &ev(101_000, "start", &[]));
        assert_eq!(a[0].kind, AlertKind::Restart);
        // OOM.
        let a = e.on_event("local", &ev(200_000, "oom", &[]));
        assert_eq!(a[0].kind, AlertKind::Oom);
        // Salida con código 0 sin kill: no es un error.
        assert!(e
            .on_event("local", &ev(300_000, "die", &[("exitCode", "0")]))
            .is_empty());
        // Otros tipos de evento no cuentan.
        let mut img = ev(0, "oom", &[]);
        img.kind = "image".into();
        assert!(e.on_event("local", &img).is_empty());
        // Desactivadas, no saltan.
        e.set_rules(AlertRules {
            oom: false,
            ..Default::default()
        });
        assert!(e.on_event("local", &ev(400_000, "oom", &[])).is_empty());
    }

    #[test]
    fn disconnection_fires_once_and_resolves_on_reconnect() {
        let mut e = AlertEngine::new(AlertRules::default());
        assert!(e.on_connection("srv", true, 0).is_empty());
        let a = e.on_connection("srv", false, 1000);
        assert_eq!(
            (a[0].kind, a[0].state),
            (AlertKind::Disconnected, AlertState::Fired)
        );
        assert!(
            e.on_connection("srv", false, 2000).is_empty(),
            "sin repetición"
        );
        let a = e.on_connection("srv", true, 3000);
        assert_eq!(a[0].state, AlertState::Resolved);
        // Un hueco no evalúa umbrales.
        assert!(e.on_sample("srv", &ServerSample::gap(4000)).is_empty());
    }

    #[test]
    fn rules_are_validated() {
        assert!(AlertRules::default().validate().is_ok());
        let mut r = AlertRules::default();
        r.cpu.clear_below = 95.0;
        assert!(r.validate().is_err(), "recuperación ≥ umbral");
        let mut r = AlertRules::default();
        r.memory.above = 150.0;
        r.memory.clear_below = 10.0;
        assert!(r.validate().is_err());
        let mut r = AlertRules::default();
        r.cpu.samples = 0;
        assert!(r.validate().is_err());
        let mut r = AlertRules::default();
        r.cpu.above = f64::NAN;
        assert!(r.validate().is_err());
    }
}
