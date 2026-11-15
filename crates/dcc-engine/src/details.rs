// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Vista tipada de `GET /containers/{id}/json` para la pantalla de detalle.

use std::collections::BTreeMap;

use bollard::models::{ContainerInspectResponse, RestartPolicyNameEnum};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::containers::PortMapping;
use crate::spec::{KeyValue, RestartName, RestartSpec};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NetworkAttachment {
    pub name: String,
    pub ip_address: String,
    pub gateway: String,
    pub mac_address: String,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MountInfo {
    /// bind, volume, tmpfs, npipe, cluster, image.
    pub kind: String,
    /// Nombre del volumen (si es un volumen).
    pub name: Option<String>,
    pub source: String,
    pub destination: String,
    pub read_only: bool,
    pub driver: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContainerDetails {
    pub id: String,
    pub name: String,
    /// Referencia de imagen usada al crear (p. ej. `nginx:1.27`).
    pub image: String,
    pub image_id: String,
    pub created: String,
    pub status: String,
    pub running: bool,
    pub paused: bool,
    pub restarting: bool,
    pub oom_killed: bool,
    #[ts(type = "number | null")]
    pub exit_code: Option<i64>,
    pub error: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub health: Option<String>,
    #[ts(type = "number")]
    pub restart_count: i64,
    #[ts(type = "number | null")]
    pub pid: Option<i64>,
    pub command: Vec<String>,
    pub entrypoint: Vec<String>,
    pub working_dir: String,
    pub user: String,
    pub hostname: String,
    pub tty: bool,
    pub open_stdin: bool,
    pub env: Vec<KeyValue>,
    pub labels: BTreeMap<String, String>,
    pub network_mode: String,
    pub networks: Vec<NetworkAttachment>,
    /// Puertos publicados realmente (incluidos los asignados por Docker) y
    /// expuestos sin publicar (`publicPort = null`).
    pub ports: Vec<PortMapping>,
    pub mounts: Vec<MountInfo>,
    pub restart: RestartSpec,
    pub cpus: Option<f64>,
    #[ts(type = "number | null")]
    pub memory_bytes: Option<u64>,
    #[ts(type = "number | null")]
    pub memory_reservation_bytes: Option<u64>,
    pub privileged: bool,
    pub auto_remove: bool,
    pub read_only_rootfs: bool,
    pub init: bool,
    pub compose_project: Option<String>,
    /// JSON completo tal como lo devuelve Docker (formato de `docker inspect`).
    pub raw: String,
}

fn enum_str<T: Serialize>(v: Option<T>) -> String {
    v.and_then(|x| serde_json::to_value(x).ok())
        .and_then(|x| x.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn nonempty(s: Option<String>) -> Option<String> {
    s.filter(|x| !x.is_empty() && x != "0001-01-01T00:00:00Z")
}

pub fn details_from(c: &ContainerInspectResponse) -> ContainerDetails {
    let cfg = c.config.clone().unwrap_or_default();
    let host = c.host_config.clone().unwrap_or_default();
    let st = c.state.clone().unwrap_or_default();
    let ns = c.network_settings.clone().unwrap_or_default();

    let mut networks: Vec<NetworkAttachment> = ns
        .networks
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|(name, e)| NetworkAttachment {
            name,
            ip_address: e.ip_address.unwrap_or_default(),
            gateway: e.gateway.unwrap_or_default(),
            mac_address: e.mac_address.unwrap_or_default(),
            aliases: e.aliases.unwrap_or_default(),
        })
        .collect();
    networks.sort_by(|a, b| a.name.cmp(&b.name));

    // Puertos reales (NetworkSettings.Ports). Si el contenedor no está en marcha
    // están vacíos: entonces se muestran los configurados (HostConfig).
    let mut ports: Vec<PortMapping> = Vec::new();
    let live = ns.ports.clone().unwrap_or_default();
    let source = if live.is_empty() {
        host.port_bindings.clone().unwrap_or_default()
    } else {
        live
    };
    for (key, binds) in source {
        let (port, proto) = key.split_once('/').unwrap_or((key.as_str(), "tcp"));
        let Ok(private_port) = port.parse::<u16>() else {
            continue;
        };
        match binds {
            Some(list) if !list.is_empty() => {
                for b in list {
                    ports.push(PortMapping {
                        ip: b.host_ip.filter(|i| !i.is_empty()),
                        private_port,
                        public_port: b.host_port.as_deref().and_then(|p| p.parse().ok()),
                        protocol: proto.to_owned(),
                    });
                }
            }
            _ => ports.push(PortMapping {
                ip: None,
                private_port,
                public_port: None,
                protocol: proto.to_owned(),
            }),
        }
    }
    for e in cfg.exposed_ports.clone().unwrap_or_default() {
        let (port, proto) = e.split_once('/').unwrap_or((e.as_str(), "tcp"));
        if let Ok(p) = port.parse::<u16>() {
            if !ports
                .iter()
                .any(|x| x.private_port == p && x.protocol == proto)
            {
                ports.push(PortMapping {
                    ip: None,
                    private_port: p,
                    public_port: None,
                    protocol: proto.to_owned(),
                });
            }
        }
    }
    ports.sort_by(|a, b| {
        (a.private_port, &a.protocol, &a.ip).cmp(&(b.private_port, &b.protocol, &b.ip))
    });

    let mut mounts: Vec<MountInfo> = c
        .mounts
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|m| MountInfo {
            kind: m.typ.unwrap_or_default(),
            name: m.name.filter(|n| !n.is_empty()),
            source: m.source.unwrap_or_default(),
            destination: m.destination.unwrap_or_default(),
            read_only: !m.rw.unwrap_or(true),
            driver: m.driver.filter(|d| !d.is_empty()),
        })
        .collect();
    mounts.sort_by(|a, b| a.destination.cmp(&b.destination));

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
    let cpus = match (host.nano_cpus, host.cpu_quota, host.cpu_period) {
        (Some(n), _, _) if n > 0 => Some(n as f64 / 1e9),
        (_, Some(q), Some(p)) if q > 0 && p > 0 => Some(q as f64 / p as f64),
        _ => None,
    };
    let labels: BTreeMap<String, String> =
        cfg.labels.clone().unwrap_or_default().into_iter().collect();

    ContainerDetails {
        id: c.id.clone().unwrap_or_default(),
        name: c
            .name
            .clone()
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_owned(),
        image: cfg.image.clone().unwrap_or_default(),
        image_id: c.image.clone().unwrap_or_default(),
        created: c
            .created
            .as_ref()
            .map(|d| d.to_string())
            .unwrap_or_default(),
        status: enum_str(st.status),
        running: st.running.unwrap_or(false),
        paused: st.paused.unwrap_or(false),
        restarting: st.restarting.unwrap_or(false),
        oom_killed: st.oom_killed.unwrap_or(false),
        exit_code: st.exit_code,
        error: nonempty(st.error),
        started_at: nonempty(st.started_at),
        finished_at: nonempty(st.finished_at),
        health: st
            .health
            .and_then(|h| h.status)
            .map(|s| enum_str(Some(s)))
            .filter(|s| !s.is_empty() && s != "none"),
        restart_count: c.restart_count.unwrap_or(0),
        pid: st.pid.filter(|p| *p > 0),
        command: cfg.cmd.clone().unwrap_or_default(),
        entrypoint: cfg.entrypoint.clone().unwrap_or_default(),
        working_dir: cfg.working_dir.clone().unwrap_or_default(),
        user: cfg.user.clone().unwrap_or_default(),
        hostname: cfg.hostname.clone().unwrap_or_default(),
        tty: cfg.tty.unwrap_or(false),
        open_stdin: cfg.open_stdin.unwrap_or(false),
        env: cfg
            .env
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|e| match e.split_once('=') {
                Some((k, v)) => KeyValue::new(k, v),
                None => KeyValue::new(e, ""),
            })
            .collect(),
        compose_project: labels.get("com.docker.compose.project").cloned(),
        labels,
        network_mode: host.network_mode.clone().unwrap_or_default(),
        networks,
        ports,
        mounts,
        restart,
        cpus,
        memory_bytes: host.memory.filter(|m| *m > 0).map(|m| m as u64),
        memory_reservation_bytes: host.memory_reservation.filter(|m| *m > 0).map(|m| m as u64),
        privileged: host.privileged.unwrap_or(false),
        auto_remove: host.auto_remove.unwrap_or(false),
        read_only_rootfs: host.readonly_rootfs.unwrap_or(false),
        init: host.init.unwrap_or(false),
        raw: serde_json::to_string_pretty(c).unwrap_or_default(),
    }
}
