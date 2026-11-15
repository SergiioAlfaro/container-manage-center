// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Proyectos Docker Compose (Fase 7, D-053): identidad de un proyecto,
//! descubrimiento a partir de las etiquetas de los contenedores, modelo de
//! servicios a partir de `docker compose config --format json` y construcción
//! de los argumentos de cada acción.
//!
//! Todos los argumentos se pasan por separado al CLI oficial (sin shell) y se
//! validan antes: nombres de proyecto, servicios y perfiles no pueden empezar
//! por `-` ni contener nada que el CLI pueda tomar como otra opción.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::ComposeStatus;
use crate::containers::ContainerSummary;
use crate::error::EngineError;

pub const LABEL_PROJECT: &str = "com.docker.compose.project";
pub const LABEL_SERVICE: &str = "com.docker.compose.service";
pub const LABEL_WORKING_DIR: &str = "com.docker.compose.project.working_dir";
pub const LABEL_CONFIG_FILES: &str = "com.docker.compose.project.config_files";
pub const LABEL_ENV_FILE: &str = "com.docker.compose.project.environment_file";
pub const LABEL_NUMBER: &str = "com.docker.compose.container-number";
pub const LABEL_ONEOFF: &str = "com.docker.compose.oneoff";
pub const LABEL_VERSION: &str = "com.docker.compose.version";

/// Nombres de archivo que Compose busca por defecto, en su orden.
pub const DEFAULT_FILES: [&str; 4] = [
    "compose.yaml",
    "compose.yml",
    "docker-compose.yaml",
    "docker-compose.yml",
];

pub const MAX_FILES: usize = 16;
const MAX_PATH: usize = 4096;

/// Cómo invocar Compose: `docker compose …` o `docker-compose …`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposeCli {
    pub program: String,
    /// `["compose"]` para el plugin; vacío para el binario independiente.
    pub prefix: Vec<String>,
    pub version: String,
}

impl ComposeCli {
    pub fn from_status(s: &ComposeStatus) -> Option<Self> {
        match s {
            ComposeStatus::Available {
                version,
                kind,
                program,
            } => Some(Self {
                program: program.clone(),
                prefix: match kind {
                    super::ComposeKind::Plugin => vec!["compose".into()],
                    super::ComposeKind::Standalone => vec![],
                },
                version: version.clone(),
            }),
            _ => None,
        }
    }

    /// Nombre corto para el comando mostrado: `docker compose` o `docker-compose`.
    pub fn display(&self) -> String {
        let base = Path::new(&self.program)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.program.clone());
        std::iter::once(base)
            .chain(self.prefix.iter().cloned())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Identidad de un proyecto sobre la que se ejecutan las acciones.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectRef {
    pub name: String,
    /// Archivos Compose (rutas absolutas), en el orden en que se combinan.
    pub files: Vec<String>,
    /// Directorio del proyecto; si falta, Compose usa el del primer archivo.
    pub working_dir: Option<String>,
    pub env_files: Vec<String>,
}

fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidInput {
        message: message.into(),
    }
}

/// Regla de Compose para el nombre de proyecto: minúsculas, dígitos, `-` y `_`,
/// empezando por letra o dígito.
pub fn valid_project_name(name: &str) -> bool {
    let mut c = name.chars();
    matches!(c.next(), Some(f) if f.is_ascii_lowercase() || f.is_ascii_digit())
        && name.len() <= 255
        && c.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
}

/// Nombre de proyecto que Compose deriva de una carpeta (normalización de Compose:
/// minúsculas y solo los caracteres permitidos).
pub fn project_name_from_dir(dir: &Path) -> String {
    let base = dir
        .file_name()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let s: String = base
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-' || *c == '_')
        .collect();
    s.trim_start_matches(['-', '_']).to_owned()
}

/// Nombres de servicio y de perfil: no pueden empezar por `-` (se tomarían como opción).
pub fn valid_service_name(name: &str) -> bool {
    let mut c = name.chars();
    matches!(c.next(), Some(f) if f.is_ascii_alphanumeric())
        && name.len() <= 255
        && c.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-'))
}

fn check_path(p: &str, what: &str) -> Result<(), EngineError> {
    if p.is_empty() || p.len() > MAX_PATH || p.contains('\0') || !p.starts_with('/') {
        return Err(invalid(format!("{what} debe ser una ruta absoluta: {p:?}")));
    }
    Ok(())
}

impl ProjectRef {
    pub fn validate(&self) -> Result<(), EngineError> {
        if !valid_project_name(&self.name) {
            return Err(invalid(format!(
                "nombre de proyecto no válido: {:?}",
                self.name
            )));
        }
        if self.files.len() > MAX_FILES || self.env_files.len() > MAX_FILES {
            return Err(invalid("demasiados archivos"));
        }
        for f in &self.files {
            check_path(f, "el archivo Compose")?;
        }
        for f in &self.env_files {
            check_path(f, "el archivo de variables")?;
        }
        if let Some(d) = &self.working_dir {
            check_path(d, "el directorio del proyecto")?;
        }
        Ok(())
    }

    /// Archivos que ya no existen en el equipo.
    pub fn missing_files(&self) -> Vec<String> {
        self.files
            .iter()
            .filter(|f| !Path::new(f).is_file())
            .cloned()
            .collect()
    }

    /// Directorio del proyecto efectivo.
    pub fn project_dir(&self) -> Option<String> {
        self.working_dir.clone().or_else(|| {
            self.files
                .first()
                .and_then(|f| Path::new(f).parent().map(|p| p.display().to_string()))
        })
    }

    /// Opciones globales: `--ansi never -p NOMBRE [-f …] [--project-directory …] [--env-file …]`.
    ///
    /// Sin archivos (`with_files = false`) Compose trabaja solo con las
    /// etiquetas de los contenedores del proyecto (parar, arrancar, eliminar,
    /// registros), lo que permite gestionar proyectos cuyos archivos ya no están.
    /// `stdin_file` sustituye el archivo de esa posición por `-` (contenido sin guardar).
    pub fn global_args(&self, with_files: bool, stdin_file: Option<usize>) -> Vec<String> {
        let mut a: Vec<String> = vec![
            "--ansi".into(),
            "never".into(),
            "-p".into(),
            self.name.clone(),
        ];
        if with_files {
            for (i, f) in self.files.iter().enumerate() {
                a.push("-f".into());
                a.push(if stdin_file == Some(i) {
                    "-".into()
                } else {
                    f.clone()
                });
            }
            if let Some(d) = self.project_dir() {
                a.push("--project-directory".into());
                a.push(d);
            }
            for e in &self.env_files {
                a.push("--env-file".into());
                a.push(e.clone());
            }
        }
        a
    }
}

