// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Comandos masivos: validación, argumentos exactos, riesgo y captura.

use super::*;

fn direct(program: &str, args: &[&str]) -> BulkCommand {
    BulkCommand {
        command: ExecCommand::Direct {
            program: program.into(),
            args: args.iter().map(|s| (*s).to_owned()).collect(),
        },
        user: None,
        workdir: None,
        timeout_secs: 30,
    }
}

fn script(interpreter: &str, flag: &str, text: &str) -> BulkCommand {
    BulkCommand {
        command: ExecCommand::Script {
            interpreter: interpreter.into(),
            flag: flag.into(),
            script: text.into(),
        },
        user: None,
        workdir: None,
        timeout_secs: 30,
    }
}

#[test]
fn argv_is_exactly_what_docker_receives_without_any_host_shell() {
    // Los signos de la shell llegan como texto: nada los interpreta en el equipo.
    let c = direct("echo", &["a b", "$(id)", "; rm -rf /", "*"]);
    assert_eq!(c.argv(), vec!["echo", "a b", "$(id)", "; rm -rf /", "*"]);
    let s = script("/bin/sh", "-c", "set -e\necho uno\necho dos >&2");
    assert_eq!(
        s.argv(),
        vec!["/bin/sh", "-c", "set -e\necho uno\necho dos >&2"]
    );
    // Sin opción: el intérprete recibe la secuencia como único argumento.
    assert_eq!(
        script("python3", "", "print(1)").argv(),
        vec!["python3", "print(1)"]
    );
}

#[test]
fn equivalent_docker_exec_includes_user_and_workdir() {
    let mut c = direct("ls", &["-la"]);
    c.user = Some(" www-data ".into());
    c.workdir = Some("/srv".into());
    assert_eq!(
        c.exec_args("abc"),
        vec![
            "exec",
            "--user",
            "www-data",
            "--workdir",
            "/srv",
            "abc",
            "ls",
            "-la"
        ]
    );
    c.user = Some("   ".into());
    c.workdir = None;
    assert_eq!(c.exec_args("abc"), vec!["exec", "abc", "ls", "-la"]);
}

#[test]
fn validation_rejects_what_docker_could_misread() {
    assert!(direct("id", &[]).validate().is_ok());
    assert!(direct("", &[]).validate().is_err());
    assert!(direct(" id", &[]).validate().is_err());
    assert!(direct("id", &["a\0b"]).validate().is_err());
    let mut c = direct("id", &[]);
    for (u, ok) in [
        ("root", true),
        ("1000", true),
        ("1000:1000", true),
        ("app:www-data", true),
        ("a b", false),
        ("x;y", false),
        (":1", false),
    ] {
        c.user = Some(u.into());
        assert_eq!(c.validate().is_ok(), ok, "usuario {u}");
    }
    c.user = None;
    c.workdir = Some("relativo".into());
    assert!(c.validate().is_err());
    c.workdir = Some("/tmp/ok".into());
    assert!(c.validate().is_ok());
    for t in [0, MAX_TIMEOUT_SECS + 1] {
        c.timeout_secs = t;
        assert!(c.validate().is_err(), "tiempo {t}");
    }
    assert!(script("/bin/sh", "-c", "   \n").validate().is_err());
    assert!(script("", "-c", "id").validate().is_err());
    assert!(script("/bin/sh", "- c", "id").validate().is_err());
    assert!(script("/bin/sh", "-c", &"x".repeat(MAX_SCRIPT_BYTES + 1))
        .validate()
        .is_err());
    let many: Vec<&str> = std::iter::repeat_n("a", MAX_ARGS + 1).collect();
    assert!(direct("echo", &many).validate().is_err());
}

