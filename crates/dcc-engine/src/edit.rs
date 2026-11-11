//! Edición de la configuración de un contenedor existente (0.9.2, D-070).
//!
//! Este módulo es la parte **pura**: lee la configuración editable de un
//! `inspect`, compara lo actual con lo deseado, clasifica cada cambio (directo
//! o con recreación) y construye los cuerpos de `POST /containers/{id}/update`
//! y de `POST /containers/create` para la recreación. Las llamadas a Docker
//! están en [`crate::edit_ops`].
//!
//! Principio de la recreación: el contenedor nuevo se crea a partir de la
//! configuración **completa** del original (`Config` y `HostConfig` de
//! inspect), cambiando solo lo que el usuario ha editado. Así se conserva
//! también lo que la aplicación no sabe editar (dispositivos, DNS, opciones de
//! seguridad, tmpfs, límites, registro…).

use std::collections::{BTreeMap, HashMap};

use bollard::models::{
    ContainerConfig, ContainerCreateBody, ContainerInspectResponse, ContainerUpdateBody,
    EndpointSettings, HostConfig, ImageConfig, Mount, MountType, NetworkingConfig, PortBinding,
    RestartPolicy, RestartPolicyNameEnum,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::spec::{
    format_bytes_cli, format_cpus, is_secret_key, issue, ContainerSpec, KeyValue, MountKind,
    MountSpec, PortSpec, Protocol, RestartName, RestartSpec, Severity, SpecIssue, REDACTED,
};

/// Etiqueta donde se guarda la descripción escrita por el usuario.
pub const DESCRIPTION_LABEL: &str = "dcc.description";
/// Marca en el nombre de la copia de seguridad: `<nombre>-dcc-backup-<fecha>`.
pub const BACKUP_INFIX: &str = "-dcc-backup-";
/// Marca en el nombre del contenedor apartado al restaurar una copia.
pub const REPLACED_INFIX: &str = "-dcc-replaced-";
pub const MAX_DESCRIPTION: usize = 500;

/// Lo que el usuario puede cambiar. Los mismos campos sirven para describir el
/// estado actual y el deseado.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct EditRequest {
    pub name: String,
    pub description: String,
    pub restart: RestartSpec,
    pub cpus: Option<f64>,
    #[ts(type = "number | null")]
    pub memory_bytes: Option<u64>,
    #[ts(type = "number | null")]
    pub memory_reservation_bytes: Option<u64>,
    /// Variables propias del contenedor (sin las que hereda de la imagen).
    pub env: Vec<KeyValue>,
    pub ports: Vec<PortSpec>,
    /// Volúmenes y carpetas. Un volumen anónimo aparece con su nombre real,
    /// para volver a montarlo (y no crear otro vacío).
    pub mounts: Vec<MountSpec>,
}

/// Red a la que está conectado (solo lectura aquí: se cambia con conectar y desconectar).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EditNetwork {
    pub name: String,
    pub id: String,
    pub ipv4: String,
    pub ipv6: String,
    pub mac: String,
    pub aliases: Vec<String>,
    /// IP fija configurada (no la asignada automáticamente).
    pub fixed_ipv4: String,
    pub fixed_ipv6: String,
}

/// Copia de seguridad que dejó una recreación anterior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BackupRef {
    pub id: String,
    pub name: String,
    /// Nombre del contenedor del que es copia.
    pub original_name: String,
    pub state: String,
    #[ts(type = "number")]
    pub created: i64,
}

