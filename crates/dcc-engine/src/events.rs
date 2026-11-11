//! Fase 8: eventos de Docker Engine (D-057).
//!
//! - `events_watch`: **una** suscripción a `GET /events` por conexión. Docker
//!   envía un objeto JSON por evento mientras la conexión siga abierta. Se
//!   registra en el panel de actividad cuando Docker la acepta (y otra vez si se
//!   interrumpe), no como una operación "en curso" permanente.
//! - `events_query`: `GET /events?since=…&until=…&filters=…`. Con `until`, Docker
//!   devuelve los eventos que conserva en memoria (los **últimos 256**, que se
//!   pierden al reiniciar el daemon; comprobado con Docker 29.9.2) y cierra.
//!
//! Se usa la petición HTTP propia (`build::http`) para saber cuándo Docker ha
//! aceptado la suscripción (bollard solo entrega algo con el primer evento).

use std::collections::BTreeMap;
use std::time::Duration;

use dcc_activity::{Category, Failure, OperationSpec, Outcome, Transport};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::build::http;
use crate::client::{ApiOp, Engine};
use crate::error::EngineError;

/// Tipos de objeto que Docker emite.
pub const EVENT_TYPES: [&str; 11] = [
    "container",
    "image",
    "volume",
    "network",
    "daemon",
    "plugin",
    "service",
    "node",
    "secret",
    "config",
    "builder",
];

/// Máximo de eventos devueltos por una consulta.
pub const MAX_QUERY_EVENTS: usize = 5000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DockerEvent {
    /// Momento del evento en nanosegundos desde 1970 (`timeNano`).
    #[ts(type = "number")]
    pub time_nano: i64,
    /// container, image, volume, network, daemon…
    pub kind: String,
    /// create, start, die, destroy, pull, connect…
    pub action: String,
    /// ID del objeto (o nombre, p. ej. en volúmenes e imágenes).
    pub actor_id: String,
    /// Nombre legible si Docker lo envía (`Attributes.name`).
    pub name: Option<String>,
    pub attributes: BTreeMap<String, String>,
    pub scope: Option<String>,
}

impl DockerEvent {
    /// Interpreta un objeto de `GET /events`. `None` si no tiene la forma esperada.
    pub fn from_json(v: &Value) -> Option<Self> {
        let kind = v.get("Type")?.as_str()?.to_owned();
        let action = v.get("Action")?.as_str()?.to_owned();
        let actor = v.get("Actor");
        let actor_id = actor
            .and_then(|a| a.get("ID"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let attributes: BTreeMap<String, String> = actor
            .and_then(|a| a.get("Attributes"))
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                    .collect()
            })
            .unwrap_or_default();
        let time_nano = v.get("timeNano").and_then(Value::as_i64).or_else(|| {
            v.get("time")
                .and_then(Value::as_i64)
                .map(|t| t * 1_000_000_000)
        })?;
        Some(Self {
            time_nano,
            name: attributes.get("name").cloned(),
            kind,
            action,
            actor_id,
            attributes,
            scope: v.get("scope").and_then(Value::as_str).map(str::to_owned),
        })
    }

    /// Texto en el que se busca (tipo, acción, ID, nombre e imagen).
    pub fn matches_text(&self, q: &str) -> bool {
        let q = q.trim().to_lowercase();
        if q.is_empty() {
            return true;
        }
        [
            Some(self.kind.as_str()),
            Some(self.action.as_str()),
            Some(self.actor_id.as_str()),
            self.name.as_deref(),
            self.attributes.get("image").map(String::as_str),
        ]
        .into_iter()
        .flatten()
        .any(|s| s.to_lowercase().contains(&q))
    }
}

/// Consulta a Docker de los eventos que conserva.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct EventQuery {
    /// Desde (segundos desde 1970).
    #[ts(type = "number")]
    pub since: i64,
    /// Hasta (segundos desde 1970); obligatorio para que la respuesta termine.
    #[ts(type = "number")]
    pub until: i64,
    /// Tipos (vacío: todos).
    pub types: Vec<String>,
    /// Acciones exactas (vacío: todas), p. ej. `start`, `die`.
    pub actions: Vec<String>,
    /// Contenedor, imagen, red o volumen (nombre o ID) según el tipo elegido.
    pub resource: Option<String>,
}

