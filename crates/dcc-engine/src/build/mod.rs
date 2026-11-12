//! Construcción de imágenes a partir de un Dockerfile (`docker build`).
//!
//! Una construcción es **una** petición `POST /build` con el contexto en tar y
//! un registro en el panel de actividad que permanece "en curso" hasta que
//! termina. Por defecto se usa BuildKit (`version=2`, como `docker build` en
//! las versiones actuales); el constructor clásico (`version=1`) queda como
//! alternativa. Ver D-041.

pub mod context;
pub mod http;
pub mod ignore;
pub mod progress;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;

use dcc_activity::{Category, Failure, OperationSpec, Outcome, Transport};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::client::Engine;
use crate::error::EngineError;
use crate::reference::{auth_key_for, ImageRef};
use crate::registry_auth::{self, PullAuth};
use crate::spec::{is_secret_key, issue, KeyValue, Severity, SpecIssue};
use crate::streams::{EndReason, StreamEnd};

pub use context::ContextSummary;
pub use progress::{BuildStep, BuildUpdate, StepState};

const FLUSH: Duration = Duration::from_millis(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Builder {
    #[default]
    BuildKit,
    Classic,
}

/// Lo que el usuario rellena en el asistente de construcción.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct BuildSpec {
    /// Carpeta de contexto (ruta absoluta).
    pub context: String,
    /// Dockerfile relativo al contexto.
    pub dockerfile: String,
    /// Etiquetas (`repo:tag`); vacío = imagen sin etiqueta.
    pub tags: Vec<String>,
    pub build_args: Vec<KeyValue>,
    pub labels: Vec<KeyValue>,
    /// Etapa final en un Dockerfile multietapa.
    pub target: String,
    pub no_cache: bool,
    /// Intentar descargar versiones más recientes de las imágenes base.
    pub pull: bool,
    pub builder: Builder,
}

impl Default for BuildSpec {
    fn default() -> Self {
        Self {
            context: String::new(),
            dockerfile: "Dockerfile".into(),
            tags: Vec::new(),
            build_args: Vec::new(),
            labels: Vec::new(),
            target: String::new(),
            no_cache: false,
            pull: false,
            builder: Builder::BuildKit,
        }
    }
}

fn err(field: impl Into<String>, code: &str, params: &[(&str, String)]) -> SpecIssue {
    issue(Severity::Error, field, code, params)
}
fn warn(field: impl Into<String>, code: &str, params: &[(&str, String)]) -> SpecIssue {
    issue(Severity::Warning, field, code, params)
}

fn valid_arg_key(k: &str) -> bool {
    !k.is_empty() && !k.contains('=') && !k.chars().any(|c| c.is_whitespace() || c.is_control())
}

fn valid_dockerfile(p: &str) -> bool {
    !p.is_empty() && !p.starts_with('/') && !p.split('/').any(|c| c == "..") && !p.contains('\0')
}

impl BuildSpec {
    /// Nombres no vacíos (las filas en blanco del formulario se ignoran).
    pub fn tag_list(&self) -> Vec<String> {
        self.tags
            .iter()
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
            .collect()
    }

    pub fn dockerfile_path(&self) -> PathBuf {
        Path::new(&self.context).join(&self.dockerfile)
    }

