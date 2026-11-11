//! Fase 8: historial persistente de operaciones (D-058).
//!
//! De cada operación terminada se guarda **qué** se hizo, **cuándo** y **con qué
//! resultado**, pero nada que pueda contener secretos: ni el cuerpo de la
//! petición (variables de entorno, comandos de exec…), ni el comando
//! equivalente (`-e CLAVE=valor`, `--build-arg`…), ni las salidas. En la ruta de
//! la API se ocultan los valores de `buildargs` (`/build?buildargs={"NPM_TOKEN":…}`).
//! Esos detalles solo existen en memoria durante la sesión.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::model::{Category, OpStatus, OperationRecord, Transport};

/// Parámetros de la query string cuyo valor nunca se guarda.
pub const SECRET_QUERY_PARAMS: [&str; 1] = ["buildargs"];
pub const HIDDEN: &str = "[oculto]";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryEntry {
    pub id: Uuid,
    pub category: Category,
    pub kind: String,
    pub target: Option<String>,
    /// Cómo se ejecutó (sin valores secretos en la ruta).
    pub transport: Transport,
    pub status: OpStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    #[ts(type = "number | null")]
    pub duration_ms: Option<u64>,
    pub summary: Option<String>,
    pub error: Option<String>,
    pub http_status: Option<u16>,
    pub exit_code: Option<i32>,
    /// Servidor de la operación (0.10.0). Las entradas anteriores no lo tienen.
    #[serde(default)]
    pub server: Option<crate::model::ServerTag>,
}

