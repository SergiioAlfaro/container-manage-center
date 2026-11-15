// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Entrega de la frase de paso o de la contraseña a `ssh` (D-077).
//!
//! `ssh` ejecuta el programa de `SSH_ASKPASS` (la propia aplicación) con la
//! pregunta como argumento. Ese proceso pide el secreto a la aplicación por un
//! socket Unix efímero (directorio 0700, socket 0600, se comprueba el uid del
//! que pregunta) y lo escribe en su salida, que lee `ssh`.
//!
//! - El secreto nunca va en argumentos, variables de entorno ni archivos.
//! - Solo se responde a la pregunta del tipo esperado (frase de paso o
//!   contraseña). Cualquier otra pregunta (p. ej. confiar en una identidad) se
//!   contesta con un error: nunca se responde «yes» por el usuario.
//! - Se responde un número limitado de veces y el socket se borra al cerrar la
//!   sesión SSH.

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;

use super::profile::SecretKind;
use super::Secret;

/// Variable con la ruta del socket (no es un secreto).
pub const SOCKET_ENV: &str = "CMC_ASKPASS_SOCKET";
const MAX_PROMPT: usize = 4096;

/// A quién corresponde el secreto (E1, CMC-SEC-02, D-087): el usuario y los
/// nombres del servidor de destino (el del perfil y el efectivo de
/// `~/.ssh/config`) y, para una frase de paso, la ruta de la clave.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PromptTarget {
    pub user: String,
    pub hosts: Vec<String>,
    pub key_path: Option<String>,
}

/// `usuario@host` de una pregunta de OpenSSH (`ana@srv's password:` o
/// `(ana@srv) Password:`), si lo tiene.
fn prompt_identity(prompt: &str) -> Option<(String, String)> {
    let at = prompt.find('@')?;
    let start = prompt[..at]
        .rfind(|c: char| c.is_whitespace() || c == '(')
        .map_or(0, |i| i + 1);
    let rest = &prompt[at + 1..];
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '\'' || c == ')')
        .unwrap_or(rest.len());
    Some((prompt[start..at].to_owned(), rest[..end].to_owned()))
}

/// ¿Es esta la pregunta que corresponde al secreto? Si la pregunta nombra a
/// otro usuario o servidor (p. ej. un servidor intermedio), o a otra clave, no
/// se responde: el secreto del destino nunca va a otro sitio.
pub fn prompt_matches(kind: SecretKind, prompt: &str, target: &PromptTarget) -> bool {
    let p = prompt.to_lowercase();
    // Preguntas de sí/no (identidad del servidor, confirmaciones): nunca.
    if p.contains("yes/no")
        || p.contains("(yes")
        || p.contains("fingerprint")
        || p.contains("continue connecting")
    {
        return false;
    }
    match kind {
        SecretKind::Passphrase => {
            if !p.contains("passphrase") {
                return false;
            }
            // `Enter passphrase for key '/ruta': `
            let quoted = prompt.split('\'').nth(1);
            match (quoted, &target.key_path) {
                (Some(path), Some(expected)) => path == expected,
                (Some(_), None) => false,
                (None, _) => true,
            }
        }
        SecretKind::Password => {
            if !p.contains("password") || p.contains("passphrase") {
                return false;
            }
            match prompt_identity(prompt) {
                Some((user, host)) => {
                    user == target.user
                        && target.hosts.iter().any(|h| h.eq_ignore_ascii_case(&host))
                }
                // `Password:` sin destino: solo la pide el `ssh` del destino
                // (el del salto no tiene askpass ni pide contraseñas).
                None => true,
            }
        }
    }
}

/// Crea (si falta) un directorio privado del usuario: 0700 y suyo.
pub fn private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(dir)?;
    if meta.uid() != unsafe { libc::getuid() } {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("{} no pertenece al usuario", dir.display()),
        ));
    }
    Ok(())
}

/// Servidor efímero que responde al programa askpass.
pub struct AskpassServer {
    path: PathBuf,
    task: tokio::task::JoinHandle<()>,
}

impl AskpassServer {
    /// `max_answers`: respuestas como máximo (una autenticación necesita una).
    pub async fn start(
        dir: &Path,
        kind: SecretKind,
        secret: Secret,
        max_answers: usize,
        target: PromptTarget,
    ) -> std::io::Result<Self> {
        private_dir(dir)?;
        let name = format!("askpass-{}.sock", random_hex(12));
        let path = dir.join(name);
        let listener = UnixListener::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        let uid = unsafe { libc::getuid() };
        let task = tokio::spawn(async move {
            let mut answered = 0;
            while answered < max_answers {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                // Solo procesos del mismo usuario.
                if stream.peer_cred().map(|c| c.uid()).ok() != Some(uid) {
                    continue;
                }
                let mut buf = Vec::new();
                let mut chunk = [0u8; 512];
                loop {
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(5),
                        stream.read(&mut chunk),
                    )
                    .await
                    {
                        Ok(Ok(0)) | Err(_) | Ok(Err(_)) => break,
                        Ok(Ok(n)) => {
                            buf.extend_from_slice(&chunk[..n]);
                            if buf.len() > MAX_PROMPT {
                                break;
                            }
                        }
                    }
                }
                let prompt = String::from_utf8_lossy(&buf);
                if prompt_matches(kind, &prompt, &target) {
                    let _ = stream.write_all(secret.as_bytes()).await;
                    answered += 1;
                }
                let _ = stream.shutdown().await;
            }
        });
        Ok(Self { path, task })
    }

    pub fn socket(&self) -> &Path {
        &self.path
    }
}

