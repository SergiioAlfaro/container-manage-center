//! Argumentos de `ssh`, construidos como lista (sin shell local), y
//! entrecomillado POSIX de la orden que se ejecuta en el servidor.
//!
//! Las opciones de seguridad van en la línea de órdenes: en OpenSSH, el primer
//! valor obtenido de una opción es el que se usa, y la línea de órdenes se lee
//! antes que `~/.ssh/config`. Así, una configuración del usuario no puede
//! desactivar la comprobación de la identidad del servidor (D-076).

use std::path::Path;

use super::profile::{expand_home, parse_jump, RemoteDocker, SshAuth, SshProfile};
use super::SshPaths;

/// Intervalo de comprobación de que el servidor sigue vivo.
pub const SERVER_ALIVE_INTERVAL: u32 = 15;
pub const SERVER_ALIVE_COUNT: u32 = 3;

/// Entrecomillado POSIX: deja tal cual lo que no necesita comillas y envuelve
/// el resto en comillas simples (`'` → `'\''`). Válido para sh, bash, dash y zsh.
pub fn shell_quote(s: &str) -> String {
    let safe = !s.is_empty()
        && s.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '_' | '-' | '.' | '/' | ':' | '=' | '@' | '%' | '+' | ',')
        });
    if safe {
        s.to_owned()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

/// Orden remota como una sola cadena con cada argumento entrecomillado.
/// `ssh` la entrega al shell del servidor, que la vuelve a separar en los
/// mismos argumentos.
pub fn remote_command(argv: &[String]) -> String {
    argv.iter()
        .map(|a| shell_quote(a))
        .collect::<Vec<_>>()
        .join(" ")
}

/// `docker [--host unix:///ruta | --context nombre]`: prefijo del CLI remoto.
pub fn remote_docker(target: &RemoteDocker) -> Vec<String> {
    let mut v = vec!["docker".to_owned()];
    match target {
        RemoteDocker::Default => {}
        RemoteDocker::Socket { path } => {
            v.push("--host".into());
            v.push(format!("unix://{path}"));
        }
        RemoteDocker::Context { name } => {
            v.push("--context".into());
            v.push(name.clone());
        }
    }
    v
}

/// `docker … system dial-stdio`: el canal por el que viaja la API.
pub fn dial_stdio(target: &RemoteDocker) -> Vec<String> {
    let mut v = remote_docker(target);
    v.push("system".into());
    v.push("dial-stdio".into());
    v
}

/// Valor de `UserKnownHostsFile` con las dos rutas (la de la aplicación primero).
/// Entre comillas dobles si contienen espacios (sintaxis de `ssh_config`).
pub fn known_hosts_value(paths: &SshPaths) -> String {
    let q = |p: &Path| {
        let s = p.display().to_string();
        if s.contains(' ') {
            format!("\"{s}\"")
        } else {
            s
        }
    };
    format!(
        "{} {}",
        q(&paths.app_known_hosts),
        q(&paths.user_known_hosts)
    )
}

/// Quita pares `-o clave=valor` cuyo valor empieza por alguno de los prefijos.
fn drop_opt(v: &mut Vec<String>, prefixes: &[&str]) {
    let mut out = Vec::with_capacity(v.len());
    let mut i = 0;
    while i < v.len() {
        if v[i] == "-o" && i + 1 < v.len() && prefixes.iter().any(|p| v[i + 1].starts_with(p)) {
            i += 2;
            continue;
        }
        out.push(std::mem::take(&mut v[i]));
        i += 1;
    }
    *v = out;
}

fn opt(v: &mut Vec<String>, kv: impl Into<String>) {
    v.push("-o".into());
    v.push(kv.into());
}

/// Cómo se comprueba la identidad del servidor en una invocación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKeyPolicy {
    /// Normal: `StrictHostKeyChecking=yes` con los `known_hosts` de la app y del usuario.
    Strict,
    /// Captura la clave presentada en un archivo temporal (solo tras confirmar la huella).
    CaptureInto(String),
    /// Sin ningún `known_hosts`: falla siempre en la verificación (sirve para ver la huella).
    StrictEmpty,
}

