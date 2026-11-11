//! Clasificación de los fallos de una conexión remota (requisito 10): a partir
//! del código de salida y de los mensajes reales de OpenSSH y del CLI de Docker.
//! Las pruebas usan mensajes copiados de OpenSSH 9.6 y de Docker 24, 26 y 29.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::profile::SecretKind;
use super::HostKeyInfo;

/// Motivo por el que no hay conexión con un servidor remoto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum RemoteProblem {
    /// No hay cliente `ssh` en este equipo.
    SshMissing,
    /// El perfil no es válido (no se ha intentado conectar).
    InvalidProfile {
        codes: Vec<String>,
    },
    /// El nombre no se resuelve.
    HostUnresolvable {
        host: String,
    },
    /// No se puede llegar al servidor (red, cortafuegos, apagado).
    HostUnreachable {
        message: String,
    },
    /// El servidor está, pero no acepta conexiones en ese puerto (SSH detenido).
    SshRefused {
        port: u16,
    },
    /// Responde, pero no habla SSH.
    NotSsh,
    /// Se agotó el tiempo de espera.
    Timeout,
    /// Identidad del servidor desconocida: hay que confirmar la huella.
    HostKeyUnknown {
        presented: Option<HostKeyInfo>,
    },
    /// La identidad cambió desde que se confió en ella: posible ataque de intermediario.
    HostKeyChanged {
        presented: Option<HostKeyInfo>,
        /// Archivo `known_hosts` con la clave anterior.
        file: Option<String>,
        /// Está en `~/.ssh/known_hosts` (la aplicación no lo modifica).
        in_user_file: bool,
    },
    /// Usuario, clave, frase o contraseña no aceptados.
    AuthFailed {
        methods: Option<String>,
    },
    /// Falta el secreto (frase de paso o contraseña) para conectar.
    SecretRequired {
        secret: SecretKind,
    },
    /// No se puede leer la clave privada indicada.
    KeyUnreadable {
        path: String,
    },
    /// El servidor no tiene el CLI `docker`.
    DockerMissing,
    /// Docker Engine no está en marcha en el servidor (o el socket no existe).
    DockerNotRunning {
        message: String,
    },
    /// El usuario no tiene permiso sobre el socket de Docker del servidor.
    SocketDenied {
        message: String,
    },
    /// El contexto de Docker indicado no existe en el servidor.
    ContextMissing {
        name: String,
    },
    /// Responde algo que no es Docker Engine.
    NotDocker {
        message: String,
    },
    ApiTooOld {
        api_version: String,
        minimum: String,
    },
    /// La conexión se perdió (el servidor dejó de responder).
    ConnectionLost,
    Other {
        message: String,
    },
}

impl RemoteProblem {
    /// Problemas tras los que no se reintenta automáticamente: repetirlos no
    /// lo arregla y podría bloquear la cuenta o insistir con una identidad dudosa.
    pub fn needs_user(&self) -> bool {
        matches!(
            self,
            RemoteProblem::SshMissing
                | RemoteProblem::InvalidProfile { .. }
                | RemoteProblem::HostKeyUnknown { .. }
                | RemoteProblem::HostKeyChanged { .. }
                | RemoteProblem::AuthFailed { .. }
                | RemoteProblem::SecretRequired { .. }
                | RemoteProblem::KeyUnreadable { .. }
                | RemoteProblem::ApiTooOld { .. }
        )
    }
}

/// Primera línea útil de un mensaje (sin las líneas de `@` de los avisos).
fn first_line(stderr: &str) -> String {
    stderr
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('@') && !l.starts_with("debug"))
        .unwrap_or("")
        .chars()
        .take(300)
        .collect()
}

