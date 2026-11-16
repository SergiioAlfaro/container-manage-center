// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Perfil de un servidor remoto accesible por SSH (0.10.0, D-075).
//!
//! El perfil solo contiene datos no sensibles. Los secretos (frase de paso,
//! contraseña) nunca forman parte de él: se piden a un [`super::SecretProvider`]
//! en el momento de conectar (D-077).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Puerto SSH por defecto.
pub const DEFAULT_PORT: u16 = 22;
/// Segundos de espera por defecto para establecer la conexión SSH.
pub const DEFAULT_CONNECT_TIMEOUT: u32 = 10;
pub const MIN_CONNECT_TIMEOUT: u32 = 3;
pub const MAX_CONNECT_TIMEOUT: u32 = 120;
pub const MAX_NAME: usize = 80;
pub const MAX_DESCRIPTION: usize = 500;
pub const MAX_PATH: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SshProfile {
    /// Identificador estable (no cambia al renombrar).
    pub id: String,
    pub name: String,
    /// Nombre DNS, IPv4, IPv6 o alias de `~/.ssh/config`.
    pub host: String,
    pub port: u16,
    pub user: String,
    #[serde(default)]
    pub description: String,
    pub auth: SshAuth,
    #[serde(default)]
    pub docker: RemoteDocker,
    #[serde(default)]
    pub options: SshOptions,
}

/// Método de autenticación SSH.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "method",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum SshAuth {
    /// Agente SSH del sistema (`SSH_AUTH_SOCK`) y la configuración de `~/.ssh`.
    Agent,
    /// Clave privada sin frase de paso. Solo se guarda la ruta: la clave no se copia ni se lee.
    Key { path: String },
    /// Clave privada protegida con frase de paso (la frase se pide al conectar).
    KeyPassphrase { path: String },
    /// Contraseña del usuario (desaconsejada; se pide al conectar).
    Password,
}

impl SshAuth {
    /// Qué secreto necesita, si alguno.
    pub fn secret_kind(&self) -> Option<SecretKind> {
        match self {
            SshAuth::Agent | SshAuth::Key { .. } => None,
            SshAuth::KeyPassphrase { .. } => Some(SecretKind::Passphrase),
            SshAuth::Password => Some(SecretKind::Password),
        }
    }

    pub fn key_path(&self) -> Option<&str> {
        match self {
            SshAuth::Key { path } | SshAuth::KeyPassphrase { path } => Some(path),
            _ => None,
        }
    }
}

/// Tipo de secreto que puede necesitar una conexión.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SecretKind {
    Passphrase,
    Password,
}

/// A qué Docker Engine del servidor se conecta `docker system dial-stdio`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "target",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum RemoteDocker {
    /// El que use el CLI `docker` del servidor por defecto (DOCKER_HOST, contexto o socket del sistema).
    #[default]
    Default,
    /// Un socket Unix concreto del servidor (p. ej. Docker rootless).
    Socket { path: String },
    /// Un contexto de Docker definido en el servidor.
    Context { name: String },
}

/// Opciones avanzadas. Solo las de esta lista: nunca opciones `-o` libres.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct SshOptions {
    /// Segundos para establecer la conexión (3–120).
    pub connect_timeout: u32,
    /// Servidor intermedio (`usuario@host:puerto`), vacío si no hay.
    pub proxy_jump: String,
}

impl Default for SshOptions {
    fn default() -> Self {
        Self {
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            proxy_jump: String::new(),
        }
    }
}

/// Problema de validación de un campo del perfil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProfileIssue {
    pub field: String,
    /// Código estable (`server.hostInvalid`, …) que la interfaz traduce.
    pub code: String,
}

fn issue(field: &str, code: &str) -> ProfileIssue {
    ProfileIssue {
        field: field.into(),
        code: format!("server.{code}"),
    }
}

/// Nombre DNS o alias: letras, dígitos, guiones, puntos y guiones bajos, sin empezar por guion.
pub fn valid_hostname(h: &str) -> bool {
    !h.is_empty()
        && h.len() <= 253
        && !h.starts_with(['-', '.'])
        && !h.ends_with('-')
        && h.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_'))
        && h.split('.').all(|l| !l.is_empty() && l.len() <= 63)
}

/// Host válido: DNS/alias, IPv4 o IPv6 (sin corchetes).
pub fn valid_host(h: &str) -> bool {
    if h.parse::<std::net::Ipv6Addr>().is_ok() || h.parse::<std::net::Ipv4Addr>().is_ok() {
        return true;
    }
    valid_hostname(h)
}

