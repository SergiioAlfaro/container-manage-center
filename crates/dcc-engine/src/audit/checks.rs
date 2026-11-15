// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Comprobaciones de la auditoría (E3, D-093). Funciones puras: reciben lo que
//! devuelve Docker (`inspect`, imagen, `compose config`) y no ejecutan nada.
//!
//! Los riesgos para el equipo salen de [`crate::risk`] (las mismas reglas que
//! al crear, editar y desplegar). Aquí se añaden las comprobaciones de
//! **endurecimiento**, que no tienen sentido como aviso al crear pero sí en una
//! auditoría: usuario root, sistema de archivos modificable, recursos sin
//! límite, `no-new-privileges` y antigüedad de la imagen.

use std::collections::BTreeMap;

use bollard::models::{ContainerInspectResponse, HostConfig};
use chrono::{DateTime, Utc};
use serde_json::Value;

use super::model::{AuditFinding, AuditFix, DaemonSecurity, TargetKind, TargetRef};
use crate::risk::{self, Risk, RiskLevel};

/// Antigüedad a partir de la cual se avisa de una imagen (días).
pub const IMAGE_OLD_DAYS: i64 = 365;

fn finding(target: &TargetRef, r: Risk, fix: Option<AuditFix>) -> AuditFinding {
    AuditFinding {
        target: target.clone(),
        code: r.code,
        level: r.level,
        field: r.field,
        key: r.key,
        params: r.params,
        fix,
    }
}

fn hardening(code: &str, level: RiskLevel, field: &str, subject: &str) -> Risk {
    Risk::new(code, level, field, subject)
}

/// ¿Es root el usuario (`""`, `root`, `0`, `0:0`, `root:grupo`)?
pub fn is_root_user(user: &str) -> bool {
    let u = user.trim();
    let name = u.split(':').next().unwrap_or("");
    u.is_empty() || name == "root" || name == "0"
}

/// ¿Activa `no-new-privileges`? (`no-new-privileges`, `…:true`, `…=true`).
pub fn has_no_new_privileges(opts: &[String]) -> bool {
    opts.iter().any(|o| {
        let o = o.trim().to_ascii_lowercase().replacen(':', "=", 1);
        o == "no-new-privileges" || o == "no-new-privileges=true"
    })
}

/// Usuario root dentro del contenedor. Con Docker rootless o `userns-remap`
/// ese root no es el del equipo: el aviso baja a bajo.
/// ¿Usa el daemon la protección a la que se refiere la opción? (`seccomp`,
/// `apparmor`, `label` = SELinux). Las demás opciones, siempre.
fn lsm_in_use(opt: &str, daemon: &DaemonSecurity) -> bool {
    let o = opt.trim().to_ascii_lowercase();
    if o.starts_with("seccomp") {
        daemon.seccomp
    } else if o.starts_with("apparmor") {
        daemon.apparmor
    } else if o.starts_with("label") {
        daemon.selinux
    } else {
        true
    }
}

fn root_user_risk(user: &str, field: &str, daemon: &DaemonSecurity) -> Option<Risk> {
    if !is_root_user(user) {
        return None;
    }
    let mapped = daemon.rootless || daemon.userns;
    let level = if mapped {
        RiskLevel::Low
    } else {
        RiskLevel::Medium
    };
    Some(hardening("audit.rootUser", level, field, "user").with("mapped", mapped.to_string()))
}

/// Antigüedad de una imagen a partir de su fecha de creación. Las fechas
/// anteriores a 2000 no son fiables (las compilaciones reproducibles usan el
/// 1-1-1970) y no se evalúan.
pub fn image_age_days(created: &str, now: DateTime<Utc>) -> Option<i64> {
    let c = DateTime::parse_from_rfc3339(created.trim())
        .ok()?
        .with_timezone(&Utc);
    if c.format("%Y").to_string().parse::<i32>().ok()? < 2000 {
        return None;
    }
    Some((now - c).num_days())
}