// ------------------------------------------------------------------ descubrimiento

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectContainer {
    pub id: String,
    pub name: String,
    pub service: String,
    pub number: Option<u32>,
    pub state: String,
    pub status: String,
    pub image: String,
    /// Contenedor de `docker compose run` (no forma parte de los servicios).
    pub oneoff: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ProjectState {
    /// Todos los contenedores en marcha.
    Running,
    /// Algunos en marcha.
    Partial,
    /// Ninguno en marcha.
    Stopped,
    /// Sin contenedores (proyecto añadido a la lista, aún no desplegado o eliminado).
    NotDeployed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ComposeProject {
    #[serde(flatten)]
    #[ts(flatten)]
    pub reference: ProjectRef,
    /// Añadido por el usuario a la lista (se conserva aunque no tenga contenedores).
    pub registered: bool,
    pub missing_files: Vec<String>,
    pub containers: Vec<ProjectContainer>,
    /// Servicios con contenedores, ordenados.
    pub services: Vec<String>,
    pub state: ProjectState,
    pub running: u32,
    pub total: u32,
    /// Versión de Compose que creó los contenedores.
    pub created_by: Option<String>,
}

fn split_files(s: &str) -> Vec<String> {
    s.split(',')
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(str::to_owned)
        .collect()
}

pub fn state_of(containers: &[ProjectContainer]) -> (ProjectState, u32, u32) {
    let svc: Vec<_> = containers.iter().filter(|c| !c.oneoff).collect();
    let total = svc.len() as u32;
    let running = svc.iter().filter(|c| c.state == "running").count() as u32;
    let st = match (total, running) {
        (0, _) => ProjectState::NotDeployed,
        (t, r) if r == t => ProjectState::Running,
        (_, 0) => ProjectState::Stopped,
        _ => ProjectState::Partial,
    };
    (st, running, total)
}

/// Agrupa los contenedores por proyecto y añade los proyectos registrados.
/// Un proyecto registrado con el mismo nombre que uno desplegado se combina:
/// se usan sus archivos y los contenedores del desplegado.
pub fn projects_from(
    containers: &[ContainerSummary],
    registered: &[ProjectRef],
) -> Vec<ComposeProject> {
    projects_from_with(containers, registered, |p| p.missing_files())
}

/// Como [`projects_from`], con la comprobación de archivos que corresponda
/// (en un servidor remoto se hace en el servidor, 0.10.0).
pub fn projects_from_with(
    containers: &[ContainerSummary],
    registered: &[ProjectRef],
    missing: impl Fn(&ProjectRef) -> Vec<String>,
) -> Vec<ComposeProject> {
    let mut map: BTreeMap<String, ComposeProject> = BTreeMap::new();
    for c in containers {
        let Some(name) = c.labels.get(LABEL_PROJECT) else {
            continue;
        };
        let entry = map.entry(name.clone()).or_insert_with(|| ComposeProject {
            reference: ProjectRef {
                name: name.clone(),
                files: c
                    .labels
                    .get(LABEL_CONFIG_FILES)
                    .map(|s| split_files(s))
                    .unwrap_or_default(),
                working_dir: c.labels.get(LABEL_WORKING_DIR).cloned(),
                env_files: c
                    .labels
                    .get(LABEL_ENV_FILE)
                    .map(|s| split_files(s))
                    .unwrap_or_default(),
            },
            registered: false,
            missing_files: vec![],
            containers: vec![],
            services: vec![],
            state: ProjectState::NotDeployed,
            running: 0,
            total: 0,
            created_by: c.labels.get(LABEL_VERSION).cloned(),
        });
        entry.containers.push(ProjectContainer {
            id: c.id.clone(),
            name: c.name.clone(),
            service: c.labels.get(LABEL_SERVICE).cloned().unwrap_or_default(),
            number: c.labels.get(LABEL_NUMBER).and_then(|n| n.parse().ok()),
            state: c.state.clone(),
            status: c.status.clone(),
            image: c.image.clone(),
            oneoff: c
                .labels
                .get(LABEL_ONEOFF)
                .is_some_and(|v| v.eq_ignore_ascii_case("true")),
        });
    }
    for r in registered {
        match map.get_mut(&r.name) {
            Some(p) => {
                p.registered = true;
                p.reference.files = r.files.clone();
                if r.working_dir.is_some() {
                    p.reference.working_dir = r.working_dir.clone();
                }
                if !r.env_files.is_empty() {
                    p.reference.env_files = r.env_files.clone();
                }
            }
            None => {
                map.insert(
                    r.name.clone(),
                    ComposeProject {
                        reference: r.clone(),
                        registered: true,
                        missing_files: vec![],
                        containers: vec![],
                        services: vec![],
                        state: ProjectState::NotDeployed,
                        running: 0,
                        total: 0,
                        created_by: None,
                    },
                );
            }
        }
    }
    map.into_values()
        .map(|mut p| {
            p.containers.sort_by(|a, b| {
                (a.oneoff, &a.service, a.number, &a.name)
                    .cmp(&(b.oneoff, &b.service, b.number, &b.name))
            });
            let mut services: Vec<String> = p
                .containers
                .iter()
                .filter(|c| !c.oneoff && !c.service.is_empty())
                .map(|c| c.service.clone())
                .collect();
            services.dedup();
            p.services = services;
            let (st, running, total) = state_of(&p.containers);
            p.state = st;
            p.running = running;
            p.total = total;
            p.missing_files = missing(&p.reference);
            p
        })
        .collect()
}

// ------------------------------------------------------------------ modelo

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ServicePort {
    pub published: Option<String>,
    pub target: String,
    pub protocol: String,
    pub host_ip: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ServiceMount {
    /// volume, bind, tmpfs…
    pub kind: String,
    pub source: Option<String>,
    pub target: String,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ServiceModel {
    pub name: String,
    pub image: Option<String>,
    /// Contexto de construcción, si el servicio se construye.
    pub build: Option<String>,
    pub ports: Vec<ServicePort>,
    pub mounts: Vec<ServiceMount>,
    pub depends_on: Vec<String>,
    pub profiles: Vec<String>,
    /// Solo los nombres de las variables: los valores pueden ser secretos.
    pub env_keys: Vec<String>,
    pub replicas: Option<u32>,
    pub container_name: Option<String>,
    /// Riesgos para el equipo de este servicio (E1, D-086): montajes, modo
    /// privilegiado, capacidades y red del equipo.
    pub risks: Vec<crate::risk::Risk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModelResource {
    /// Clave en el archivo.
    pub key: String,
    /// Nombre real en Docker (`proyecto_clave` salvo que se indique otro).
    pub name: String,
    pub external: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectModel {
    /// Nombre que resulta de los archivos (puede diferir si se forzó otro con `-p`).
    pub name: String,
    pub services: Vec<ServiceModel>,
    pub networks: Vec<ModelResource>,
    pub volumes: Vec<ModelResource>,
    /// Todos los perfiles que aparecen en los servicios.
    pub profiles: Vec<String>,
}

fn str_of(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn keys_or_items(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Object(m)) => m.keys().cloned().collect(),
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|x| x.as_str())
            .map(|s| s.split('=').next().unwrap_or(s).to_owned())
            .collect(),
        _ => vec![],
    }
}

fn resources(v: Option<&Value>, project: &str) -> Vec<ModelResource> {
    let Some(Value::Object(m)) = v else {
        return vec![];
    };
    m.iter()
        .map(|(k, r)| {
            let external = match r.get("external") {
                Some(Value::Bool(b)) => *b,
                Some(Value::Object(_)) => true,
                _ => false,
            };
            ModelResource {
                key: k.clone(),
                name: r
                    .get("name")
                    .and_then(str_of)
                    .unwrap_or_else(|| format!("{project}_{k}")),
                external,
            }
        })
        .collect()
}

/// Riesgos de un servicio de `compose config --format json` (D-086). La clave
/// de cada riesgo incluye el servicio: aceptarlo en uno no lo acepta en otro.
fn service_risks(service: &str, s: &Value, mounts: &[ServiceMount]) -> Vec<crate::risk::Risk> {
    use crate::risk;
    let field = |f: &str| format!("services.{service}.{f}");
    let mut out = Vec::new();
    for (i, m) in mounts.iter().enumerate() {
        if m.kind != "bind" {
            continue;
        }
        if let Some(src) = &m.source {
            out.extend(risk::mount_risk(
                src,
                m.read_only,
                false,
                &field(&format!("volumes.{i}")),
            ));
        }
    }
    if s.get("privileged").and_then(Value::as_bool) == Some(true) {
        out.push(risk::privileged_risk(&field("privileged")));
    }
    let caps: Vec<String> = s
        .get("cap_add")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(str_of).collect())
        .unwrap_or_default();
    out.extend(risk::cap_risks(&caps, &field("cap_add")));
    if let Some(n) = s.get("network_mode").and_then(str_of) {
        out.extend(risk::network_risk(&n, &field("network_mode")));
    }
    // E2 (D-088): espacios de nombres, opciones de seguridad, dispositivos,
    // cgroup y puertos en todas las interfaces.
    for (key, kind) in [
        ("pid", "pid"),
        ("ipc", "ipc"),
        ("userns_mode", "userns"),
        ("uts", "uts"),
        ("cgroup", "cgroup"),
    ] {
        if let Some(v) = s.get(key).and_then(str_of) {
            out.extend(risk::namespace_risk(kind, &v, &field(key)));
        }
    }
    let strings = |k: &str| -> Vec<String> {
        s.get(k)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(str_of).collect())
            .unwrap_or_default()
    };
    for o in strings("security_opt") {
        out.extend(risk::security_opt_risk(&o, &field("security_opt")));
    }
    if let Some(devs) = s.get("devices").and_then(Value::as_array) {
        for (i, d) in devs.iter().enumerate() {
            // Compose normaliza a objetos `{source, target, permissions}`; las
            // versiones antiguas dejan la cadena `origen:destino:permisos`.
            let src = d.get("source").and_then(str_of).or_else(|| str_of(d));
            if let Some(src) = src {
                out.extend(risk::device_risk(&src, &field(&format!("devices.{i}"))));
            }
        }
    }
    if let Some(c) = s.get("cgroup_parent").and_then(str_of) {
        out.extend(risk::cgroup_parent_risk(&c, &field("cgroup_parent")));
    }
    if s.get("network_mode").and_then(str_of).as_deref() != Some("host") {
        if let Some(ports) = s.get("ports").and_then(Value::as_array) {
            for (i, p) in ports.iter().enumerate() {
                let num = |k: &str| {
                    p.get(k)
                        .and_then(|v| str_of(v).or_else(|| v.as_u64().map(|n| n.to_string())))
                        .unwrap_or_default()
                };
                out.extend(risk::port_risk(
                    &num("host_ip"),
                    &num("published"),
                    &num("target"),
                    &field(&format!("ports.{i}")),
                ));
            }
        }
    }
    for r in &mut out {
        r.key = format!("{}@{service}", r.key);
        r.params.insert("service".into(), service.to_owned());
    }
    out
}

/// Riesgos de los servicios que va a arrancar `up` (todos si `services` está
/// vacío).
pub fn risks_for(model: &ProjectModel, services: &[String]) -> Vec<crate::risk::Risk> {
    model
        .services
        .iter()
        .filter(|s| services.is_empty() || services.contains(&s.name))
        .flat_map(|s| s.risks.iter().cloned())
        .collect()
}

/// Interpreta `docker compose config --format json`.
pub fn parse_model(json: &str) -> Result<ProjectModel, EngineError> {
    let v: Value = serde_json::from_str(json).map_err(|e| EngineError::InvalidResponse {
        message: format!("salida de compose config no válida: {e}"),
    })?;
    let name = v.get("name").and_then(str_of).unwrap_or_default();
    let mut services = Vec::new();
    let mut profiles: Vec<String> = Vec::new();
    if let Some(Value::Object(svcs)) = v.get("services") {
        for (sname, s) in svcs {
            let ports = s
                .get("ports")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .map(|p| match p {
                            Value::Object(_) => ServicePort {
                                published: p
                                    .get("published")
                                    .and_then(str_of)
                                    .filter(|x| !x.is_empty()),
                                target: p.get("target").and_then(str_of).unwrap_or_default(),
                                protocol: p
                                    .get("protocol")
                                    .and_then(str_of)
                                    .unwrap_or_else(|| "tcp".into()),
                                host_ip: p
                                    .get("host_ip")
                                    .and_then(str_of)
                                    .filter(|x| !x.is_empty()),
                            },
                            other => ServicePort {
                                published: None,
                                target: str_of(other).unwrap_or_default(),
                                protocol: "tcp".into(),
                                host_ip: None,
                            },
                        })
                        .collect()
                })
                .unwrap_or_default();
            let mounts: Vec<ServiceMount> = s
                .get("volumes")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|m| {
                            Some(ServiceMount {
                                kind: m
                                    .get("type")
                                    .and_then(str_of)
                                    .unwrap_or_else(|| "volume".into()),
                                source: m.get("source").and_then(str_of).filter(|x| !x.is_empty()),
                                target: m.get("target").and_then(str_of)?,
                                read_only: m
                                    .get("read_only")
                                    .and_then(Value::as_bool)
                                    .unwrap_or(false),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            let svc_profiles: Vec<String> = s
                .get("profiles")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(str_of).collect())
                .unwrap_or_default();
            for p in &svc_profiles {
                if !profiles.contains(p) {
                    profiles.push(p.clone());
                }
            }
            let build = match s.get("build") {
                Some(Value::Object(b)) => b.get("context").and_then(str_of).or(Some(".".into())),
                Some(Value::String(c)) => Some(c.clone()),
                _ => None,
            };
            let mut env_keys = keys_or_items(s.get("environment"));
            env_keys.sort();
            let risks = service_risks(sname, s, &mounts);
            services.push(ServiceModel {
                name: sname.clone(),
                image: s.get("image").and_then(str_of),
                build,
                ports,
                mounts,
                depends_on: keys_or_items(s.get("depends_on")),
                profiles: svc_profiles,
                env_keys,
                replicas: s
                    .get("deploy")
                    .and_then(|d| d.get("replicas"))
                    .and_then(Value::as_u64)
                    .or_else(|| s.get("scale").and_then(Value::as_u64))
                    .map(|n| n as u32),
                container_name: s.get("container_name").and_then(str_of),
                risks,
            });
        }
    }
    services.sort_by(|a, b| a.name.cmp(&b.name));
    profiles.sort();
    Ok(ProjectModel {
        networks: resources(v.get("networks"), &name),
        volumes: resources(v.get("volumes"), &name),
        name,
        services,
        profiles,
    })
}

// ------------------------------------------------------------------ acciones

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PullPolicy {
    /// Descarga solo las imágenes que faltan (lo que hace Compose por defecto).
    #[default]
    Missing,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DownImages {
    #[default]
    None,
    /// Solo las imágenes sin etiqueta personalizada (construidas por Compose).
    Local,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ComposeAction {
    Up {
        #[serde(default)]
        services: Vec<String>,
        #[serde(default)]
        build: bool,
        #[serde(default)]
        pull: PullPolicy,
        #[serde(default)]
        force_recreate: bool,
        #[serde(default)]
        remove_orphans: bool,
        #[serde(default)]
        profiles: Vec<String>,
    },
    Down {
        #[serde(default)]
        volumes: bool,
        #[serde(default)]
        images: DownImages,
        #[serde(default)]
        remove_orphans: bool,
    },
    Start {
        #[serde(default)]
        services: Vec<String>,
    },
    Stop {
        #[serde(default)]
        services: Vec<String>,
    },
    Restart {
        #[serde(default)]
        services: Vec<String>,
    },
    Pull {
        #[serde(default)]
        services: Vec<String>,
    },
    Build {
        #[serde(default)]
        services: Vec<String>,
        #[serde(default)]
        no_cache: bool,
        #[serde(default)]
        pull: bool,
    },
}

impl ComposeAction {
    pub fn verb(&self) -> &'static str {
        match self {
            Self::Up { .. } => "up",
            Self::Down { .. } => "down",
            Self::Start { .. } => "start",
            Self::Stop { .. } => "stop",
            Self::Restart { .. } => "restart",
            Self::Pull { .. } => "pull",
            Self::Build { .. } => "build",
        }
    }

    pub fn kind(&self) -> String {
        format!("compose.{}", self.verb())
    }

    /// Necesita los archivos del proyecto (las demás trabajan con las etiquetas).
    pub fn needs_files(&self) -> bool {
        matches!(
            self,
            Self::Up { .. } | Self::Pull { .. } | Self::Build { .. }
        )
    }

    /// Crea, arranca o para contenedores: tras ella se confirma el estado real
    /// de cada contenedor con inspect (D-056). `down` los elimina y descargar o
    /// construir no tocan contenedores.
    pub fn changes_state(&self) -> bool {
        matches!(
            self,
            Self::Up { .. } | Self::Start { .. } | Self::Stop { .. } | Self::Restart { .. }
        )
    }

    /// Elimina recursos: exige confirmación explícita.
    pub fn destructive(&self) -> bool {
        matches!(self, Self::Down { .. })
    }

    fn services(&self) -> &[String] {
        match self {
            Self::Up { services, .. }
            | Self::Start { services }
            | Self::Stop { services }
            | Self::Restart { services }
            | Self::Pull { services }
            | Self::Build { services, .. } => services,
            Self::Down { .. } => &[],
        }
    }

    pub fn profiles(&self) -> &[String] {
        match self {
            Self::Up { profiles, .. } => profiles,
            _ => &[],
        }
    }

    /// Actúa sobre todos los contenedores del proyecto, también los de perfiles
    /// no activos. Con archivos, Compose (2.26 y 5.6) los omite en silencio y
    /// termina con código 0: `down` dejaba un servicio de perfil en marcha y su
    /// red "en uso" (D-054).
    pub fn all_profiles(&self) -> bool {
        matches!(
            self,
            Self::Down { .. } | Self::Stop { .. } | Self::Start { .. } | Self::Restart { .. }
        )
    }

    pub fn validate(&self) -> Result<(), EngineError> {
        for s in self.services() {
            if !valid_service_name(s) {
                return Err(invalid(format!("nombre de servicio no válido: {s:?}")));
            }
        }
        for p in self.profiles() {
            if !valid_service_name(p) {
                return Err(invalid(format!("nombre de perfil no válido: {p:?}")));
            }
        }
        if self.services().len() > 500 || self.profiles().len() > 100 {
            return Err(invalid("demasiados servicios o perfiles"));
        }
        Ok(())
    }

    /// Argumentos tras las opciones globales (los perfiles son opción global y
    /// los añade [`action_args`]).
    pub fn sub_args(&self) -> Vec<String> {
        let mut a: Vec<String> = vec![self.verb().into()];
        match self {
            Self::Up {
                build,
                pull,
                force_recreate,
                remove_orphans,
                ..
            } => {
                a.push("--detach".into());
                if *build {
                    a.push("--build".into());
                }
                match pull {
                    PullPolicy::Missing => {}
                    PullPolicy::Always => a.extend(["--pull".into(), "always".into()]),
                    PullPolicy::Never => a.extend(["--pull".into(), "never".into()]),
                }
                if *force_recreate {
                    a.push("--force-recreate".into());
                }
                if *remove_orphans {
                    a.push("--remove-orphans".into());
                }
            }
            Self::Down {
                volumes,
                images,
                remove_orphans,
            } => {
                if *volumes {
                    a.push("--volumes".into());
                }
                match images {
                    DownImages::None => {}
                    DownImages::Local => a.extend(["--rmi".into(), "local".into()]),
                    DownImages::All => a.extend(["--rmi".into(), "all".into()]),
                }
                if *remove_orphans {
                    a.push("--remove-orphans".into());
                }
            }
            Self::Build { no_cache, pull, .. } => {
                if *no_cache {
                    a.push("--no-cache".into());
                }
                if *pull {
                    a.push("--pull".into());
                }
            }
            Self::Start { .. } | Self::Stop { .. } | Self::Restart { .. } | Self::Pull { .. } => {}
        }
        let svcs = self.services();
        if !svcs.is_empty() {
            // `--` cierra las opciones: un servicio nunca se interpreta como opción.
            a.push("--".into());
            a.extend(svcs.iter().cloned());
        }
        a
    }
}

/// Argumentos completos (sin el programa) de una acción sobre un proyecto.
pub fn action_args(cli: &ComposeCli, project: &ProjectRef, action: &ComposeAction) -> Vec<String> {
    action_args_with(cli, project, action, project.missing_files().is_empty())
}

/// Como [`action_args`], sabiendo si están todos los archivos (comprobado
/// donde están: en el equipo o en el servidor remoto).
pub fn action_args_with(
    cli: &ComposeCli,
    project: &ProjectRef,
    action: &ComposeAction,
    files_present: bool,
) -> Vec<String> {
    let with_files = action.needs_files() || files_present && !project.files.is_empty();
    let mut a = cli.prefix.clone();
    a.extend(project.global_args(with_files, None));
    if action.all_profiles() {
        a.extend(["--profile".into(), "*".into()]);
    }
    for p in action.profiles() {
        a.push("--profile".into());
        a.push(p.clone());
    }
    a.extend(action.sub_args());
    a
}

/// Argumentos de `config --format json` con todos los perfiles activos, para
/// conocer todos los servicios del archivo.
pub fn config_args(
    cli: &ComposeCli,
    project: &ProjectRef,
    stdin_file: Option<usize>,
) -> Vec<String> {
    let mut a = cli.prefix.clone();
    a.extend(project.global_args(true, stdin_file));
    a.extend([
        "--profile".into(),
        "*".into(),
        "config".into(),
        "--format".into(),
        "json".into(),
    ]);
    a
}

/// Argumentos de `logs --follow`.
pub fn logs_args(
    cli: &ComposeCli,
    project: &ProjectRef,
    services: &[String],
    tail: u32,
    timestamps: bool,
) -> Vec<String> {
    logs_args_with(
        cli,
        project,
        services,
        tail,
        timestamps,
        project.missing_files().is_empty(),
    )
}

/// Como [`logs_args`], sabiendo si están todos los archivos.
pub fn logs_args_with(
    cli: &ComposeCli,
    project: &ProjectRef,
    services: &[String],
    tail: u32,
    timestamps: bool,
    files_present: bool,
) -> Vec<String> {
    let with_files = files_present && !project.files.is_empty();
    let mut a = cli.prefix.clone();
    a.extend(project.global_args(with_files, None));
    a.extend([
        "logs".into(),
        "--follow".into(),
        "--no-color".into(),
        "--tail".into(),
        tail.to_string(),
    ]);
    if timestamps {
        a.push("--timestamps".into());
    }
    if !services.is_empty() {
        a.push("--".into());
        a.extend(services.iter().cloned());
    }
    a
}

#[cfg(test)]
mod tests {

    /// E1 (D-086, §9.5): servicios legítimos de Compose sin riesgos; los
    /// peligrosos, con su clave por servicio.
    #[test]
    fn compose_services_risks_without_false_positives() {
        let json = r#"{"name":"demo","services":{
          "web":{"image":"nginx:1.27","volumes":[{"type":"bind","source":"/home/ana/demo/html","target":"/usr/share/nginx/html","read_only":true},{"type":"tmpfs","target":"/tmp"}],"ports":[{"host_ip":"127.0.0.1","published":"8080","target":80,"protocol":"tcp"}],"networks":{"front":null}},
          "db":{"image":"postgres:16","volumes":[{"type":"volume","source":"pgdata","target":"/var/lib/postgresql/data"}],"cap_drop":["ALL"],"cap_add":["CHOWN","SETUID","SETGID","DAC_READ_SEARCH_NOT"]},
          "proxy":{"image":"traefik:3","volumes":[{"type":"bind","source":"/var/run/docker.sock","target":"/var/run/docker.sock","read_only":true}],"network_mode":"host"},
          "tools":{"image":"busybox","privileged":true,"cap_add":["SYS_ADMIN","NET_RAW"]}
        },"networks":{"front":{}},"volumes":{"pgdata":{}}}"#;
        let m = parse_model(json).unwrap();
        let of = |n: &str| {
            m.services
                .iter()
                .find(|s| s.name == n)
                .unwrap()
                .risks
                .clone()
        };
        assert!(of("web").is_empty(), "{:?}", of("web"));
        assert!(of("db").is_empty(), "{:?}", of("db"));
        let keys: Vec<String> = of("proxy").iter().map(|r| r.key.clone()).collect();
        assert_eq!(
            keys,
            [
                "risk.runtimeSocket:/var/run/docker.sock@proxy",
                "risk.networkHost:host@proxy"
            ]
        );
        let tools: Vec<(String, crate::risk::RiskLevel)> = of("tools")
            .iter()
            .map(|r| (r.code.clone(), r.level))
            .collect();
        use crate::risk::RiskLevel::*;
        assert_eq!(
            tools,
            [
                ("risk.privileged".to_owned(), Critical),
                ("risk.capability".to_owned(), Critical),
                ("risk.capability".to_owned(), Low)
            ]
        );
        assert_eq!(risks_for(&m, &["web".into(), "db".into()]).len(), 0);
        assert_eq!(risks_for(&m, &[]).len(), 5);
    }

    /// E2 (D-088): espacios de nombres del equipo, opciones de seguridad,
    /// dispositivos, cgroup y puertos en todas las interfaces.
    #[test]
    fn compose_namespaces_security_devices_and_ports() {
        let json = r#"{"name":"demo","services":{
          "mon":{"image":"x","pid":"host","ipc":"host","userns_mode":"host","uts":"host","cgroup":"host",
                 "security_opt":["seccomp:unconfined","apparmor=unconfined","label:disable","no-new-privileges:true"],
                 "devices":[{"source":"/dev/sda","target":"/dev/sda","permissions":"rwm"},"/dev/dri:/dev/dri"],
                 "cgroup_parent":"grupo","ports":[{"published":"9100","target":9100,"protocol":"tcp"},{"host_ip":"0.0.0.0","published":"81","target":80},{"host_ip":"127.0.0.1","published":"82","target":80}]},
          "ok":{"image":"x","pid":"service:mon","ipc":"shareable","security_opt":["no-new-privileges:true"],"devices":[]}
        }}"#;
        let m = parse_model(json).unwrap();
        let of = |n: &str| {
            m.services
                .iter()
                .find(|s| s.name == n)
                .unwrap()
                .risks
                .clone()
        };
        assert!(of("ok").is_empty(), "{:?}", of("ok"));
        use crate::risk::RiskLevel::*;
        let got: Vec<(String, crate::risk::RiskLevel)> =
            of("mon").iter().map(|r| (r.key.clone(), r.level)).collect();
        assert_eq!(
            got,
            vec![
                ("risk.hostNamespace:pid@mon".to_owned(), High),
                ("risk.hostNamespace:ipc@mon".to_owned(), High),
                ("risk.hostNamespace:userns@mon".to_owned(), Medium),
                ("risk.hostNamespace:uts@mon".to_owned(), Low),
                ("risk.hostNamespace:cgroup@mon".to_owned(), Medium),
                ("risk.securityOpt:seccomp=unconfined@mon".to_owned(), High),
                ("risk.securityOpt:apparmor=unconfined@mon".to_owned(), High),
                ("risk.securityOpt:label=disable@mon".to_owned(), High),
                ("risk.device:/dev/sda@mon".to_owned(), High),
                ("risk.device:/dev/dri@mon".to_owned(), Low),
                ("risk.cgroupParent:grupo@mon".to_owned(), Low),
                ("risk.portAllInterfaces:9100@mon".to_owned(), Low),
                ("risk.portAllInterfaces:81@mon".to_owned(), Low),
            ]
        );
    }

    use super::*;

    fn cli() -> ComposeCli {
        ComposeCli {
            program: "/usr/bin/docker".into(),
            prefix: vec!["compose".into()],
            version: "5.6.1".into(),
        }
    }

    fn proj(files: &[&str]) -> ProjectRef {
        ProjectRef {
            name: "tienda".into(),
            files: files.iter().map(|s| s.to_string()).collect(),
            working_dir: None,
            env_files: vec![],
        }
    }

    #[test]
    fn project_names_follow_compose_rules() {
        for ok in ["web", "mi-app_2", "0x"] {
            assert!(valid_project_name(ok), "{ok}");
        }
        for bad in ["", "Web", "-x", "_x", "a b", "a.b", "--rm"] {
            assert!(!valid_project_name(bad), "{bad}");
        }
        assert_eq!(
            project_name_from_dir(Path::new("/home/ana/Mi Tienda.v2")),
            "mitiendav2"
        );
        assert_eq!(project_name_from_dir(Path::new("/srv/_demo")), "demo");
    }

    #[test]
    fn service_names_cannot_look_like_options() {
        assert!(valid_service_name("web.1"));
        assert!(!valid_service_name("-d"));
        assert!(!valid_service_name("--rmi=all"));
        assert!(!valid_service_name("a b"));
        let a = ComposeAction::Stop {
            services: vec!["--volumes".into()],
        };
        assert!(a.validate().is_err());
    }

    #[test]
    fn project_ref_validation() {
        assert!(proj(&["/srv/tienda/compose.yaml"]).validate().is_ok());
        assert!(proj(&["compose.yaml"]).validate().is_err(), "ruta relativa");
        let mut p = proj(&[]);
        p.name = "Tienda".into();
        assert!(p.validate().is_err());
    }

    #[test]
    fn up_arguments_are_exact() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("compose.yaml");
        std::fs::write(&f, "services: {}\n").unwrap();
        let p = proj(&[f.to_str().unwrap()]);
        let a = action_args(
            &cli(),
            &p,
            &ComposeAction::Up {
                services: vec!["web".into(), "db".into()],
                build: true,
                pull: PullPolicy::Always,
                force_recreate: false,
                remove_orphans: true,
                profiles: vec!["debug".into()],
            },
        );
        let dir = d.path().display().to_string();
        let fs = f.display().to_string();
        assert_eq!(
            a,
            [
                "compose",
                "--ansi",
                "never",
                "-p",
                "tienda",
                "-f",
                &fs,
                "--project-directory",
                &dir,
                "--profile",
                "debug",
                "up",
                "--detach",
                "--build",
                "--pull",
                "always",
                "--remove-orphans",
                "--",
                "web",
                "db"
            ]
        );
    }

    #[test]
    fn lifecycle_actions_work_without_files_when_they_are_gone() {
        let p = proj(&["/no/existe/compose.yaml"]);
        let a = action_args(&cli(), &p, &ComposeAction::Stop { services: vec![] });
        assert_eq!(
            a,
            [
                "compose",
                "--ansi",
                "never",
                "-p",
                "tienda",
                "--profile",
                "*",
                "stop"
            ]
        );
        let down = action_args(
            &cli(),
            &p,
            &ComposeAction::Down {
                volumes: true,
                images: DownImages::Local,
                remove_orphans: false,
            },
        );
        assert_eq!(
            down,
            [
                "compose",
                "--ansi",
                "never",
                "-p",
                "tienda",
                "--profile",
                "*",
                "down",
                "--volumes",
                "--rmi",
                "local"
            ]
        );
        assert!(!ComposeAction::Stop { services: vec![] }.needs_files());
        let pull = action_args(&cli(), &p, &ComposeAction::Pull { services: vec![] });
        assert!(
            !pull.contains(&"*".to_string()),
            "descargar sigue los perfiles activos, como desplegar"
        );
        assert!(ComposeAction::Pull { services: vec![] }.needs_files());
    }

    #[test]
    fn standalone_binary_has_no_prefix_and_stdin_replaces_one_file() {
        let sa = ComposeCli {
            program: "/usr/bin/docker-compose".into(),
            prefix: vec![],
            version: "2.26.1".into(),
        };
        assert_eq!(sa.display(), "docker-compose");
        assert_eq!(cli().display(), "docker compose");
        let mut p = proj(&["/srv/t/compose.yaml", "/srv/t/override.yaml"]);
        p.env_files = vec!["/srv/t/.env.prod".into()];
        assert_eq!(
            config_args(&sa, &p, Some(1)),
            [
                "--ansi",
                "never",
                "-p",
                "tienda",
                "-f",
                "/srv/t/compose.yaml",
                "-f",
                "-",
                "--project-directory",
                "/srv/t",
                "--env-file",
                "/srv/t/.env.prod",
                "--profile",
                "*",
                "config",
                "--format",
                "json"
            ]
        );
    }

    #[test]
    fn logs_arguments() {
        let p = proj(&[]);
        assert_eq!(
            logs_args(&cli(), &p, &["web".into()], 200, true),
            [
                "compose",
                "--ansi",
                "never",
                "-p",
                "tienda",
                "logs",
                "--follow",
                "--no-color",
                "--tail",
                "200",
                "--timestamps",
                "--",
                "web"
            ]
        );
    }

    fn container(
        name: &str,
        project: &str,
        service: &str,
        state: &str,
        oneoff: bool,
    ) -> ContainerSummary {
        let mut labels = BTreeMap::new();
        labels.insert(LABEL_PROJECT.into(), project.into());
        labels.insert(LABEL_SERVICE.into(), service.into());
        labels.insert(
            LABEL_CONFIG_FILES.into(),
            "/srv/a/compose.yaml,/srv/a/extra.yaml".into(),
        );
        labels.insert(LABEL_WORKING_DIR.into(), "/srv/a".into());
        labels.insert(LABEL_NUMBER.into(), "1".into());
        labels.insert(
            LABEL_ONEOFF.into(),
            if oneoff { "True" } else { "False" }.into(),
        );
        labels.insert(LABEL_VERSION.into(), "5.6.1".into());
        ContainerSummary {
            id: format!("id-{name}"),
            name: name.into(),
            image: "img".into(),
            image_id: String::new(),
            command: String::new(),
            created: 0,
            state: state.into(),
            status: String::new(),
            ports: vec![],
            networks: vec![],
            mounts: 0,
            mount_points: vec![],
            compose_project: Some(project.into()),
            labels,
        }
    }

    #[test]
    fn projects_are_discovered_from_labels_and_merged_with_registered() {
        let mut plain = container("suelto", "x", "x", "running", false);
        plain.labels.clear();
        let list = [
            container("a-web-1", "a", "web", "running", false),
            container("a-db-1", "a", "db", "exited", false),
            container("a-web-run-1", "a", "web", "running", true),
            container("b-api-1", "b", "api", "running", false),
            plain,
        ];
        let reg = [
            ProjectRef {
                name: "b".into(),
                files: vec!["/home/ana/b/compose.yaml".into()],
                working_dir: None,
                env_files: vec![],
            },
            ProjectRef {
                name: "nuevo".into(),
                files: vec!["/home/ana/nuevo/compose.yaml".into()],
                working_dir: None,
                env_files: vec![],
            },
        ];
        let ps = projects_from(&list, &reg);
        assert_eq!(
            ps.iter()
                .map(|p| p.reference.name.as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "nuevo"]
        );
        let a = &ps[0];
        assert_eq!(
            a.reference.files,
            ["/srv/a/compose.yaml", "/srv/a/extra.yaml"]
        );
        assert_eq!(a.reference.working_dir.as_deref(), Some("/srv/a"));
        assert_eq!(
            (a.state, a.running, a.total),
            (ProjectState::Partial, 1, 2),
            "el contenedor de run no cuenta"
        );
        assert_eq!(a.services, ["db", "web"]);
        assert!(!a.registered);
        assert_eq!(a.created_by.as_deref(), Some("5.6.1"));
        let b = &ps[1];
        assert!(b.registered);
        assert_eq!(
            b.reference.files,
            ["/home/ana/b/compose.yaml"],
            "manda el archivo registrado"
        );
        assert_eq!(b.state, ProjectState::Running);
        let n = &ps[2];
        assert_eq!(n.state, ProjectState::NotDeployed);
        assert_eq!(n.missing_files, ["/home/ana/nuevo/compose.yaml"]);
    }

    fn pc(service: &str, state: &str, oneoff: bool) -> ProjectContainer {
        ProjectContainer {
            id: format!("id-{service}-{state}"),
            name: format!("p-{service}-1"),
            service: service.into(),
            number: Some(1),
            state: state.into(),
            status: String::new(),
            image: String::new(),
            oneoff,
        }
    }

    #[test]
    fn project_state_distinguishes_every_case() {
        use ProjectState::*;
        // Sin contenedores: sin desplegar (proyecto añadido o eliminado).
        assert_eq!(state_of(&[]), (NotDeployed, 0, 0));
        // Todos en marcha.
        assert_eq!(
            state_of(&[pc("web", "running", false), pc("db", "running", false)]),
            (Running, 2, 2)
        );
        // Todos detenidos (también creados sin arrancar, `up --no-start`).
        assert_eq!(
            state_of(&[pc("web", "exited", false), pc("db", "created", false)]),
            (Stopped, 0, 2)
        );
        // Parcial: nunca "detenido" mientras quede alguno en marcha.
        assert_eq!(
            state_of(&[pc("web", "exited", false), pc("db", "running", false)]),
            (Partial, 1, 2)
        );
        // En pausa o reiniciándose no cuentan como en marcha.
        assert_eq!(
            state_of(&[pc("web", "paused", false), pc("db", "running", false)]),
            (Partial, 1, 2)
        );
        assert_eq!(state_of(&[pc("web", "restarting", false)]), (Stopped, 0, 1));
        // Los contenedores de `docker compose run` no cuentan.
        assert_eq!(
            state_of(&[pc("web", "exited", false), pc("web", "running", true)]),
            (Stopped, 0, 1)
        );
        assert_eq!(state_of(&[pc("web", "running", true)]), (NotDeployed, 0, 0));
    }

    #[test]
    fn profiles_missing_files_and_uncreated_services_in_discovery() {
        let mut list = vec![
            container("t-web-1", "t", "web", "running", false),
            container("t-db-1", "t", "db", "exited", false),
            // Servicio de un perfil, ya desplegado: cuenta como cualquier otro.
            container("t-extra-1", "t", "extra", "running", false),
        ];
        // Archivos que ya no existen: el estado se calcula igual, con las etiquetas.
        for c in &mut list {
            c.labels
                .insert(LABEL_CONFIG_FILES.into(), "/no/existe/compose.yaml".into());
        }
        let p = &projects_from(&list, &[])[0];
        assert_eq!((p.state, p.running, p.total), (ProjectState::Partial, 2, 3));
        assert_eq!(p.missing_files, ["/no/existe/compose.yaml"]);
        assert_eq!(p.services, ["db", "extra", "web"]);
        // Un servicio del archivo que aún no tiene contenedor no altera el estado:
        // el proyecto está "en marcha" si lo están todos los contenedores que existen.
        let only_running: Vec<_> = list.into_iter().filter(|c| c.state == "running").collect();
        let p = &projects_from(&only_running, &[])[0];
        assert_eq!((p.state, p.running, p.total), (ProjectState::Running, 2, 2));
    }

    #[test]
    fn model_is_parsed_from_real_compose_output() {
        // Salida real de Compose 5.6.1 (recortada) con perfiles activados.
        let json = r#"{
          "name": "demo7",
          "networks": {"default": {"name": "demo7_default", "ipam": {}}, "ext": {"name": "compartida", "external": true}},
          "services": {
            "db": {"build": {"context": "/srv/demo", "dockerfile": "Dockerfile"}, "networks": {"default": null}},
            "web": {
              "image": "nginx:1.27",
              "environment": {"PLAIN": "hola", "SECRET": "supersecreto"},
              "volumes": [
                {"type": "volume", "source": "datos", "target": "/data", "volume": {}},
                {"type": "bind", "source": "/srv/demo", "target": "/src", "read_only": true, "bind": {}}
              ],
              "depends_on": {"db": {"condition": "service_started", "required": true}},
              "ports": [{"mode": "ingress", "target": 80, "published": "18080", "protocol": "tcp"}],
              "deploy": {"replicas": 2}
            },
            "extra": {"image": "busybox", "profiles": ["debug", "test"]}
          },
          "volumes": {"datos": {"name": "demo7_datos"}}
        }"#;
        let m = parse_model(json).unwrap();
        assert_eq!(m.name, "demo7");
        assert_eq!(
            m.services
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>(),
            ["db", "extra", "web"]
        );
        let db = &m.services[0];
        assert_eq!(db.build.as_deref(), Some("/srv/demo"));
        assert_eq!(db.image, None);
        let web = &m.services[2];
        assert_eq!(web.env_keys, ["PLAIN", "SECRET"]);
        assert!(
            !format!("{m:?}").contains("supersecreto"),
            "los valores no llegan al modelo"
        );
        assert_eq!(
            web.ports,
            [ServicePort {
                published: Some("18080".into()),
                target: "80".into(),
                protocol: "tcp".into(),
                host_ip: None
            }]
        );
        assert_eq!(
            web.mounts[1],
            ServiceMount {
                kind: "bind".into(),
                source: Some("/srv/demo".into()),
                target: "/src".into(),
                read_only: true
            }
        );
        assert_eq!(web.depends_on, ["db"]);
        assert_eq!(web.replicas, Some(2));
        assert_eq!(m.profiles, ["debug", "test"]);
        assert_eq!(
            m.networks,
            [
                ModelResource {
                    key: "default".into(),
                    name: "demo7_default".into(),
                    external: false
                },
                ModelResource {
                    key: "ext".into(),
                    name: "compartida".into(),
                    external: true
                }
            ]
        );
        assert_eq!(m.volumes[0].name, "demo7_datos");
        assert!(parse_model("no es json").is_err());
    }
}