#[test]
fn risk_levels_from_informative_to_destructive() {
    let lvl = |c: &BulkCommand| classify(c).level;
    assert_eq!(
        lvl(&direct("cat", &["/etc/os-release"])),
        CommandRiskLevel::Informative
    );
    assert_eq!(lvl(&direct("df", &["-h"])), CommandRiskLevel::Informative);
    assert_eq!(
        lvl(&script("/bin/sh", "-c", "sed 's/a/b/' f")),
        CommandRiskLevel::Informative
    );
    assert_eq!(
        lvl(&script("/bin/sh", "-c", "sed -i 's/a/b/' f")),
        CommandRiskLevel::Modification
    );
    assert_eq!(
        lvl(&script("/bin/sh", "-c", "echo hola > /tmp/x")),
        CommandRiskLevel::Modification
    );
    assert_eq!(
        lvl(&script("/bin/sh", "-c", "echo hola >> /tmp/x 2>&1")),
        CommandRiskLevel::Modification
    );
    // Redirigir a la salida de error o a /dev/null no escribe en archivos.
    assert_eq!(
        lvl(&script(
            "/bin/sh",
            "-c",
            "echo aviso >&2; ls 2>&1 >/dev/null; id 2> /dev/null"
        )),
        CommandRiskLevel::Informative
    );
    assert_eq!(
        lvl(&direct("touch", &["/tmp/x"])),
        CommandRiskLevel::Modification
    );
    assert_eq!(
        lvl(&direct("/usr/bin/apt-get", &["update"])),
        CommandRiskLevel::Administrative
    );
    assert_eq!(
        lvl(&script(
            "/bin/sh",
            "-c",
            "apk add curl && service nginx reload"
        )),
        CommandRiskLevel::Administrative
    );
    assert_eq!(
        lvl(&direct("rm", &["-rf", "/data"])),
        CommandRiskLevel::Destructive
    );
    assert_eq!(
        lvl(&script("/bin/bash", "-c", "cd /x; rm -fr *")),
        CommandRiskLevel::Destructive
    );
    assert_eq!(
        lvl(&direct("rm", &["--recursive", "/data"])),
        CommandRiskLevel::Destructive
    );
    assert_eq!(
        lvl(&script("/bin/sh", "-c", "psql -c 'DROP TABLE users'")),
        CommandRiskLevel::Destructive
    );
    assert_eq!(
        lvl(&direct("mkfs.ext4", &["/dev/sdb"])),
        CommandRiskLevel::Destructive
    );
    assert_eq!(
        lvl(&direct("kill", &["-9", "1"])),
        CommandRiskLevel::Destructive
    );
    let r = classify(&script(
        "/bin/sh",
        "-c",
        "apt-get update; rm -rf /var/cache",
    ));
    assert_eq!(r.reasons, vec!["rmRecursive", "packages", "delete"]);
}

#[test]
fn commands_that_can_show_secrets_are_flagged() {
    assert!(classify(&direct("env", &[])).exposes_secrets);
    assert!(classify(&direct("printenv", &["DB_PASSWORD"])).exposes_secrets);
    assert!(classify(&direct("cat", &["/proc/1/environ"])).exposes_secrets);
    assert!(!classify(&direct("df", &["-h"])).exposes_secrets);
}

#[test]
fn output_is_capped_per_stream() {
    let mut c = Capture::default();
    c.push(&vec![b'a'; OUTPUT_LIMIT - 1]);
    assert!(!c.truncated);
    c.push(b"bcd");
    assert!(c.truncated);
    let t = c.text();
    assert_eq!(t.len(), OUTPUT_LIMIT);
    assert!(t.ends_with('b'));
    // UTF-8 inválido no rompe nada.
    let mut c = Capture::default();
    c.push(&[0xff, b'o', b'k']);
    assert!(c.text().ends_with("ok"));
}

#[test]
fn docker_launch_failures_are_recognised() {
    let oci = "OCI runtime exec failed: exec failed: unable to start container process: exec: \"bash\": executable file not found in $PATH: unknown";
    assert!(is_launch_failure(Some(127), oci, ""));
    assert!(is_launch_failure(Some(126), "", oci));
    // Un 127 del propio comando (orden no encontrada dentro de sh) no lo es.
    assert!(!is_launch_failure(Some(127), "", "sh: 1: foo: not found"));
    assert!(!is_launch_failure(Some(0), oci, ""));
}

#[test]
fn wire_format_for_the_interface() {
    let c: BulkCommand = serde_json::from_str(
        r#"{"command":{"mode":"script","interpreter":"/bin/sh","flag":"-c","script":"id"},"user":null,"workdir":"/","timeoutSecs":5}"#,
    )
    .unwrap();
    assert_eq!(c.argv(), vec!["/bin/sh", "-c", "id"]);
    let d: BulkCommand = serde_json::from_str(
        r#"{"command":{"mode":"direct","program":"id","args":["-u"]},"user":"1000","workdir":null,"timeoutSecs":5}"#,
    )
    .unwrap();
    assert_eq!(
        d.exec_args("x"),
        vec!["exec", "--user", "1000", "x", "id", "-u"]
    );
    assert_eq!(render_command(&d), "id -u");
}
