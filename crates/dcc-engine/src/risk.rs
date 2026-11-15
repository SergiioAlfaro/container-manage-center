// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Reglas de riesgo de la configuración de contenedores (E1, DCC-SEC-01 y
//! DCC-SEC-03, D-086).
//!
//! Un único lugar decide qué configuraciones dan a un contenedor acceso
//! excesivo al equipo. Lo usan el asistente de creación, la edición, el
//! despliegue de Compose y la pantalla de directorios vinculados (y, más
//! adelante, la auditoría de seguridad). Las reglas solo **avisan**: nunca se
//! cambia lo que el usuario configura. Las de nivel alto o crítico exigen una
//! aceptación explícita, que el motor comprueba (no solo la interfaz).
//!
//! Importante: montar el socket de Docker **en solo lectura no protege nada**:
//! `:ro` impide escribir en el archivo del socket, no usar la API a través de
//! él. Quien tiene el socket puede crear un contenedor privilegiado con `/`
//! montado y ser root en el equipo.

use std::collections::BTreeMap;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Nivel de un riesgo. Ordenado de menor a mayor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    /// Los niveles alto y crítico exigen una aceptación explícita.
    pub fn needs_acceptance(self) -> bool {
        self >= Self::High
    }
}

/// Un riesgo detectado. `key` identifica el riesgo concreto (código + sujeto)
/// y es lo que la interfaz devuelve al aceptarlo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "HostRisk")]
pub struct Risk {
    /// Código estable (`risk.runtimeSocket`, `risk.capability`…), traducido en
    /// la interfaz con `issues.<código>`.
    pub code: String,
    pub level: RiskLevel,
    /// Campo de la especificación (`mounts.0.source`, `capAdd`, `privileged`,
    /// `network`) o, en Compose, `services.<servicio>.<campo>`.
    pub field: String,
    /// Identificador del riesgo para aceptarlo: `código:sujeto`.
    pub key: String,
    pub params: BTreeMap<String, String>,
}

impl Risk {
    pub fn new(code: &str, level: RiskLevel, field: impl Into<String>, subject: &str) -> Self {
        let mut params = BTreeMap::new();
        params.insert("level".to_owned(), level.as_str().to_owned());
        Self {
            code: code.to_owned(),
            level,
            field: field.into(),
            key: format!("{code}:{subject}"),
            params,
        }
    }

    pub fn with(mut self, k: &str, v: impl Into<String>) -> Self {
        self.params.insert(k.to_owned(), v.into());
        self
    }
}

/// Riesgos de nivel alto o crítico que faltan en `accepted` (por `key`).
pub fn unaccepted<'a>(risks: &'a [Risk], accepted: &[String]) -> Vec<&'a Risk> {
    risks
        .iter()
        .filter(|r| r.level.needs_acceptance() && !accepted.iter().any(|a| a == &r.key))
        .collect()
}

// ------------------------------------------------------------------ montajes

/// Sockets de motores de contenedores: dan control total del equipo.
const RUNTIME_SOCKETS: [&str; 6] = [
    "/var/run/docker.sock",
    "/run/docker.sock",
    "/run/containerd/containerd.sock",
    "/var/run/containerd/containerd.sock",
    "/run/podman/podman.sock",
    "/var/run/podman/podman.sock",
];
const SOCKET_NAMES: [&str; 3] = ["docker.sock", "containerd.sock", "podman.sock"];

/// Archivos y carpetas con credenciales del equipo: alto aunque sea `:ro`.
const CREDENTIAL_PATHS: [&str; 6] = [
    "/etc/shadow",
    "/etc/gshadow",
    "/etc/sudoers",
    "/etc/sudoers.d",
    "/etc/ssh",
    "/root",
];
/// Carpetas de cada usuario con credenciales (`/home/<usuario>/…`).
const HOME_CREDENTIAL_DIRS: [&str; 5] = [".ssh", ".gnupg", ".docker", ".kube", ".aws"];

/// Carpetas del sistema: alto con escritura, medio en solo lectura.
const SYSTEM_DIRS: [&str; 6] = [
    "/boot",
    "/proc",
    "/sys",
    "/dev",
    "/var/lib/docker",
    "/var/lib/containerd",
];
/// Programas y bibliotecas del sistema: medio con escritura.
const SYSTEM_BINARIES: [&str; 6] = ["/usr", "/bin", "/sbin", "/lib", "/lib64", "/lib32"];