impl Drop for AskpassServer {
    fn drop(&mut self) {
        self.task.abort();
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Hexadecimal aleatorio (de `/dev/urandom`).
pub fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut buf);
    }
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Modo «askpass» del programa: si `ssh` lo ha lanzado como `SSH_ASKPASS`,
/// pide el secreto a la aplicación, lo escribe y devuelve el código de salida.
/// Devuelve `None` si el proceso no se lanzó como askpass.
pub fn client_main() -> Option<i32> {
    let socket = std::env::var_os(SOCKET_ENV)?;
    let prompt = std::env::args().nth(1).unwrap_or_default();
    let code = match ask(Path::new(&socket), &prompt) {
        Some(answer) if !answer.is_empty() => {
            let mut out = std::io::stdout().lock();
            let ok = out.write_all(answer.as_bytes()).is_ok() && out.write_all(b"\n").is_ok();
            drop(answer);
            if ok {
                0
            } else {
                1
            }
        }
        _ => 1,
    };
    Some(code)
}

fn ask(socket: &Path, prompt: &str) -> Option<Secret> {
    let mut s = std::os::unix::net::UnixStream::connect(socket).ok()?;
    s.set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .ok()?;
    s.write_all(prompt.as_bytes()).ok()?;
    s.shutdown(std::net::Shutdown::Write).ok()?;
    let mut answer = Secret::new(String::new());
    s.read_to_string(&mut answer).ok()?;
    Some(answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> PromptTarget {
        PromptTarget {
            user: "cmca".into(),
            hosts: vec!["127.0.0.1".into(), "srv.example".into()],
            key_path: Some("/home/a/.ssh/id_ed25519".into()),
        }
    }

    #[test]
    fn only_the_expected_prompt_gets_an_answer() {
        let t = target();
        assert!(prompt_matches(
            SecretKind::Passphrase,
            "Enter passphrase for key '/home/a/.ssh/id_ed25519': ",
            &t
        ));
        assert!(!prompt_matches(
            SecretKind::Passphrase,
            "Enter passphrase for key '/home/a/.ssh/otra': ",
            &t
        ));
        assert!(!prompt_matches(
            SecretKind::Passphrase,
            "cmca@127.0.0.1's password: ",
            &t
        ));
        assert!(prompt_matches(
            SecretKind::Password,
            "cmca@127.0.0.1's password: ",
            &t
        ));
        assert!(prompt_matches(
            SecretKind::Password,
            "cmca@SRV.example's password: ",
            &t
        ));
        assert!(prompt_matches(
            SecretKind::Password,
            "(cmca@srv.example) Password: ",
            &t
        ));
        assert!(prompt_matches(SecretKind::Password, "Password: ", &t));
        assert!(!prompt_matches(
            SecretKind::Password,
            "Enter passphrase for key 'x': ",
            &t
        ));
        for q in [
            "Are you sure you want to continue connecting (yes/no/[fingerprint])? ",
            "Allow use of key? (yes/no) password",
        ] {
            assert!(!prompt_matches(SecretKind::Password, q, &t), "{q}");
            assert!(!prompt_matches(SecretKind::Passphrase, q, &t), "{q}");
        }
    }

    /// CMC-SEC-02: la pregunta de otro servidor (el intermedio) o de otro
    /// usuario nunca recibe la contraseña del destino.
    #[test]
    fn a_jump_host_prompt_never_gets_the_target_secret() {
        let t = target();
        for q in [
            "bastion@jump.example's password: ",
            "cmca@jump.example's password: ",
            "root@127.0.0.1's password: ",
            "(cmca@jump.example) Password: ",
        ] {
            assert!(!prompt_matches(SecretKind::Password, q, &t), "{q}");
        }
    }

    #[tokio::test]
    async fn serves_the_secret_only_for_its_prompt_and_at_most_twice() {
        let dir = tempfile::tempdir().unwrap();
        let rt = dir.path().join("rt");
        let server = AskpassServer::start(
            &rt,
            SecretKind::Password,
            Secret::new("s3cr3t ñ".into()),
            2,
            PromptTarget {
                user: "a".into(),
                hosts: vec!["b".into()],
                key_path: None,
            },
        )
        .await
        .unwrap();
        let sock = server.socket().to_owned();
        use std::os::unix::fs::MetadataExt;
        assert_eq!(std::fs::metadata(&rt).unwrap().mode() & 0o777, 0o700);
        assert_eq!(std::fs::metadata(&sock).unwrap().mode() & 0o777, 0o600);
        let q = |p: &'static str| {
            let s = sock.clone();
            tokio::task::spawn_blocking(move || ask(&s, p).map(|a| a.to_string()))
        };
        assert_eq!(
            q("Are you sure you want to continue connecting (yes/no)?")
                .await
                .unwrap()
                .as_deref(),
            Some("")
        );
        assert_eq!(
            q("Enter passphrase for key 'k': ")
                .await
                .unwrap()
                .as_deref(),
            Some("")
        );
        assert_eq!(
            q("a@jump's password: ").await.unwrap().as_deref(),
            Some(""),
            "otro servidor: sin respuesta"
        );
        assert_eq!(
            q("a@b's password: ").await.unwrap().as_deref(),
            Some("s3cr3t ñ")
        );
        assert_eq!(
            q("a@b's password: ").await.unwrap().as_deref(),
            Some("s3cr3t ñ")
        );
        // Tercera vez: ya no responde (el servidor ha terminado).
        let third = q("a@b's password: ").await.unwrap();
        assert!(third.as_deref().unwrap_or("").is_empty());
        drop(server);
        assert!(!sock.exists());
    }
}