/// Opciones comunes a todas las invocaciones de `ssh` para un perfil.
/// `interactive_secret`: la autenticación usará `SSH_ASKPASS`.
pub fn base_args(p: &SshProfile, paths: &SshPaths, policy: &HostKeyPolicy) -> Vec<String> {
    let mut v = Vec::new();
    match policy {
        HostKeyPolicy::Strict => {
            opt(&mut v, "StrictHostKeyChecking=yes");
            opt(
                &mut v,
                format!("UserKnownHostsFile={}", known_hosts_value(paths)),
            );
        }
        HostKeyPolicy::CaptureInto(file) => {
            opt(&mut v, "StrictHostKeyChecking=accept-new");
            opt(&mut v, format!("UserKnownHostsFile={file}"));
            opt(&mut v, "GlobalKnownHostsFile=/dev/null");
            opt(&mut v, "HashKnownHosts=no");
        }
        HostKeyPolicy::StrictEmpty => {
            opt(&mut v, "StrictHostKeyChecking=yes");
            opt(&mut v, "UserKnownHostsFile=/dev/null");
            opt(&mut v, "GlobalKnownHostsFile=/dev/null");
        }
    }
    // ssh no debe reescribir ningún known_hosts por su cuenta.
    opt(&mut v, "UpdateHostKeys=no");
    opt(
        &mut v,
        format!("ConnectTimeout={}", p.options.connect_timeout),
    );
    opt(
        &mut v,
        format!("ServerAliveInterval={SERVER_ALIVE_INTERVAL}"),
    );
    opt(&mut v, format!("ServerAliveCountMax={SERVER_ALIVE_COUNT}"));
    opt(&mut v, "TCPKeepAlive=yes");
    opt(&mut v, "ForwardAgent=no");
    opt(&mut v, "ForwardX11=no");
    opt(&mut v, "ClearAllForwardings=yes");
    opt(&mut v, "PermitLocalCommand=no");
    opt(&mut v, "LogLevel=ERROR");
    match &p.auth {
        SshAuth::Agent => {
            opt(&mut v, "BatchMode=yes");
        }
        SshAuth::Key { path } => {
            opt(&mut v, "BatchMode=yes");
            opt(&mut v, "IdentitiesOnly=yes");
            v.push("-i".into());
            v.push(expand_home(path));
        }
        SshAuth::KeyPassphrase { path } => {
            // La frase la entrega SSH_ASKPASS; sin intentos de contraseña.
            opt(&mut v, "BatchMode=no");
            opt(&mut v, "IdentitiesOnly=yes");
            opt(&mut v, "PasswordAuthentication=no");
            opt(&mut v, "KbdInteractiveAuthentication=no");
            opt(&mut v, "NumberOfPasswordPrompts=1");
            v.push("-i".into());
            v.push(expand_home(path));
        }
        SshAuth::Password => {
            opt(&mut v, "BatchMode=no");
            opt(&mut v, "PubkeyAuthentication=no");
            opt(
                &mut v,
                "PreferredAuthentications=password,keyboard-interactive",
            );
            // Un solo intento: no provocar bloqueos de cuenta.
            opt(&mut v, "NumberOfPasswordPrompts=1");
        }
    }
    if let Some(cmd) = proxy_command(p, paths) {
        opt(&mut v, format!("ProxyCommand={cmd}"));
    }
    v.push("-p".into());
    v.push(p.port.to_string());
    v.push("-l".into());
    v.push(p.user.clone());
    v
}