fn invalid(m: impl Into<String>) -> EngineError {
    EngineError::InvalidInput { message: m.into() }
}

fn valid_word(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 255
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':' | '/' | '@'))
}

impl EventQuery {
    pub fn validate(&self, now: i64) -> Result<(), EngineError> {
        if self.since < 0 || self.until <= 0 {
            return Err(invalid("indica el intervalo de fechas"));
        }
        if self.since > self.until {
            return Err(invalid("la fecha inicial es posterior a la final"));
        }
        if self.until > now + 60 {
            return Err(invalid("la fecha final no puede estar en el futuro"));
        }
        if let Some(t) = self
            .types
            .iter()
            .find(|t| !EVENT_TYPES.contains(&t.as_str()))
        {
            return Err(invalid(format!("tipo de evento desconocido: {t:?}")));
        }
        if let Some(a) = self.actions.iter().find(|a| !valid_word(a)) {
            return Err(invalid(format!("acción no válida: {a:?}")));
        }
        if let Some(r) = self.resource.as_deref().filter(|r| !r.is_empty()) {
            if !valid_word(r) {
                return Err(invalid(format!("recurso no válido: {r:?}")));
            }
            let objects = self
                .types
                .iter()
                .filter(|t| matches!(t.as_str(), "container" | "image" | "network" | "volume"))
                .count();
            if objects != 1 || self.types.len() != 1 {
                return Err(invalid(
                    "para filtrar por recurso elige un solo tipo: contenedor, imagen, red o volumen",
                ));
            }
        }
        if self.types.len() > EVENT_TYPES.len() || self.actions.len() > 50 {
            return Err(invalid("demasiados filtros"));
        }
        Ok(())
    }

    /// Filtros de Docker (`type`, `event` y `container|image|network|volume`).
    pub fn filters(&self) -> BTreeMap<String, Vec<String>> {
        let mut f = BTreeMap::new();
        if !self.types.is_empty() {
            f.insert("type".to_owned(), self.types.clone());
        }
        if !self.actions.is_empty() {
            f.insert("event".to_owned(), self.actions.clone());
        }
        if let (Some(r), Some(t)) = (
            self.resource.as_deref().filter(|r| !r.is_empty()),
            self.types.first(),
        ) {
            f.insert(t.clone(), vec![r.to_owned()]);
        }
        f
    }

    /// Valor de `until` enviado. Si la fecha final es el segundo actual (o
    /// posterior), se envía el instante exacto con nanosegundos: con segundos
    /// enteros Docker excluye los eventos ocurridos en la fracción de segundo
    /// en curso (comprobado con Docker 29.9.2).
    pub fn until_param(&self, now: chrono::DateTime<chrono::Utc>) -> String {
        if self.until >= now.timestamp() {
            format!("{}.{:09}", now.timestamp(), now.timestamp_subsec_nanos())
        } else {
            self.until.to_string()
        }
    }

    /// Ruta exacta de la petición.
    pub fn path(&self, now: chrono::DateTime<chrono::Utc>) -> String {
        let mut q: Vec<(&str, String)> = vec![
            ("since", self.since.to_string()),
            ("until", self.until_param(now)),
        ];
        let f = self.filters();
        if !f.is_empty() {
            q.push(("filters", serde_json::to_string(&f).unwrap_or_default()));
        }
        format!(
            "/events?{}",
            serde_urlencoded::to_string(&q).unwrap_or_default()
        )
    }