/// Usuario de Unix habitual (`useradd`): `[a-z_][a-z0-9_.-]*`, hasta 32.
pub fn valid_user(u: &str) -> bool {
    let mut cs = u.chars();
    matches!(cs.next(), Some(c) if c.is_ascii_lowercase() || c == '_')
        && u.len() <= 32
        && cs.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.' | '-'))
}

/// Ruta absoluta de este equipo (en Windows, `C:\…`; 1.9.1), sin caracteres de
/// control. Admite `~/` (se expande al conectar).
fn valid_local_path(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= MAX_PATH
        && (crate::hostpath::is_local_absolute(p) || p.starts_with("~/"))
        && !p.chars().any(|c| c.is_control())
}

/// Ruta absoluta del servidor, con caracteres seguros (se pasa al CLI remoto).
pub fn valid_remote_path(p: &str) -> bool {
    p.starts_with('/')
        && p.len() <= MAX_PATH
        && !p.contains("//")
        && p.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.' | '@' | '+'))
}

/// Nombre de contexto de Docker (mismas reglas que el CLI).
pub fn valid_context(n: &str) -> bool {
    let mut cs = n.chars();
    matches!(cs.next(), Some(c) if c.is_ascii_alphanumeric())
        && n.len() <= 64
        && cs.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

/// `usuario@host:puerto`, `host:puerto`, `host` o una lista separada por comas.
pub fn valid_proxy_jump(j: &str) -> bool {
    !j.is_empty()
        && j.len() <= 512
        && j.split(',').all(|hop| {
            let (user, rest) = match hop.split_once('@') {
                Some((u, r)) => (Some(u), r),
                None => (None, hop),
            };
            if user.is_some_and(|u| !valid_user(u)) {
                return false;
            }
            let (host, port) = if let Some(r) = rest.strip_prefix('[') {
                match r.split_once(']') {
                    Some((h, p)) => (h, p.strip_prefix(':')),
                    None => return false,
                }
            } else {
                match rest.rsplit_once(':') {
                    Some((h, p)) if !h.contains(':') => (h, Some(p)),
                    _ => (rest, None),
                }
            };
            valid_host(host) && port.map_or(true, |p| p.parse::<u16>().is_ok_and(|n| n > 0))
        })
}

/// Servidor intermedio de un perfil (E1, D-087). Se admite **un** salto: la
/// identidad de cada salto se verifica con el `known_hosts` de la aplicación y
/// eso solo se puede ofrecer de forma fiable para el primero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpHost {
    /// Usuario del salto; sin él, el de `~/.ssh/config` o el del equipo (como `ssh -J`).
    pub user: Option<String>,
    pub host: String,
    /// Puerto del salto; sin él, el de `~/.ssh/config` o el 22.
    pub port: Option<u16>,
}

impl JumpHost {
    /// `usuario@host:puerto` tal como se muestra.
    pub fn display(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        let mut s = match &self.user {
            Some(u) => format!("{u}@{host}"),
            None => host,
        };
        if let Some(p) = self.port {
            s.push_str(&format!(":{p}"));
        }
        s
    }
}

/// Interpreta un único salto `[usuario@]host[:puerto]` (IPv6 entre corchetes).
pub fn parse_jump(j: &str) -> Option<JumpHost> {
    if j.contains(',') || !valid_proxy_jump(j) {
        return None;
    }
    let (user, rest) = match j.split_once('@') {
        Some((u, r)) => (Some(u.to_owned()), r),
        None => (None, j),
    };
    let (host, port) = if let Some(r) = rest.strip_prefix('[') {
        let (h, p) = r.split_once(']')?;
        (h, p.strip_prefix(':'))
    } else {
        match rest.rsplit_once(':') {
            Some((h, p)) if !h.contains(':') => (h, Some(p)),
            _ => (rest, None),
        }
    };
    Some(JumpHost {
        user,
        host: host.to_owned(),
        port: port.and_then(|p| p.parse().ok()),
    })
}

/// Perfil con el que se comprueba la identidad del servidor intermedio (se
/// conecta directamente a él). No sirve para autenticarse en el destino.
pub fn jump_profile(p: &SshProfile) -> Option<SshProfile> {
    let j = parse_jump(&p.options.proxy_jump)?;
    let mut jp = p.clone();
    jp.host = j.host;
    jp.port = j.port.unwrap_or(22);
    if let Some(u) = j.user {
        jp.user = u;
    }
    jp.auth = SshAuth::Agent;
    jp.options.proxy_jump = String::new();
    Some(jp)
}