/// Argumentos del `ssh` que abre el servidor intermedio (E1, DCC-SEC-02,
/// D-087), sin el programa ni el destino. Con `ssh -J`, OpenSSH solo pasa al
/// salto `-l`, `-p` y `-v`: ni la comprobación estricta de la identidad ni el
/// `known_hosts` de la aplicación. Aquí el salto recibe las mismas reglas que
/// el destino y **solo** se autentica con clave o agente (`BatchMode`): nunca
/// pide contraseñas ni usa el askpass, así que el secreto del destino no
/// puede llegarle.
pub fn jump_args(p: &SshProfile, paths: &SshPaths) -> Option<Vec<String>> {
    let j = parse_jump(&p.options.proxy_jump)?;
    let mut v = Vec::new();
    opt(&mut v, "StrictHostKeyChecking=yes");
    opt(
        &mut v,
        format!("UserKnownHostsFile={}", known_hosts_value(paths)),
    );
    opt(&mut v, "UpdateHostKeys=no");
    opt(&mut v, "BatchMode=yes");
    opt(&mut v, "PasswordAuthentication=no");
    opt(&mut v, "KbdInteractiveAuthentication=no");
    opt(
        &mut v,
        format!("ConnectTimeout={}", p.options.connect_timeout),
    );
    opt(
        &mut v,
        format!("ServerAliveInterval={SERVER_ALIVE_INTERVAL}"),
    );
    opt(&mut v, format!("ServerAliveCountMax={SERVER_ALIVE_COUNT}"));
    opt(&mut v, "ForwardAgent=no");
    opt(&mut v, "ForwardX11=no");
    opt(&mut v, "ClearAllForwardings=yes");
    opt(&mut v, "PermitLocalCommand=no");
    opt(&mut v, "ControlMaster=no");
    opt(&mut v, "ControlPath=none");
    opt(&mut v, "ProxyCommand=none");
    opt(&mut v, "ProxyJump=none");
    opt(&mut v, "LogLevel=ERROR");
    // La clave del perfil (sin frase de paso) también vale para el salto; las
    // del agente y las de `~/.ssh/config` se siguen probando.
    if let SshAuth::Key { path } = &p.auth {
        v.push("-i".into());
        v.push(expand_home(path));
    }
    if let Some(port) = j.port {
        v.push("-p".into());
        v.push(port.to_string());
    }
    if let Some(user) = &j.user {
        v.push("-l".into());
        v.push(user.clone());
    }
    Some(v)
}

/// Comprobación de la autenticación en el servidor intermedio: se pide un
/// reenvío a un puerto cerrado del propio salto (`-W 127.0.0.1:1`). No se
/// ejecuta ninguna orden en él y basta para saber si aceptó la clave o el
/// agente (`-v`: `Authenticated to …`).
pub fn jump_probe_args(p: &SshProfile, paths: &SshPaths) -> Option<Vec<String>> {
    let j = parse_jump(&p.options.proxy_jump)?;
    let mut v = jump_args(p, paths)?;
    drop_opt(&mut v, &["LogLevel="]);
    v.extend([
        "-v".to_owned(),
        "-T".into(),
        "-W".into(),
        "127.0.0.1:1".into(),
        "--".into(),
        j.host,
    ]);
    Some(v)
}

/// `ProxyCommand` que sustituye a `-J` (D-087). OpenSSH lo ejecuta con un
/// shell (`$SHELL -c`; la aplicación fija `SHELL=/bin/sh` en sus procesos `ssh`)
/// tras sustituir `%h`/`%p`: cada argumento va entrecomillado y los `%` de lo
/// que no es nuestro se escapan como `%%`. El proceso del salto se lanza sin
/// las variables del askpass.
pub fn proxy_command(p: &SshProfile, paths: &SshPaths) -> Option<String> {
    let j = parse_jump(&p.options.proxy_jump)?;
    let args = jump_args(p, paths)?;
    let esc = |s: &str| shell_quote(s).replace('%', "%%");
    // OpenSSH ya antepone `exec` al ejecutarlo.
    let mut parts: Vec<String> = vec![
        "/usr/bin/env".into(),
        "-u".into(),
        "SSH_ASKPASS".into(),
        "-u".into(),
        super::askpass::SOCKET_ENV.into(),
        "SSH_ASKPASS_REQUIRE=never".into(),
        esc(&paths.ssh.display().to_string()),
    ];
    parts.extend(args.iter().map(|a| esc(a)));
    parts.push("-W".into());
    // Lo sustituye OpenSSH por el destino (`[%h]` admite IPv6).
    parts.push("'[%h]:%p'".into());
    parts.push("--".into());
    parts.push(esc(&j.host));
    Some(parts.join(" "))
}