/// Normalización léxica: une barras, quita `.` y la barra final. `None` si la
/// ruta no es absoluta o tiene `..` (eso ya lo rechaza la validación).
pub fn normalize(path: &str) -> Option<String> {
    let p = path.trim();
    if !p.starts_with('/') || p.contains('\0') {
        return None;
    }
    let mut parts = Vec::new();
    for c in Path::new(p).components() {
        match c {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
            Component::ParentDir | Component::Prefix(_) => return None,
        }
    }
    Some(format!("/{}", parts.join("/")))
}

fn under(p: &str, base: &str) -> bool {
    p == base || (p.starts_with(base) && p.as_bytes().get(base.len()) == Some(&b'/'))
}

/// ¿Contiene `dir` (o es) alguno de los sockets conocidos?
fn holds_runtime_socket(p: &str) -> bool {
    if SOCKET_NAMES.iter().any(|n| p.rsplit('/').next() == Some(n)) {
        return true;
    }
    if RUNTIME_SOCKETS.iter().any(|s| under(s, p)) {
        return true;
    }
    // Docker o Podman rootless: /run/user/<uid>/{docker,podman}.sock.
    let seg: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
    matches!(
        seg.as_slice(),
        ["run", "user"] | ["run", "user", _] | ["var", "run", "user"] | ["var", "run", "user", _]
    )
}

fn home_credentials(p: &str) -> bool {
    let seg: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
    match seg.as_slice() {
        // /home o /home/<usuario> enteros contienen las credenciales.
        ["home"] | ["home", _] => true,
        ["home", _, dir, ..] => HOME_CREDENTIAL_DIRS.contains(dir),
        _ => false,
    }
}

/// Riesgo de montar `path` del equipo (ya normalizada).
fn classify(p: &str, read_only: bool) -> Option<(&'static str, RiskLevel)> {
    use RiskLevel::*;
    if p == "/" {
        return Some(("risk.hostRoot", Critical));
    }
    if holds_runtime_socket(p) {
        return Some(("risk.runtimeSocket", Critical));
    }
    // /etc entero, sus archivos de credenciales, /root y las carpetas de
    // credenciales de los usuarios: alto aunque sea en solo lectura.
    if p == "/etc" || CREDENTIAL_PATHS.iter().any(|c| under(p, c)) || home_credentials(p) {
        return Some(("risk.credentials", High));
    }
    // El resto de /etc: con escritura cambia la configuración del equipo; en
    // solo lectura (`/etc/nginx:ro`, `/etc/localtime:ro`) es habitual.
    if under(p, "/etc") {
        return (!read_only).then_some(("risk.systemDir", High));
    }
    // También una carpeta que las contiene (`/var/lib` contiene /var/lib/docker).
    if SYSTEM_DIRS.iter().any(|d| under(p, d) || under(d, p)) {
        return Some(("risk.systemDir", if read_only { Medium } else { High }));
    }
    if !read_only && SYSTEM_BINARIES.iter().any(|d| under(p, d)) {
        return Some(("risk.systemBinaries", Medium));
    }
    None
}

/// Riesgo de un montaje del equipo. `local`: el daemon está en este equipo, así
/// que se resuelven además los enlaces simbólicos (solo para avisar; lo que se
/// envía a Docker no cambia).
pub fn mount_risk(source: &str, read_only: bool, local: bool, field: &str) -> Option<Risk> {
    let norm = normalize(source)?;
    let mut found = classify(&norm, read_only).map(|(c, l)| (c, l, norm.clone()));
    if local {
        if let Ok(real) = std::fs::canonicalize(&norm) {
            let real = real.to_string_lossy().into_owned();
            if real != norm {
                if let Some((c, l)) = classify(&real, read_only) {
                    if found.as_ref().map_or(true, |f| l > f.1) {
                        found = Some((c, l, real));
                    }
                }
            }
        }
    }
    let (code, level, resolved) = found?;
    let mut r = Risk::new(code, level, field, &norm)
        .with("path", norm.clone())
        .with("readOnly", read_only.to_string());
    if resolved != norm {
        r = r.with("resolved", resolved);
    }
    Some(r)
}