    /// Valida la especificación; con `check_fs` comprueba también la carpeta y el Dockerfile.
    pub fn validate(&self, check_fs: bool) -> Vec<SpecIssue> {
        let mut v = Vec::new();
        let ctx = self.context.trim();
        if ctx.is_empty() {
            v.push(err("context", "context.required", &[]));
        } else if !ctx.starts_with('/') || ctx.split('/').any(|c| c == "..") {
            v.push(err("context", "path.notAbsolute", &[]));
        } else if check_fs {
            match std::fs::metadata(ctx) {
                Ok(m) if m.is_dir() => {}
                Ok(_) => v.push(err("context", "context.notDir", &[("path", ctx.into())])),
                Err(_) => v.push(err("context", "context.missing", &[("path", ctx.into())])),
            }
            if ["/", "/home", "/usr", "/etc", "/var"].contains(&ctx.trim_end_matches('/').max("/"))
            {
                v.push(warn("context", "context.huge", &[("path", ctx.into())]));
            }
        }
        if !valid_dockerfile(&self.dockerfile) {
            v.push(err("dockerfile", "dockerfile.invalid", &[]));
        } else if check_fs && v.iter().all(|i| i.field != "context") {
            match std::fs::metadata(self.dockerfile_path()) {
                Ok(m) if m.is_file() => {}
                _ => v.push(err(
                    "dockerfile",
                    "dockerfile.missing",
                    &[("path", self.dockerfile.clone())],
                )),
            }
        }
        if self.tag_list().is_empty() {
            v.push(warn("tags", "tags.none", &[]));
        }
        let mut seen = std::collections::HashSet::new();
        for (i, t) in self.tags.iter().enumerate() {
            let t = t.trim();
            if t.is_empty() {
                continue;
            }
            match ImageRef::parse(t) {
                Ok(r) if r.digest.is_none() => {
                    if !seen.insert(r.familiar()) {
                        v.push(err(
                            format!("tags.{i}"),
                            "tag.duplicate",
                            &[("tag", t.to_owned())],
                        ));
                    }
                }
                _ => v.push(err(
                    format!("tags.{i}"),
                    "tag.invalid",
                    &[("tag", t.to_owned())],
                )),
            }
        }
        for (field, list) in [("buildArgs", &self.build_args), ("labels", &self.labels)] {
            let mut keys = std::collections::HashSet::new();
            for (i, kv) in list.iter().enumerate() {
                if !valid_arg_key(&kv.key) {
                    v.push(err(
                        format!("{field}.{i}.key"),
                        "arg.keyInvalid",
                        &[("key", kv.key.clone())],
                    ));
                } else if !keys.insert(kv.key.clone()) {
                    v.push(err(
                        format!("{field}.{i}.key"),
                        "arg.duplicate",
                        &[("key", kv.key.clone())],
                    ));
                }
            }
        }
        if !self.target.is_empty()
            && !self
                .target
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
        {
            v.push(err("target", "target.invalid", &[]));
        }
        for (i, kv) in self.build_args.iter().enumerate() {
            if is_secret_key(&kv.key) && !kv.value.is_empty() {
                v.push(warn(
                    format!("buildArgs.{i}.value"),
                    "arg.secret",
                    &[("key", kv.key.clone())],
                ));
            }
        }
        v
    }

    /// Copia con los valores sensibles de los argumentos ocultos (D-039).
    pub fn redacted(&self) -> (BuildSpec, usize) {
        let mut s = self.clone();
        let mut n = 0;
        for kv in &mut s.build_args {
            let shown = crate::spec::shown_value(&kv.key, &kv.value);
            if shown != kv.value {
                kv.value = shown;
                n += 1;
            }
        }
        (s, n)
    }

    /// Parámetros de `POST /build`, en el orden en que se envían.
    pub fn query(&self) -> Vec<(String, String)> {
        let mut q: Vec<(String, String)> = vec![("dockerfile".into(), self.dockerfile.clone())];
        for t in self.tag_list() {
            q.push(("t".into(), t));
        }
        if !self.build_args.is_empty() {
            let m: BTreeMap<&str, &str> = self
                .build_args
                .iter()
                .map(|kv| (kv.key.as_str(), kv.value.as_str()))
                .collect();
            q.push((
                "buildargs".into(),
                serde_json::to_string(&m).unwrap_or_default(),
            ));
        }
        if !self.labels.is_empty() {
            let m: BTreeMap<&str, &str> = self
                .labels
                .iter()
                .map(|kv| (kv.key.as_str(), kv.value.as_str()))
                .collect();
            q.push((
                "labels".into(),
                serde_json::to_string(&m).unwrap_or_default(),
            ));
        }
        if !self.target.is_empty() {
            q.push(("target".into(), self.target.clone()));
        }
        if self.no_cache {
            q.push(("nocache".into(), "1".into()));
        }
        if self.pull {
            q.push(("pull".into(), "1".into()));
        }
        q.push(("rm".into(), "1".into()));
        if self.builder == Builder::Classic {
            // Sin esto, un RUN que falla deja su contenedor intermedio.
            q.push(("forcerm".into(), "1".into()));
        }
        q.push((
            "version".into(),
            match self.builder {
                Builder::BuildKit => "2",
                Builder::Classic => "1",
            }
            .into(),
        ));
        q
    }

    pub fn path(&self) -> String {
        format!(
            "/build?{}",
            serde_urlencoded::to_string(self.query()).unwrap_or_default()
        )
    }