/// Valida el perfil completo (también se valida en cada conexión).
pub fn validate(p: &SshProfile) -> Vec<ProfileIssue> {
    let mut out = Vec::new();
    let name = p.name.trim();
    if name.is_empty() {
        out.push(issue("name", "nameRequired"));
    } else if name.chars().count() > MAX_NAME || name.chars().any(|c| c.is_control()) {
        out.push(issue("name", "nameInvalid"));
    }
    if p.host.is_empty() {
        out.push(issue("host", "hostRequired"));
    } else if !valid_host(&p.host) {
        out.push(issue("host", "hostInvalid"));
    }
    if p.port == 0 {
        out.push(issue("port", "portInvalid"));
    }
    if p.user.is_empty() {
        out.push(issue("user", "userRequired"));
    } else if !valid_user(&p.user) {
        out.push(issue("user", "userInvalid"));
    }
    if p.description.chars().count() > MAX_DESCRIPTION {
        out.push(issue("description", "descriptionTooLong"));
    }
    if let Some(path) = p.auth.key_path() {
        if !valid_local_path(path) {
            out.push(issue("keyPath", "keyPathInvalid"));
        }
    }
    match &p.docker {
        RemoteDocker::Default => {}
        RemoteDocker::Socket { path } => {
            if !valid_remote_path(path) {
                out.push(issue("dockerSocket", "socketInvalid"));
            }
        }
        RemoteDocker::Context { name } => {
            if !valid_context(name) {
                out.push(issue("dockerContext", "contextInvalid"));
            }
        }
    }
    if !(MIN_CONNECT_TIMEOUT..=MAX_CONNECT_TIMEOUT).contains(&p.options.connect_timeout) {
        out.push(issue("connectTimeout", "timeoutInvalid"));
    }
    if !p.options.proxy_jump.is_empty() && !valid_proxy_jump(&p.options.proxy_jump) {
        out.push(issue("proxyJump", "proxyJumpInvalid"));
    } else if p.options.proxy_jump.contains(',') {
        // Varios saltos encadenados: no se puede verificar la identidad de
        // cada uno con las reglas de la aplicación (D-087).
        out.push(issue("proxyJump", "proxyJumpChain"));
    }
    if !valid_server_id(&p.id) {
        out.push(issue("id", "idInvalid"));
    }
    out
}

/// Identificador interno: `srv-` seguido de hexadecimal. `local` está reservado.
pub fn valid_server_id(id: &str) -> bool {
    id.strip_prefix("srv-").is_some_and(|h| {
        (8..=32).contains(&h.len())
            && h.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    })
}

/// Dirección para mostrar: `usuario@host:puerto` (IPv6 entre corchetes).
pub fn display_address(p: &SshProfile) -> String {
    let host = if p.host.contains(':') {
        format!("[{}]", p.host)
    } else {
        p.host.clone()
    };
    format!("{}@{}:{}", p.user, host, p.port)
}

/// `ssh://usuario@host:puerto`, como lo escribe el CLI de Docker.
pub fn docker_host_uri(p: &SshProfile) -> String {
    format!("ssh://{}", display_address(p))
}

/// Etiqueta con la que se registran sus operaciones y eventos.
pub fn server_tag(p: &SshProfile) -> cmc_activity::ServerTag {
    cmc_activity::ServerTag {
        id: p.id.clone(),
        name: p.name.clone(),
        address: display_address(p),
        remote: true,
    }
}