/// Conexión maestra: autentica y queda a la espera de canales.
pub fn master_args(p: &SshProfile, paths: &SshPaths, control: &Path) -> Vec<String> {
    let mut v = base_args(p, paths, &HostKeyPolicy::Strict);
    opt(&mut v, "ControlMaster=yes");
    opt(&mut v, format!("ControlPath={}", control.display()));
    opt(&mut v, "ControlPersist=no");
    opt(&mut v, "ExitOnForwardFailure=yes");
    v.push("-N".into());
    v.push("-T".into());
    v.push("--".into());
    v.push(p.host.clone());
    v
}

/// Canal por la conexión maestra que ejecuta `remote` en el servidor.
/// `tty`: pide un terminal remoto (shell del servidor).
pub fn channel_args(
    p: &SshProfile,
    paths: &SshPaths,
    control: &Path,
    remote: &[String],
    tty: bool,
) -> Vec<String> {
    let mut v = base_args(p, paths, &HostKeyPolicy::Strict);
    opt(&mut v, "ControlMaster=no");
    opt(&mut v, format!("ControlPath={}", control.display()));
    v.push(if tty { "-tt" } else { "-T" }.into());
    v.push("--".into());
    v.push(p.host.clone());
    if !remote.is_empty() {
        v.push(remote_command(remote));
    }
    v
}

/// `ssh -O <orden>` sobre la conexión maestra (`check`, `exit`).
pub fn control_args(p: &SshProfile, control: &Path, command: &str) -> Vec<String> {
    vec![
        "-o".into(),
        format!("ControlPath={}", control.display()),
        "-O".into(),
        command.into(),
        "-p".into(),
        p.port.to_string(),
        "-l".into(),
        p.user.clone(),
        "--".into(),
        p.host.clone(),
    ]
}

/// Comprobación de identidad sin autenticarse: muestra la huella presentada
/// (`-v`) y termina en la verificación o al no poder autenticar (`none`).
pub fn identity_probe_args(
    p: &SshProfile,
    paths: &SshPaths,
    policy: &HostKeyPolicy,
) -> Vec<String> {
    let mut v = base_args(p, paths, policy);
    // Aquí solo interesa la identidad: fuera el modo de autenticación del perfil
    // y el nivel de registro (con `-v` ssh muestra la huella presentada).
    drop_opt(
        &mut v,
        &[
            "BatchMode=",
            "LogLevel=",
            "IdentitiesOnly=",
            "PreferredAuthentications=",
            "PubkeyAuthentication=",
            "PasswordAuthentication=",
            "KbdInteractiveAuthentication=",
            "NumberOfPasswordPrompts=",
        ],
    );
    for kv in [
        "BatchMode=yes",
        "PreferredAuthentications=none",
        "PubkeyAuthentication=no",
        "PasswordAuthentication=no",
        "KbdInteractiveAuthentication=no",
        "ControlMaster=no",
        "ControlPath=none",
    ] {
        opt(&mut v, kv);
    }
    v.push("-v".into());
    v.push("-T".into());
    v.push("--".into());
    v.push(p.host.clone());
    v.push("true".into());
    v
}