/// `Server host key: ssh-ed25519 SHA256:…` (con `-v`) o la huella del aviso de cambio.
pub fn presented_key(stderr: &str) -> Option<HostKeyInfo> {
    for l in stderr.lines() {
        if let Some(rest) = l.trim().strip_prefix("debug1: Server host key: ") {
            let mut it = rest.split_whitespace();
            if let (Some(t), Some(fp)) = (it.next(), it.next()) {
                if fp.starts_with("SHA256:") {
                    return Some(HostKeyInfo {
                        key_type: t.to_owned(),
                        fingerprint: fp.to_owned(),
                    });
                }
            }
        }
    }
    // Aviso de cambio: «The fingerprint for the ED25519 key sent by the remote host is\nSHA256:….»
    let lines: Vec<&str> = stderr.lines().collect();
    for (i, l) in lines.iter().enumerate() {
        if let Some(rest) = l.strip_prefix("The fingerprint for the ") {
            let ty = rest.split_whitespace().next().unwrap_or("").to_owned();
            if let Some(fp) = lines.get(i + 1).map(|s| s.trim().trim_end_matches('.')) {
                if fp.starts_with("SHA256:") {
                    return Some(HostKeyInfo {
                        key_type: key_type_name(&ty),
                        fingerprint: fp.to_owned(),
                    });
                }
            }
        }
    }
    None
}

/// `ED25519` → `ssh-ed25519`, como lo muestra `-v`.
fn key_type_name(upper: &str) -> String {
    match upper {
        "ED25519" => "ssh-ed25519".into(),
        "RSA" => "ssh-rsa".into(),
        "ECDSA" => "ecdsa-sha2".into(),
        other => other.to_lowercase(),
    }
}

/// Clasifica el fallo de `ssh` (código 255) o de la orden remota.
/// `port`: el del perfil, para el mensaje de SSH detenido.
pub fn classify_ssh(code: Option<i32>, stderr: &str, host: &str, port: u16) -> RemoteProblem {
    let s = stderr;
    let lower = s.to_lowercase();
    if s.contains("REMOTE HOST IDENTIFICATION HAS CHANGED")
        || (lower.contains("host key for") && lower.contains("has changed"))
    {
        let file = s.lines().find_map(|l| {
            l.trim()
                .strip_prefix("Offending ")
                .and_then(|r| r.split_once(" key in "))
                .map(|(_, f)| f.rsplit_once(':').map_or(f, |(p, _)| p).to_owned())
        });
        let in_user_file = file
            .as_deref()
            .is_some_and(|f| f.ends_with("/.ssh/known_hosts"));
        return RemoteProblem::HostKeyChanged {
            presented: presented_key(s),
            file,
            in_user_file,
        };
    }
    if (lower.contains("host key is known for") && lower.contains("strict checking"))
        || lower.contains("no matching host key")
        || (lower.contains("host key verification failed") && !lower.contains("has changed"))
    {
        return RemoteProblem::HostKeyUnknown {
            presented: presented_key(s),
        };
    }
    if lower.contains("could not resolve hostname") {
        return RemoteProblem::HostUnresolvable { host: host.into() };
    }
    if lower.contains("connection refused") {
        return RemoteProblem::SshRefused { port };
    }
    if lower.contains("connection timed out") || lower.contains("operation timed out") {
        return RemoteProblem::Timeout;
    }
    if lower.contains("no route to host") || lower.contains("network is unreachable") {
        return RemoteProblem::HostUnreachable {
            message: first_line(s),
        };
    }
    // Algo responde en el puerto, pero no completa el saludo SSH.
    if lower.contains("kex_exchange_identification") || lower.contains("banner exchange") {
        return RemoteProblem::NotSsh;
    }
    if (lower.contains("identity file") && lower.contains("not accessible"))
        || (lower.contains("load key") && lower.contains("no such file"))
    {
        let path = s
            .lines()
            .find_map(|l| l.split('"').nth(1).map(str::to_owned))
            .unwrap_or_default();
        return RemoteProblem::KeyUnreadable { path };
    }
    if lower.contains("permission denied (") || lower.contains("too many authentication failures") {
        let methods = s.lines().find_map(|l| {
            l.split_once("Permission denied (")
                .map(|(_, r)| r.trim_end_matches(").").trim_end_matches(')').to_owned())
        });
        return RemoteProblem::AuthFailed { methods };
    }
    if code == Some(127)
        || lower.contains("docker: command not found")
        || lower.contains("docker: not found")
    {
        return RemoteProblem::DockerMissing;
    }
    classify_docker(stderr).unwrap_or_else(|| {
        if lower.contains("broken pipe")
            || lower.contains("connection reset")
            || lower.contains("timeout, server")
        {
            RemoteProblem::ConnectionLost
        } else {
            RemoteProblem::Other {
                message: first_line(s),
            }
        }
    })
}

