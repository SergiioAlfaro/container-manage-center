//! Fase 6: volúmenes. Listado (cruzado con los montajes de los contenedores),
//! tamaño bajo demanda, inspección, creación con validación, eliminación y
//! limpieza. Todas son peticiones reales registradas en la actividad.
//!
//! Comportamiento de Docker comprobado (Docker 29.9 y 24.0, ver D-051):
//! - `POST /volumes/create` con un nombre que ya existe devuelve 201 con el
//!   volumen existente, sin aplicar el driver, las opciones ni las etiquetas
//!   pedidas. La aplicación comprueba antes si existe.
//! - `DELETE /volumes/{name}` falla con 409 si lo usa cualquier contenedor, en
//!   marcha o detenido, también con `force` (que solo sirve para errores del
//!   driver). La aplicación no ofrece forzar.
//! - `POST /volumes/prune` elimina por defecto solo los volúmenes anónimos sin
//!   uso (API ≥ 1.42); con el filtro `all=true`, también los que tienen nombre.
//! - El tamaño solo lo da `GET /system/df?type=volume`, con formato distinto
//!   según la versión: se lee la respuesta sin tipar.

use std::collections::{BTreeMap, HashMap, HashSet};

use bollard::models::VolumeCreateRequest;
use dcc_activity::{quote::render_argv, Category};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::build::http;
use crate::client::{ApiOp, Engine};
use crate::containers::ContainerSummary;
use crate::error::EngineError;
use crate::resources::path_with_query;
use crate::spec::{issue, KeyValue, Severity, SpecIssue};