// --------------------------------------------------------------- capacidades

/// Nombre normalizado de una capacidad (`cap_sys_admin` → `SYS_ADMIN`).
pub fn normalize_cap(cap: &str) -> String {
    let c = cap.trim().to_ascii_uppercase();
    c.strip_prefix("CAP_").map(str::to_owned).unwrap_or(c)
}

/// Capacidades que Docker concede por defecto (las de `cap_add` repetidas no
/// cambian nada, pero se avisa igualmente porque la configuración las pide).
const DEFAULT_CAPS: [&str; 14] = [
    "CHOWN",
    "DAC_OVERRIDE",
    "FSETID",
    "FOWNER",
    "MKNOD",
    "NET_RAW",
    "SETGID",
    "SETUID",
    "SETFCAP",
    "SETPCAP",
    "NET_BIND_SERVICE",
    "SYS_CHROOT",
    "KILL",
    "AUDIT_WRITE",
];

/// Nivel de riesgo de añadir una capacidad.
pub fn cap_level(cap: &str) -> Option<RiskLevel> {
    use RiskLevel::*;
    match normalize_cap(cap).as_str() {
        "ALL" | "SYS_ADMIN" => Some(Critical),
        "SYS_PTRACE" | "SYS_MODULE" | "SYS_RAWIO" | "DAC_READ_SEARCH" | "NET_ADMIN" | "BPF"
        | "PERFMON" | "MAC_ADMIN" | "MAC_OVERRIDE" | "SYS_BOOT" | "SYS_TIME" | "SYSLOG"
        | "LINUX_IMMUTABLE" => Some(High),
        "DAC_OVERRIDE" | "SYS_RESOURCE" | "SYS_NICE" | "IPC_LOCK" => Some(Medium),
        "NET_RAW" => Some(Low),
        _ => None,
    }
}

/// Riesgos de `cap_add` (Docker: lo añadido prevalece sobre `cap_drop`, también
/// con `cap_drop: [ALL]`). `cap_drop` no reduce estos avisos; quitar
/// capacidades no es un riesgo.
pub fn cap_risks(cap_add: &[String], field: &str) -> Vec<Risk> {
    let mut seen = std::collections::BTreeSet::new();
    cap_add
        .iter()
        .filter_map(|c| {
            let n = normalize_cap(c);
            let level = cap_level(&n)?;
            seen.insert(n.clone()).then(|| {
                Risk::new("risk.capability", level, field, &n)
                    .with("cap", n.clone())
                    .with("default", DEFAULT_CAPS.contains(&n.as_str()).to_string())
            })
        })
        .collect()
}

// ------------------------------------------------------------------- resto

pub fn privileged_risk(field: &str) -> Risk {
    Risk::new("risk.privileged", RiskLevel::Critical, field, "privileged")
}

/// Red del equipo (`host`): el contenedor ve todos los servicios del equipo,
/// también los que solo escuchan en 127.0.0.1.
pub fn network_risk(mode: &str, field: &str) -> Option<Risk> {
    (mode.trim() == "host").then(|| Risk::new("risk.networkHost", RiskLevel::High, field, "host"))
}

/// Espacios de nombres del equipo (E2, D-088): `pid: host` permite ver y
/// depurar los procesos del equipo y `ipc: host` su memoria compartida (alto);
/// `userns_mode: host` anula la reasignación de usuarios del daemon y
/// `cgroup: host` comparte el árbol de cgroups (medio); `uts: host` solo el
/// nombre del equipo (bajo).
pub fn namespace_risk(kind: &str, mode: &str, field: &str) -> Option<Risk> {
    if mode.trim() != "host" {
        return None;
    }
    let level = match kind {
        "pid" | "ipc" => RiskLevel::High,
        "userns" | "cgroup" => RiskLevel::Medium,
        _ => RiskLevel::Low,
    };
    Some(Risk::new("risk.hostNamespace", level, field, kind).with("namespace", kind))
}