    /// `docker events --since … --until … --filter …` equivalente.
    pub fn cli_args(&self, now: chrono::DateTime<chrono::Utc>) -> Vec<String> {
        let mut a = vec![
            "events".to_owned(),
            "--since".to_owned(),
            self.since.to_string(),
            "--until".to_owned(),
            self.until_param(now),
        ];
        for (k, vs) in self.filters() {
            for v in vs {
                a.push("--filter".to_owned());
                a.push(format!("{k}={v}"));
            }
        }
        a
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EventQueryResult {
    pub events: Vec<DockerEvent>,
    /// Se alcanzó el máximo y se descartaron los más antiguos.
    pub truncated: bool,
    /// Comando `docker events` equivalente.
    pub command: String,
}

impl Engine {
    /// Eventos que Docker conserva en el intervalo, con sus filtros.
    pub async fn events_query(&self, q: &EventQuery) -> Result<EventQueryResult, EngineError> {
        let now = chrono::Utc::now();
        q.validate(now.timestamp())?;
        let path = q.path(now);
        let args = q.cli_args(now);
        let op = ApiOp {
            category: Category::System,
            kind: "events.query",
            target: None,
            method: "GET",
            path: path.clone(),
            body: None,
            equivalent: Some(args.clone()),
            timeout: Some(Duration::from_secs(30)),
        };
        let dialer = self.dialer();
        let (events, truncated) = self
            .call(
                op,
                async move {
                    let mut r = http::get_stream(&dialer, &path).await?;
                    if !(200..300).contains(&r.status) {
                        let message = r.error_message().await;
                        return Err(EngineError::Docker {
                            status: r.status,
                            message,
                        });
                    }
                    let mut out = std::collections::VecDeque::new();
                    let mut truncated = false;
                    while let Some(v) = r.next_json().await? {
                        if let Some(e) = DockerEvent::from_json(&v) {
                            if out.len() == MAX_QUERY_EVENTS {
                                out.pop_front();
                                truncated = true;
                            }
                            out.push_back(e);
                        }
                    }
                    Ok::<_, EngineError>((Vec::from(out), truncated))
                },
                |(ev, _): &(Vec<DockerEvent>, bool)| Some(format!("{} eventos", ev.len())),
            )
            .await?;
        Ok(EventQueryResult {
            events,
            truncated,
            command: self.docker_cli(&args),
        })
    }

    /// Mantiene abierta la suscripción a `GET /events` y entrega cada evento.
    ///
    /// Se registra al abrirse ("Suscripción a los eventos de Docker abierta") y,
    /// si se interrumpe por un error, como operación fallida. Termina con `Ok`
    /// al cancelarse o si `on_event` devuelve `false`.
    pub async fn events_watch<F>(
        &self,
        cancel: CancellationToken,
        mut on_event: F,
    ) -> Result<(), EngineError>
    where
        F: FnMut(DockerEvent) -> bool,
    {
        let path = "/events".to_owned();
        let spec = |kind: &str| OperationSpec {
            category: Category::System,
            kind: kind.to_owned(),
            target: None,
            transport: Transport::Api {
                method: "GET".into(),
                path: path.clone(),
                endpoint: self.endpoint().socket_path.clone(),
            },
            request_body: None,
            equivalent_command: Some(self.docker_cli(&["events".to_owned()])),
        };
        let handle = self.activity().begin(spec("events.watch"));
        let dialer = self.dialer();
        let opened = tokio::select! {
            r = http::get_stream(&dialer, &path) => r,
            _ = cancel.cancelled() => {
                handle.cancel(Failure { error: "Cancelada antes de abrirse".into(), ..Default::default() });
                return Ok(());
            }
        };
        let mut r = match opened {
            Ok(r) if (200..300).contains(&r.status) => r,
            Ok(mut r) => {
                let status = r.status;
                let message = r.error_message().await;
                handle.fail(Failure {
                    error: message.clone(),
                    http_status: Some(status),
                    ..Default::default()
                });
                return Err(EngineError::Docker { status, message });
            }
            Err(e) => {
                handle.fail(Failure {
                    error: e.to_string(),
                    ..Default::default()
                });
                return Err(e);
            }
        };
        handle.succeed(Outcome {
            summary: Some(
                "Suscripción a los eventos de Docker abierta: la aplicación se actualiza en tiempo real mientras dure".into(),
            ),
            http_status: Some(r.status),
            ..Default::default()
        });
        loop {
            let next = tokio::select! {
                n = r.next_json() => n,
                _ = cancel.cancelled() => return Ok(()),
            };
            let err = match next {
                Ok(Some(v)) => {
                    if let Some(e) = DockerEvent::from_json(&v) {
                        if !on_event(e) {
                            return Ok(());
                        }
                    }
                    continue;
                }
                Ok(None) => EngineError::Io {
                    message: "Docker cerró la conexión de eventos".into(),
                },
                Err(e) => e,
            };
            let h = self.activity().begin(spec("events.watch"));
            h.fail(Failure {
                error: format!("Suscripción a los eventos interrumpida: {err}"),
                ..Default::default()
            });
            return Err(err);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_are_parsed_from_docker_json() {
        // Objeto real de Docker 29.9.2 (`docker events --format '{{json .}}'`).
        let v: Value = serde_json::from_str(r#"{"status":"start","id":"4f3c","from":"nginx:1.27","Type":"container","Action":"start","Actor":{"ID":"4f3c","Attributes":{"image":"nginx:1.27","name":"web","com.docker.compose.project":"tienda"}},"scope":"local","time":1794067503,"timeNano":1794067503964175002}"#).unwrap();
        let e = DockerEvent::from_json(&v).unwrap();
        assert_eq!(e.kind, "container");
        assert_eq!(e.action, "start");
        assert_eq!(e.actor_id, "4f3c");
        assert_eq!(e.name.as_deref(), Some("web"));
        assert_eq!(e.time_nano, 1794067503964175002);
        assert_eq!(e.attributes["com.docker.compose.project"], "tienda");
        assert!(e.matches_text("NGINX"));
        assert!(e.matches_text("web"));
        assert!(!e.matches_text("redis"));
        // Sin timeNano se usa time; sin Type no es un evento.
        let old: Value = serde_json::from_str(
            r#"{"Type":"volume","Action":"create","Actor":{"ID":"datos"},"time":10}"#,
        )
        .unwrap();
        assert_eq!(
            DockerEvent::from_json(&old).unwrap().time_nano,
            10_000_000_000
        );
        assert!(DockerEvent::from_json(&serde_json::json!({"Action":"x"})).is_none());
    }

    #[test]
    fn query_validation_and_exact_path() {
        let now = 2_000_000_000;
        let q = EventQuery {
            since: 1_000,
            until: 2_000,
            types: vec!["container".into()],
            actions: vec!["start".into(), "die".into()],
            resource: Some("web".into()),
        };
        q.validate(now).unwrap();
        let t = chrono::DateTime::from_timestamp(now, 123).unwrap();
        assert_eq!(
            q.path(t),
            "/events?since=1000&until=2000&filters=%7B%22container%22%3A%5B%22web%22%5D%2C%22event%22%3A%5B%22start%22%2C%22die%22%5D%2C%22type%22%3A%5B%22container%22%5D%7D"
        );
        assert_eq!(
            q.cli_args(t),
            [
                "events",
                "--since",
                "1000",
                "--until",
                "2000",
                "--filter",
                "container=web",
                "--filter",
                "event=start",
                "--filter",
                "event=die",
                "--filter",
                "type=container"
            ]
        );
        let bad = |f: &dyn Fn(&mut EventQuery)| {
            let mut x = q.clone();
            f(&mut x);
            x.validate(now).is_err()
        };
        assert!(bad(&|x| x.since = 3_000), "desde > hasta");
        assert!(bad(&|x| x.until = now + 3600), "futuro");
        assert!(bad(&|x| x.until = 0), "sin hasta");
        assert!(bad(&|x| x.types = vec!["bogus".into()]));
        assert!(bad(&|x| x.actions = vec!["a b".into()]));
        assert!(bad(&|x| x.resource = Some("x\"y".into())));
        assert!(
            bad(&|x| x.types = vec!["container".into(), "image".into()]),
            "recurso con dos tipos"
        );
        assert!(
            bad(&|x| x.types = vec!["daemon".into()]),
            "recurso con un tipo sin objetos"
        );
        let all = EventQuery {
            since: 1,
            until: 2,
            ..Default::default()
        };
        all.validate(now).unwrap();
        assert_eq!(all.path(t), "/events?since=1&until=2");
        // Hasta "ahora": el instante exacto, para no perder la fracción de segundo en curso.
        let to_now = EventQuery {
            since: 1,
            until: now,
            ..Default::default()
        };
        assert_eq!(to_now.until_param(t), format!("{now}.000000123"));
        assert_eq!(
            to_now.path(t),
            format!("/events?since=1&until={now}.000000123")
        );
    }
}