fn image_old_risk(created: Option<&str>, now: DateTime<Utc>, field: &str) -> Option<Risk> {
    let days = image_age_days(created?, now)?;
    (days > IMAGE_OLD_DAYS).then(|| {
        hardening("audit.imageOld", RiskLevel::Low, field, "image")
            .with("days", days.to_string())
            .with(
                "created",
                created
                    .unwrap_or_default()
                    .chars()
                    .take(10)
                    .collect::<String>(),
            )
    })
}

/// Añade sin repetir la misma clave (p. ej. AppArmor `unconfined` aparece en
/// `SecurityOpt` y en `AppArmorProfile`).
fn push(out: &mut Vec<AuditFinding>, f: AuditFinding) {
    if !out
        .iter()
        .any(|x| x.key == f.key && x.target.id == f.target.id)
    {
        out.push(f);
    }
}

fn enum_str<T: serde::Serialize>(v: &Option<T>) -> String {
    v.as_ref()
        .and_then(|x| serde_json::to_value(x).ok())
        .and_then(|x| x.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Comprobaciones de un contenedor a partir de su `inspect`.
/// `local`: Docker de este equipo (los enlaces simbólicos se resuelven aquí).
/// `image_created`: fecha de creación de su imagen, si se conoce.
pub fn check_container(
    c: &ContainerInspectResponse,
    image_created: Option<&str>,
    daemon: &DaemonSecurity,
    local: bool,
    now: DateTime<Utc>,
) -> Vec<AuditFinding> {
    let id = c.id.clone().unwrap_or_default();
    let name = c
        .name
        .clone()
        .unwrap_or_default()
        .trim_start_matches('/')
        .to_owned();
    let target = TargetRef {
        kind: TargetKind::Container,
        id: id.clone(),
        name,
        project: c
            .config
            .as_ref()
            .and_then(|cfg| cfg.labels.as_ref())
            .and_then(|l| l.get("com.docker.compose.project").cloned()),
    };
    let default_hc = HostConfig::default();
    let hc = c.host_config.as_ref().unwrap_or(&default_hc);
    let mut out = Vec::new();

    // --- riesgos para el equipo (reglas de `risk`)
    let privileged = hc.privileged.unwrap_or(false);
    if privileged {
        push(
            &mut out,
            finding(&target, risk::privileged_risk("privileged"), None),
        );
    }
    for r in risk::cap_risks(hc.cap_add.as_deref().unwrap_or_default(), "capAdd") {
        push(&mut out, finding(&target, r, None));
    }
    for m in c.mounts.as_deref().unwrap_or_default() {
        if enum_str(&m.typ) != "bind" {
            continue;
        }
        let (Some(src), Some(dst)) = (&m.source, &m.destination) else {
            continue;
        };
        let rw = m.rw.unwrap_or(true);
        let field = format!("mounts.{dst}");
        if let Some(r) = risk::mount_risk(src, !rw, local, &field) {
            // Corrección propuesta: solo lectura, si eso baja el riesgo.
            let ro_level = risk::mount_risk(src, true, local, &field).map(|x| x.level);
            let fix =
                (rw && ro_level.map_or(true, |l| l < r.level)).then(|| AuditFix::MountReadOnly {
                    target: dst.clone(),
                });
            push(
                &mut out,
                finding(&target, r.with("target", dst.clone()), fix),
            );
        }
    }
    let net = hc.network_mode.clone().unwrap_or_default();
    if let Some(r) = risk::network_risk(&net, "network") {
        push(&mut out, finding(&target, r, None));
    }
    let ns = [
        ("pid", hc.pid_mode.clone().unwrap_or_default()),
        ("ipc", hc.ipc_mode.clone().unwrap_or_default()),
        ("uts", hc.uts_mode.clone().unwrap_or_default()),
        ("userns", hc.userns_mode.clone().unwrap_or_default()),
        ("cgroup", enum_str(&hc.cgroupns_mode)),
    ];
    for (kind, mode) in ns {
        // Con cgroup v1, Docker usa por defecto el espacio de cgroups del
        // equipo: no es una elección del contenedor y no se avisa. Con v2 el
        // valor por defecto es privado, así que `host` se pidió expresamente.
        if kind == "cgroup" && daemon.cgroup_version.as_deref() != Some("2") {
            continue;
        }
        if let Some(r) = risk::namespace_risk(kind, &mode, kind) {
            push(&mut out, finding(&target, r, None));
        }
    }
    let sec = hc.security_opt.clone().unwrap_or_default();
    for o in &sec {
        // Desactivar una protección que el daemon no usa no cambia nada (y
        // Docker añade `label=disable` por su cuenta con `--pid host`).
        if !lsm_in_use(o, daemon) {
            continue;
        }
        if let Some(r) = risk::security_opt_risk(o, "securityOpt") {
            push(&mut out, finding(&target, r, None));
        }
    }
    // Un contenedor privilegiado ya no tiene AppArmor: no se repite.
    if !privileged && c.app_armor_profile.as_deref() == Some("unconfined") && daemon.apparmor {
        if let Some(r) = risk::security_opt_risk("apparmor=unconfined", "securityOpt") {
            push(&mut out, finding(&target, r, None));
        }
    }
    for d in hc.devices.as_deref().unwrap_or_default() {
        if let Some(p) = &d.path_on_host {
            if let Some(r) = risk::device_risk(p, "devices") {
                push(&mut out, finding(&target, r, None));
            }
        }
    }
    if let Some(r) =
        risk::cgroup_parent_risk(hc.cgroup_parent.as_deref().unwrap_or(""), "cgroupParent")
    {
        push(&mut out, finding(&target, r, None));
    }
    if net != "host" {
        let mut ports: Vec<_> = hc
            .port_bindings
            .as_ref()
            .map(|p| p.iter().collect::<Vec<_>>())
            .unwrap_or_default();
        ports.sort_by(|a, b| a.0.cmp(b.0));
        for (spec, bindings) in ports {
            let (cport, proto) = spec.split_once('/').unwrap_or((spec.as_str(), "tcp"));
            for b in bindings.as_deref().unwrap_or_default() {
                let ip = b.host_ip.clone().unwrap_or_default();
                let hp = b.host_port.clone().unwrap_or_default();
                if let Some(r) = risk::port_risk(&ip, &hp, cport, "ports") {
                    let fix = cport.parse::<u16>().ok().map(|p| AuditFix::PortLocalhost {
                        container_port: p,
                        protocol: proto.to_owned(),
                        host_port: hp.clone(),
                    });
                    push(&mut out, finding(&target, r.with("protocol", proto), fix));
                }
            }
        }
    }

    // --- endurecimiento
    let user = c
        .config
        .as_ref()
        .and_then(|cfg| cfg.user.clone())
        .unwrap_or_default();
    if let Some(r) = root_user_risk(&user, "user", daemon) {
        push(&mut out, finding(&target, r, None));
    }
    if !hc.readonly_rootfs.unwrap_or(false) {
        push(
            &mut out,
            finding(
                &target,
                hardening(
                    "audit.writableRootfs",
                    RiskLevel::Low,
                    "readOnlyRootfs",
                    "rootfs",
                ),
                None,
            ),
        );
    }
    if hc.memory.unwrap_or(0) <= 0 {
        push(
            &mut out,
            finding(
                &target,
                hardening("audit.noMemoryLimit", RiskLevel::Medium, "memory", "memory"),
                Some(AuditFix::Resources),
            ),
        );
    }
    if hc.nano_cpus.unwrap_or(0) <= 0 && hc.cpu_quota.unwrap_or(0) <= 0 {
        push(
            &mut out,
            finding(
                &target,
                hardening("audit.noCpuLimit", RiskLevel::Low, "cpus", "cpus"),
                Some(AuditFix::Resources),
            ),
        );
    }
    if hc.pids_limit.unwrap_or(0) <= 0 {
        push(
            &mut out,
            finding(
                &target,
                hardening("audit.noPidsLimit", RiskLevel::Low, "pidsLimit", "pids"),
                None,
            ),
        );
    }
    if !privileged && !has_no_new_privileges(&sec) {
        push(
            &mut out,
            finding(
                &target,
                hardening(
                    "audit.noNewPrivileges",
                    RiskLevel::Low,
                    "securityOpt",
                    "nnp",
                ),
                None,
            ),
        );
    }
    if let Some(r) = image_old_risk(image_created, now, "image") {
        let img = c
            .config
            .as_ref()
            .and_then(|cfg| cfg.image.clone())
            .unwrap_or_default();
        push(&mut out, finding(&target, r.with("image", img), None));
    }
    out
}

/// Comprobaciones de una imagen: usuario por defecto y antigüedad.
pub fn check_image(
    id: &str,
    name: &str,
    user: &str,
    created: Option<&str>,
    daemon: &DaemonSecurity,
    now: DateTime<Utc>,
) -> Vec<AuditFinding> {
    let target = TargetRef {
        kind: TargetKind::Image,
        id: id.to_owned(),
        name: name.to_owned(),
        project: None,
    };
    let mut out = Vec::new();
    if let Some(mut r) = root_user_risk(user, "user", daemon) {
        // Mismo criterio que en los contenedores, con su propio texto: es el
        // usuario por defecto de la imagen.
        r.code = "audit.imageRootUser".into();
        r.key = "audit.imageRootUser:user".into();
        out.push(finding(&target, r, None));
    }
    if let Some(r) = image_old_risk(created, now, "created") {
        out.push(finding(&target, r, None));
    }
    out
}

fn as_bool(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s == "true",
        _ => false,
    }
}

fn positive(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Number(n)) => n.as_f64().is_some_and(|x| x > 0.0),
        Some(Value::String(s)) => {
            let s = s.trim();
            !s.is_empty() && s != "0" && !s.starts_with('-')
        }
        _ => false,
    }
}