/// Opciones de seguridad (`security_opt`) que desactivan protecciones del
/// núcleo (E2, D-088). Se aceptan las dos sintaxis (`seccomp=…`, `seccomp:…`).
pub fn security_opt_risk(opt: &str, field: &str) -> Option<Risk> {
    let o = opt.trim().to_ascii_lowercase().replacen(':', "=", 1);
    let (level, what) = match o.as_str() {
        "seccomp=unconfined" => (RiskLevel::High, "seccomp"),
        "apparmor=unconfined" => (RiskLevel::High, "apparmor"),
        "label=disable" => (RiskLevel::High, "selinux"),
        "systempaths=unconfined" => (RiskLevel::High, "systempaths"),
        "no-new-privileges=false" => (RiskLevel::Low, "noNewPrivileges"),
        _ => return None,
    };
    Some(
        Risk::new("risk.securityOpt", level, field, &o)
            .with("option", opt.trim())
            .with("protection", what),
    )
}

/// Dispositivos del equipo (`devices`): memoria del núcleo, crítico; discos,
/// alto (acceso directo a los datos sin permisos de archivo); otros, medio;
/// los habituales de multimedia, red virtual o FUSE, bajo.
pub fn device_risk(device: &str, field: &str) -> Option<Risk> {
    // `origen[:destino[:permisos]]` o solo el origen.
    let src = device.trim().split(':').next().unwrap_or("").trim();
    let path = normalize(src)?;
    let name = path.trim_start_matches("/dev/");
    let level = if matches!(name, "mem" | "kmem" | "port") {
        RiskLevel::Critical
    } else if [
        "sd", "hd", "vd", "xvd", "nvme", "mmcblk", "loop", "dm-", "md", "sr",
    ]
    .iter()
    .any(|p| name.starts_with(p))
        || name.starts_with("disk/")
        || name.starts_with("mapper/")
    {
        RiskLevel::High
    } else if [
        "dri", "snd", "video", "fuse", "net/tun", "kvm", "ttyUSB", "ttyACM", "bus/usb", "input",
        "hidraw", "nvidia",
    ]
    .iter()
    .any(|p| name.starts_with(p))
    {
        RiskLevel::Low
    } else {
        RiskLevel::Medium
    };
    Some(Risk::new("risk.device", level, field, &path).with("device", path.clone()))
}

/// `cgroup_parent`: el contenedor queda en un grupo de control elegido a mano,
/// fuera de los límites que gestiona Docker (bajo).
pub fn cgroup_parent_risk(parent: &str, field: &str) -> Option<Risk> {
    let p = parent.trim();
    (!p.is_empty())
        .then(|| Risk::new("risk.cgroupParent", RiskLevel::Low, field, p).with("parent", p))
}