/// Configuración de un contenedor preparada para la pantalla de edición.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EditableConfig {
    pub id: String,
    /// created, running, paused, restarting, exited, removing, dead.
    pub status: String,
    pub current: EditRequest,
    /// Variables que vienen de la imagen (solo lectura).
    pub image_env: Vec<KeyValue>,
    /// Nombres de los volúmenes anónimos que aparecen en `current.mounts`.
    pub anonymous_volumes: Vec<String>,
    /// Montajes de otros tipos (tmpfs…): se conservan, no se editan.
    pub other_mounts: Vec<String>,
    pub networks: Vec<EditNetwork>,
    pub network_mode: String,
    // --- solo lectura (pestaña Avanzado)
    pub image: String,
    pub image_id: String,
    pub entrypoint: Vec<String>,
    pub command: Vec<String>,
    pub working_dir: String,
    pub user: String,
    pub hostname: String,
    pub privileged: bool,
    pub tty: bool,
    pub interactive: bool,
    pub read_only_rootfs: bool,
    pub init: bool,
    pub auto_remove: bool,
    pub cap_add: Vec<String>,
    pub cap_drop: Vec<String>,
    pub extra_hosts: Vec<String>,
    pub labels: Vec<KeyValue>,
    pub compose_project: Option<String>,
    pub swarm_service: bool,
    /// Bytes escritos en la capa del contenedor (se pierden al recrear).
    #[ts(type = "number | null")]
    pub size_rw: Option<i64>,
    /// CPU y memoria del equipo, para validar los límites.
    pub host_cpus: u32,
    #[ts(type = "number")]
    pub host_memory: u64,
    pub backups: Vec<BackupRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ChangeKind {
    /// Docker lo aplica sobre el contenedor existente (rename, update).
    Direct,
    /// Solo se puede aplicar creando un contenedor nuevo.
    Recreate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChangeItem {
    /// added, removed, modified.
    pub op: String,
    pub label: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Change {
    /// name, description, restart, cpus, memory, memoryReservation, env, ports, mounts.
    pub field: String,
    pub kind: ChangeKind,
    /// Motivo (código) cuando un cambio normalmente directo exige recreación.
    pub reason: Option<String>,
    pub before: String,
    pub after: String,
    /// Detalle elemento a elemento (variables, puertos, montajes).
    pub items: Vec<ChangeItem>,
}

/// Riesgo que el usuario debe aceptar expresamente antes de recrear.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Risk {
    pub code: String,
    pub params: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EditPlan {
    pub container_id: String,
    pub name: String,
    pub status: String,
    pub changes: Vec<Change>,
    /// Campos editables que no cambian.
    pub kept: Vec<String>,
    pub needs_recreate: bool,
    pub issues: Vec<SpecIssue>,
    /// Motivos (códigos) por los que la recreación no se puede hacer con seguridad.
    pub blockers: Vec<Risk>,
    pub risks: Vec<Risk>,
    /// Avisos que no requieren aceptación (códigos).
    pub notices: Vec<Risk>,
    /// La recreación detiene el contenedor (está en ejecución).
    pub will_stop: bool,
    pub backup_name: Option<String>,
    /// Imagen con la que se creará el contenedor nuevo.
    pub image_ref: Option<String>,
    /// Órdenes `docker` equivalentes, sin valores sensibles.
    pub equivalent: Vec<String>,
    /// Cuerpo de `POST /containers/create` de la recreación, sin valores sensibles.
    pub create_body: Option<String>,
}

impl EditPlan {
    pub fn has_errors(&self) -> bool {
        ContainerSpec::has_errors(&self.issues)
    }
}

/// Paso ejecutado al aplicar los cambios.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EditStep {
    /// rename, update, stop, backup, create, connect, disconnect, start, verify,
    /// rollbackRemove, rollbackRename, rollbackStart.
    pub code: String,
    pub ok: bool,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EditOutcome {
    /// ID del contenedor que queda con la configuración (el nuevo si se recreó).
    pub container_id: String,
    pub name: String,
    pub recreated: bool,
    /// El contenedor nuevo se inició y sigue en ejecución.
    pub started: bool,
    pub backup_id: Option<String>,
    pub backup_name: Option<String>,
    /// La recreación falló y se volvió al contenedor original.
    pub rolled_back: bool,
    /// No se pudo volver del todo al estado anterior (ver `steps`).
    pub rollback_failed: bool,
    pub steps: Vec<EditStep>,
    pub error: Option<String>,
    pub warnings: Vec<String>,
}

// ------------------------------------------------------------ lectura

/// Montaje del original con la forma exacta en que hay que volver a declararlo.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum RawMount {
    /// Entrada de `HostConfig.Binds`, tal cual (conserva opciones como `:z`).
    Bind(String),
    /// Entrada de `HostConfig.Mounts`, tal cual.
    Mount(Box<Mount>),
    /// Volumen anónimo: se vuelve a montar por su nombre.
    Anonymous,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OrigMount {
    pub spec: MountSpec,
    pub raw: RawMount,
}

fn parse_bind(b: &str) -> Option<MountSpec> {
    let parts: Vec<&str> = b.splitn(3, ':').collect();
    if parts.len() < 2 {
        return None;
    }
    let ro = parts
        .get(2)
        .is_some_and(|o| o.split(',').any(|x| x == "ro"));
    Some(MountSpec {
        kind: if parts[0].starts_with('/') {
            MountKind::Bind
        } else {
            MountKind::Volume
        },
        source: parts[0].to_owned(),
        target: parts[1].to_owned(),
        read_only: ro,
    })
}

/// Volúmenes y carpetas del contenedor, y los montajes de otros tipos.
pub(crate) fn original_mounts(c: &ContainerInspectResponse) -> (Vec<OrigMount>, Vec<Mount>) {
    let host = c.host_config.clone().unwrap_or_default();
    let points = c.mounts.clone().unwrap_or_default();
    // Nombre real del volumen montado en una ruta (para los anónimos).
    let volume_at = |target: &str| {
        points
            .iter()
            .find(|p| {
                p.typ.as_deref() == Some("volume") && p.destination.as_deref() == Some(target)
            })
            .and_then(|p| p.name.clone())
            .filter(|n| !n.is_empty())
    };
    let mut out: Vec<OrigMount> = Vec::new();
    let mut other: Vec<Mount> = Vec::new();
    for b in host.binds.unwrap_or_default() {
        if let Some(spec) = parse_bind(&b) {
            out.push(OrigMount {
                spec,
                raw: RawMount::Bind(b),
            });
        }
    }
    for m in host.mounts.unwrap_or_default() {
        let kind = match m.typ {
            Some(MountType::BIND) => MountKind::Bind,
            Some(MountType::VOLUME) => MountKind::Volume,
            _ => {
                other.push(m);
                continue;
            }
        };
        let target = m.target.clone().unwrap_or_default();
        let source = m.source.clone().unwrap_or_default();
        if kind == MountKind::Volume && source.is_empty() {
            // `--mount type=volume,dst=/x` sin origen: volumen anónimo.
            if let Some(name) = volume_at(&target) {
                out.push(OrigMount {
                    spec: MountSpec {
                        kind,
                        source: name,
                        target,
                        read_only: m.read_only.unwrap_or(false),
                    },
                    raw: RawMount::Anonymous,
                });
                continue;
            }
        }
        out.push(OrigMount {
            spec: MountSpec {
                kind,
                source,
                target,
                read_only: m.read_only.unwrap_or(false),
            },
            raw: RawMount::Mount(Box::new(m)),
        });
    }
    // Volúmenes anónimos de `VOLUME` de la imagen o de `-v /ruta`.
    for p in &points {
        if p.typ.as_deref() != Some("volume") {
            continue;
        }
        let (Some(name), Some(dest)) = (p.name.clone(), p.destination.clone()) else {
            continue;
        };
        if name.is_empty() || out.iter().any(|m| m.spec.target == dest) {
            continue;
        }
        out.push(OrigMount {
            spec: MountSpec {
                kind: MountKind::Volume,
                source: name,
                target: dest,
                read_only: p.rw == Some(false),
            },
            raw: RawMount::Anonymous,
        });
    }
    (out, other)
}

fn split_env(e: &str) -> KeyValue {
    match e.split_once('=') {
        Some((k, v)) => KeyValue::new(k, v),
        None => KeyValue::new(e, ""),
    }
}

/// Límite de CPU en núcleos, venga de `NanoCpus` o de `CpuQuota`/`CpuPeriod`.
fn cpus_of(host: &HostConfig) -> Option<f64> {
    match (host.nano_cpus, host.cpu_quota, host.cpu_period) {
        (Some(n), _, _) if n > 0 => Some(n as f64 / 1e9),
        (_, Some(q), p) if q > 0 => Some(q as f64 / p.filter(|p| *p > 0).unwrap_or(100_000) as f64),
        _ => None,
    }
}

fn positive(v: Option<i64>) -> Option<u64> {
    v.filter(|x| *x > 0).map(|x| x as u64)
}

/// Estado actual de lo editable.
pub fn current_request(c: &ContainerInspectResponse, image: Option<&ImageConfig>) -> EditRequest {
    let dup = crate::spec::from_inspect(c, image);
    let cfg = c.config.clone().unwrap_or_default();
    let host = c.host_config.clone().unwrap_or_default();
    EditRequest {
        name: c
            .name
            .clone()
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_owned(),
        description: cfg
            .labels
            .as_ref()
            .and_then(|l| l.get(DESCRIPTION_LABEL).cloned())
            .unwrap_or_default(),
        restart: dup.spec.restart,
        cpus: cpus_of(&host),
        memory_bytes: positive(host.memory),
        memory_reservation_bytes: positive(host.memory_reservation),
        env: dup.spec.env,
        ports: dup.spec.ports,
        mounts: original_mounts(c).0.into_iter().map(|m| m.spec).collect(),
    }
}

fn describe_other_mount(m: &Mount) -> String {
    let typ = m
        .typ
        .as_ref()
        .and_then(|t| serde_json::to_value(t).ok())
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default();
    format!("{typ} → {}", m.target.clone().unwrap_or_default())
}

/// Configuración para la pantalla de edición. `host_cpus`, `host_memory` y
/// `backups` los rellena quien llama (necesitan otras consultas).
pub fn editable_config(
    c: &ContainerInspectResponse,
    image: Option<&ImageConfig>,
) -> EditableConfig {
    let cfg = c.config.clone().unwrap_or_default();
    let host = c.host_config.clone().unwrap_or_default();
    let current = current_request(c, image);
    let own: Vec<String> = current
        .env
        .iter()
        .map(|kv| format!("{}={}", kv.key, kv.value))
        .collect();
    let image_env = cfg
        .env
        .clone()
        .unwrap_or_default()
        .iter()
        .filter(|e| !own.contains(e))
        .map(|e| split_env(e))
        .collect();
    let (orig, other) = original_mounts(c);
    let labels_map = cfg.labels.clone().unwrap_or_default();
    let mut labels: Vec<KeyValue> = labels_map
        .iter()
        .map(|(k, v)| KeyValue::new(k.clone(), v.clone()))
        .collect();
    labels.sort_by(|a, b| a.key.cmp(&b.key));
    let mut networks: Vec<EditNetwork> = c
        .network_settings
        .as_ref()
        .and_then(|n| n.networks.clone())
        .unwrap_or_default()
        .into_iter()
        .map(|(name, e)| {
            let ipam = e.ipam_config.clone().unwrap_or_default();
            EditNetwork {
                name,
                id: e.network_id.unwrap_or_default(),
                ipv4: e.ip_address.unwrap_or_default(),
                ipv6: e.global_ipv6_address.unwrap_or_default(),
                mac: e.mac_address.unwrap_or_default(),
                aliases: e.aliases.unwrap_or_default(),
                fixed_ipv4: ipam.ipv4_address.unwrap_or_default(),
                fixed_ipv6: ipam.ipv6_address.unwrap_or_default(),
            }
        })
        .collect();
    networks.sort_by(|a, b| a.name.cmp(&b.name));
    EditableConfig {
        id: c.id.clone().unwrap_or_default(),
        status: status_of(c),
        current,
        image_env,
        anonymous_volumes: orig
            .iter()
            .filter(|m| m.raw == RawMount::Anonymous)
            .map(|m| m.spec.source.clone())
            .collect(),
        other_mounts: other.iter().map(describe_other_mount).collect(),
        networks,
        network_mode: host.network_mode.clone().unwrap_or_default(),
        image: cfg.image.clone().unwrap_or_default(),
        image_id: c.image.clone().unwrap_or_default(),
        entrypoint: cfg.entrypoint.clone().unwrap_or_default(),
        command: cfg.cmd.clone().unwrap_or_default(),
        working_dir: cfg.working_dir.clone().unwrap_or_default(),
        user: cfg.user.clone().unwrap_or_default(),
        hostname: cfg.hostname.clone().unwrap_or_default(),
        privileged: host.privileged.unwrap_or(false),
        tty: cfg.tty.unwrap_or(false),
        interactive: cfg.open_stdin.unwrap_or(false),
        read_only_rootfs: host.readonly_rootfs.unwrap_or(false),
        init: host.init.unwrap_or(false),
        auto_remove: host.auto_remove.unwrap_or(false),
        cap_add: host.cap_add.clone().unwrap_or_default(),
        cap_drop: host.cap_drop.clone().unwrap_or_default(),
        extra_hosts: host.extra_hosts.clone().unwrap_or_default(),
        labels,
        compose_project: labels_map.get("com.docker.compose.project").cloned(),
        swarm_service: labels_map.contains_key("com.docker.swarm.service.id"),
        size_rw: c.size_rw,
        host_cpus: 0,
        host_memory: 0,
        backups: Vec::new(),
    }
}

pub(crate) fn status_of(c: &ContainerInspectResponse) -> String {
    c.state
        .as_ref()
        .and_then(|s| s.status.as_ref())
        .and_then(|s| serde_json::to_value(s).ok())
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

// ------------------------------------------------------------ comparación

fn restart_text(r: &RestartSpec) -> String {
    match r.name {
        RestartName::No => "no".into(),
        RestartName::Always => "always".into(),
        RestartName::UnlessStopped => "unless-stopped".into(),
        RestartName::OnFailure if r.max_retries > 0 => format!("on-failure:{}", r.max_retries),
        RestartName::OnFailure => "on-failure".into(),
    }
}

fn normalized_restart(r: &RestartSpec) -> RestartSpec {
    RestartSpec {
        name: r.name,
        max_retries: if r.name == RestartName::OnFailure {
            r.max_retries
        } else {
            0
        },
    }
}

fn port_text(p: &PortSpec) -> String {
    let host = p.host_port.map(|x| x.to_string()).unwrap_or_default();
    let ip = p.host_ip.trim();
    let left = if ip.is_empty() {
        host
    } else if ip.contains(':') {
        format!("[{ip}]:{host}")
    } else {
        format!("{ip}:{host}")
    };
    format!("{left}:{}/{}", p.container_port, p.protocol.as_str())
}

fn mount_text(m: &MountSpec) -> String {
    format!(
        "{}:{}{}",
        m.source.trim(),
        m.target.trim(),
        if m.read_only { ":ro" } else { "" }
    )
}

fn shown_env(kv: &KeyValue) -> String {
    if is_secret_key(&kv.key) && !kv.value.is_empty() {
        REDACTED.to_owned()
    } else {
        kv.value.clone()
    }
}

fn opt_text<T>(v: Option<T>, f: impl Fn(T) -> String) -> String {
    v.map(f).unwrap_or_default()
}

fn same_cpus(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => (x - y).abs() < 1e-6,
        _ => false,
    }
}

/// Normaliza lo que escribe el usuario (espacios) sin tocar los valores.
pub fn normalize(req: &EditRequest) -> EditRequest {
    let mut r = req.clone();
    r.name = r.name.trim().to_owned();
    r.description = r.description.trim().to_owned();
    r.restart = normalized_restart(&r.restart);
    for kv in &mut r.env {
        kv.key = kv.key.trim().to_owned();
    }
    for p in &mut r.ports {
        p.host_ip = p.host_ip.trim().to_owned();
    }
    for m in &mut r.mounts {
        m.source = m.source.trim().to_owned();
        m.target = m.target.trim().to_owned();
    }
    r
}

/// Cambios entre lo actual y lo deseado, con su clasificación (2.4.1).
pub fn diff(current: &EditRequest, desired: &EditRequest) -> (Vec<Change>, Vec<String>) {
    let mut changes = Vec::new();
    let mut kept = Vec::new();
    let simple = |field: &str, kind, reason: Option<&str>, before: String, after: String| Change {
        field: field.into(),
        kind,
        reason: reason.map(str::to_owned),
        before,
        after,
        items: Vec::new(),
    };

    if current.name != desired.name {
        changes.push(simple(
            "name",
            ChangeKind::Direct,
            None,
            current.name.clone(),
            desired.name.clone(),
        ));
    } else {
        kept.push("name".into());
    }

    if current.description != desired.description {
        // Las etiquetas de un contenedor no se pueden cambiar una vez creado.
        changes.push(simple(
            "description",
            ChangeKind::Recreate,
            Some("labelsImmutable"),
            current.description.clone(),
            desired.description.clone(),
        ));
    } else {
        kept.push("description".into());
    }

    if normalized_restart(&current.restart) != normalized_restart(&desired.restart) {
        changes.push(simple(
            "restart",
            ChangeKind::Direct,
            None,
            restart_text(&current.restart),
            restart_text(&desired.restart),
        ));
    } else {
        kept.push("restart".into());
    }

    // `docker update` no sabe quitar un límite: 0 significa «no cambiar».
    if !same_cpus(current.cpus, desired.cpus) {
        let removing = desired.cpus.is_none();
        changes.push(simple(
            "cpus",
            if removing {
                ChangeKind::Recreate
            } else {
                ChangeKind::Direct
            },
            removing.then_some("limitRemoval"),
            opt_text(current.cpus, format_cpus),
            opt_text(desired.cpus, format_cpus),
        ));
    } else {
        kept.push("cpus".into());
    }
    for (field, cur, des) in [
        ("memory", current.memory_bytes, desired.memory_bytes),
        (
            "memoryReservation",
            current.memory_reservation_bytes,
            desired.memory_reservation_bytes,
        ),
    ] {
        if cur != des {
            let removing = des.is_none();
            changes.push(simple(
                field,
                if removing {
                    ChangeKind::Recreate
                } else {
                    ChangeKind::Direct
                },
                removing.then_some("limitRemoval"),
                opt_text(cur, format_bytes_cli),
                opt_text(des, format_bytes_cli),
            ));
        } else {
            kept.push(field.into());
        }
    }

    // Variables: por clave.
    let mut items = Vec::new();
    for kv in &desired.env {
        match current.env.iter().find(|c| c.key == kv.key) {
            None => items.push(ChangeItem {
                op: "added".into(),
                label: kv.key.clone(),
                before: None,
                after: Some(shown_env(kv)),
            }),
            Some(c) if c.value != kv.value => items.push(ChangeItem {
                op: "modified".into(),
                label: kv.key.clone(),
                before: Some(shown_env(c)),
                after: Some(shown_env(kv)),
            }),
            _ => {}
        }
    }
    for c in &current.env {
        if !desired.env.iter().any(|kv| kv.key == c.key) {
            items.push(ChangeItem {
                op: "removed".into(),
                label: c.key.clone(),
                before: Some(shown_env(c)),
                after: None,
            });
        }
    }
    push_list(
        &mut changes,
        &mut kept,
        "env",
        items,
        current.env.len(),
        desired.env.len(),
    );

    // Puertos: por puerto del contenedor y protocolo.
    let key = |p: &PortSpec| (p.container_port, p.protocol);
    let group = |list: &[PortSpec], k: (u16, Protocol)| {
        let mut v: Vec<String> = list.iter().filter(|p| key(p) == k).map(port_text).collect();
        v.sort();
        v
    };
    let mut keys: Vec<(u16, Protocol)> = current
        .ports
        .iter()
        .chain(&desired.ports)
        .map(key)
        .collect();
    keys.sort_by_key(|k| (k.0, k.1.as_str()));
    keys.dedup();
    let mut items = Vec::new();
    for k in keys {
        let (b, a) = (group(&current.ports, k), group(&desired.ports, k));
        if a == b {
            continue;
        }
        let label = format!("{}/{}", k.0, k.1.as_str());
        let join = |v: Vec<String>| (!v.is_empty()).then(|| v.join(", "));
        items.push(ChangeItem {
            op: match (b.is_empty(), a.is_empty()) {
                (true, _) => "added",
                (_, true) => "removed",
                _ => "modified",
            }
            .into(),
            label,
            before: join(b),
            after: join(a),
        });
    }
    push_list(
        &mut changes,
        &mut kept,
        "ports",
        items,
        current.ports.len(),
        desired.ports.len(),
    );

    // Montajes: por ruta dentro del contenedor.
    let mut items = Vec::new();
    for m in &desired.mounts {
        match current.mounts.iter().find(|c| c.target == m.target) {
            None => items.push(ChangeItem {
                op: "added".into(),
                label: m.target.clone(),
                before: None,
                after: Some(mount_text(m)),
            }),
            Some(c) if c != m => items.push(ChangeItem {
                op: "modified".into(),
                label: m.target.clone(),
                before: Some(mount_text(c)),
                after: Some(mount_text(m)),
            }),
            _ => {}
        }
    }
    for c in &current.mounts {
        if !desired.mounts.iter().any(|m| m.target == c.target) {
            items.push(ChangeItem {
                op: "removed".into(),
                label: c.target.clone(),
                before: Some(mount_text(c)),
                after: None,
            });
        }
    }
    push_list(
        &mut changes,
        &mut kept,
        "mounts",
        items,
        current.mounts.len(),
        desired.mounts.len(),
    );

    (changes, kept)
}

fn push_list(
    changes: &mut Vec<Change>,
    kept: &mut Vec<String>,
    field: &str,
    items: Vec<ChangeItem>,
    before: usize,
    after: usize,
) {
    if items.is_empty() {
        kept.push(field.into());
    } else {
        changes.push(Change {
            field: field.into(),
            kind: ChangeKind::Recreate,
            reason: None,
            before: before.to_string(),
            after: after.to_string(),
            items,
        });
    }
}

// ------------------------------------------------------------ validación

/// Valida lo deseado reutilizando las reglas del asistente de creación. Solo
/// se devuelven los problemas de los apartados que el usuario ha cambiado: lo
/// que ya estaba no debe impedir, por ejemplo, un cambio de nombre.
pub fn validate(
    current: &EditRequest,
    desired: &EditRequest,
    changes: &[Change],
    auto_remove: bool,
    check_fs: bool,
) -> Vec<SpecIssue> {
    let changed = |f: &str| changes.iter().any(|c| c.field == f);
    let spec = ContainerSpec {
        name: desired.name.clone(),
        image: "placeholder".into(),
        env: desired.env.clone(),
        ports: desired.ports.clone(),
        mounts: desired.mounts.clone(),
        cpus: desired.cpus,
        memory_bytes: desired.memory_bytes,
        memory_reservation_bytes: desired.memory_reservation_bytes,
        restart: desired.restart.clone(),
        auto_remove,
        ..Default::default()
    };
    let mut out: Vec<SpecIssue> = spec
        .validate(check_fs)
        .into_iter()
        .filter(|i| {
            let f = i.field.as_str();
            if f == "name" {
                changed("name")
            } else if f.starts_with("env") {
                changed("env")
            } else if f.starts_with("ports") {
                changed("ports")
            } else if f.starts_with("mounts") {
                // Un montaje que no se toca no bloquea el resto.
                changed("mounts")
                    && f.split('.')
                        .nth(1)
                        .and_then(|i| i.parse::<usize>().ok())
                        .and_then(|i| desired.mounts.get(i))
                        .map(|m| !current.mounts.contains(m))
                        .unwrap_or(true)
            } else if f == "cpus" {
                changed("cpus")
            } else if f.starts_with("memory") {
                changed("memory") || changed("memoryReservation")
            } else if f == "restart" || f == "autoRemove" {
                changed("restart")
            } else {
                false
            }
        })
        .collect();
    if desired.name.is_empty() {
        out.push(issue(Severity::Error, "name", "name.required", &[]));
    }
    if changed("name")
        && (desired.name.contains(BACKUP_INFIX) || desired.name.contains(REPLACED_INFIX))
    {
        out.push(issue(Severity::Error, "name", "name.backupReserved", &[]));
    }
    if changed("description") {
        if desired.description.chars().count() > MAX_DESCRIPTION {
            out.push(issue(
                Severity::Error,
                "description",
                "description.tooLong",
                &[("max", MAX_DESCRIPTION.to_string())],
            ));
        }
        if desired
            .description
            .chars()
            .any(|c| c.is_control() && c != '\n')
        {
            out.push(issue(
                Severity::Error,
                "description",
                "description.invalid",
                &[],
            ));
        }
    }
    if changed("mounts") {
        for (i, m) in desired.mounts.iter().enumerate() {
            if m.kind == MountKind::Volume && m.source.is_empty() && !current.mounts.contains(m) {
                // Un volumen anónimo nuevo no tendría nombre que conservar después.
                out.push(issue(
                    Severity::Error,
                    format!("mounts.{i}.source"),
                    "volume.nameRequired",
                    &[],
                ));
            }
        }
    }
    out
}

// ------------------------------------------------------------ cuerpos

fn restart_policy(r: &RestartSpec) -> RestartPolicy {
    RestartPolicy {
        name: Some(match r.name {
            RestartName::No => RestartPolicyNameEnum::NO,
            RestartName::Always => RestartPolicyNameEnum::ALWAYS,
            RestartName::UnlessStopped => RestartPolicyNameEnum::UNLESS_STOPPED,
            RestartName::OnFailure => RestartPolicyNameEnum::ON_FAILURE,
        }),
        maximum_retry_count: Some(if r.name == RestartName::OnFailure {
            r.max_retries as i64
        } else {
            0
        }),
    }
}

/// `MemorySwap` que acompaña a un límite de memoria nuevo. Docker rechaza una
/// memoria mayor que el total memoria+swap ya fijado, así que se conserva la
/// cantidad de swap que tenía el contenedor.
fn swap_for(host: &HostConfig, new_memory: i64) -> Option<i64> {
    let old_mem = host.memory.unwrap_or(0);
    match host.memory_swap.unwrap_or(0) {
        -1 => Some(-1),
        s if s > 0 && old_mem > 0 && s >= old_mem => Some(new_memory + (s - old_mem)),
        s if s > 0 => Some(new_memory.max(s)),
        _ => None,
    }
}

/// Estilo con el que el original fija la CPU, para no mezclar `NanoCpus` con
/// `CpuQuota` (Docker los considera incompatibles).
fn cpu_fields(host: &HostConfig, cpus: f64) -> (Option<i64>, Option<i64>, Option<i64>) {
    let uses_quota = host.nano_cpus.unwrap_or(0) == 0 && host.cpu_quota.unwrap_or(0) > 0;
    if uses_quota {
        let period = host.cpu_period.filter(|p| *p > 0).unwrap_or(100_000);
        (
            None,
            Some((cpus * period as f64).round() as i64),
            Some(period),
        )
    } else {
        (Some((cpus * 1e9).round() as i64), None, None)
    }
}

/// Cuerpo de `POST /containers/{id}/update` con los cambios directos, o `None`
/// si no hay ninguno. Devuelve también los argumentos de `docker update`.
pub fn update_body(
    host: &HostConfig,
    current: &EditRequest,
    desired: &EditRequest,
) -> Option<(ContainerUpdateBody, Vec<String>)> {
    let mut body = ContainerUpdateBody::default();
    let mut args: Vec<String> = Vec::new();
    if normalized_restart(&current.restart) != normalized_restart(&desired.restart) {
        body.restart_policy = Some(restart_policy(&desired.restart));
        args.push("--restart".into());
        args.push(restart_text(&desired.restart));
    }
    if let Some(c) = desired
        .cpus
        .filter(|_| !same_cpus(current.cpus, desired.cpus))
    {
        let (nano, quota, period) = cpu_fields(host, c);
        body.nano_cpus = nano;
        body.cpu_quota = quota;
        body.cpu_period = period;
        match (quota, period) {
            (Some(q), Some(p)) => {
                args.extend(["--cpu-quota".into(), q.to_string()]);
                args.extend(["--cpu-period".into(), p.to_string()]);
            }
            _ => args.extend(["--cpus".into(), format_cpus(c)]),
        }
    }
    if let Some(m) = desired
        .memory_bytes
        .filter(|_| current.memory_bytes != desired.memory_bytes)
    {
        body.memory = Some(m as i64);
        args.extend(["--memory".into(), format_bytes_cli(m)]);
        if let Some(s) = swap_for(host, m as i64) {
            body.memory_swap = Some(s);
            args.extend([
                "--memory-swap".into(),
                if s < 0 {
                    "-1".into()
                } else {
                    format_bytes_cli(s as u64)
                },
            ]);
        }
    }
    if let Some(r) = desired
        .memory_reservation_bytes
        .filter(|_| current.memory_reservation_bytes != desired.memory_reservation_bytes)
    {
        body.memory_reservation = Some(r as i64);
        args.extend(["--memory-reservation".into(), format_bytes_cli(r)]);
    }
    (!args.is_empty()).then_some((body, args))
}

/// Qué redes hay que tratar después de crear el contenedor nuevo.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NetworkPlan {
    /// Redes que se conectan después de crear (la API antigua solo admite una al crear).
    pub connect: Vec<(String, EndpointSettings)>,
    /// La red del modo de red original ya no estaba conectada: se desconecta
    /// del nuevo para dejarlo igual.
    pub disconnect: Vec<String>,
}

fn nonempty(s: &Option<String>) -> bool {
    s.as_ref().is_some_and(|x| !x.is_empty())
}

/// Configuración de una conexión de red para el contenedor nuevo: solo lo que
/// configuró el usuario (IP fija, alias, enlaces), no lo que Docker asignó.
fn endpoint_for(
    e: &EndpointSettings,
    ids: &[String],
    user_defined: bool,
    mac: Option<&String>,
) -> EndpointSettings {
    let ipam = e.ipam_config.clone().filter(|i| {
        nonempty(&i.ipv4_address)
            || nonempty(&i.ipv6_address)
            || i.link_local_ips.as_ref().is_some_and(|l| !l.is_empty())
    });
    // Docker añade el ID corto como alias; el del contenedor nuevo será otro.
    let aliases: Vec<String> = e
        .aliases
        .clone()
        .unwrap_or_default()
        .into_iter()
        .filter(|a| !ids.contains(a))
        .collect();
    EndpointSettings {
        ipam_config: ipam,
        links: e.links.clone().filter(|l| !l.is_empty()),
        aliases: (user_defined && !aliases.is_empty()).then_some(aliases),
        driver_opts: e.driver_opts.clone().filter(|d| !d.is_empty()),
        gw_priority: e.gw_priority.filter(|p| *p != 0),
        mac_address: mac.cloned().filter(|m| !m.is_empty()),
        ..Default::default()
    }
}

fn is_empty_endpoint(e: &EndpointSettings) -> bool {
    *e == EndpointSettings::default()
}

/// `/origen:/este/alias` (inspect) → `origen:alias` (create).
fn fix_link(l: &str) -> String {
    match l.split_once(':') {
        Some((src, alias)) if src.starts_with('/') => format!(
            "{}:{}",
            src.trim_start_matches('/'),
            alias.rsplit('/').next().unwrap_or(alias)
        ),
        _ => l.to_owned(),
    }
}

/// Cuerpo de `POST /containers/create` del contenedor nuevo y el plan de redes.
///
/// `macs` son las direcciones MAC que debe conservar el contenedor nuevo, por
/// red ([`macs_to_keep`]).
pub fn recreate_body(
    c: &ContainerInspectResponse,
    image: &str,
    current: &EditRequest,
    desired: &EditRequest,
    macs: &HashMap<String, String>,
) -> Result<(ContainerCreateBody, NetworkPlan), String> {
    let cfg: ContainerConfig = c.config.clone().ok_or("inspect sin Config")?;
    let mut host: HostConfig = c.host_config.clone().ok_or("inspect sin HostConfig")?;
    let id = c.id.clone().unwrap_or_default();
    let short: String = id.chars().take(12).collect();

    // `ContainerConfig` y el cuerpo de creación comparten los campos de Config.
    let value = serde_json::to_value(&cfg).map_err(|e| e.to_string())?;
    let mut body: ContainerCreateBody = serde_json::from_value(value).map_err(|e| e.to_string())?;
    body.image = Some(image.to_owned());

    let mode = host.network_mode.clone().unwrap_or_default();
    let shared_ns = mode.starts_with("container:") || mode == "host";
    if body.hostname.as_deref() == Some(short.as_str()) || mode.starts_with("container:") {
        body.hostname = None;
    }
    if shared_ns {
        body.domainname = body.domainname.filter(|d| !d.is_empty());
    }
    if mode.starts_with("container:") {
        body.domainname = None;
    }
    host.links = host
        .links
        .map(|l| l.iter().map(|x| fix_link(x)).collect::<Vec<_>>())
        .filter(|l| !l.is_empty());

    // --- variables
    if current.env != desired.env {
        // Solo las propias: las de la imagen se vuelven a heredar.
        let own_before: Vec<String> = current
            .env
            .iter()
            .map(|kv| format!("{}={}", kv.key, kv.value))
            .collect();
        let inherited: Vec<String> = cfg
            .env
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter(|e| !own_before.contains(e))
            .collect();
        let mut env = inherited;
        for kv in &desired.env {
            let prefix = format!("{}=", kv.key);
            env.retain(|e| !e.starts_with(&prefix) && *e != kv.key);
            env.push(format!("{}={}", kv.key, kv.value));
        }
        body.env = Some(env);
    }

    // --- descripción
    if current.description != desired.description {
        let mut labels = body.labels.take().unwrap_or_default();
        if desired.description.is_empty() {
            labels.remove(DESCRIPTION_LABEL);
        } else {
            labels.insert(DESCRIPTION_LABEL.into(), desired.description.clone());
        }
        body.labels = Some(labels);
    }

    // --- puertos
    if current.ports != desired.ports {
        let tmp = ContainerSpec {
            ports: desired.ports.clone(),
            ..Default::default()
        };
        let (_, b) = tmp.to_create_body();
        let new_bindings: HashMap<String, Option<Vec<PortBinding>>> = b
            .host_config
            .and_then(|h| h.port_bindings)
            .unwrap_or_default();
        // Los puertos que solo estaban expuestos por una publicación retirada dejan de estarlo.
        let old_keys: Vec<String> = host
            .port_bindings
            .clone()
            .unwrap_or_default()
            .into_keys()
            .collect();
        let mut exposed: Vec<String> = body.exposed_ports.take().unwrap_or_default();
        exposed.retain(|k| !old_keys.contains(k) || new_bindings.contains_key(k));
        for k in new_bindings.keys() {
            if !exposed.contains(k) {
                exposed.push(k.clone());
            }
        }
        exposed.sort();
        body.exposed_ports = (!exposed.is_empty()).then_some(exposed);
        host.port_bindings = Some(new_bindings);
    }

    // --- montajes
    let (orig, other) = original_mounts(c);
    let mut binds: Vec<String> = Vec::new();
    let mut mounts: Vec<Mount> = other;
    let bind_string = |m: &MountSpec| {
        format!(
            "{}:{}{}",
            m.source,
            m.target,
            if m.read_only { ":ro" } else { "" }
        )
    };
    for m in &desired.mounts {
        match orig.iter().find(|o| o.spec == *m) {
            // Sin cambios: se declara exactamente como estaba.
            Some(OrigMount {
                raw: RawMount::Bind(b),
                ..
            }) => binds.push(b.clone()),
            Some(OrigMount {
                raw: RawMount::Mount(x),
                ..
            }) => mounts.push((**x).clone()),
            // Anónimo (se monta por su nombre) o nuevo.
            _ => binds.push(bind_string(m)),
        }
    }
    host.binds = (!binds.is_empty()).then_some(binds);
    host.mounts = (!mounts.is_empty()).then_some(mounts);
    // Una ruta de volumen declarada con `-v /ruta` y retirada deja de declararse.
    if let Some(vols) = body.volumes.as_mut() {
        let removed: Vec<&str> = orig
            .iter()
            .filter(|o| !desired.mounts.iter().any(|m| m.target == o.spec.target))
            .map(|o| o.spec.target.as_str())
            .collect();
        vols.retain(|v| !removed.contains(&v.as_str()));
    }
    body.volumes = body.volumes.filter(|v| !v.is_empty());

    // --- reinicio y recursos
    if normalized_restart(&current.restart) != normalized_restart(&desired.restart) {
        host.restart_policy = Some(restart_policy(&desired.restart));
    }
    if !same_cpus(current.cpus, desired.cpus) {
        match desired.cpus {
            Some(cpus) => {
                let (nano, quota, period) = cpu_fields(&host, cpus);
                host.nano_cpus = nano;
                host.cpu_quota = quota;
                host.cpu_period = period;
            }
            None => {
                host.nano_cpus = None;
                host.cpu_quota = None;
                host.cpu_period = None;
            }
        }
    }
    if current.memory_bytes != desired.memory_bytes {
        match desired.memory_bytes {
            Some(m) => {
                host.memory_swap = swap_for(&host, m as i64);
                host.memory = Some(m as i64);
            }
            None => {
                host.memory = None;
                // El swap era relativo al límite que desaparece.
                host.memory_swap = host.memory_swap.filter(|s| *s == -1);
            }
        }
    }
    if current.memory_reservation_bytes != desired.memory_reservation_bytes {
        host.memory_reservation = desired.memory_reservation_bytes.map(|m| m as i64);
    }

    // --- redes
    let mut plan = NetworkPlan::default();
    let no_endpoints = shared_ns || mode == "none";
    if !no_endpoints {
        let ids = [short.clone(), id.clone()];
        let mut nets: Vec<(String, EndpointSettings)> = c
            .network_settings
            .as_ref()
            .and_then(|n| n.networks.clone())
            .unwrap_or_default()
            .into_iter()
            .collect();
        nets.sort_by(|a, b| a.0.cmp(&b.0));
        let primary_name = if mode.is_empty() || mode == "default" {
            "bridge".to_owned()
        } else {
            mode.clone()
        };
        let primary = nets.iter().position(|(n, e)| {
            *n == primary_name
                || e.network_id
                    .as_deref()
                    .is_some_and(|i| !primary_name.is_empty() && i.starts_with(&primary_name))
        });
        match primary {
            Some(i) => {
                let (name, e) = nets.remove(i);
                let ep = endpoint_for(&e, &ids, name != "bridge", macs.get(&name));
                if !is_empty_endpoint(&ep) {
                    body.networking_config = Some(NetworkingConfig {
                        endpoints_config: Some(HashMap::from([(name, ep)])),
                    });
                }
            }
            None => plan.disconnect.push(primary_name),
        }
        for (name, e) in nets {
            let ep = endpoint_for(&e, &ids, name != "bridge", macs.get(&name));
            plan.connect.push((name, ep));
        }
    }

    body.host_config = Some(host);
    Ok((body, plan))
}

/// Direcciones MAC que conserva el contenedor nuevo, por red (D-071).
///
/// Docker no permite saber si la MAC de un contenedor que se ha iniciado la
/// fijó el usuario (`--mac-address`) o se asignó sola, y deja de mostrarla al
/// detenerlo. Por eso:
/// - en un contenedor que nunca se inició, la MAC que figura es la configurada
///   y se conserva siempre;
/// - en uno en ejecución se conservan las actuales solo si el usuario lo pide
///   (`keep_running`);
/// - en uno detenido no se pueden conocer.
pub fn macs_to_keep(c: &ContainerInspectResponse, keep_running: bool) -> HashMap<String, String> {
    let status = status_of(c);
    if !(status == "created" || (status == "running" && keep_running)) {
        return HashMap::new();
    }
    c.network_settings
        .as_ref()
        .and_then(|n| n.networks.clone())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(k, e)| e.mac_address.filter(|m| !m.is_empty()).map(|m| (k, m)))
        .collect()
}