/// Expande `~/` con el directorio personal del usuario.
pub fn expand_home(path: &str) -> String {
    match path.strip_prefix("~/") {
        Some(rest) => dirs::home_dir()
            .map(|h| h.join(rest).display().to_string())
            .unwrap_or_else(|| path.to_owned()),
        None => path.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> SshProfile {
        SshProfile {
            id: "srv-0123456789abcdef".into(),
            name: "Producción A".into(),
            host: "docker-a.example.com".into(),
            port: 22,
            user: "deploy".into(),
            description: String::new(),
            auth: SshAuth::Agent,
            docker: RemoteDocker::Default,
            options: SshOptions::default(),
        }
    }

    fn codes(p: &SshProfile) -> Vec<String> {
        validate(p).into_iter().map(|i| i.code).collect()
    }

    #[test]
    fn a_normal_profile_is_valid() {
        assert!(codes(&base()).is_empty());
        for host in [
            "192.168.1.10",
            "::1",
            "fe80::1",
            "srv_01",
            "a.b-c.d",
            "prod",
        ] {
            assert!(valid_host(host), "{host}");
        }
    }

    #[test]
    fn hostile_values_are_rejected() {
        for host in [
            "-oProxyCommand=sh",
            "a b",
            "host;rm",
            "$(id)",
            "",
            "a..b",
            ".a",
            "a-",
            "x\n",
            "[::1]",
            "user@host",
        ] {
            let mut p = base();
            p.host = host.into();
            assert!(!codes(&p).is_empty(), "host {host:?}");
        }
        for user in [
            "-l",
            "Root",
            "a b",
            "a;b",
            "",
            "1abc",
            "x".repeat(33).as_str(),
        ] {
            let mut p = base();
            p.user = user.into();
            assert!(!codes(&p).is_empty(), "user {user:?}");
        }
        let mut p = base();
        p.port = 0;
        assert_eq!(codes(&p), ["server.portInvalid"]);
    }

    #[test]
    fn key_paths_remote_targets_and_options() {
        let mut p = base();
        p.auth = SshAuth::Key {
            path: "~/.ssh/id_ed25519".into(),
        };
        assert!(codes(&p).is_empty());
        p.auth = SshAuth::KeyPassphrase {
            path: "relativa/id".into(),
        };
        assert_eq!(codes(&p), ["server.keyPathInvalid"]);
        p.auth = SshAuth::Agent;
        p.docker = RemoteDocker::Socket {
            path: "/run/user/1000/docker.sock".into(),
        };
        assert!(codes(&p).is_empty());
        for bad in ["relative.sock", "/a b", "/a;b", "/a/$(id)", "/a//b"] {
            p.docker = RemoteDocker::Socket { path: bad.into() };
            assert_eq!(codes(&p), ["server.socketInvalid"], "{bad}");
        }
        p.docker = RemoteDocker::Context {
            name: "rootless".into(),
        };
        assert!(codes(&p).is_empty());
        p.docker = RemoteDocker::Context { name: "-x".into() };
        assert_eq!(codes(&p), ["server.contextInvalid"]);
        p.docker = RemoteDocker::Default;
        p.options.connect_timeout = 1;
        assert_eq!(codes(&p), ["server.timeoutInvalid"]);
        p.options.connect_timeout = 10;
        for ok in ["bastion.example.com", "ops@bastion:2222", "[fe80::1]:22"] {
            p.options.proxy_jump = ok.into();
            assert!(codes(&p).is_empty(), "{ok}");
        }
        // E1 (D-087): un solo salto.
        p.options.proxy_jump = "a@b:22,c@d".into();
        assert_eq!(codes(&p), ["server.proxyJumpChain"]);
        for bad in ["-oProxyCommand=x", "a b", "u@h:0", "u@h:x", "ROOT@h"] {
            p.options.proxy_jump = bad.into();
            assert_eq!(codes(&p), ["server.proxyJumpInvalid"], "{bad}");
        }
    }

    #[test]
    fn jump_hosts_are_parsed_and_profiled() {
        assert_eq!(
            parse_jump("ops@bastion:2222"),
            Some(JumpHost {
                user: Some("ops".into()),
                host: "bastion".into(),
                port: Some(2222)
            })
        );
        assert_eq!(
            parse_jump("[fe80::1]:22"),
            Some(JumpHost {
                user: None,
                host: "fe80::1".into(),
                port: Some(22)
            })
        );
        assert_eq!(parse_jump("bastion").unwrap().display(), "bastion");
        assert_eq!(
            parse_jump("u@[fe80::1]:2").unwrap().display(),
            "u@[fe80::1]:2"
        );
        assert!(parse_jump("a,b").is_none());
        let mut p = base();
        p.auth = SshAuth::Password;
        assert!(jump_profile(&p).is_none());
        p.options.proxy_jump = "ops@bastion:2222".into();
        let j = jump_profile(&p).unwrap();
        assert_eq!(
            (j.host.as_str(), j.port, j.user.as_str()),
            ("bastion", 2222, "ops")
        );
        assert_eq!(
            j.auth,
            SshAuth::Agent,
            "el salto nunca usa el secreto del destino"
        );
        assert!(j.options.proxy_jump.is_empty());
    }

    #[test]
    fn ids_names_and_addresses() {
        assert!(valid_server_id("srv-0123abcd"));
        assert!(!valid_server_id("local"));
        assert!(!valid_server_id("srv-XYZ12345"));
        assert!(!valid_server_id("srv-01"));
        let mut p = base();
        p.name = "   ".into();
        assert_eq!(codes(&p), ["server.nameRequired"]);
        p.name = "a\u{7}".into();
        assert_eq!(codes(&p), ["server.nameInvalid"]);
        let mut p = base();
        p.port = 2222;
        assert_eq!(display_address(&p), "deploy@docker-a.example.com:2222");
        p.host = "fe80::1".into();
        assert_eq!(docker_host_uri(&p), "ssh://deploy@[fe80::1]:2222");
    }
}