/// Puerto publicado en todas las interfaces (sin IP o con 0.0.0.0 / ::): se
/// puede acceder desde otros equipos, y en Docker las reglas del cortafuegos
/// de UFW o firewalld no lo impiden en su configuración habitual (bajo, E2,
/// DCC-SEC-08).
pub fn port_risk(
    host_ip: &str,
    host_port: &str,
    container_port: &str,
    field: &str,
) -> Option<Risk> {
    let ip = host_ip.trim().trim_matches(['[', ']']);
    if !(ip.is_empty() || ip == "0.0.0.0" || ip == "::") {
        return None;
    }
    let shown = if host_port.is_empty() {
        container_port
    } else {
        host_port
    };
    Some(
        Risk::new("risk.portAllInterfaces", RiskLevel::Low, field, shown)
            .with("port", shown)
            .with("containerPort", container_port),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use RiskLevel::*;

    fn m(p: &str, ro: bool) -> Option<(String, RiskLevel)> {
        mount_risk(p, ro, false, "mounts.0.source").map(|r| (r.code, r.level))
    }

    #[test]
    fn docker_socket_is_critical_even_read_only() {
        for p in [
            "/var/run/docker.sock",
            "/run/docker.sock",
            "//var/run//docker.sock",
            "/var/./run/docker.sock/",
            "/run/containerd/containerd.sock",
            "/run/user/1000/docker.sock",
            "/run/user/1000",
            "/run/podman/podman.sock",
            "/var/run",
            "/run",
            "/srv/otro/docker.sock",
        ] {
            for ro in [false, true] {
                assert_eq!(
                    m(p, ro),
                    Some(("risk.runtimeSocket".into(), Critical)),
                    "{p} ro={ro}"
                );
            }
        }
    }

    #[test]
    fn host_root_and_credentials() {
        assert_eq!(m("/", true), Some(("risk.hostRoot".into(), Critical)));
        assert_eq!(m("//", false), Some(("risk.hostRoot".into(), Critical)));
        for p in [
            "/etc",
            "/etc/",
            "//etc",
            "/./etc",
            "/etc/shadow",
            "/etc/sudoers.d/x",
            "/etc/ssh",
            "/etc/ssh/ssh_host_ed25519_key",
            "/root",
            "/root/.bashrc",
            "/home",
            "/home/ana",
            "/home/ana/.ssh",
            "/home/ana/.ssh/id_ed25519",
            "/home/ana/.docker/config.json",
            "/home/ana/.gnupg",
        ] {
            for ro in [false, true] {
                assert_eq!(
                    m(p, ro),
                    Some(("risk.credentials".into(), High)),
                    "{p} ro={ro}"
                );
            }
        }
    }

    #[test]
    fn system_dirs_and_binaries() {
        assert_eq!(m("/proc", false), Some(("risk.systemDir".into(), High)));
        assert_eq!(m("/proc", true), Some(("risk.systemDir".into(), Medium)));
        assert_eq!(
            m("/var/lib/docker/volumes", false),
            Some(("risk.systemDir".into(), High))
        );
        assert_eq!(m("/dev/kmsg", false), Some(("risk.systemDir".into(), High)));
        assert_eq!(
            m("/usr", false),
            Some(("risk.systemBinaries".into(), Medium))
        );
        assert_eq!(m("/usr/lib", true), None);
        assert_eq!(m("/var/lib", true), Some(("risk.systemDir".into(), Medium)));
        assert_eq!(
            m("/var", true),
            Some(("risk.runtimeSocket".into(), Critical))
        );
    }

    #[test]
    fn legitimate_mounts_have_no_risk() {
        for (p, ro) in [
            ("/home/ana/proyecto", false),
            ("/home/ana/proyecto/html", true),
            ("/srv/html", true),
            ("/srv/datos", false),
            ("/opt/app/config", true),
            ("/var/lib/app", false),
            ("/etc/localtime", true),
            ("/etc/timezone", true),
            ("/etc/ssl/certs", true),
            ("/etc/nginx/conf.d", true),
            ("/mnt/backup", false),
            ("/tmp/x", false),
            ("/home/ana/proyecto/.dockerignore", true),
            ("/var/run-app/data", false),
            ("/runner/data", false),
            ("/etcetera", false),
        ] {
            assert_eq!(m(p, ro), None, "{p} ro={ro}");
        }
        // Con escritura, cualquier parte de /etc es un riesgo.
        assert_eq!(
            m("/etc/localtime", false),
            Some(("risk.systemDir".into(), High))
        );
        assert_eq!(m("/etc/nginx", true), None);
    }

    #[test]
    fn symlinks_are_resolved_locally() {
        let d = tempfile::tempdir().unwrap();
        let link = d.path().join("enlace");
        std::os::unix::fs::symlink("/etc", &link).unwrap();
        let r = mount_risk(link.to_str().unwrap(), true, true, "mounts.0.source").unwrap();
        assert_eq!((r.code.as_str(), r.level), ("risk.credentials", High));
        assert_eq!(r.params.get("resolved").map(String::as_str), Some("/etc"));
        // En un servidor remoto no se mira el sistema de archivos local.
        assert!(mount_risk(link.to_str().unwrap(), true, false, "x").is_none());
    }

    #[test]
    fn capabilities() {
        let caps = |v: &[&str]| -> Vec<(String, RiskLevel)> {
            cap_risks(
                &v.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                "capAdd",
            )
            .into_iter()
            .map(|r| (r.params["cap"].clone(), r.level))
            .collect()
        };
        assert_eq!(caps(&["ALL"]), vec![("ALL".into(), Critical)]);
        assert_eq!(
            caps(&["CAP_SYS_ADMIN", "sys_admin"]),
            vec![("SYS_ADMIN".into(), Critical)]
        );
        assert_eq!(
            caps(&[
                "SYS_PTRACE",
                "NET_ADMIN",
                "CAP_NET_RAW",
                "DAC_OVERRIDE",
                "DAC_READ_SEARCH"
            ]),
            vec![
                ("SYS_PTRACE".into(), High),
                ("NET_ADMIN".into(), High),
                ("NET_RAW".into(), Low),
                ("DAC_OVERRIDE".into(), Medium),
                ("DAC_READ_SEARCH".into(), High),
            ]
        );
        assert!(caps(&["NET_BIND_SERVICE", "CHOWN", "SETUID"]).is_empty());
        let r = &cap_risks(&["NET_RAW".into()], "capAdd")[0];
        assert_eq!(r.params["default"], "true");
        assert_eq!(r.key, "risk.capability:NET_RAW");
    }

    #[test]
    fn namespaces_security_options_devices_and_ports() {
        let lv = |r: Option<Risk>| r.map(|r| (r.code, r.level));
        assert_eq!(
            lv(namespace_risk("pid", "host", "pid")),
            Some(("risk.hostNamespace".into(), High))
        );
        assert_eq!(
            lv(namespace_risk("ipc", "host", "ipc")),
            Some(("risk.hostNamespace".into(), High))
        );
        assert_eq!(
            lv(namespace_risk("userns", "host", "u")),
            Some(("risk.hostNamespace".into(), Medium))
        );
        assert_eq!(
            lv(namespace_risk("uts", "host", "u")),
            Some(("risk.hostNamespace".into(), Low))
        );
        assert_eq!(namespace_risk("pid", "service:db", "pid"), None);
        assert_eq!(namespace_risk("ipc", "shareable", "ipc"), None);
        for o in [
            "seccomp=unconfined",
            "seccomp:unconfined",
            "apparmor=unconfined",
            "label=disable",
            "label:disable",
            "systempaths=unconfined",
        ] {
            assert_eq!(
                security_opt_risk(o, "s").map(|r| r.level),
                Some(High),
                "{o}"
            );
        }
        assert_eq!(
            security_opt_risk("no-new-privileges=false", "s").map(|r| r.level),
            Some(Low)
        );
        for ok in [
            "no-new-privileges",
            "no-new-privileges:true",
            "seccomp=/perfil.json",
            "apparmor=docker-default",
            "label=type:svirt_lxc_net_t",
        ] {
            assert_eq!(security_opt_risk(ok, "s"), None, "{ok}");
        }
        let dl = |d: &str| device_risk(d, "devices").map(|r| r.level);
        assert_eq!(dl("/dev/mem"), Some(Critical));
        assert_eq!(dl("/dev/sda:/dev/xvda:rwm"), Some(High));
        assert_eq!(dl("/dev/nvme0n1"), Some(High));
        assert_eq!(dl("/dev/disk/by-id/x"), Some(High));
        assert_eq!(dl("/dev/dri"), Some(Low));
        assert_eq!(dl("/dev/net/tun"), Some(Low));
        assert_eq!(dl("/dev/ttyS0"), Some(Medium));
        assert_eq!(cgroup_parent_risk("", "c"), None);
        assert_eq!(
            cgroup_parent_risk("mi-grupo", "c").map(|r| r.level),
            Some(Low)
        );
        assert_eq!(
            lv(port_risk("", "8080", "80", "p")),
            Some(("risk.portAllInterfaces".into(), Low))
        );
        assert!(port_risk("0.0.0.0", "8080", "80", "p").is_some());
        assert!(port_risk("::", "", "80", "p").is_some());
        assert_eq!(port_risk("127.0.0.1", "8080", "80", "p"), None);
        assert_eq!(port_risk("192.168.1.10", "8080", "80", "p"), None);
    }

    #[test]
    fn acceptance() {
        let risks = vec![
            privileged_risk("privileged"),
            network_risk("host", "network").unwrap(),
            cap_risks(&["NET_RAW".into()], "capAdd").remove(0),
        ];
        let missing: Vec<_> = unaccepted(&risks, &[])
            .iter()
            .map(|r| r.key.clone())
            .collect();
        assert_eq!(
            missing,
            vec!["risk.privileged:privileged", "risk.networkHost:host"]
        );
        assert!(unaccepted(
            &risks,
            &[
                "risk.privileged:privileged".into(),
                "risk.networkHost:host".into()
            ]
        )
        .is_empty());
        assert!(network_risk("bridge", "network").is_none());
    }
}