/// Comprobaciones de un servicio de Compose (de `docker compose config
/// --format json`): sus riesgos para el equipo (ya calculados por el modelo)
/// y el endurecimiento.
pub fn check_compose_service(
    project: &str,
    service: &str,
    s: &Value,
    risks: &[Risk],
    daemon: &DaemonSecurity,
) -> Vec<AuditFinding> {
    let target = TargetRef {
        kind: TargetKind::Service,
        id: format!("{project}/{service}"),
        name: service.to_owned(),
        project: Some(project.to_owned()),
    };
    let mut out = Vec::new();
    for r in risks {
        push(&mut out, finding(&target, r.clone(), None));
    }
    let field = |f: &str| format!("services.{service}.{f}");
    let user = s.get("user").and_then(Value::as_str).unwrap_or("");
    if let Some(r) = root_user_risk(user, &field("user"), daemon) {
        push(&mut out, finding(&target, r, None));
    }
    if !as_bool(s.get("read_only")) {
        push(
            &mut out,
            finding(
                &target,
                hardening(
                    "audit.writableRootfs",
                    RiskLevel::Low,
                    &field("read_only"),
                    "rootfs",
                ),
                None,
            ),
        );
    }
    let limits = s.pointer("/deploy/resources/limits");
    if !positive(s.get("mem_limit")) && !positive(limits.and_then(|l| l.get("memory"))) {
        push(
            &mut out,
            finding(
                &target,
                hardening(
                    "audit.noMemoryLimit",
                    RiskLevel::Medium,
                    &field("mem_limit"),
                    "memory",
                ),
                None,
            ),
        );
    }
    if !positive(s.get("cpus"))
        && !positive(limits.and_then(|l| l.get("cpus")))
        && !positive(s.get("cpu_quota"))
    {
        push(
            &mut out,
            finding(
                &target,
                hardening("audit.noCpuLimit", RiskLevel::Low, &field("cpus"), "cpus"),
                None,
            ),
        );
    }
    if !positive(s.get("pids_limit")) && !positive(limits.and_then(|l| l.get("pids"))) {
        push(
            &mut out,
            finding(
                &target,
                hardening(
                    "audit.noPidsLimit",
                    RiskLevel::Low,
                    &field("pids_limit"),
                    "pids",
                ),
                None,
            ),
        );
    }
    let sec: Vec<String> = s
        .get("security_opt")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    if !as_bool(s.get("privileged")) && !has_no_new_privileges(&sec) {
        push(
            &mut out,
            finding(
                &target,
                hardening(
                    "audit.noNewPrivileges",
                    RiskLevel::Low,
                    &field("security_opt"),
                    "nnp",
                ),
                None,
            ),
        );
    }
    out
}