/// `ssh -G`: configuración efectiva (sin conectar), para saber el nombre real,
/// el puerto y si hay un servidor intermedio en `~/.ssh/config`.
pub fn effective_config_args(p: &SshProfile) -> Vec<String> {
    let mut v = vec![
        "-G".to_owned(),
        "-p".into(),
        p.port.to_string(),
        "-l".into(),
        p.user.clone(),
    ];
    if !p.options.proxy_jump.is_empty() {
        v.push("-J".into());
        v.push(p.options.proxy_jump.clone());
    }
    v.push("--".into());
    v.push(p.host.clone());
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ssh::profile::SshOptions;

    fn paths() -> SshPaths {
        SshPaths {
            ssh: "/usr/bin/ssh".into(),
            ssh_keygen: "/usr/bin/ssh-keygen".into(),
            runtime_dir: "/run/user/1000/docker-control-center/ssh".into(),
            app_known_hosts: "/home/ana/.config/docker-control-center/known_hosts".into(),
            user_known_hosts: "/home/ana/.ssh/known_hosts".into(),
            askpass: "/usr/bin/docker-control-center".into(),
        }
    }

    fn profile(auth: SshAuth) -> SshProfile {
        SshProfile {
            id: "srv-0123456789abcdef".into(),
            name: "A".into(),
            host: "docker-a.example.com".into(),
            port: 2222,
            user: "deploy".into(),
            description: String::new(),
            auth,
            docker: RemoteDocker::Default,
            options: SshOptions::default(),
        }
    }

    fn has(v: &[String], kv: &str) -> bool {
        v.windows(2).any(|w| w[0] == "-o" && w[1] == kv)
    }

    #[test]
    fn quoting_survives_hostile_input() {
        assert_eq!(shell_quote("docker"), "docker");
        assert_eq!(
            shell_quote("unix:///run/user/1000/docker.sock"),
            "unix:///run/user/1000/docker.sock"
        );
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote("$(rm -rf /)"), "'$(rm -rf /)'");
        assert_eq!(shell_quote("`id`;x&&y|z>w"), "'`id`;x&&y|z>w'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
        assert_eq!(shell_quote("\n"), "'\n'");
        assert_eq!(shell_quote("*?[]~#!{}"), "'*?[]~#!{}'");
    }

    /// El shell real del sistema vuelve a separar la orden en exactamente los
    /// mismos argumentos, sin ejecutar nada de lo que contienen.
    #[test]
    fn remote_command_roundtrips_through_a_real_shell() {
        let dir = tempfile::tempdir().unwrap();
        let canary = dir.path().join("canary");
        let hostile = [
            "simple".to_owned(),
            "con espacios".into(),
            "comilla ' simple".into(),
            "\"dobles\"".into(),
            format!("$(touch {})", canary.display()),
            format!("`touch {}`", canary.display()),
            format!("; touch {}", canary.display()),
            "a\nb".into(),
            "*".into(),
            "~".into(),
            "$HOME".into(),
            "\\".into(),
            "-n".into(),
            "".into(),
            "ñandú/é".into(),
        ];
        let cmd = format!("printf '%s\\0' {}", remote_command(&hostile));
        let out = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(&cmd)
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(out.status.success());
        let got: Vec<String> = out
            .stdout
            .split(|b| *b == 0)
            .filter(|_| true)
            .map(|b| String::from_utf8(b.to_vec()).unwrap())
            .collect();
        assert_eq!(&got[..hostile.len()], &hostile);
        assert!(!canary.exists(), "se ejecutó algo del argumento");
    }

    #[test]
    fn security_options_are_always_present_and_first() {
        for auth in [
            SshAuth::Agent,
            SshAuth::Key {
                path: "~/.ssh/id_ed25519".into(),
            },
            SshAuth::KeyPassphrase {
                path: "/home/ana/.ssh/id_rsa".into(),
            },
            SshAuth::Password,
        ] {
            let v = master_args(&profile(auth.clone()), &paths(), Path::new("/run/c"));
            assert_eq!(&v[..2], ["-o", "StrictHostKeyChecking=yes"], "{auth:?}");
            assert!(has(&v, "UserKnownHostsFile=/home/ana/.config/docker-control-center/known_hosts /home/ana/.ssh/known_hosts"));
            for kv in [
                "UpdateHostKeys=no",
                "ForwardAgent=no",
                "ForwardX11=no",
                "ClearAllForwardings=yes",
                "PermitLocalCommand=no",
                "ControlMaster=yes",
                "ControlPersist=no",
            ] {
                assert!(has(&v, kv), "{kv} en {auth:?}");
            }
            // El destino va después de `--` y es el último argumento.
            assert_eq!(&v[v.len() - 2..], ["--", "docker-a.example.com"]);
            assert!(v.windows(2).any(|w| w == ["-p", "2222"]));
            assert!(v.windows(2).any(|w| w == ["-l", "deploy"]));
            assert!(!v
                .iter()
                .any(|a| a.contains("accept-new") || a.contains("StrictHostKeyChecking=no")));
        }
    }

    #[test]
    fn authentication_options_match_the_method() {
        let a = base_args(&profile(SshAuth::Agent), &paths(), &HostKeyPolicy::Strict);
        assert!(has(&a, "BatchMode=yes") && !a.contains(&"-i".to_owned()));
        let k = base_args(
            &profile(SshAuth::Key {
                path: "/k/id".into(),
            }),
            &paths(),
            &HostKeyPolicy::Strict,
        );
        assert!(has(&k, "BatchMode=yes") && has(&k, "IdentitiesOnly=yes"));
        assert!(k.windows(2).any(|w| w == ["-i", "/k/id"]));
        let kp = base_args(
            &profile(SshAuth::KeyPassphrase {
                path: "/k/id".into(),
            }),
            &paths(),
            &HostKeyPolicy::Strict,
        );
        assert!(
            has(&kp, "BatchMode=no")
                && has(&kp, "PasswordAuthentication=no")
                && has(&kp, "NumberOfPasswordPrompts=1")
        );
        let pw = base_args(
            &profile(SshAuth::Password),
            &paths(),
            &HostKeyPolicy::Strict,
        );
        assert!(has(&pw, "PubkeyAuthentication=no") && has(&pw, "NumberOfPasswordPrompts=1"));
        assert!(has(
            &pw,
            "PreferredAuthentications=password,keyboard-interactive"
        ));
    }

    #[test]
    fn remote_docker_targets() {
        assert_eq!(
            dial_stdio(&RemoteDocker::Default),
            ["docker", "system", "dial-stdio"]
        );
        assert_eq!(
            dial_stdio(&RemoteDocker::Socket {
                path: "/run/user/1000/docker.sock".into()
            }),
            [
                "docker",
                "--host",
                "unix:///run/user/1000/docker.sock",
                "system",
                "dial-stdio"
            ]
        );
        assert_eq!(
            dial_stdio(&RemoteDocker::Context {
                name: "rootless".into()
            }),
            ["docker", "--context", "rootless", "system", "dial-stdio"]
        );
        let p = profile(SshAuth::Agent);
        let v = channel_args(
            &p,
            &paths(),
            Path::new("/run/c"),
            &dial_stdio(&p.docker),
            false,
        );
        assert_eq!(
            &v[v.len() - 3..],
            ["--", "docker-a.example.com", "docker system dial-stdio"]
        );
        assert!(has(&v, "ControlMaster=no") && v.contains(&"-T".to_owned()));
    }

    #[test]
    fn proxy_jump_and_capture_policy() {
        let mut p = profile(SshAuth::Agent);
        p.options.proxy_jump = "bastion@jump.example.com:2200".into();
        let v = base_args(&p, &paths(), &HostKeyPolicy::Strict);
        assert!(!v.contains(&"-J".to_owned()), "sin -J (D-087)");
        let pc = v
            .iter()
            .find(|a| a.starts_with("ProxyCommand="))
            .expect("ProxyCommand");
        for want in [
            "/usr/bin/env -u SSH_ASKPASS -u DCC_ASKPASS_SOCKET SSH_ASKPASS_REQUIRE=never /usr/bin/ssh",
            "StrictHostKeyChecking=yes",
            "BatchMode=yes",
            "PasswordAuthentication=no",
            "UserKnownHostsFile=/home/ana/.config/docker-control-center/known_hosts /home/ana/.ssh/known_hosts",
            "-p 2200 -l bastion -W '[%h]:%p' -- jump.example.com",
        ] {
            assert!(pc.contains(want), "falta {want:?} en {pc}");
        }
        let c = identity_probe_args(&p, &paths(), &HostKeyPolicy::CaptureInto("/tmp/x".into()));
        assert!(
            has(&c, "StrictHostKeyChecking=accept-new") && has(&c, "UserKnownHostsFile=/tmp/x")
        );
        assert!(has(&c, "PreferredAuthentications=none") && has(&c, "BatchMode=yes"));
        assert!(!has(&c, "BatchMode=no"));
        assert_eq!(c.last().unwrap(), "true");
        // Ningún `-o` queda sin su valor y no hay restos de la autenticación del perfil.
        for w in c.windows(2) {
            if w[0] == "-o" {
                assert!(w[1].contains('='), "{c:?}");
            }
        }
        assert!(!has(&c, "LogLevel=ERROR") && c.contains(&"-v".to_owned()));
        let k = identity_probe_args(
            &profile(SshAuth::Password),
            &paths(),
            &HostKeyPolicy::Strict,
        );
        assert!(!has(
            &k,
            "PreferredAuthentications=password,keyboard-interactive"
        ));
        assert!(has(&k, "PreferredAuthentications=none"));
    }

    /// El `ProxyCommand` pasa por la sustitución de `%` de OpenSSH y por
    /// `sh -c`: el salto recibe exactamente los argumentos previstos y no
    /// hereda el askpass (DCC-SEC-02).
    #[test]
    fn proxy_command_roundtrips_through_ssh_tokens_and_a_real_shell() {
        let d = tempfile::tempdir().unwrap();
        let fake = d.path().join("ssh 100%");
        std::fs::write(
            &fake,
            "#!/bin/sh\nprintf '%s\\n' \"$@\"\necho \"askpass=${SSH_ASKPASS:-}|${DCC_ASKPASS_SOCKET:-}|${SSH_ASKPASS_REQUIRE:-}\"\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut ps = paths();
        ps.ssh = fake.clone();
        ps.app_known_hosts = "/home/ana maría/kh%x".into();
        let mut p = profile(SshAuth::Key {
            path: "/home/ana/.ssh/id it's".into(),
        });
        p.options.proxy_jump = "ops@[fe80::1]:2200".into();
        let cmd = proxy_command(&p, &ps).unwrap();
        // Sustitución de OpenSSH: %h, %p y %% (como en sshconnect.c).
        let expanded = format!(
            "exec {}",
            cmd.replace("%%", "\u{0}")
                .replace("%h", "destino.example")
                .replace("%p", "22")
                .replace('\u{0}', "%")
        );
        let out = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(&expanded)
            .env("SSH_ASKPASS", "/usr/bin/docker-control-center")
            .env("SSH_ASKPASS_REQUIRE", "force")
            .env("DCC_ASKPASS_SOCKET", "/run/x.sock")
            .output()
            .unwrap();
        let text = String::from_utf8(out.stdout).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert!(
            lines.contains(
                &"UserKnownHostsFile=\"/home/ana maría/kh%x\" /home/ana/.ssh/known_hosts"
            ),
            "{text}"
        );
        assert!(lines.contains(&"/home/ana/.ssh/id it's"), "{text}");
        let tail = &lines[lines.len() - 5..];
        assert_eq!(
            tail,
            [
                "-W",
                "[destino.example]:22",
                "--",
                "fe80::1",
                "askpass=||never"
            ],
            "{text}"
        );
        assert!(
            lines.windows(2).any(|w| w == ["-p", "2200"])
                && lines.windows(2).any(|w| w == ["-l", "ops"])
        );
    }

    #[test]
    fn known_hosts_paths_with_spaces_are_quoted() {
        let mut ps = paths();
        ps.app_known_hosts = "/home/ana maría/.config/dcc/known_hosts".into();
        assert_eq!(
            known_hosts_value(&ps),
            "\"/home/ana maría/.config/dcc/known_hosts\" /home/ana/.ssh/known_hosts"
        );
    }
}