/// Etiqueta que Docker (≥ 23) pone a los volúmenes anónimos.
pub const ANONYMOUS_LABEL: &str = "com.docker.volume.anonymous";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VolumeUser {
    pub id: String,
    pub name: String,
    pub state: String,
    /// Ruta dentro del contenedor.
    pub destination: String,
    pub rw: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VolumeSummary {
    pub name: String,
    pub driver: String,
    pub scope: String,
    pub mountpoint: String,
    pub created: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub options: BTreeMap<String, String>,
    /// Creado por Docker sin nombre (etiqueta `com.docker.volume.anonymous` o
    /// nombre de 64 caracteres hexadecimales en versiones antiguas).
    pub anonymous: bool,
    pub compose_project: Option<String>,
    /// Contenedores (de cualquier estado) que lo montan.
    pub containers: Vec<VolumeUser>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VolumeDetails {
    pub summary: VolumeSummary,
    /// Estado que informa el driver (`Status`), si lo hay.
    pub status: BTreeMap<String, String>,
    /// JSON completo devuelto por Docker.
    pub raw: String,
}

/// Tamaño de un volumen según `GET /system/df?type=volume`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VolumeSize {
    pub name: String,
    /// Bytes; `None` si Docker no lo puede calcular (p. ej. drivers remotos).
    #[ts(type = "number | null")]
    pub size: Option<i64>,
    /// Contenedores que lo referencian; `None` si Docker no lo indica.
    #[ts(type = "number | null")]
    pub ref_count: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VolumePruneResult {
    pub deleted: Vec<String>,
    #[ts(type = "number")]
    pub space_reclaimed: i64,
}

/// Volumen nuevo: fuente única de la validación, del cuerpo de
/// `POST /volumes/create` y del comando `docker volume create`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct VolumeSpec {
    /// Vacío = Docker genera un nombre (volumen anónimo).
    pub name: String,
    pub driver: String,
    pub driver_opts: Vec<KeyValue>,
    pub labels: Vec<KeyValue>,
}

impl Default for VolumeSpec {
    fn default() -> Self {
        Self {
            name: String::new(),
            driver: "local".into(),
            driver_opts: vec![],
            labels: vec![],
        }
    }
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Regla de Docker para volúmenes locales: `[a-zA-Z0-9][a-zA-Z0-9_.-]+`.
pub fn valid_volume_name(s: &str) -> bool {
    s.len() >= 2
        && s.len() <= 255
        && s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

fn valid_driver(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 255
        && s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | ':' | '/'))
}

fn valid_key(k: &str) -> bool {
    !k.is_empty()
        && k.len() <= 255
        && !k.contains('=')
        && !k.chars().any(|c| c.is_whitespace() || c.is_control())
}

fn kv_issues(v: &mut Vec<SpecIssue>, items: &[KeyValue], field: &str, code: &str) {
    let mut seen = HashSet::new();
    for (i, kv) in items.iter().enumerate() {
        let k = kv.key.trim();
        if k.is_empty() && kv.value.trim().is_empty() {
            continue;
        }
        if !valid_key(k) {
            v.push(issue(
                Severity::Error,
                format!("{field}.{i}.key"),
                &format!("{code}.keyInvalid"),
                &[("key", k.to_owned())],
            ));
        } else if !seen.insert(k.to_owned()) {
            v.push(issue(
                Severity::Error,
                format!("{field}.{i}.key"),
                &format!("{code}.duplicate"),
                &[("key", k.to_owned())],
            ));
        }
    }
}

fn kv_map(items: &[KeyValue]) -> HashMap<String, String> {
    items
        .iter()
        .filter(|kv| !kv.key.trim().is_empty())
        .map(|kv| (kv.key.trim().to_owned(), kv.value.clone()))
        .collect()
}

fn kv_sorted(items: &[KeyValue]) -> Vec<(String, String)> {
    let mut v: Vec<_> = kv_map(items).into_iter().collect();
    v.sort();
    v
}

impl VolumeSpec {
    pub fn validate(&self) -> Vec<SpecIssue> {
        use Severity::*;
        let mut v = Vec::new();
        let name = self.name.trim();
        let driver = self.driver.trim();
        if !name.is_empty() && !valid_volume_name(name) {
            // Docker exige esta regla al driver local; otros drivers deciden.
            let sev = if driver == "local" { Error } else { Warning };
            let code = if name.len() < 2 {
                "volumeName.short"
            } else {
                "volumeName.invalid"
            };
            v.push(issue(sev, "name", code, &[("value", name.to_owned())]));
        }
        if !valid_driver(driver) {
            v.push(issue(
                Error,
                "driver",
                "driver.invalid",
                &[("value", driver.to_owned())],
            ));
        }
        kv_issues(&mut v, &self.driver_opts, "driverOpts", "option");
        kv_issues(&mut v, &self.labels, "labels", "label");
        if driver == "local" {
            let opts = kv_map(&self.driver_opts);
            // Opciones del driver local: type, device y o (como mount(8)).
            for k in opts.keys() {
                if !matches!(k.as_str(), "type" | "device" | "o" | "size") {
                    v.push(issue(
                        Warning,
                        "driverOpts",
                        "volumeOpt.unknownLocal",
                        &[("key", k.clone())],
                    ));
                }
            }
            if opts.contains_key("device") && !opts.contains_key("type") {
                v.push(issue(
                    Warning,
                    "driverOpts",
                    "volumeOpt.deviceWithoutType",
                    &[],
                ));
            }
        }
        v
    }

    pub fn has_errors(issues: &[SpecIssue]) -> bool {
        issues.iter().any(|i| i.severity == Severity::Error)
    }

    pub fn to_create_body(&self) -> VolumeCreateRequest {
        let opts = kv_map(&self.driver_opts);
        let labels = kv_map(&self.labels);
        let name = self.name.trim();
        VolumeCreateRequest {
            name: (!name.is_empty()).then(|| name.to_owned()),
            driver: Some(self.driver.trim().to_owned()),
            driver_opts: (!opts.is_empty()).then_some(opts),
            labels: (!labels.is_empty()).then_some(labels),
            cluster_volume_spec: None,
        }
    }

    pub fn cli_args(&self) -> Vec<String> {
        let mut a: Vec<String> = vec!["volume".into(), "create".into()];
        a.push("--driver".into());
        a.push(self.driver.trim().to_owned());
        for (k, v) in kv_sorted(&self.driver_opts) {
            a.push("--opt".into());
            a.push(format!("{k}={v}"));
        }
        for (k, v) in kv_sorted(&self.labels) {
            a.push("--label".into());
            a.push(format!("{k}={v}"));
        }
        let name = self.name.trim();
        if !name.is_empty() {
            a.push(name.to_owned());
        }
        a
    }

    pub fn command(&self) -> String {
        render_argv("docker", &self.cli_args())
    }
}

/// Contenedores de la lista que montan el volumen `name`.
fn users_of(name: &str, containers: &[ContainerSummary]) -> Vec<VolumeUser> {
    let mut v: Vec<VolumeUser> = containers
        .iter()
        .flat_map(|c| {
            c.mount_points
                .iter()
                .filter(|m| m.kind == "volume" && m.name.as_deref() == Some(name))
                .map(move |m| VolumeUser {
                    id: c.id.clone(),
                    name: c.name.clone(),
                    state: c.state.clone(),
                    destination: m.destination.clone(),
                    rw: m.rw,
                })
        })
        .collect();
    v.sort_by(|a, b| (&a.name, &a.destination).cmp(&(&b.name, &b.destination)));
    v
}

fn summary_from(v: bollard::models::Volume, containers: &[ContainerSummary]) -> VolumeSummary {
    let labels: BTreeMap<String, String> = v.labels.into_iter().collect();
    let anonymous = labels.contains_key(ANONYMOUS_LABEL) || is_hex64(&v.name);
    VolumeSummary {
        containers: users_of(&v.name, containers),
        compose_project: labels.get("com.docker.compose.project").cloned(),
        anonymous,
        driver: v.driver,
        scope: v
            .scope
            .and_then(|s| serde_json::to_value(s).ok())
            .and_then(|s| s.as_str().map(str::to_owned))
            .unwrap_or_default(),
        mountpoint: v.mountpoint,
        created: v.created_at,
        options: v.options.into_iter().collect(),
        labels,
        name: v.name,
    }
}

/// Lee los tamaños de `GET /system/df?type=volume` en cualquiera de los dos
/// formatos: `Volumes` (Docker ≤ 28) o `VolumeUsage.Items` (Docker ≥ 29).
/// `-1` en `Size`/`RefCount` significa "no disponible".
pub fn parse_volume_sizes(v: &serde_json::Value) -> Vec<VolumeSize> {
    let items = v["VolumeUsage"]["Items"]
        .as_array()
        .filter(|a| !a.is_empty())
        .or_else(|| v["Volumes"].as_array())
        .cloned()
        .unwrap_or_default();
    let mut out: Vec<VolumeSize> = items
        .iter()
        .filter_map(|i| {
            let name = i["Name"].as_str()?.to_owned();
            let size = i["UsageData"]["Size"].as_i64().filter(|s| *s >= 0);
            let ref_count = i["UsageData"]["RefCount"].as_i64().filter(|s| *s >= 0);
            Some(VolumeSize {
                name,
                size,
                ref_count,
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn validate_volume_ref(r: &str) -> Result<(), EngineError> {
    // Docker admite en otros drivers nombres que no cumplen la regla local;
    // aquí solo se descarta lo que rompería la ruta HTTP.
    let ok = !r.is_empty()
        && r.len() <= 255
        && !r
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '?' | '#' | '%' | ' '))
        && r != "."
        && r != "..";
    if ok {
        Ok(())
    } else {
        Err(EngineError::InvalidInput {
            message: format!("nombre de volumen no válido: {r:?}"),
        })
    }
}

impl Engine {
    /// Volúmenes con los contenedores que los montan (dos peticiones reales).
    pub async fn list_volumes(
        &self,
        containers: Option<&[ContainerSummary]>,
    ) -> Result<Vec<VolumeSummary>, EngineError> {
        let op =
            ApiOp::get(Category::Volume, "volume.list", "/volumes").equivalent(["volume", "ls"]);
        let d = self.docker().clone();
        let r = self
            .call(
                op,
                async move {
                    d.list_volumes(None::<bollard::query_parameters::ListVolumesOptions>)
                        .await
                },
                |r| {
                    Some(format!(
                        "{} volúmenes",
                        r.volumes.as_ref().map_or(0, |v| v.len())
                    ))
                },
            )
            .await?;
        let owned;
        let containers = match containers {
            Some(c) => c,
            None => {
                owned = self.list_containers().await?;
                &owned
            }
        };
        let mut v: Vec<VolumeSummary> = r
            .volumes
            .unwrap_or_default()
            .into_iter()
            .map(|v| summary_from(v, containers))
            .collect();
        v.sort_by(|a, b| (a.anonymous, &a.name).cmp(&(b.anonymous, &b.name)));
        Ok(v)
    }

    /// `docker volume inspect` más los contenedores que lo montan.
    pub async fn volume_details(&self, name: &str) -> Result<VolumeDetails, EngineError> {
        let n = name.trim();
        validate_volume_ref(n)?;
        let op = ApiOp {
            category: Category::Volume,
            kind: "volume.inspect",
            target: Some(n.to_owned()),
            method: "GET",
            path: format!("/volumes/{n}"),
            body: None,
            equivalent: Some(vec!["volume".into(), "inspect".into(), n.into()]),
            timeout: None,
        };
        let d = self.docker().clone();
        let key = n.to_owned();
        let v = self
            .call(op, async move { d.inspect_volume(&key).await }, |_| None)
            .await?;
        let raw = serde_json::to_string_pretty(&v).unwrap_or_default();
        let status: BTreeMap<String, String> = v
            .status
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|s| match s.split_once('=') {
                Some((k, val)) => (k.to_owned(), val.to_owned()),
                None => (s, String::new()),
            })
            .collect();
        let containers = self.list_containers().await?;
        Ok(VolumeDetails {
            summary: summary_from(v, &containers),
            status,
            raw,
        })
    }

    /// Tamaño y referencias de cada volumen (`docker system df --verbose`).
    /// Puede tardar con volúmenes grandes: se pide bajo demanda.
    pub async fn volume_sizes(&self) -> Result<Vec<VolumeSize>, EngineError> {
        let path = "/system/df?type=volume";
        let op = ApiOp {
            category: Category::Volume,
            kind: "volume.usage",
            target: None,
            method: "GET",
            path: path.into(),
            body: None,
            equivalent: Some(vec!["system".into(), "df".into(), "--verbose".into()]),
            timeout: Some(std::time::Duration::from_secs(600)),
        };
        let socket = self.endpoint().socket_path.clone();
        let v = self
            .call(
                op,
                async move { http::get_json(&socket, path).await },
                |v| Some(format!("{} volúmenes medidos", parse_volume_sizes(v).len())),
            )
            .await?;
        Ok(parse_volume_sizes(&v))
    }

    /// `docker volume create`. Comprueba antes si ya existe un volumen con ese
    /// nombre: Docker devolvería el existente sin aplicar lo pedido (D-051).
    pub async fn create_volume(&self, spec: &VolumeSpec) -> Result<VolumeSummary, EngineError> {
        let mut issues = spec.validate();
        if VolumeSpec::has_errors(&issues) {
            return Err(EngineError::InvalidSpec { issues });
        }
        let name = spec.name.trim().to_owned();
        if !name.is_empty() {
            let mut filters = HashMap::new();
            filters.insert("name".to_owned(), vec![name.clone()]);
            let opts = bollard::query_parameters::ListVolumesOptions {
                filters: Some(filters),
            };
            let op = ApiOp {
                category: Category::Volume,
                kind: "volume.list",
                target: Some(name.clone()),
                method: "GET",
                path: path_with_query("/volumes", &opts),
                body: None,
                equivalent: Some(vec![
                    "volume".into(),
                    "ls".into(),
                    "--filter".into(),
                    format!("name={name}"),
                ]),
                timeout: None,
            };
            let d = self.docker().clone();
            let found = self
                .call(op, async move { d.list_volumes(Some(opts)).await }, |r| {
                    Some(format!(
                        "{} volúmenes con ese nombre o parecido",
                        r.volumes.as_ref().map_or(0, |v| v.len())
                    ))
                })
                .await?;
            if found
                .volumes
                .unwrap_or_default()
                .iter()
                .any(|v| v.name == name)
            {
                issues.push(issue(
                    Severity::Error,
                    "name",
                    "volumeName.exists",
                    &[("value", name.clone())],
                ));
                return Err(EngineError::InvalidSpec { issues });
            }
        }
        let body = spec.to_create_body();
        let op = ApiOp {
            category: Category::Volume,
            kind: "volume.create",
            target: Some(if name.is_empty() {
                "(anónimo)".into()
            } else {
                name.clone()
            }),
            method: "POST",
            path: "/volumes/create".into(),
            body: Some(serde_json::to_string_pretty(&body).unwrap_or_default()),
            equivalent: Some(spec.cli_args()),
            timeout: None,
        };
        let d = self.docker().clone();
        let v = self
            .call(op, async move { d.create_volume(body).await }, |v| {
                Some(format!("Volumen creado: {}", v.name))
            })
            .await?;
        Ok(summary_from(v, &[]))
    }

    /// `docker volume rm`. Docker lo rechaza (409) si lo monta cualquier
    /// contenedor, en marcha o detenido; la interfaz lo avisa antes.
    pub async fn remove_volume(&self, name: &str, confirmed: bool) -> Result<(), EngineError> {
        if !confirmed {
            return Err(EngineError::NotConfirmed);
        }
        let n = name.trim();
        validate_volume_ref(n)?;
        let _guard = self.locks().acquire(&format!("volume:{n}"))?;
        let op = ApiOp {
            category: Category::Volume,
            kind: "volume.remove",
            target: Some(n.to_owned()),
            method: "DELETE",
            path: format!("/volumes/{n}"),
            body: None,
            equivalent: Some(vec!["volume".into(), "rm".into(), n.into()]),
            timeout: Some(std::time::Duration::from_secs(300)),
        };
        let d = self.docker().clone();
        let key = n.to_owned();
        let shown = n.to_owned();
        self.call(
            op,
            async move {
                d.remove_volume(&key, None::<bollard::query_parameters::RemoveVolumeOptions>)
                    .await
            },
            move |_| Some(format!("Volumen eliminado: {shown}")),
        )
        .await
    }

    /// `docker volume prune --force [--all]`: sin `all`, solo los anónimos sin
    /// uso; con `all`, también los que tienen nombre. `label` limita a una
    /// etiqueta (`clave=valor`); las pruebas lo usan.
    pub async fn prune_volumes(
        &self,
        all: bool,
        label: Option<&str>,
        confirmed: bool,
    ) -> Result<VolumePruneResult, EngineError> {
        if !confirmed {
            return Err(EngineError::NotConfirmed);
        }
        let mut eq = vec![
            "volume".to_owned(),
            "prune".to_owned(),
            "--force".to_owned(),
        ];
        let mut filters = HashMap::new();
        if all {
            filters.insert("all".to_owned(), vec!["true".to_owned()]);
            eq.push("--all".into());
        }
        if let Some(l) = label {
            if l.is_empty() || l.chars().any(|c| c.is_whitespace() || c.is_control()) {
                return Err(EngineError::InvalidInput {
                    message: format!("filtro de etiqueta no válido: {l:?}"),
                });
            }
            filters.insert("label".to_owned(), vec![l.to_owned()]);
            eq.push("--filter".into());
            eq.push(format!("label={l}"));
        }
        let opts = bollard::query_parameters::PruneVolumesOptions {
            filters: (!filters.is_empty()).then_some(filters),
        };
        let op = ApiOp {
            category: Category::Volume,
            kind: "volume.prune",
            target: None,
            method: "POST",
            path: path_with_query("/volumes/prune", &opts),
            body: None,
            equivalent: Some(eq),
            timeout: Some(std::time::Duration::from_secs(600)),
        };
        let d = self.docker().clone();
        let r = self
            .call(op, async move { d.prune_volumes(Some(opts)).await }, |r| {
                Some(format!(
                    "{} volúmenes eliminados; espacio recuperado: {} bytes",
                    r.volumes_deleted.as_ref().map_or(0, |v| v.len()),
                    r.space_reclaimed.unwrap_or(0)
                ))
            })
            .await?;
        Ok(VolumePruneResult {
            deleted: r.volumes_deleted.unwrap_or_default(),
            space_reclaimed: r.space_reclaimed.unwrap_or(0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::containers::MountPoint;

    fn codes(s: &VolumeSpec) -> Vec<(String, Severity)> {
        s.validate()
            .into_iter()
            .map(|i| (i.code, i.severity))
            .collect()
    }

    #[test]
    fn name_rules_match_docker_local_driver() {
        for ok in ["ab", "datos", "app_db-1.v2", "0x"] {
            assert!(valid_volume_name(ok), "{ok}");
        }
        for bad in ["a", "", "-x", ".x", "a b", "a/b", "ñu", &"a".repeat(256)] {
            assert!(!valid_volume_name(bad), "{bad}");
        }
        let s = |n: &str, d: &str| VolumeSpec {
            name: n.into(),
            driver: d.into(),
            ..Default::default()
        };
        assert!(codes(&s("", "local")).is_empty(), "sin nombre: anónimo");
        assert_eq!(
            codes(&s("a", "local")),
            [("volumeName.short".into(), Severity::Error)]
        );
        assert_eq!(
            codes(&s("a b", "local")),
            [("volumeName.invalid".into(), Severity::Error)]
        );
        // Otros drivers deciden sus propias reglas: solo aviso.
        assert_eq!(
            codes(&s("a b", "rclone")),
            [("volumeName.invalid".into(), Severity::Warning)]
        );
        assert_eq!(
            codes(&s("ok", "")),
            [("driver.invalid".into(), Severity::Error)]
        );
        assert!(codes(&s("ok", "vieux/sshfs:latest")).is_empty());
    }

    #[test]
    fn local_driver_options_and_labels() {
        let mut s = VolumeSpec {
            name: "nfs-datos".into(),
            driver_opts: vec![
                KeyValue::new("type", "nfs"),
                KeyValue::new("o", "addr=10.0.0.5,rw"),
                KeyValue::new("device", ":/export/datos"),
            ],
            labels: vec![KeyValue::new("app", "web"), KeyValue::new("app", "otra")],
            ..Default::default()
        };
        let i = s.validate();
        assert_eq!(
            i.iter()
                .map(|i| (i.field.as_str(), i.code.as_str()))
                .collect::<Vec<_>>(),
            [("labels.1.key", "label.duplicate")]
        );
        s.labels.clear();
        s.driver_opts = vec![
            KeyValue::new("device", "/dev/sdb1"),
            KeyValue::new("uid", "1000"),
        ];
        let got: Vec<_> = s.validate().into_iter().map(|i| i.code).collect();
        assert_eq!(
            got,
            ["volumeOpt.unknownLocal", "volumeOpt.deviceWithoutType"]
        );
        assert!(!VolumeSpec::has_errors(&s.validate()));
        s.driver_opts = vec![KeyValue::new("a b", "x")];
        assert_eq!(s.validate()[0].code, "option.keyInvalid");
    }

    #[test]
    fn body_and_command() {
        let s = VolumeSpec::default();
        assert_eq!(
            serde_json::to_value(s.to_create_body()).unwrap(),
            serde_json::json!({ "Driver": "local" })
        );
        assert_eq!(s.command(), "docker volume create --driver local");
        let s = VolumeSpec {
            name: " datos ".into(),
            driver: "local".into(),
            driver_opts: vec![
                KeyValue::new("type", "tmpfs"),
                KeyValue::new("device", "tmpfs"),
                KeyValue::new("o", "size=64m"),
            ],
            labels: vec![KeyValue::new("app", "mi web")],
        };
        assert_eq!(
            serde_json::to_value(s.to_create_body()).unwrap(),
            serde_json::json!({
                "Name": "datos",
                "Driver": "local",
                "DriverOpts": { "type": "tmpfs", "device": "tmpfs", "o": "size=64m" },
                "Labels": { "app": "mi web" }
            })
        );
        assert_eq!(
            s.command(),
            "docker volume create --driver local --opt device=tmpfs --opt o=size=64m --opt type=tmpfs --label 'app=mi web' datos"
        );
    }

    #[test]
    fn sizes_are_read_in_both_formats() {
        // Docker ≤ 28.
        let old = serde_json::json!({ "Volumes": [
            { "Name": "b", "UsageData": { "Size": 2048, "RefCount": 1 } },
            { "Name": "a", "UsageData": { "Size": -1, "RefCount": -1 } }
        ]});
        assert_eq!(
            parse_volume_sizes(&old),
            [
                VolumeSize {
                    name: "a".into(),
                    size: None,
                    ref_count: None
                },
                VolumeSize {
                    name: "b".into(),
                    size: Some(2048),
                    ref_count: Some(1)
                },
            ]
        );
        // Docker 29: los dos campos (se prefiere el nuevo si tiene datos).
        let new = serde_json::json!({
            "Volumes": [],
            "VolumeUsage": { "ActiveCount": 1, "Items": [ { "Name": "c", "UsageData": { "Size": 0, "RefCount": 0 } } ] }
        });
        assert_eq!(
            parse_volume_sizes(&new),
            [VolumeSize {
                name: "c".into(),
                size: Some(0),
                ref_count: Some(0)
            }]
        );
        let only_old_with_empty_new = serde_json::json!({ "Volumes": [ { "Name": "d", "UsageData": { "Size": 1, "RefCount": 0 } } ], "VolumeUsage": {} });
        assert_eq!(parse_volume_sizes(&only_old_with_empty_new).len(), 1);
        assert!(parse_volume_sizes(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn users_come_from_container_mounts() {
        let mp = |kind: &str, name: Option<&str>, dest: &str, rw: bool| MountPoint {
            kind: kind.into(),
            name: name.map(str::to_owned),
            source: format!("/var/lib/docker/volumes/{}/_data", name.unwrap_or("x")),
            destination: dest.into(),
            rw,
            mode: String::new(),
            propagation: String::new(),
        };
        let c = |id: &str, name: &str, state: &str, mounts: Vec<MountPoint>| ContainerSummary {
            id: id.into(),
            name: name.into(),
            image: String::new(),
            image_id: String::new(),
            command: String::new(),
            created: 0,
            state: state.into(),
            status: String::new(),
            ports: vec![],
            networks: vec![],
            mounts: mounts.len() as u32,
            mount_points: mounts,
            compose_project: None,
            labels: Default::default(),
        };
        let list = [
            c(
                "1",
                "web",
                "running",
                vec![
                    mp("volume", Some("datos"), "/data", true),
                    mp("bind", None, "/src", false),
                ],
            ),
            c(
                "2",
                "copia",
                "exited",
                vec![mp("volume", Some("datos"), "/backup", false)],
            ),
            c(
                "3",
                "otro",
                "running",
                vec![mp("volume", Some("otro"), "/x", true)],
            ),
        ];
        let u = users_of("datos", &list);
        assert_eq!(
            u.iter()
                .map(|u| (u.name.as_str(), u.destination.as_str(), u.rw))
                .collect::<Vec<_>>(),
            [("copia", "/backup", false), ("web", "/data", true)]
        );
        assert!(users_of("nada", &list).is_empty());
    }

    #[test]
    fn refs_that_would_break_the_path_are_rejected() {
        for ok in [
            "datos",
            "237e51847da08c4b1540eb0d76abbb5e62fb09ee9c600e9869cb0c4d26ac8b6b",
            "a:b",
        ] {
            assert!(validate_volume_ref(ok).is_ok(), "{ok}");
        }
        for bad in ["", "a/b", "..", "a?x", "a b", "a%2F"] {
            assert!(validate_volume_ref(bad).is_err(), "{bad}");
        }
    }
}