/// Seguridad del daemon a partir de `SecurityOptions` de `docker info`.
pub fn daemon_security(
    options: &[String],
    rootless: bool,
    cgroup_version: Option<&str>,
) -> DaemonSecurity {
    let names: Vec<BTreeMap<String, String>> = options
        .iter()
        .map(|o| {
            o.split(',')
                .filter_map(|p| p.split_once('='))
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect()
        })
        .collect();
    let has = |n: &str| {
        names
            .iter()
            .any(|m| m.get("name").map(String::as_str) == Some(n))
    };
    let seccomp_profile = names
        .iter()
        .find(|m| m.get("name").map(String::as_str) == Some("seccomp"))
        .and_then(|m| m.get("profile").cloned());
    DaemonSecurity {
        rootless: rootless || has("rootless"),
        userns: has("userns"),
        seccomp: has("seccomp"),
        seccomp_profile,
        apparmor: has("apparmor"),
        selinux: has("selinux"),
        cgroup_version: cgroup_version.map(str::to_owned),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-13T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn daemon() -> DaemonSecurity {
        daemon_security(
            &[
                "name=apparmor".into(),
                "name=seccomp,profile=builtin".into(),
                "name=cgroupns".into(),
            ],
            false,
            Some("2"),
        )
    }

    fn inspect(v: Value) -> ContainerInspectResponse {
        serde_json::from_value(v).unwrap()
    }

    fn codes(f: &[AuditFinding]) -> Vec<String> {
        let mut c: Vec<String> = f
            .iter()
            .map(|x| format!("{}:{}", x.code, x.level.as_str()))
            .collect();
        c.sort();
        c
    }

    /// Contenedor peligroso: cada comprobación de riesgo y de endurecimiento.
    #[test]
    fn dangerous_container_triggers_every_check() {
        let c = inspect(serde_json::json!({
            "Id": "abc", "Name": "/peligroso",
            "Config": { "User": "", "Image": "viejo:1" },
            "AppArmorProfile": "unconfined",
            "HostConfig": {
                "Privileged": false,
                "CapAdd": ["SYS_ADMIN", "NET_RAW"],
                "NetworkMode": "bridge",
                "PidMode": "host", "IpcMode": "host", "UTSMode": "host", "UsernsMode": "host",
                "CgroupnsMode": "host",
                "SecurityOpt": ["seccomp=unconfined", "label=disable"],
                "Devices": [{ "PathOnHost": "/dev/sda", "PathInContainer": "/dev/sda", "CgroupPermissions": "rwm" }],
                "CgroupParent": "propio",
                "PortBindings": { "80/tcp": [{ "HostIp": "", "HostPort": "8080" }], "53/udp": [{ "HostIp": "127.0.0.1", "HostPort": "53" }] },
                "ReadonlyRootfs": false, "Memory": 0, "NanoCpus": 0, "CpuQuota": 0, "PidsLimit": null
            },
            "Mounts": [
                { "Type": "bind", "Source": "/var/run/docker.sock", "Destination": "/var/run/docker.sock", "RW": false },
                { "Type": "bind", "Source": "/etc/nginx", "Destination": "/etc/nginx", "RW": true },
                { "Type": "volume", "Name": "datos", "Source": "/var/lib/docker/volumes/datos/_data", "Destination": "/datos", "RW": true }
            ]
        }));
        let f = check_container(&c, Some("2023-01-01T00:00:00Z"), &daemon(), false, now());
        assert_eq!(
            codes(&f),
            vec![
                "audit.imageOld:low",
                "audit.noCpuLimit:low",
                "audit.noMemoryLimit:medium",
                "audit.noNewPrivileges:low",
                "audit.noPidsLimit:low",
                "audit.rootUser:medium",
                "audit.writableRootfs:low",
                "risk.capability:critical",
                "risk.capability:low",
                "risk.cgroupParent:low",
                "risk.device:high",
                "risk.hostNamespace:high",
                "risk.hostNamespace:high",
                "risk.hostNamespace:low",
                "risk.hostNamespace:medium",
                "risk.hostNamespace:medium",
                "risk.portAllInterfaces:low",
                "risk.runtimeSocket:critical",
                "risk.securityOpt:high",
                "risk.securityOpt:high",
                "risk.systemDir:high",
            ],
            "{f:#?}"
        );
        // Correcciones propuestas: el puerto, la carpeta de /etc con escritura
        // y los límites. El socket no (en solo lectura sigue siendo crítico).
        let fix = |code: &str| {
            f.iter()
                .find(|x| x.code == code)
                .and_then(|x| x.fix.clone())
        };
        assert_eq!(
            fix("risk.portAllInterfaces"),
            Some(AuditFix::PortLocalhost {
                container_port: 80,
                protocol: "tcp".into(),
                host_port: "8080".into()
            })
        );
        assert_eq!(
            fix("risk.systemDir"),
            Some(AuditFix::MountReadOnly {
                target: "/etc/nginx".into()
            })
        );
        assert_eq!(fix("risk.runtimeSocket"), None);
        // label=disable solo cuenta con SELinux activo en el daemon.
        assert!(!f.iter().any(|x| x.key.contains("label=disable")));
        let mut selinux = daemon();
        selinux.selinux = true;
        let f2 = check_container(&c, None, &selinux, false, now());
        assert!(f2.iter().any(|x| x.key.contains("label=disable")));
        // Sin seccomp ni AppArmor en el daemon, desactivarlos no cambia nada.
        let none = daemon_security(&[], false, Some("2"));
        let f3 = check_container(&c, None, &none, false, now());
        assert!(!f3.iter().any(|x| x.code == "risk.securityOpt"), "{f3:#?}");
        assert_eq!(fix("audit.noMemoryLimit"), Some(AuditFix::Resources));
        assert!(f
            .iter()
            .all(|x| x.target.name == "peligroso" && x.target.kind == TargetKind::Container));
    }

    /// Contenedor legítimo y endurecido: ningún hallazgo.
    #[test]
    fn hardened_legitimate_container_has_no_findings() {
        let c = inspect(serde_json::json!({
            "Id": "def", "Name": "/web",
            "Config": { "User": "101:101", "Image": "nginx:1.27" },
            "AppArmorProfile": "docker-default",
            "HostConfig": {
                "CapDrop": ["ALL"], "CapAdd": ["NET_BIND_SERVICE"],
                "NetworkMode": "propia", "IpcMode": "private", "PidMode": "", "CgroupnsMode": "private",
                "SecurityOpt": ["no-new-privileges:true"],
                "PortBindings": { "80/tcp": [{ "HostIp": "127.0.0.1", "HostPort": "8080" }] },
                "ReadonlyRootfs": true, "Memory": 268435456, "NanoCpus": 500000000, "PidsLimit": 100
            },
            "Mounts": [
                { "Type": "bind", "Source": "/home/ana/web/html", "Destination": "/usr/share/nginx/html", "RW": false },
                { "Type": "volume", "Name": "cache", "Source": "/var/lib/docker/volumes/cache/_data", "Destination": "/var/cache/nginx", "RW": true },
                { "Type": "tmpfs", "Destination": "/tmp", "RW": true }
            ]
        }));
        let f = check_container(&c, Some("2026-10-01T00:00:00Z"), &daemon(), false, now());
        assert!(f.is_empty(), "{f:#?}");
    }

    #[test]
    fn privileged_is_not_reported_twice_and_root_is_lower_when_mapped() {
        let c = inspect(serde_json::json!({
            "Id": "p", "Name": "/p", "Config": { "User": "root" }, "AppArmorProfile": "unconfined",
            "HostConfig": { "Privileged": true, "ReadonlyRootfs": true, "Memory": 1, "NanoCpus": 1, "PidsLimit": 10 }
        }));
        let f = check_container(&c, None, &daemon(), false, now());
        assert_eq!(
            codes(&f),
            vec!["audit.rootUser:medium", "risk.privileged:critical"]
        );
        // cgroup v1: el espacio de cgroups del equipo es el valor por defecto.
        let v1 = daemon_security(&[], false, Some("1"));
        let c1 = inspect(serde_json::json!({
            "Id": "c", "Name": "/c", "Config": { "User": "1000" },
            "HostConfig": { "CgroupnsMode": "host", "ReadonlyRootfs": true, "Memory": 1, "NanoCpus": 1, "PidsLimit": 10,
                            "SecurityOpt": ["no-new-privileges"] }
        }));
        assert!(check_container(&c1, None, &v1, false, now()).is_empty());
        assert_eq!(
            codes(&check_container(&c1, None, &daemon(), false, now())),
            vec!["risk.hostNamespace:medium"]
        );
        let mut rootless = daemon();
        rootless.rootless = true;
        let f = check_container(&c, None, &rootless, false, now());
        assert!(codes(&f).contains(&"audit.rootUser:low".to_owned()));
    }

    #[test]
    fn users_dates_and_options() {
        for root in ["", "root", "0", "0:0", "root:staff", " root "] {
            assert!(is_root_user(root), "{root:?}");
        }
        for not_root in ["1000", "app", "101:101", "nobody", "rooty"] {
            assert!(!is_root_user(not_root), "{not_root:?}");
        }
        assert!(has_no_new_privileges(&["no-new-privileges".into()]));
        assert!(has_no_new_privileges(&["no-new-privileges=true".into()]));
        assert!(has_no_new_privileges(&["no-new-privileges:true".into()]));
        assert!(!has_no_new_privileges(&["no-new-privileges=false".into()]));
        assert_eq!(image_age_days("2025-11-13T12:00:00Z", now()), Some(365));
        assert_eq!(
            image_age_days("1970-01-01T00:00:00Z", now()),
            None,
            "fecha de compilación reproducible"
        );
        assert_eq!(image_age_days("no es fecha", now()), None);
        let d = daemon_security(&["name=rootless".into(), "name=userns".into()], false, None);
        assert!(d.rootless && d.userns && !d.seccomp);
        assert_eq!(daemon().seccomp_profile.as_deref(), Some("builtin"));
    }

    #[test]
    fn images_root_and_age() {
        let f = check_image(
            "sha256:1",
            "viejo:1",
            "",
            Some("2020-01-01T00:00:00Z"),
            &daemon(),
            now(),
        );
        assert_eq!(
            codes(&f),
            vec!["audit.imageOld:low", "audit.imageRootUser:medium"]
        );
        let f = check_image(
            "sha256:2",
            "nuevo:1",
            "app",
            Some("2026-11-01T00:00:00Z"),
            &daemon(),
            now(),
        );
        assert!(f.is_empty());
        let f = check_image(
            "sha256:3",
            "distroless",
            "nonroot",
            Some("1970-01-01T00:00:00Z"),
            &daemon(),
            now(),
        );
        assert!(f.is_empty(), "fecha no fiable: no se evalúa");
    }

    #[test]
    fn compose_services_dangerous_and_hardened() {
        let bad = serde_json::json!({ "image": "x", "privileged": false });
        let risks = vec![risk::privileged_risk("services.web.privileged")];
        let f = check_compose_service("tienda", "web", &bad, &risks, &daemon());
        assert_eq!(
            codes(&f),
            vec![
                "audit.noCpuLimit:low",
                "audit.noMemoryLimit:medium",
                "audit.noNewPrivileges:low",
                "audit.noPidsLimit:low",
                "audit.rootUser:medium",
                "audit.writableRootfs:low",
                "risk.privileged:critical",
            ]
        );
        assert!(f
            .iter()
            .all(|x| x.target.id == "tienda/web" && x.target.project.as_deref() == Some("tienda")));
        let good = serde_json::json!({
            "image": "x", "user": "1000", "read_only": true,
            "security_opt": ["no-new-privileges:true"],
            "deploy": { "resources": { "limits": { "memory": "268435456", "cpus": "0.5", "pids": 100 } } }
        });
        assert!(check_compose_service("tienda", "web", &good, &[], &daemon()).is_empty());
        let good2 = serde_json::json!({
            "image": "x", "user": "app", "read_only": "true", "mem_limit": "268435456", "cpus": 0.5, "pids_limit": 50,
            "security_opt": ["no-new-privileges"]
        });
        assert!(check_compose_service("tienda", "db", &good2, &[], &daemon()).is_empty());
    }
}