    /// Argumentos de `docker build` equivalentes (sin el programa).
    pub fn cli_args(&self) -> Vec<String> {
        let mut a = vec!["build".to_owned()];
        if self.dockerfile != "Dockerfile" {
            a.push("--file".into());
            a.push(self.dockerfile_path().to_string_lossy().into_owned());
        }
        for t in self.tag_list() {
            a.push("--tag".into());
            a.push(t);
        }
        for kv in &self.build_args {
            a.push("--build-arg".into());
            a.push(format!("{}={}", kv.key, kv.value));
        }
        for kv in &self.labels {
            a.push("--label".into());
            a.push(format!("{}={}", kv.key, kv.value));
        }
        if !self.target.is_empty() {
            a.push("--target".into());
            a.push(self.target.clone());
        }
        if self.no_cache {
            a.push("--no-cache".into());
        }
        if self.pull {
            a.push("--pull".into());
        }
        if self.builder == Builder::Classic {
            a.push("--force-rm".into());
        }
        a.push(self.context.clone());
        a
    }

    /// Imágenes base citadas en `FROM` (sin etapas internas, `scratch` ni variables).
    pub fn base_images(&self) -> Vec<ImageRef> {
        let Ok(text) = std::fs::read_to_string(self.dockerfile_path()) else {
            return Vec::new();
        };
        base_images_in(&text)
    }
}

pub fn base_images_in(dockerfile: &str) -> Vec<ImageRef> {
    let mut stages = std::collections::HashSet::new();
    let mut out: Vec<ImageRef> = Vec::new();
    for line in dockerfile.lines() {
        let l = line.trim();
        let mut words = l.split_whitespace();
        if !words.next().is_some_and(|w| w.eq_ignore_ascii_case("FROM")) {
            continue;
        }
        let rest: Vec<&str> = words.filter(|w| !w.starts_with("--")).collect();
        let Some(img) = rest.first() else { continue };
        if rest.len() >= 3 && rest[1].eq_ignore_ascii_case("AS") {
            stages.insert(rest[2].to_lowercase());
        }
        if img.contains('$')
            || img.eq_ignore_ascii_case("scratch")
            || stages.contains(&img.to_lowercase())
        {
            continue;
        }
        if let Ok(r) = ImageRef::parse(img) {
            if !out.contains(&r) {
                out.push(r);
            }
        }
    }
    out
}

/// Resultado de una construcción terminada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BuildEnd {
    pub end: StreamEnd,
    pub image_id: Option<String>,
    pub tags: Vec<String>,
}

fn human(b: u64) -> String {
    match b {
        b if b >= 1 << 30 => format!("{:.2} GiB", b as f64 / (1u64 << 30) as f64),
        b if b >= 1 << 20 => format!("{:.1} MiB", b as f64 / (1u64 << 20) as f64),
        b if b >= 1 << 10 => format!("{:.1} KiB", b as f64 / 1024.0),
        b => format!("{b} B"),
    }
}

