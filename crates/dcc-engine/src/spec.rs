//! Especificación de un contenedor nuevo (asistente de creación y duplicado).
//!
//! Fuente única de verdad (D-034): de un `ContainerSpec` salen la validación,
//! el cuerpo de `POST /containers/create` y el comando `docker` equivalente.
//! La interfaz nunca construye ninguno de los dos por su cuenta.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::IpAddr;
use std::path::Path;

use bollard::models::{
    ContainerCreateBody, ContainerInspectResponse, HostConfig, ImageConfig, Mount, MountType,
    PortBinding, RestartPolicy, RestartPolicyNameEnum,
};
use dcc_activity::quote::render_argv;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::risk::{self, Risk};

pub const MIB: u64 = 1024 * 1024;
pub const GIB: u64 = 1024 * MIB;
/// Mínimo de memoria que acepta Docker.
pub const MIN_MEMORY: u64 = 6 * MIB;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KeyValue {
    pub key: String,
    pub value: String,
}

impl KeyValue {
    pub fn new(k: impl Into<String>, v: impl Into<String>) -> Self {
        Self {
            key: k.into(),
            value: v.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Protocol {
    #[default]
    Tcp,
    Udp,
    Sctp,
}

impl Protocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
            Self::Sctp => "sctp",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        match s {
            "tcp" => Some(Self::Tcp),
            "udp" => Some(Self::Udp),
            "sctp" => Some(Self::Sctp),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PortSpec {
    /// IP del equipo donde escuchar; vacío = todas.
    pub host_ip: String,
    /// Puerto del equipo; `None` = Docker elige uno libre.
    pub host_port: Option<u16>,
    pub container_port: u16,
    pub protocol: Protocol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MountKind {
    /// Volumen gestionado por Docker (con nombre o anónimo).
    #[default]
    Volume,
    /// Carpeta del equipo (bind mount).
    Bind,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MountSpec {
    pub kind: MountKind,
    /// Nombre del volumen (vacío = anónimo) o ruta absoluta del equipo.
    pub source: String,
    /// Ruta absoluta dentro del contenedor.
    pub target: String,
    pub read_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RestartName {
    #[default]
    No,
    Always,
    UnlessStopped,
    OnFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RestartSpec {
    pub name: RestartName,
    /// Solo con `onFailure`; 0 = sin límite.
    pub max_retries: u32,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct ContainerSpec {
    /// Vacío = Docker genera un nombre.
    pub name: String,
    pub image: String,
    /// Vacío = el de la imagen.
    pub command: Vec<String>,
    /// Vacío = el de la imagen.
    pub entrypoint: Vec<String>,
    pub working_dir: String,
    pub user: String,
    pub hostname: String,
    pub env: Vec<KeyValue>,
    pub labels: Vec<KeyValue>,
    /// Vacío = red por defecto (bridge).
    pub network: String,
    pub ports: Vec<PortSpec>,
    pub publish_all: bool,
    pub mounts: Vec<MountSpec>,
    pub cpus: Option<f64>,
    #[ts(type = "number | null")]
    pub memory_bytes: Option<u64>,
    #[ts(type = "number | null")]
    pub memory_reservation_bytes: Option<u64>,
    pub restart: RestartSpec,
    pub privileged: bool,
    pub auto_remove: bool,
    pub init: bool,
    pub tty: bool,
    pub interactive: bool,
    pub read_only_rootfs: bool,
    pub cap_add: Vec<String>,
    pub cap_drop: Vec<String>,
    /// `key` = nombre de host, `value` = IP (o `host-gateway`).
    pub extra_hosts: Vec<KeyValue>,
    /// Riesgos altos o críticos que el usuario ha aceptado explícitamente
    /// (`Risk::key`, E1, D-086). No se envía a Docker.
    pub accepted_risks: Vec<String>,
}

// ---------------------------------------------------------------- validación

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Severity {
    Error,
    Warning,
}

/// Problema encontrado en la especificación. `code` es estable y traducible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SpecIssue {
    pub severity: Severity,
    /// Campo afectado, p. ej. `ports.0.hostPort`.
    pub field: String,
    pub code: String,
    pub params: BTreeMap<String, String>,
}

pub(crate) fn issue(
    sev: Severity,
    field: impl Into<String>,
    code: &str,
    params: &[(&str, String)],
) -> SpecIssue {
    SpecIssue {
        severity: sev,
        field: field.into(),
        code: code.into(),
        params: params
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect(),
    }
}

fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 255
        && s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

/// Referencia de imagen: solo caracteres válidos en `repo/nombre:tag@digest`.
pub fn valid_image_ref(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 255
        && s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':' | '/' | '@'))
        && !s.contains("..")
        && !s.ends_with(':')
        && !s.ends_with('/')
}

fn absolute(p: &str) -> bool {
    p.starts_with('/') && !p.split('/').any(|c| c == "..") && !p.contains('\0')
}

/// Rutas del equipo cuyo montaje da control sobre el sistema.
/// Valor que sustituye a los secretos en lo que se muestra o se registra.
pub use dcc_activity::redact::REDACTED;

/// Variables cuyo valor se oculta al mostrarlo o registrarlo (no al enviarlo a
/// Docker). Reglas únicas en `dcc_activity::redact` (E1, D-085); la interfaz
/// usa las mismas (`ui/src/shared/secrets.ts`).
pub fn is_secret_key(key: &str) -> bool {
    dcc_activity::redact::is_secret_name(key)
}

/// Valor de una variable tal como se muestra o se registra: oculto si el
/// nombre es secreto o si contiene credenciales (URL con contraseña).
pub fn shown_value(key: &str, value: &str) -> String {
    dcc_activity::redact::redact_value(key, value)
}

impl ContainerSpec {
    /// Riesgos para el equipo de esta configuración (E1, D-086): montajes,
    /// modo privilegiado, capacidades y red del equipo. `local`: Docker está en
    /// este equipo (se resuelven los enlaces simbólicos de los montajes).
    pub fn risks(&self, local: bool) -> Vec<Risk> {
        let mut out = Vec::new();
        for (i, m) in self.mounts.iter().enumerate() {
            if m.kind == MountKind::Bind {
                if let Some(r) = risk::mount_risk(
                    m.source.trim(),
                    m.read_only,
                    local,
                    &format!("mounts.{i}.source"),
                ) {
                    out.push(r);
                }
            }
        }
        if self.privileged {
            out.push(risk::privileged_risk("privileged"));
        }
        out.extend(risk::cap_risks(&self.cap_add, "capAdd"));
        if let Some(r) = risk::network_risk(&self.network, "network") {
            out.push(r);
        }
        // Con la red del equipo no se publican puertos: ya se avisa arriba.
        if self.network.trim() != "host" {
            for (i, p) in self.ports.iter().enumerate() {
                out.extend(risk::port_risk(
                    &p.host_ip,
                    &p.host_port.map(|n| n.to_string()).unwrap_or_default(),
                    &p.container_port.to_string(),
                    &format!("ports.{i}.hostIp"),
                ));
            }
        }
        out
    }

    /// Valida la especificación. `check_fs` comprueba que existan las carpetas
    /// de los bind mounts (el daemon local ve el mismo sistema de archivos).
    pub fn validate(&self, check_fs: bool) -> Vec<SpecIssue> {
        use Severity::*;
        let mut v = Vec::new();
        let name = self.name.trim();
        if !name.is_empty() && !valid_name(name) {
            v.push(issue(Error, "name", "name.invalid", &[]));
        }
        let image = self.image.trim();
        if image.is_empty() {
            v.push(issue(Error, "image", "image.required", &[]));
        } else if !valid_image_ref(image) {
            v.push(issue(Error, "image", "image.invalid", &[]));
        }
        for (field, list) in [("command", &self.command), ("entrypoint", &self.entrypoint)] {
            if list.iter().any(|a| a.contains('\0')) {
                v.push(issue(Error, field, "argv.nul", &[]));
            }
        }
        if !self.working_dir.trim().is_empty() && !absolute(self.working_dir.trim()) {
            v.push(issue(Error, "workingDir", "path.notAbsolute", &[]));
        }
        if self.user.contains(char::is_whitespace) || self.user.contains('\0') {
            v.push(issue(Error, "user", "user.invalid", &[]));
        }
        let hn = self.hostname.trim();
        if !hn.is_empty()
            && (hn.len() > 63
                || !hn
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
                || hn.starts_with('-'))
        {
            v.push(issue(Error, "hostname", "hostname.invalid", &[]));
        }

        let mut seen = HashSet::new();
        for (i, kv) in self.env.iter().enumerate() {
            let k = kv.key.trim();
            if k.is_empty() {
                v.push(issue(Error, format!("env.{i}.key"), "env.keyRequired", &[]));
            } else if k.contains('=')
                || k.contains('\0')
                || k.contains(char::is_whitespace)
                || kv.value.contains('\0')
            {
                v.push(issue(
                    Error,
                    format!("env.{i}.key"),
                    "env.keyInvalid",
                    &[("key", k.to_owned())],
                ));
            } else if !seen.insert(k.to_owned()) {
                v.push(issue(
                    Warning,
                    format!("env.{i}.key"),
                    "env.duplicate",
                    &[("key", k.to_owned())],
                ));
            }
        }
        let mut seen = HashSet::new();
        for (i, kv) in self.labels.iter().enumerate() {
            let k = kv.key.trim();
            if k.is_empty() || k.contains('\0') || k.contains('=') {
                v.push(issue(
                    Error,
                    format!("labels.{i}.key"),
                    "label.keyInvalid",
                    &[],
                ));
            } else if !seen.insert(k.to_owned()) {
                v.push(issue(
                    Error,
                    format!("labels.{i}.key"),
                    "label.duplicate",
                    &[("key", k.to_owned())],
                ));
            }
        }

        let net = self.network.trim();
        if !net.is_empty() && !valid_name(net) {
            v.push(issue(Error, "network", "network.invalid", &[]));
        }
        if (net == "host" || net == "none") && (!self.ports.is_empty() || self.publish_all) {
            v.push(issue(
                Warning,
                "ports",
                "ports.ignoredWithNetwork",
                &[("network", net.to_owned())],
            ));
        }
        let mut bindings = HashSet::new();
        for (i, p) in self.ports.iter().enumerate() {
            if p.container_port == 0 {
                v.push(issue(
                    Error,
                    format!("ports.{i}.containerPort"),
                    "port.invalid",
                    &[],
                ));
            }
            if p.host_port == Some(0) {
                v.push(issue(
                    Error,
                    format!("ports.{i}.hostPort"),
                    "port.invalid",
                    &[],
                ));
            }
            let ip = p.host_ip.trim();
            if !ip.is_empty() && ip.parse::<IpAddr>().is_err() {
                v.push(issue(
                    Error,
                    format!("ports.{i}.hostIp"),
                    "ip.invalid",
                    &[("ip", ip.to_owned())],
                ));
            }
            if let Some(hp) = p.host_port {
                let key = (ip.to_owned(), hp, p.protocol);
                if !bindings.insert(key) {
                    v.push(issue(
                        Error,
                        format!("ports.{i}.hostPort"),
                        "port.duplicate",
                        &[("port", hp.to_string())],
                    ));
                }
                if hp < 1024 {
                    v.push(issue(
                        Warning,
                        format!("ports.{i}.hostPort"),
                        "port.privileged",
                        &[("port", hp.to_string())],
                    ));
                }
            }
        }

        let mut targets = HashSet::new();
        for (i, m) in self.mounts.iter().enumerate() {
            let t = m.target.trim();
            if !absolute(t) || t == "/" {
                v.push(issue(
                    Error,
                    format!("mounts.{i}.target"),
                    "mount.targetInvalid",
                    &[],
                ));
            } else if !targets.insert(t.trim_end_matches('/').to_owned()) {
                v.push(issue(
                    Error,
                    format!("mounts.{i}.target"),
                    "mount.targetDuplicate",
                    &[("target", t.to_owned())],
                ));
            }
            let s = m.source.trim();
            match m.kind {
                MountKind::Volume => {
                    if !s.is_empty() && !valid_name(s) {
                        v.push(issue(
                            Error,
                            format!("mounts.{i}.source"),
                            "volume.invalid",
                            &[],
                        ));
                    }
                }
                MountKind::Bind => {
                    if s.is_empty() {
                        v.push(issue(
                            Error,
                            format!("mounts.{i}.source"),
                            "bind.required",
                            &[],
                        ));
                    } else if !absolute(s) {
                        v.push(issue(
                            Error,
                            format!("mounts.{i}.source"),
                            "path.notAbsolute",
                            &[],
                        ));
                    } else {
                        if check_fs && std::fs::metadata(Path::new(s)).is_err() {
                            v.push(issue(
                                Error,
                                format!("mounts.{i}.source"),
                                "bind.missing",
                                &[("path", s.to_owned())],
                            ));
                        }
                    }
                }
            }
        }

        if let Some(c) = self.cpus {
            if !c.is_finite() || !(0.01..=1024.0).contains(&c) {
                v.push(issue(Error, "cpus", "cpus.range", &[]));
            }
        }
        if let Some(m) = self.memory_bytes {
            if m < MIN_MEMORY {
                v.push(issue(Error, "memoryBytes", "memory.min", &[]));
            }
        }
        if let Some(r) = self.memory_reservation_bytes {
            if r < MIN_MEMORY {
                v.push(issue(Error, "memoryReservationBytes", "memory.min", &[]));
            }
            if let Some(m) = self.memory_bytes {
                if r > m {
                    v.push(issue(
                        Error,
                        "memoryReservationBytes",
                        "memory.reservationAboveLimit",
                        &[],
                    ));
                }
            }
        }
        if self.auto_remove && self.restart.name != RestartName::No {
            v.push(issue(Error, "autoRemove", "autoRemove.restart", &[]));
        }
        // Riesgos para el equipo (E1, D-086): avisos, nunca errores.
        for r in self.risks(check_fs) {
            v.push(SpecIssue {
                severity: Warning,
                field: r.field,
                code: r.code,
                params: r.params,
            });
        }
        for (field, caps) in [("capAdd", &self.cap_add), ("capDrop", &self.cap_drop)] {
            for c in caps {
                let c = c.trim();
                if c != "ALL"
                    && (c.is_empty() || !c.chars().all(|ch| ch.is_ascii_uppercase() || ch == '_'))
                {
                    v.push(issue(Error, field, "cap.invalid", &[("cap", c.to_owned())]));
                }
            }
        }
        for (i, h) in self.extra_hosts.iter().enumerate() {
            let host = h.key.trim();
            if host.is_empty() || host.contains(':') || host.contains(char::is_whitespace) {
                v.push(issue(
                    Error,
                    format!("extraHosts.{i}.key"),
                    "host.invalid",
                    &[],
                ));
            }
            let ip = h.value.trim();
            if ip != "host-gateway" && ip.parse::<IpAddr>().is_err() {
                v.push(issue(
                    Error,
                    format!("extraHosts.{i}.value"),
                    "ip.invalid",
                    &[("ip", ip.to_owned())],
                ));
            }
        }
        v
    }

    pub fn has_errors(issues: &[SpecIssue]) -> bool {
        issues.iter().any(|i| i.severity == Severity::Error)
    }

    /// Entrypoint y comando tal como se envían (D-035): la CLI de Docker solo
    /// admite un ejecutable en `--entrypoint`, así que los argumentos extra del
    /// entrypoint se anteponen al comando. La ejecución es idéntica.
    fn normalized_exec(&self) -> (Option<String>, Vec<String>) {
        match self.entrypoint.split_first() {
            Some((first, rest)) => {
                let mut cmd: Vec<String> = rest.to_vec();
                cmd.extend(self.command.iter().cloned());
                (Some(first.clone()), cmd)
            }
            None => (None, self.command.clone()),
        }
    }

    fn port_key(p: &PortSpec) -> String {
        format!("{}/{}", p.container_port, p.protocol.as_str())
    }

    /// Nombre (o `None`) y cuerpo de `POST /containers/create`.
    pub fn to_create_body(&self) -> (Option<String>, ContainerCreateBody) {
        let t = |s: &str| {
            let s = s.trim();
            (!s.is_empty()).then(|| s.to_owned())
        };
        let (entry, cmd) = self.normalized_exec();

        let mut exposed: Vec<String> = Vec::new();
        let mut bindings: HashMap<String, Option<Vec<PortBinding>>> = HashMap::new();
        for p in &self.ports {
            let key = Self::port_key(p);
            if !exposed.contains(&key) {
                exposed.push(key.clone());
            }
            bindings
                .entry(key)
                .or_insert_with(|| Some(Vec::new()))
                .as_mut()
                .expect("Some")
                .push(PortBinding {
                    host_ip: t(&p.host_ip),
                    host_port: Some(p.host_port.map(|x| x.to_string()).unwrap_or_default()),
                });
        }

        let mounts: Vec<Mount> = self
            .mounts
            .iter()
            .map(|m| Mount {
                target: Some(m.target.trim().to_owned()),
                source: t(&m.source),
                typ: Some(match m.kind {
                    MountKind::Volume => MountType::VOLUME,
                    MountKind::Bind => MountType::BIND,
                }),
                read_only: m.read_only.then_some(true),
                ..Default::default()
            })
            .collect();

        let restart = RestartPolicy {
            name: Some(match self.restart.name {
                RestartName::No => RestartPolicyNameEnum::NO,
                RestartName::Always => RestartPolicyNameEnum::ALWAYS,
                RestartName::UnlessStopped => RestartPolicyNameEnum::UNLESS_STOPPED,
                RestartName::OnFailure => RestartPolicyNameEnum::ON_FAILURE,
            }),
            maximum_retry_count: (self.restart.name == RestartName::OnFailure
                && self.restart.max_retries > 0)
                .then_some(self.restart.max_retries as i64),
        };

        let nonempty = |v: Vec<String>| (!v.is_empty()).then_some(v);
        let host = HostConfig {
            port_bindings: (!bindings.is_empty()).then_some(bindings),
            publish_all_ports: self.publish_all.then_some(true),
            network_mode: t(&self.network),
            mounts: (!mounts.is_empty()).then_some(mounts),
            nano_cpus: self.cpus.map(|c| (c * 1e9).round() as i64),
            memory: self.memory_bytes.map(|m| m as i64),
            memory_reservation: self.memory_reservation_bytes.map(|m| m as i64),
            restart_policy: Some(restart),
            privileged: self.privileged.then_some(true),
            auto_remove: self.auto_remove.then_some(true),
            init: self.init.then_some(true),
            readonly_rootfs: self.read_only_rootfs.then_some(true),
            cap_add: nonempty(self.cap_add.iter().map(|c| c.trim().to_owned()).collect()),
            cap_drop: nonempty(self.cap_drop.iter().map(|c| c.trim().to_owned()).collect()),
            extra_hosts: nonempty(
                self.extra_hosts
                    .iter()
                    .map(|h| format!("{}:{}", h.key.trim(), h.value.trim()))
                    .collect(),
            ),
            ..Default::default()
        };

        let body = ContainerCreateBody {
            image: Some(self.image.trim().to_owned()),
            cmd: nonempty(cmd),
            entrypoint: entry.map(|e| vec![e]),
            working_dir: t(&self.working_dir),
            user: t(&self.user),
            hostname: t(&self.hostname),
            env: nonempty(
                self.env
                    .iter()
                    .map(|kv| format!("{}={}", kv.key.trim(), kv.value))
                    .collect(),
            ),
            labels: (!self.labels.is_empty()).then(|| {
                self.labels
                    .iter()
                    .map(|kv| (kv.key.trim().to_owned(), kv.value.clone()))
                    .collect()
            }),
            exposed_ports: (!exposed.is_empty()).then_some(exposed),
            tty: self.tty.then_some(true),
            open_stdin: self.interactive.then_some(true),
            host_config: Some(host),
            ..Default::default()
        };
        (t(&self.name), body)
    }

    /// Copia con los valores de las variables sensibles sustituidos por
    /// [`REDACTED`], para mostrarla o registrarla sin exponer credenciales
    /// (el panel de actividad, la vista previa). Devuelve cuántos se ocultaron.
    pub fn redacted(&self) -> (ContainerSpec, usize) {
        let mut out = self.clone();
        let mut n = 0;
        for kv in &mut out.env {
            let shown = shown_value(&kv.key, &kv.value);
            if shown != kv.value {
                kv.value = shown;
                n += 1;
            }
        }
        (out, n)
    }

    /// Argumentos de `docker create` (o `docker run --detach`) equivalentes.
    pub fn cli_args(&self, run: bool) -> Vec<String> {
        let mut a: Vec<String> = if run {
            vec!["run".into(), "--detach".into()]
        } else {
            vec!["create".into()]
        };
        fn push(a: &mut Vec<String>, k: &str, v: String) {
            a.push(k.to_owned());
            a.push(v);
        }
        let t = |s: &str| s.trim().to_owned();
        if !self.name.trim().is_empty() {
            push(&mut a, "--name", t(&self.name));
        }
        if !self.hostname.trim().is_empty() {
            push(&mut a, "--hostname", t(&self.hostname));
        }
        if !self.user.trim().is_empty() {
            push(&mut a, "--user", t(&self.user));
        }
        if !self.working_dir.trim().is_empty() {
            push(&mut a, "--workdir", t(&self.working_dir));
        }
        for kv in &self.env {
            push(&mut a, "--env", format!("{}={}", kv.key.trim(), kv.value));
        }
        for kv in &self.labels {
            push(&mut a, "--label", format!("{}={}", kv.key.trim(), kv.value));
        }
        if !self.network.trim().is_empty() {
            push(&mut a, "--network", t(&self.network));
        }
        for p in &self.ports {
            let ip = p.host_ip.trim();
            let ipf = if ip.contains(':') {
                format!("[{ip}]")
            } else {
                ip.to_owned()
            };
            let v = match (ip.is_empty(), p.host_port) {
                (true, None) => format!("{}/{}", p.container_port, p.protocol.as_str()),
                (true, Some(h)) => format!("{h}:{}/{}", p.container_port, p.protocol.as_str()),
                (false, h) => format!(
                    "{ipf}:{}:{}/{}",
                    h.map(|x| x.to_string()).unwrap_or_default(),
                    p.container_port,
                    p.protocol.as_str()
                ),
            };
            push(&mut a, "--publish", v);
        }
        if self.publish_all {
            a.push("--publish-all".into());
        }
        for m in &self.mounts {
            let mut v = format!(
                "type={}",
                if m.kind == MountKind::Bind {
                    "bind"
                } else {
                    "volume"
                }
            );
            if !m.source.trim().is_empty() {
                v.push_str(&format!(",source={}", m.source.trim()));
            }
            v.push_str(&format!(",target={}", m.target.trim()));
            if m.read_only {
                v.push_str(",readonly");
            }
            push(&mut a, "--mount", v);
        }
        if let Some(c) = self.cpus {
            push(&mut a, "--cpus", format_cpus(c));
        }
        if let Some(m) = self.memory_bytes {
            push(&mut a, "--memory", format_bytes_cli(m));
        }
        if let Some(m) = self.memory_reservation_bytes {
            push(&mut a, "--memory-reservation", format_bytes_cli(m));
        }
        let restart = match self.restart.name {
            RestartName::No => "no".to_owned(),
            RestartName::Always => "always".to_owned(),
            RestartName::UnlessStopped => "unless-stopped".to_owned(),
            RestartName::OnFailure if self.restart.max_retries > 0 => {
                format!("on-failure:{}", self.restart.max_retries)
            }
            RestartName::OnFailure => "on-failure".to_owned(),
        };
        push(&mut a, "--restart", restart);
        for c in &self.cap_add {
            push(&mut a, "--cap-add", c.trim().to_owned());
        }
        for c in &self.cap_drop {
            push(&mut a, "--cap-drop", c.trim().to_owned());
        }
        for h in &self.extra_hosts {
            push(
                &mut a,
                "--add-host",
                format!("{}:{}", h.key.trim(), h.value.trim()),
            );
        }
        for (flag, on) in [
            ("--privileged", self.privileged),
            ("--rm", self.auto_remove),
            ("--init", self.init),
            ("--tty", self.tty),
            ("--interactive", self.interactive),
            ("--read-only", self.read_only_rootfs),
        ] {
            if on {
                a.push(flag.into());
            }
        }
        let (entry, cmd) = self.normalized_exec();
        if let Some(e) = entry {
            push(&mut a, "--entrypoint", e);
        }
        a.push(self.image.trim().to_owned());
        a.extend(cmd);
        a
    }

    pub fn cli_command(&self, run: bool) -> String {
        render_argv("docker", &self.cli_args(run))
    }
}

pub fn format_cpus(c: f64) -> String {
    let s = format!("{c:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_owned()
}

pub fn format_bytes_cli(b: u64) -> String {
    if b % GIB == 0 {
        format!("{}g", b / GIB)
    } else if b % MIB == 0 {
        format!("{}m", b / MIB)
    } else if b % 1024 == 0 {
        format!("{}k", b / 1024)
    } else {
        format!("{b}b")
    }
}

// ---------------------------------------------------------------- duplicado

/// Resultado de leer la configuración de un contenedor para duplicarlo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DuplicateSpec {
    pub spec: ContainerSpec,
    /// Códigos de ajustes que no se trasladan (ver `from_inspect`).
    pub notes: Vec<String>,
    /// Nombre del contenedor original.
    pub source_name: String,
}

fn strip_image_defaults(values: Vec<String>, defaults: Option<&Vec<String>>) -> Vec<String> {
    match defaults {
        Some(d) => values.into_iter().filter(|v| !d.contains(v)).collect(),
        None => values,
    }
}

/// Construye una especificación a partir de `docker inspect`. Se descartan los
/// valores que vienen de la imagen (el duplicado los volverá a heredar) y se
/// anotan los ajustes que el asistente no gestiona.
pub fn from_inspect(c: &ContainerInspectResponse, image: Option<&ImageConfig>) -> DuplicateSpec {
    let cfg = c.config.clone().unwrap_or_default();
    let host = c.host_config.clone().unwrap_or_default();
    let mut notes: Vec<String> = Vec::new();
    let img_cmd = image.and_then(|i| i.cmd.clone()).unwrap_or_default();
    let img_entry = image.and_then(|i| i.entrypoint.clone()).unwrap_or_default();

    let entrypoint = cfg.entrypoint.clone().unwrap_or_default();
    let entry_from_image = image.is_some() && entrypoint == img_entry;
    let command = cfg.cmd.clone().unwrap_or_default();
    // Docker solo hereda el CMD de la imagen si no se cambió el entrypoint.
    let cmd_from_image = image.is_some() && entry_from_image && command == img_cmd;

    let env = strip_image_defaults(
        cfg.env.clone().unwrap_or_default(),
        image.and_then(|i| i.env.as_ref()),
    )
    .into_iter()
    .map(|e| match e.split_once('=') {
        Some((k, v)) => KeyValue::new(k, v),
        None => KeyValue::new(e, ""),
    })
    .collect();

    let img_labels: HashMap<String, String> =
        image.and_then(|i| i.labels.clone()).unwrap_or_default();
    let mut labels: Vec<KeyValue> = Vec::new();
    let mut had_compose = false;
    let mut container_labels: Vec<(String, String)> =
        cfg.labels.clone().unwrap_or_default().into_iter().collect();
    container_labels.sort();
    for (k, v) in container_labels {
        if k.starts_with("com.docker.compose.") {
            had_compose = true;
            continue;
        }
        if img_labels.get(&k) == Some(&v) {
            continue;
        }
        labels.push(KeyValue::new(k, v));
    }
    if had_compose {
        notes.push("composeLabels".into());
    }

    let mut ports: Vec<PortSpec> = Vec::new();
    // Docker guarda los puertos en un mapa sin orden: se ordenan por puerto y protocolo.
    let keys: Vec<(String, Option<Vec<PortBinding>>)> = host
        .port_bindings
        .clone()
        .unwrap_or_default()
        .into_iter()
        .collect();
    for (key, binds) in keys {
        let (port, proto) = key.split_once('/').unwrap_or((key.as_str(), "tcp"));
        let (Ok(port), Some(proto)) = (port.parse::<u16>(), Protocol::parse(proto)) else {
            continue;
        };
        for b in binds.unwrap_or_default() {
            ports.push(PortSpec {
                host_ip: b.host_ip.unwrap_or_default(),
                host_port: b.host_port.as_deref().and_then(|p| p.parse().ok()),
                container_port: port,
                protocol: proto,
            });
        }
    }
    ports.sort_by_key(|p| {
        (
            p.container_port,
            p.protocol.as_str(),
            p.host_ip.clone(),
            p.host_port,
        )
    });
    if ports.iter().any(|p| p.host_port.is_some()) {
        notes.push("portsConflict".into());
    }

    let mut mounts: Vec<MountSpec> = Vec::new();
    for m in host.mounts.clone().unwrap_or_default() {
        let kind = match m.typ {
            Some(MountType::BIND) => MountKind::Bind,
            Some(MountType::VOLUME) => MountKind::Volume,
            _ => {
                notes.push("mountTypeUnsupported".into());
                continue;
            }
        };
        mounts.push(MountSpec {
            kind,
            source: m.source.unwrap_or_default(),
            target: m.target.unwrap_or_default(),
            read_only: m.read_only.unwrap_or(false),
        });
    }
    for b in host.binds.clone().unwrap_or_default() {
        let parts: Vec<&str> = b.splitn(3, ':').collect();
        if parts.len() < 2 {
            continue;
        }
        let ro = parts
            .get(2)
            .is_some_and(|o| o.split(',').any(|x| x == "ro"));
        let kind = if parts[0].starts_with('/') {
            MountKind::Bind
        } else {
            MountKind::Volume
        };
        mounts.push(MountSpec {
            kind,
            source: parts[0].to_owned(),
            target: parts[1].to_owned(),
            read_only: ro,
        });
    }
    let img_vols: Vec<String> = image.and_then(|i| i.volumes.clone()).unwrap_or_default();
    for v in cfg.volumes.clone().unwrap_or_default() {
        if !img_vols.contains(&v) && !mounts.iter().any(|m| m.target == v) {
            mounts.push(MountSpec {
                kind: MountKind::Volume,
                source: String::new(),
                target: v,
                read_only: false,
            });
        }
    }
    if mounts
        .iter()
        .any(|m| m.kind == MountKind::Volume && !m.source.is_empty())
    {
        notes.push("namedVolumesShared".into());
    }

    let network = match host.network_mode.as_deref() {
        None | Some("default") | Some("bridge") | Some("") => String::new(),
        Some(n) => n.to_owned(),
    };
    if let Some(ns) = &c.network_settings {
        if ns.networks.as_ref().is_some_and(|n| n.len() > 1) {
            notes.push("extraNetworks".into());
        }
    }

    let cpus = match (host.nano_cpus, host.cpu_quota, host.cpu_period) {
        (Some(n), _, _) if n > 0 => Some(n as f64 / 1e9),
        (_, Some(q), Some(p)) if q > 0 && p > 0 => Some(q as f64 / p as f64),
        _ => None,
    };
    let restart = match host.restart_policy.as_ref().and_then(|r| r.name) {
        Some(RestartPolicyNameEnum::ALWAYS) => RestartSpec {
            name: RestartName::Always,
            max_retries: 0,
        },
        Some(RestartPolicyNameEnum::UNLESS_STOPPED) => RestartSpec {
            name: RestartName::UnlessStopped,
            max_retries: 0,
        },
        Some(RestartPolicyNameEnum::ON_FAILURE) => RestartSpec {
            name: RestartName::OnFailure,
            max_retries: host
                .restart_policy
                .as_ref()
                .and_then(|r| r.maximum_retry_count)
                .unwrap_or(0)
                .max(0) as u32,
        },
        _ => RestartSpec::default(),
    };

    let short_id: String = c.id.clone().unwrap_or_default().chars().take(12).collect();
    let hostname = cfg.hostname.clone().unwrap_or_default();
    let hostname = if hostname == short_id {
        String::new()
    } else {
        hostname
    };

    // Ajustes que el asistente no gestiona: se avisa en lugar de perderlos en silencio.
    let nonempty_vec = |v: &Option<Vec<String>>| v.as_ref().is_some_and(|x| !x.is_empty());
    if host.devices.as_ref().is_some_and(|d| !d.is_empty()) {
        notes.push("devices".into());
    }
    if nonempty_vec(&host.dns) {
        notes.push("dns".into());
    }
    if nonempty_vec(&host.security_opt) {
        notes.push("securityOpt".into());
    }
    if host.tmpfs.as_ref().is_some_and(|t| !t.is_empty()) {
        notes.push("tmpfs".into());
    }
    if nonempty_vec(&host.volumes_from) {
        notes.push("volumesFrom".into());
    }
    if host.ulimits.as_ref().is_some_and(|u| !u.is_empty()) {
        notes.push("ulimits".into());
    }
    if host.cpuset_cpus.as_ref().is_some_and(|s| !s.is_empty()) {
        notes.push("cpuset".into());
    }
    if host.pids_limit.is_some_and(|p| p > 0) {
        notes.push("pidsLimit".into());
    }
    if host.log_config.as_ref().is_some_and(|l| {
        l.typ.as_deref().is_some_and(|t| t != "json-file")
            || l.config.as_ref().is_some_and(|c| !c.is_empty())
    }) {
        notes.push("logConfig".into());
    }
    if cfg.healthcheck.is_some() && image.is_some_and(|i| i.healthcheck != cfg.healthcheck) {
        notes.push("healthcheck".into());
    }
    if cfg.stop_signal.is_some() && image.is_some_and(|i| i.stop_signal != cfg.stop_signal) {
        notes.push("stopSignal".into());
    }
    if image.is_none() {
        notes.push("imageMissing".into());
    }

    let img_user = image.and_then(|i| i.user.clone()).unwrap_or_default();
    let img_wd = image
        .and_then(|i| i.working_dir.clone())
        .unwrap_or_default();
    let user = cfg.user.clone().unwrap_or_default();
    let wd = cfg.working_dir.clone().unwrap_or_default();

    let spec = ContainerSpec {
        name: String::new(),
        image: cfg.image.clone().unwrap_or_default(),
        command: if cmd_from_image { Vec::new() } else { command },
        entrypoint: if entry_from_image {
            Vec::new()
        } else {
            entrypoint
        },
        working_dir: if image.is_some() && wd == img_wd {
            String::new()
        } else {
            wd
        },
        user: if image.is_some() && user == img_user {
            String::new()
        } else {
            user
        },
        hostname,
        env,
        labels,
        network,
        ports,
        publish_all: host.publish_all_ports.unwrap_or(false),
        mounts,
        cpus,
        memory_bytes: host.memory.filter(|m| *m > 0).map(|m| m as u64),
        memory_reservation_bytes: host.memory_reservation.filter(|m| *m > 0).map(|m| m as u64),
        restart,
        privileged: host.privileged.unwrap_or(false),
        auto_remove: host.auto_remove.unwrap_or(false),
        init: host.init.unwrap_or(false),
        tty: cfg.tty.unwrap_or(false),
        interactive: cfg.open_stdin.unwrap_or(false),
        read_only_rootfs: host.readonly_rootfs.unwrap_or(false),
        cap_add: host.cap_add.clone().unwrap_or_default(),
        cap_drop: host.cap_drop.clone().unwrap_or_default(),
        extra_hosts: host
            .extra_hosts
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|h| h.split_once(':').map(|(a, b)| KeyValue::new(a, b)))
            .collect(),
        // Una copia vuelve a pedir la aceptación de sus riesgos.
        accepted_risks: Vec::new(),
    };
    notes.sort();
    notes.dedup();
    DuplicateSpec {
        spec,
        notes,
        source_name: c
            .name
            .clone()
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_owned(),
    }
}

#[cfg(test)]
mod tests;