/// Copia del cuerpo con los valores sensibles ocultos, para mostrar y registrar.
pub fn redacted_body(body: &ContainerCreateBody) -> ContainerCreateBody {
    let mut b = body.clone();
    if let Some(env) = b.env.as_mut() {
        for e in env.iter_mut() {
            if let Some((k, v)) = e.split_once('=') {
                if is_secret_key(k) && !v.is_empty() {
                    *e = format!("{k}={REDACTED}");
                }
            }
        }
    }
    b
}

/// `<nombre>-dcc-backup-<AAAAMMDD-HHMMSS>` (hora UTC). Si el nombre ya existe
/// (dos recreaciones en el mismo segundo) se avanza la hora hasta que quede libre.
pub fn backup_name(
    name: &str,
    infix: &str,
    now: chrono::DateTime<chrono::Utc>,
    taken: &[String],
) -> String {
    let mut t = now;
    loop {
        let candidate = format!("{name}{infix}{}", t.format("%Y%m%d-%H%M%S"));
        if !taken.contains(&candidate) {
            return candidate;
        }
        t += chrono::Duration::seconds(1);
    }
}

/// Nombre original de una copia de seguridad, si el nombre tiene esa forma.
pub fn backup_origin(name: &str) -> Option<&str> {
    let (orig, stamp) = name.rsplit_once(BACKUP_INFIX)?;
    let ok = stamp.len() == 15
        && stamp.chars().enumerate().all(
            |(i, c)| {
                if i == 8 {
                    c == '-'
                } else {
                    c.is_ascii_digit()
                }
            },
        );
    (ok && !orig.is_empty()).then_some(orig)
}