/// Oculta los valores de los parámetros secretos de una ruta con query string.
/// Para `buildargs` (un objeto JSON) se conservan los nombres de los argumentos.
pub fn redact_path(path: &str) -> String {
    let Some((base, query)) = path.split_once('?') else {
        return path.to_owned();
    };
    let parts: Vec<String> = query
        .split('&')
        .map(|kv| {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            if !SECRET_QUERY_PARAMS.contains(&k) {
                return kv.to_owned();
            }
            let decoded = percent_decode(v);
            let hidden = match serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(
                &decoded,
            ) {
                Ok(m) => {
                    let masked: serde_json::Map<String, serde_json::Value> = m
                        .keys()
                        .map(|k| (k.clone(), serde_json::Value::String(HIDDEN.into())))
                        .collect();
                    serde_json::Value::Object(masked).to_string()
                }
                Err(_) => HIDDEN.to_owned(),
            };
            format!("{k}={}", percent_encode(&hidden))
        })
        .collect();
    format!("{base}?{}", parts.join("&"))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = (b[i] == b'%' && i + 2 < b.len())
            .then(|| std::str::from_utf8(&b[i + 1..i + 3]).ok())
            .flatten()
            .and_then(|x| u8::from_str_radix(x, 16).ok());
        match (b[i], hex) {
            (_, Some(v)) => {
                out.push(v);
                i += 3;
            }
            (b'+', None) => {
                out.push(b' ');
                i += 1;
            }
            (c, None) => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

impl HistoryEntry {
    /// `None` si la operación sigue en curso (solo se guardan las terminadas).
    pub fn from_record(r: &OperationRecord) -> Option<Self> {
        if r.status == OpStatus::Running {
            return None;
        }
        let transport = match &r.transport {
            Transport::Api {
                method,
                path,
                endpoint,
            } => Transport::Api {
                method: method.clone(),
                path: redact_path(path),
                endpoint: endpoint.clone(),
            },
            other => other.clone(),
        };
        Some(Self {
            id: r.id,
            category: r.category,
            kind: r.kind.clone(),
            target: r.target.clone(),
            transport,
            status: r.status,
            started_at: r.started_at,
            finished_at: r.finished_at,
            duration_ms: r.duration_ms,
            summary: r.summary.clone(),
            error: r.error.clone(),
            http_status: r.http_status,
            exit_code: r.exit_code,
            server: r.server.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ActivityLog, Failure, OperationSpec, Outcome};

    #[test]
    fn build_args_values_never_reach_the_history() {
        // Ruta real de `build` (prueba de la Fase 4).
        let path = "/build?dockerfile=Dockerfile&t=miapp%3A1.0&buildargs=%7B%22NPM_TOKEN%22%3A%22abc%22%2C%22VERSION%22%3A%221.0%22%7D&labels=%7B%22equipo%22%3A%22web%22%7D&rm=1";
        let r = redact_path(path);
        assert!(!r.contains("abc"), "{r}");
        assert!(r.starts_with("/build?dockerfile=Dockerfile&t=miapp%3A1.0&buildargs="));
        assert!(
            r.ends_with("&labels=%7B%22equipo%22%3A%22web%22%7D&rm=1"),
            "el resto no cambia: {r}"
        );
        let shown = percent_decode(
            r.split("buildargs=")
                .nth(1)
                .unwrap()
                .split('&')
                .next()
                .unwrap(),
        );
        assert_eq!(shown, r#"{"NPM_TOKEN":"[oculto]","VERSION":"[oculto]"}"#);
        // Valor que no es JSON: se oculta entero. Rutas sin query: iguales.
        assert_eq!(
            redact_path("/build?buildargs=xyz"),
            "/build?buildargs=%5Boculto%5D"
        );
        assert_eq!(redact_path("/containers/json"), "/containers/json");
        assert_eq!(
            redact_path("/images/create?fromImage=nginx&tag=1.27"),
            "/images/create?fromImage=nginx&tag=1.27"
        );
    }

    #[test]
    fn history_keeps_what_when_and_result_but_no_bodies_commands_or_output() {
        let log = ActivityLog::new(10);
        let h = log.begin(OperationSpec {
            category: Category::Container,
            kind: "container.create".into(),
            target: Some("web".into()),
            transport: Transport::Api {
                method: "POST".into(),
                path: "/containers/create?name=web".into(),
                endpoint: "/var/run/docker.sock".into(),
            },
            request_body: Some(r#"{"Env":["DB_PASSWORD=s3creto"]}"#.into()),
            equivalent_command: Some("docker run -e DB_PASSWORD=s3creto nginx".into()),
        });
        let running = log.snapshot()[0].clone();
        assert!(
            HistoryEntry::from_record(&running).is_none(),
            "en curso: no se guarda"
        );
        h.succeed(Outcome {
            summary: Some("Contenedor creado".into()),
            stdout: Some("salida s3creto".into()),
            ..Default::default()
        });
        let e = HistoryEntry::from_record(&log.snapshot()[0]).unwrap();
        let json = serde_json::to_string(&e).unwrap();
        assert!(!json.contains("s3creto"), "{json}");
        assert_eq!(e.kind, "container.create");
        assert_eq!(e.target.as_deref(), Some("web"));
        assert_eq!(e.status, OpStatus::Succeeded);
        assert_eq!(e.summary.as_deref(), Some("Contenedor creado"));
        assert!(e.finished_at.is_some() && e.duration_ms.is_some());

        let h = log.begin(OperationSpec {
            category: Category::Compose,
            kind: "compose.up".into(),
            target: Some("tienda".into()),
            transport: Transport::Cli {
                program: "/usr/bin/docker".into(),
                args: vec!["compose".into(), "up".into()],
            },
            request_body: None,
            equivalent_command: None,
        });
        h.fail(Failure {
            error: "El proceso terminó con código 1".into(),
            exit_code: Some(1),
            stderr: Some("detalle".into()),
            ..Default::default()
        });
        let e = HistoryEntry::from_record(&log.snapshot()[0]).unwrap();
        assert_eq!((e.status, e.exit_code), (OpStatus::Failed, Some(1)));
        assert_eq!(
            e.transport,
            Transport::Cli {
                program: "/usr/bin/docker".into(),
                args: vec!["compose".into(), "up".into()]
            }
        );
        // Un registro guardado se vuelve a leer igual.
        let back: HistoryEntry = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(back, e);
    }
}