impl Engine {
    /// Construye una imagen. `sink` recibe pasos y líneas; si devuelve `false`
    /// la construcción se cancela.
    pub async fn build_image<F>(
        &self,
        spec: &BuildSpec,
        cancel: CancellationToken,
        mut sink: F,
    ) -> Result<BuildEnd, EngineError>
    where
        F: FnMut(BuildUpdate) -> bool + Send,
    {
        let issues = spec.validate(true);
        if issues.iter().any(|i| i.severity == Severity::Error) {
            return Err(EngineError::InvalidSpec { issues });
        }
        let ctx = PathBuf::from(&spec.context);
        let summary = {
            let (c, d) = (ctx.clone(), spec.dockerfile.clone());
            tokio::task::spawn_blocking(move || context::scan(&c, &d))
                .await
                .map_err(|e| EngineError::Other {
                    message: e.to_string(),
                })?
                .map_err(|e| EngineError::InvalidInput {
                    message: format!("no se pudo leer el contexto: {e}"),
                })?
        };
        if let Some(f) = summary.unreadable.first() {
            return Err(EngineError::InvalidInput {
                message: format!("no se puede leer {f} del contexto"),
            });
        }

        // Credenciales solo para los registros de las imágenes base (como mucho).
        let mut creds = HashMap::new();
        for r in spec.base_images() {
            let key = auth_key_for(&r.registry);
            if creds.contains_key(&key) {
                continue;
            }
            let res = registry_auth::resolve(self.activity(), &key, &PullAuth::DockerConfig).await;
            if let Some(c) = res.credentials {
                creds.insert(key, c);
            }
        }
        let mut headers = Vec::new();
        if !creds.is_empty() {
            headers.push((
                "X-Registry-Config".to_owned(),
                registry_auth::registry_config_header(&creds),
            ));
        }

        let (shown, _) = spec.redacted();
        let mut equivalent = self.docker_cli(&shown.cli_args());
        if spec.builder == Builder::Classic {
            equivalent = format!("DOCKER_BUILDKIT=0 {equivalent}");
        }
        let tags = spec.tag_list();
        let target = tags
            .first()
            .cloned()
            .unwrap_or_else(|| spec.context.clone());
        let handle = self.activity().begin(OperationSpec {
            category: Category::Image,
            kind: "image.build".into(),
            target: Some(target),
            transport: Transport::Api {
                method: "POST".into(),
                path: shown.path(),
                endpoint: self.endpoint().socket_path.clone(),
            },
            request_body: Some(format!(
                "Contexto en tar: {} archivos, {} ({} excluidos por .dockerignore)",
                summary.files,
                human(summary.bytes),
                summary.excluded
            )),
            equivalent_command: Some(equivalent),
        });

        let (rx, tar_task) = context::stream_tar(ctx, spec.dockerfile.clone());
        let mut tracker = progress::BuildTracker::default();
        let real_path = spec.path();
        let result: Result<EndReason, EngineError> = async {
            let dialer = self.dialer();
            let mut resp = tokio::select! {
                _ = cancel.cancelled() => return Ok(EndReason::Closed),
                r = http::post_stream(&dialer, &real_path, &headers, rx) => r?,
            };
            if resp.status != 200 {
                let message = resp.error_message().await;
                return Err(EngineError::Docker {
                    status: resp.status,
                    message,
                });
            }
            let mut tick = tokio::time::interval(FLUSH);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => return Ok(EndReason::Closed),
                    msg = resp.next_json() => match msg? {
                        Some(v) => tracker.push_json(&v),
                        None => return Ok(EndReason::Finished),
                    },
                    _ = tick.tick() => {
                        if let Some(u) = tracker.take_update() {
                            if !sink(u) {
                                return Ok(EndReason::Closed);
                            }
                        }
                    }
                }
            }
        }
        .await;
        let sent = match tar_task.await {
            Ok(Ok(n)) => Some(n),
            Ok(Err(e)) if result.is_ok() && tracker.error.is_none() => {
                tracker.error = Some(format!("no se pudo leer el contexto: {e}"));
                None
            }
            _ => None,
        };
        tracker.finish();
        if let Some(u) = tracker.take_update() {
            let _ = sink(u);
        }
        let steps = tracker.step_count();
        let sent_note = sent
            .map(|n| format!("; contexto enviado: {}", human(n)))
            .unwrap_or_default();
        match result {
            Ok(EndReason::Finished) if tracker.error.is_none() => {
                let id = tracker.image_id.clone();
                let summary = format!(
                    "Imagen construida{}{}: {steps} pasos, {} de la caché{sent_note}",
                    id.as_deref()
                        .map(|i| format!(" {}", &i[..i.len().min(19)]))
                        .unwrap_or_default(),
                    if tags.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", tags.join(", "))
                    },
                    tracker.cached,
                );
                handle.succeed(Outcome {
                    summary: Some(summary.clone()),
                    ..Default::default()
                });
                Ok(BuildEnd {
                    end: StreamEnd {
                        reason: EndReason::Finished,
                        summary,
                        exit_code: None,
                        container: None,
                    },
                    image_id: id,
                    tags: tags.clone(),
                })
            }
            Ok(EndReason::Finished) => {
                let e = tracker.error.clone().unwrap_or_default();
                handle.fail(Failure {
                    error: format!("La construcción falló: {e}"),
                    ..Default::default()
                });
                Err(EngineError::Docker {
                    status: 500,
                    message: e,
                })
            }
            Ok(EndReason::Closed) => {
                let summary =
                    format!("Construcción cancelada por el usuario ({steps} pasos iniciados)");
                handle.fail(Failure {
                    error: summary.clone(),
                    ..Default::default()
                });
                Ok(BuildEnd {
                    end: StreamEnd {
                        reason: EndReason::Closed,
                        summary,
                        exit_code: None,
                        container: None,
                    },
                    image_id: None,
                    tags: tags.clone(),
                })
            }
            Err(e) => {
                handle.fail(Failure {
                    error: e.to_string(),
                    http_status: e.http_status(),
                    ..Default::default()
                });
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(dir: &Path) -> BuildSpec {
        BuildSpec {
            context: dir.to_string_lossy().into_owned(),
            tags: vec![
                "miapp:1.0".into(),
                "registro.local:5000/miapp:latest".into(),
            ],
            build_args: vec![
                KeyValue::new("VERSION", "1.0"),
                KeyValue::new("NPM_TOKEN", "abc"),
            ],
            labels: vec![KeyValue::new("equipo", "web")],
            target: "final".into(),
            no_cache: true,
            pull: true,
            ..Default::default()
        }
    }

    #[test]
    fn query_and_cli_equivalent_match() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("Dockerfile"), "FROM scratch").unwrap();
        let s = spec(d.path());
        assert_eq!(
            s.path(),
            "/build?dockerfile=Dockerfile&t=miapp%3A1.0&t=registro.local%3A5000%2Fmiapp%3Alatest&buildargs=%7B%22NPM_TOKEN%22%3A%22abc%22%2C%22VERSION%22%3A%221.0%22%7D&labels=%7B%22equipo%22%3A%22web%22%7D&target=final&nocache=1&pull=1&rm=1&version=2"
        );
        let ctx = d.path().to_string_lossy();
        assert_eq!(
            s.cli_args().join(" "),
            format!("build --tag miapp:1.0 --tag registro.local:5000/miapp:latest --build-arg VERSION=1.0 --build-arg NPM_TOKEN=abc --label equipo=web --target final --no-cache --pull {ctx}")
        );
        let (r, n) = s.redacted();
        assert_eq!(n, 1);
        assert!(!r.path().contains("abc") && !r.cli_args().join(" ").contains("abc"));
        let classic = BuildSpec {
            builder: Builder::Classic,
            dockerfile: "docker/Dockerfile.prod".into(),
            ..s.clone()
        };
        assert!(classic.path().ends_with("rm=1&forcerm=1&version=1"));
        assert!(classic.cli_args().contains(&"--force-rm".to_string()));
        assert!(classic
            .cli_args()
            .contains(&format!("{ctx}/docker/Dockerfile.prod")));
    }

    #[test]
    fn validation() {
        let d = tempfile::tempdir().unwrap();
        let mut s = spec(d.path());
        let codes = |s: &BuildSpec| {
            s.validate(true)
                .into_iter()
                .map(|i| i.code)
                .collect::<Vec<_>>()
        };
        assert_eq!(codes(&s), vec!["dockerfile.missing", "arg.secret"]);
        std::fs::write(d.path().join("Dockerfile"), "FROM scratch").unwrap();
        assert_eq!(codes(&s), vec!["arg.secret"]);
        s.tags = vec!["Mal:tag".into(), "ok:1".into(), "ok:1".into()];
        s.build_args = vec![
            KeyValue::new("A B", "x"),
            KeyValue::new("X", "1"),
            KeyValue::new("X", "2"),
        ];
        s.target = "eta pa".into();
        s.dockerfile = "../fuera".into();
        assert_eq!(
            codes(&s),
            vec![
                "dockerfile.invalid",
                "tag.invalid",
                "tag.duplicate",
                "arg.keyInvalid",
                "arg.duplicate",
                "target.invalid"
            ]
        );
        let blanks = BuildSpec {
            tags: vec!["".into(), "  ".into()],
            ..s.clone()
        };
        assert!(codes(&blanks).contains(&"tags.none".to_string()));
        assert!(
            !blanks.path().contains("&t="),
            "las filas en blanco no se envían"
        );
        let empty = BuildSpec::default();
        assert_eq!(codes(&empty), vec!["context.required", "tags.none"]);
        let rel = BuildSpec {
            context: "relativa".into(),
            ..Default::default()
        };
        assert!(codes(&rel).contains(&"path.notAbsolute".to_string()));
        let missing = BuildSpec {
            context: "/no/existe/dcc".into(),
            tags: vec!["a".into()],
            ..Default::default()
        };
        assert_eq!(codes(&missing), vec!["context.missing"]);
    }

    #[test]
    fn base_images_skip_stages_scratch_and_variables() {
        let df = "ARG BASE=alpine\nFROM --platform=linux/amd64 golang:1.23 AS compila\nRUN go build\nFROM ${BASE}\nFROM compila AS otra\nFROM scratch\nfrom registro.local:5000/base:1\nCOPY --from=compila /app /app\n";
        let v: Vec<String> = base_images_in(df)
            .into_iter()
            .map(|r| r.familiar())
            .collect();
        assert_eq!(v, vec!["golang:1.23", "registro.local:5000/base:1"]);
    }
}