/// Puertos del equipo que ya publica otro contenedor en ejecución.
pub fn port_conflicts(
    desired: &[PortSpec],
    others: &[crate::containers::ContainerSummary],
    own_id: &str,
) -> Vec<SpecIssue> {
    let any = |ip: &str| ip.is_empty() || ip == "0.0.0.0" || ip == "::";
    let mut out = Vec::new();
    for (i, p) in desired.iter().enumerate() {
        let Some(hp) = p.host_port else { continue };
        for o in others {
            if o.id == own_id || o.state != "running" {
                continue;
            }
            let clash = o.ports.iter().any(|q| {
                let qip = q.ip.as_deref().unwrap_or("");
                q.public_port == Some(hp)
                    && q.protocol == p.protocol.as_str()
                    && (any(qip) || any(&p.host_ip) || qip == p.host_ip)
            });
            if clash {
                out.push(issue(
                    Severity::Error,
                    format!("ports.{i}.hostPort"),
                    "port.inUseByContainer",
                    &[("port", hp.to_string()), ("container", o.name.clone())],
                ));
                break;
            }
        }
    }
    out
}

pub(crate) fn risk(code: &str, params: &[(&str, String)]) -> Risk {
    Risk {
        code: code.into(),
        params: params
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect(),
    }
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