/// Mensajes del CLI de Docker en el servidor (24 a 29).
pub fn classify_docker(stderr: &str) -> Option<RemoteProblem> {
    let lower = stderr.to_lowercase();
    let line = first_line(stderr);
    if lower.contains("permission denied while trying to connect") {
        return Some(RemoteProblem::SocketDenied { message: line });
    }
    if lower.contains("cannot connect to the docker daemon")
        || lower.contains("failed to connect to the docker api")
        || lower.contains("is the docker daemon running")
    {
        return Some(RemoteProblem::DockerNotRunning { message: line });
    }
    if lower.contains("context \"") && lower.contains("not found") {
        let name = stderr
            .split("context \"")
            .nth(1)
            .and_then(|r| r.split('"').next())
            .unwrap_or("")
            .to_owned();
        return Some(RemoteProblem::ContextMissing { name });
    }
    if lower.contains("docker: command not found") || lower.contains("docker: not found") {
        return Some(RemoteProblem::DockerMissing);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mensajes reales (OpenSSH 9.6p1, Docker 29.9.2 y anteriores).
    const UNKNOWN: &str = "No ED25519 host key is known for [127.0.0.1]:2201 and you have requested strict checking.\nHost key verification failed.\n";
    const UNKNOWN_V: &str = "debug1: Connection established.\ndebug1: kex: host key algorithm: ssh-ed25519\ndebug1: Server host key: ssh-ed25519 SHA256:iHgHJYlX3wOd7kwqzTZJpiOy7ALikJI0cSlcmKEHVjo\nNo ED25519 host key is known for [127.0.0.1]:2201 and you have requested strict checking.\nHost key verification failed.\n";
    const CHANGED: &str = "@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
@    WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!     @
@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
IT IS POSSIBLE THAT SOMEONE IS DOING SOMETHING NASTY!
Someone could be eavesdropping on you right now (man-in-the-middle attack)!
It is also possible that a host key has just been changed.
The fingerprint for the ED25519 key sent by the remote host is
SHA256:iHgHJYlX3wOd7kwqzTZJpiOy7ALikJI0cSlcmKEHVjo.
Please contact your system administrator.
Add correct host key in /home/ana/.config/docker-control-center/known_hosts to get rid of this message.
Offending ED25519 key in /home/ana/.config/docker-control-center/known_hosts:1
  remove with:
  ssh-keygen -f '/home/ana/.config/docker-control-center/known_hosts' -R '[127.0.0.1]:2201'
Host key for [127.0.0.1]:2201 has changed and you have requested strict checking.
Host key verification failed.
";

    fn c(code: i32, s: &str) -> RemoteProblem {
        classify_ssh(Some(code), s, "srv.example.com", 2201)
    }

    #[test]
    fn identity_problems() {
        assert_eq!(
            c(255, UNKNOWN),
            RemoteProblem::HostKeyUnknown { presented: None }
        );
        assert_eq!(
            c(255, UNKNOWN_V),
            RemoteProblem::HostKeyUnknown {
                presented: Some(HostKeyInfo {
                    key_type: "ssh-ed25519".into(),
                    fingerprint: "SHA256:iHgHJYlX3wOd7kwqzTZJpiOy7ALikJI0cSlcmKEHVjo".into()
                })
            }
        );
        let changed = c(255, CHANGED);
        assert_eq!(
            changed,
            RemoteProblem::HostKeyChanged {
                presented: Some(HostKeyInfo {
                    key_type: "ssh-ed25519".into(),
                    fingerprint: "SHA256:iHgHJYlX3wOd7kwqzTZJpiOy7ALikJI0cSlcmKEHVjo".into()
                }),
                file: Some("/home/ana/.config/docker-control-center/known_hosts".into()),
                in_user_file: false,
            }
        );
        assert!(changed.needs_user());
        let user = CHANGED.replace(
            "/home/ana/.config/docker-control-center/known_hosts",
            "/home/ana/.ssh/known_hosts",
        );
        assert!(matches!(
            c(255, &user),
            RemoteProblem::HostKeyChanged {
                in_user_file: true,
                ..
            }
        ));
    }

    #[test]
    fn network_and_ssh_problems() {
        assert_eq!(
            c(
                255,
                "ssh: Could not resolve hostname no-existe.invalid: Name or service not known\n"
            ),
            RemoteProblem::HostUnresolvable {
                host: "srv.example.com".into()
            }
        );
        assert_eq!(
            c(
                255,
                "ssh: connect to host 127.0.0.1 port 2299: Connection refused\n"
            ),
            RemoteProblem::SshRefused { port: 2201 }
        );
        assert_eq!(
            c(
                255,
                "ssh: connect to host 10.0.0.9 port 22: Connection timed out\n"
            ),
            RemoteProblem::Timeout
        );
        assert!(matches!(
            c(
                255,
                "ssh: connect to host 10.0.0.9 port 22: No route to host\n"
            ),
            RemoteProblem::HostUnreachable { .. }
        ));
        assert_eq!(
            c(
                255,
                "kex_exchange_identification: Connection closed by remote host\n"
            ),
            RemoteProblem::NotSsh
        );
    }

    #[test]
    fn authentication_problems() {
        assert_eq!(
            c(
                255,
                "dccb@127.0.0.1: Permission denied (publickey,password).\n"
            ),
            RemoteProblem::AuthFailed {
                methods: Some("publickey,password".into())
            }
        );
        assert!(matches!(
            c(255, "Warning: Identity file /home/ana/.ssh/nope not accessible: No such file or directory.\ndccb@127.0.0.1: Permission denied (publickey).\n"),
            RemoteProblem::KeyUnreadable { .. }
        ));
        assert!(c(255, "x: Permission denied (publickey).").needs_user());
    }

    #[test]
    fn remote_docker_problems() {
        assert_eq!(
            c(127, "bash: line 1: docker: command not found\n"),
            RemoteProblem::DockerMissing
        );
        assert_eq!(
            c(127, "sh: 1: docker: not found\n"),
            RemoteProblem::DockerMissing
        );
        // Docker 29
        assert!(matches!(c(1, "permission denied while trying to connect to the docker API at unix:///var/run/docker.sock\n"), RemoteProblem::SocketDenied { .. }));
        assert!(matches!(c(1, "failed to connect to the docker API at unix:///run/nope.sock; check if the path is correct and if the daemon is running: dial unix /run/nope.sock: connect: no such file or directory\n"), RemoteProblem::DockerNotRunning { .. }));
        // Docker 20.10–26
        assert!(matches!(c(1, "Got permission denied while trying to connect to the Docker daemon socket at unix:///var/run/docker.sock: Get \"http://%2Fvar%2Frun%2Fdocker.sock/v1.24/version\": dial unix /var/run/docker.sock: connect: permission denied\n"), RemoteProblem::SocketDenied { .. }));
        assert!(matches!(c(1, "Cannot connect to the Docker daemon at unix:///var/run/docker.sock. Is the docker daemon running?\n"), RemoteProblem::DockerNotRunning { .. }));
        assert_eq!(
            c(1, "Failed to initialize: unable to resolve docker endpoint: context \"nope\": context not found: open /home/dccb/.docker/contexts/meta/x/meta.json: no such file or directory\n"),
            RemoteProblem::ContextMissing { name: "nope".into() }
        );
        assert!(!RemoteProblem::DockerNotRunning {
            message: String::new()
        }
        .needs_user());
    }

    #[test]
    fn unknown_messages_keep_only_a_short_first_line() {
        let long = format!("@@@\n\n  algo raro {}\nsegunda", "x".repeat(1000));
        match c(1, &long) {
            RemoteProblem::Other { message } => {
                assert!(message.starts_with("algo raro"));
                assert!(message.chars().count() <= 300);
            }
            other => panic!("{other:?}"),
        }
    }
}
