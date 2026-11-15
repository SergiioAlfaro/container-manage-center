// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Detección y ocultación de secretos (E1, CMC-SEC-05, D-085).
//!
//! Un único lugar decide qué es un secreto. Lo usan el registro de actividad
//! (antes de guardar o emitir cualquier operación: cuerpo, orden equivalente,
//! argumentos, ruta, resumen, salidas y error), el historial, los mensajes de
//! error de las órdenes y el motor (vistas de la configuración de los
//! contenedores). La interfaz aplica las mismas reglas
//! (`ui/src/shared/secrets.ts`); una prueba comprueba que las listas coinciden.
//!
//! Se oculta **solo el valor**: el nombre de la variable, el esquema y el
//! servidor de una URL o el código de un error siguen visibles para poder
//! diagnosticar. No se aplica a los registros de los contenedores ni a la salida
//! de las terminales: son datos del usuario y se muestran tal cual (decisión del
//! propietario, D-085).

use std::borrow::Cow;

/// Texto que sustituye a un valor secreto.
pub const REDACTED: &str = "********";

/// Palabras que, en cualquier parte del nombre, lo hacen secreto.
pub const SECRET_NAME_WORDS: [&str; 7] = [
    "PASSWORD",
    "PASSWD",
    "SECRET",
    "TOKEN",
    "KEY",
    "CREDENTIAL",
    "PRIVATE",
];

/// Terminaciones de una parte del nombre (separadas por `_`, `-` o `.`) que lo
/// hacen secreto: `DB_PASS`, `PGPASS`, `MYSQL_PWD`, `REDIS_AUTH`, `SENTRY_DSN`…
pub const SECRET_NAME_PART_ENDINGS: [&str; 5] = ["PASS", "PWD", "AUTH", "DSN", "COOKIE"];

/// Nombres que coinciden con las reglas pero nunca son secretos.
pub const NOT_SECRET_NAMES: [&str; 2] = ["PWD", "OLDPWD"];

/// Terminaciones que indican una **ruta** a un secreto (Docker secrets,
/// `MYSQL_PASSWORD_FILE`), no el secreto: se muestran.
pub const PATH_SUFFIXES: [&str; 2] = ["_FILE", "-FILE"];

/// ¿El valor de una variable, argumento o campo con este nombre es secreto?
pub fn is_secret_name(name: &str) -> bool {
    let n = name.trim().to_ascii_uppercase();
    if n.is_empty() || NOT_SECRET_NAMES.contains(&n.as_str()) {
        return false;
    }
    if PATH_SUFFIXES.iter().any(|s| n.ends_with(s)) {
        return false;
    }
    if SECRET_NAME_WORDS.iter().any(|w| n.contains(w)) {
        return true;
    }
    n.split(['_', '-', '.'])
        .any(|part| SECRET_NAME_PART_ENDINGS.iter().any(|e| part.ends_with(e)))
}

/// Valores que se dejan visibles aunque el nombre sea secreto: no son
/// credenciales y ayudan a diagnosticar (`AUTH_ENABLED=true`).
fn is_trivial_value(v: &str) -> bool {
    v.is_empty()
        || v == REDACTED
        || matches!(
            v.to_ascii_lowercase().as_str(),
            "true" | "false" | "yes" | "no" | "on" | "off" | "none" | "null"
        )
}

/// Valor de `NOMBRE=valor` como se mostrará: oculto si el nombre es secreto o
/// si el valor contiene credenciales (URL o DSN con contraseña).
pub fn redact_value(name: &str, value: &str) -> String {
    if is_secret_name(name) && !is_trivial_value(value) {
        return REDACTED.to_owned();
    }
    redact_credentials(value, true).into_owned()
}

/// ¿Tiene el valor credenciales incrustadas (`esquema://usuario:clave@…`, o
/// `usuario:clave@host` si `schemeless`)?
pub fn has_credentials(value: &str) -> bool {
    matches!(redact_credentials(value, true), Cow::Owned(_))
}

/// `NOMBRE=valor` → `NOMBRE=********` si corresponde. Sin `=`, sin cambios.
pub fn redact_assignment(s: &str) -> String {
    match s.split_once('=') {
        Some((k, v)) if is_name(k) => format!("{k}={}", redact_value(k, v)),
        _ => redact_text(s).into_owned(),
    }
}

/// Argumentos de una orden: `-e NOMBRE=valor`, `--build-arg NOMBRE=valor`,
/// `--password valor`, `--token=valor`, URL con contraseña…
pub fn redact_args(args: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut hide_next = false;
    for a in args {
        if hide_next {
            hide_next = false;
            out.push(if is_trivial_value(a) {
                a.clone()
            } else {
                REDACTED.to_owned()
            });
            continue;
        }
        if let Some(flag) = a.strip_prefix("--") {
            if let Some((name, v)) = flag.split_once('=') {
                if is_secret_flag(name) && !is_trivial_value(v) {
                    out.push(format!("--{name}={REDACTED}"));
                    continue;
                }
            } else if is_secret_flag(flag) {
                out.push(a.clone());
                hide_next = true;
                continue;
            }
        }
        out.push(redact_assignment(a));
    }
    out
}

/// Opciones de línea de órdenes cuyo valor es un secreto (`--password`,
/// `--token`, `--client-secret`), pero no las que reciben una ruta, leen de la
/// entrada estándar o son solo interruptores conocidos.
fn is_secret_flag(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    if n.ends_with("-stdin") || n.ends_with("-file") || n == "secret" {
        // `docker build --secret id=x,src=ruta`: es una referencia, no un valor.
        return false;
    }
    is_secret_name(&n)
}

fn is_name_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.')
}

fn is_name(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty() && is_name_start(b[0]) && b.iter().all(|&c| is_name_char(c))
}

/// Oculta los secretos de un texto libre: cuerpos JSON, órdenes equivalentes,
/// salidas de procesos y mensajes de error. Devuelve el mismo texto si no hay
/// nada que ocultar.
pub fn redact_text(text: &str) -> Cow<'_, str> {
    let mut s: Cow<str> = Cow::Borrowed(text);
    for pass in [
        redact_pem as fn(&str) -> Option<String>,
        redact_http_auth,
        |t| match redact_credentials(t, false) {
            Cow::Owned(o) => Some(o),
            Cow::Borrowed(_) => None,
        },
        redact_json_fields,
        redact_assignments,
        redact_yaml_lines,
        redact_flags,
    ] {
        if let Some(changed) = pass(&s) {
            s = Cow::Owned(changed);
        }
    }
    s
}

/// Bloques PEM de claves privadas: se conserva la cabecera y el pie.
fn redact_pem(t: &str) -> Option<String> {
    const BEGIN: &str = "-----BEGIN ";
    let mut out = String::new();
    let mut rest = t;
    let mut changed = false;
    while let Some(i) = rest.find(BEGIN) {
        let after = &rest[i..];
        let Some(h_end) = after[BEGIN.len()..].find("-----") else {
            break;
        };
        let header_end = BEGIN.len() + h_end + 5;
        let label = &after[BEGIN.len()..BEGIN.len() + h_end];
        if !label.contains("PRIVATE KEY") {
            out.push_str(&rest[..i + header_end]);
            rest = &rest[i + header_end..];
            continue;
        }
        let body = &after[header_end..];
        let end = body.find("-----END ").unwrap_or(body.len());
        out.push_str(&rest[..i + header_end]);
        out.push('\n');
        out.push_str(REDACTED);
        out.push('\n');
        rest = &body[end..];
        changed = true;
    }
    changed.then(|| {
        out.push_str(rest);
        out
    })
}

/// `Authorization: Bearer x`, `Basic x`, `Token x`.
fn redact_http_auth(t: &str) -> Option<String> {
    let lower = t.to_ascii_lowercase();
    let mut spans = Vec::new();
    let mut from = 0;
    while let Some(p) = lower[from..].find("authorization:") {
        let mut s = from + p + "authorization:".len();
        from = s;
        while s < t.len() && t.as_bytes()[s] == b' ' {
            s += 1;
        }
        // `Bearer x`, `Basic x`, `Token x`: se oculta lo que sigue al esquema.
        let after_scheme = ["bearer ", "basic ", "token "]
            .iter()
            .find(|sc| lower[s..].starts_with(*sc))
            .map(|sc| s + sc.len());
        let start = after_scheme.unwrap_or(s);
        let end = t[start..]
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ',')
            .map_or(t.len(), |e| start + e);
        if end > start && &t[start..end] != REDACTED {
            spans.push((start, end));
        }
    }
    if spans.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(t.len());
    let mut last = 0;
    for (s, e) in spans {
        out.push_str(&t[last..s]);
        out.push_str(REDACTED);
        last = e;
    }
    out.push_str(&t[last..]);
    Some(out)
}

/// Contraseña de `esquema://usuario:clave@host` (y, con `schemeless`, de
/// `usuario:clave@host` al principio del valor). El usuario y el host se
/// conservan.
fn redact_credentials(t: &str, schemeless: bool) -> Cow<'_, str> {
    let b = t.as_bytes();
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let stop = |c: u8| c.is_ascii_whitespace() || matches!(c, b'"' | b'\'' | b'/' | b'<' | b'>');
    let find_from = |start: usize, spans: &mut Vec<(usize, usize)>| {
        // userinfo: [start .. '@'), con un ':' dentro.
        let mut colon = None;
        let mut i = start;
        while i < b.len() && !stop(b[i]) && b[i] != b'@' {
            if b[i] == b':' && colon.is_none() {
                colon = Some(i);
            }
            i += 1;
        }
        if i < b.len() && b[i] == b'@' {
            if let Some(c) = colon {
                if c + 1 < i && &t[c + 1..i] != REDACTED {
                    spans.push((c + 1, i));
                }
            }
        }
    };
    let mut from = 0;
    while let Some(p) = t[from..].find("://") {
        let start = from + p + 3;
        find_from(start, &mut spans);
        from = start;
    }
    if schemeless && !t.contains("://") {
        find_from(0, &mut spans);
    }
    if spans.is_empty() {
        return Cow::Borrowed(t);
    }
    let mut out = String::with_capacity(t.len());
    let mut last = 0;
    for (s, e) in spans {
        out.push_str(&t[last..s]);
        out.push_str(REDACTED);
        last = e;
    }
    out.push_str(&t[last..]);
    Cow::Owned(out)
}

/// Fin de un valor que empieza en `start`, según cómo esté delimitado.
fn value_end(t: &str, start: usize, quote: Option<u8>) -> usize {
    let b = t.as_bytes();
    let mut i = start;
    match quote {
        Some(b'"') => {
            while i < b.len() {
                match b[i] {
                    b'\\' => i += 2,
                    b'"' => return i,
                    _ => i += 1,
                }
            }
            b.len()
        }
        Some(b'\'') => {
            // Entrecomillado POSIX: `'it'\''s'` es un único valor.
            while i < b.len() {
                if b[i] == b'\'' {
                    if t[i..].starts_with("'\\''") {
                        i += 4;
                        continue;
                    }
                    return i;
                }
                i += 1;
            }
            b.len()
        }
        _ => {
            while i < b.len()
                && !(b[i].is_ascii_whitespace() || matches!(b[i], b'&' | b'"' | b'\''))
            {
                i += 1;
            }
            i
        }
    }
}

/// `NOMBRE=valor` dentro de un texto (cuerpos JSON `"Env":["A=b"]`, órdenes
/// `-e 'A=b'`, `--build-arg A=b`, consultas `?token=x&…`).
fn redact_assignments(t: &str) -> Option<String> {
    let b = t.as_bytes();
    let mut out = String::new();
    let mut last = 0;
    let mut i = 0;
    let mut changed = false;
    while i < b.len() {
        let starts = is_name_start(b[i])
            && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_'));
        if !starts {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < b.len() && is_name_char(b[j]) {
            j += 1;
        }
        if j < b.len() && b[j] == b'=' && (j + 1 >= b.len() || b[j + 1] != b'=') {
            let name = &t[i..j];
            let quote = (i > 0 && matches!(b[i - 1], b'"' | b'\'')).then(|| b[i - 1]);
            let vs = j + 1;
            let ve = value_end(t, vs, quote);
            let value = &t[vs..ve];
            let shown = if is_secret_name(name) && !is_trivial_value(value) {
                Cow::Borrowed(REDACTED)
            } else {
                redact_credentials(value, true)
            };
            if shown != value {
                out.push_str(&t[last..vs]);
                out.push_str(&shown);
                last = ve;
                changed = true;
            }
            i = ve.max(j + 1);
        } else {
            i = j;
        }
    }
    changed.then(|| {
        out.push_str(&t[last..]);
        out
    })
}

/// Campos JSON `"nombre": "valor"` con nombre secreto.
fn redact_json_fields(t: &str) -> Option<String> {
    let b = t.as_bytes();
    let mut out = String::new();
    let mut last = 0;
    let mut i = 0;
    let mut changed = false;
    while i < b.len() {
        if b[i] != b'"' {
            i += 1;
            continue;
        }
        let ks = i + 1;
        let ke = value_end(t, ks, Some(b'"'));
        if ke >= b.len() {
            break;
        }
        let mut j = ke + 1;
        while j < b.len() && b[j].is_ascii_whitespace() {
            j += 1;
        }
        if j < b.len() && b[j] == b':' {
            j += 1;
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < b.len() && b[j] == b'"' && is_secret_name(&t[ks..ke]) {
                let vs = j + 1;
                let ve = value_end(t, vs, Some(b'"'));
                if !is_trivial_value(&t[vs..ve]) {
                    out.push_str(&t[last..vs]);
                    out.push_str(REDACTED);
                    last = ve;
                    changed = true;
                }
                i = ve + 1;
                continue;
            }
        }
        i = ke + 1;
    }
    changed.then(|| {
        out.push_str(&t[last.min(t.len())..]);
        out
    })
}

/// Líneas YAML `NOMBRE: valor` (p. ej. `docker compose config`).
fn redact_yaml_lines(t: &str) -> Option<String> {
    let mut changed = false;
    let lines: Vec<Cow<str>> = t
        .split_inclusive('\n')
        .map(|line| {
            let body = line.trim_end_matches(['\n', '\r']);
            let indent = body.len() - body.trim_start().len();
            let mut rest = &body[indent..];
            let mut prefix = indent;
            if let Some(r) = rest.strip_prefix("- ") {
                rest = r;
                prefix += 2;
            }
            let Some((k, v)) = rest.split_once(": ") else {
                return Cow::Borrowed(line);
            };
            let key = k.trim_matches(['"', '\'']);
            let val = v.trim();
            if !is_name(key)
                || !is_secret_name(key)
                || is_trivial_value(val.trim_matches(['"', '\'']))
                || val.starts_with(['{', '[', '|', '>', '&', '*'])
            {
                return Cow::Borrowed(line);
            }
            changed = true;
            let eol = &line[body.len()..];
            Cow::Owned(format!("{}{}: {REDACTED}{eol}", &body[..prefix], k))
        })
        .collect();
    changed.then(|| lines.concat())
}

/// `--password valor` / `--token=valor` en un texto.
fn redact_flags(t: &str) -> Option<String> {
    let b = t.as_bytes();
    let mut out = String::new();
    let mut last = 0;
    let mut changed = false;
    let mut i = 0;
    while let Some(p) = t[i..].find("--") {
        let fs = i + p + 2;
        i = fs;
        if fs >= 3 && !b[fs - 3].is_ascii_whitespace() && !matches!(b[fs - 3], b'"' | b'\'') {
            continue;
        }
        let mut fe = fs;
        while fe < b.len() && is_name_char(b[fe]) {
            fe += 1;
        }
        if fe == fs || !is_secret_flag(&t[fs..fe]) {
            continue;
        }
        let (vs, quote) = match b.get(fe) {
            Some(b'=') => (fe + 1, None),
            Some(c) if c.is_ascii_whitespace() => {
                let mut k = fe;
                while k < b.len() && b[k].is_ascii_whitespace() {
                    k += 1;
                }
                if t[k..].starts_with('-') {
                    continue;
                }
                (k, None)
            }
            _ => continue,
        };
        let (vs, quote) = match b.get(vs) {
            Some(q @ (b'"' | b'\'')) if quote.is_none() => (vs + 1, Some(*q)),
            _ => (vs, quote),
        };
        let ve = value_end(t, vs, quote);
        if vs < last || is_trivial_value(&t[vs..ve]) {
            continue;
        }
        out.push_str(&t[last..vs]);
        out.push_str(REDACTED);
        last = ve;
        changed = true;
        i = ve.max(fs);
    }
    changed.then(|| {
        out.push_str(&t[last..]);
        out
    })
}

/// Ruta de la API con los valores secretos de la query ocultos. `buildargs`
/// conserva los nombres y oculta los valores con nombre secreto (en el panel)
/// o todos (`all_buildargs`, en el historial). Las URL con contraseña también
/// se ocultan.
pub fn redact_api_path(path: &str, all_buildargs: bool) -> String {
    crate::history::redact_path_with(path, all_buildargs)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Valores ficticios: ninguno es una credencial real.
    const V: &str = "valor-ficticio-123";

    #[test]
    fn secret_names_required_by_the_specification() {
        for n in [
            "MYSQL_PWD",
            "DB_PASS",
            "PGPASS",
            "REDIS_AUTH",
            "SMTP_PASS",
            "MYSQL_PASSWORD",
            "MYSQL_ROOT_PASSWORD",
            "POSTGRES_PASSWORD",
            "GITHUB_TOKEN",
            "NPM_TOKEN",
            "AWS_SECRET_ACCESS_KEY",
            "AWS_ACCESS_KEY_ID",
            "SSH_PRIVATE_KEY",
            "API_KEY",
            "APIKEY",
            "SENTRY_DSN",
            "SESSION_COOKIE",
            "CLIENT_CREDENTIALS",
            "db.password",
            "db-passwd",
            "jwt_secret",
        ] {
            assert!(is_secret_name(n), "{n} debería ser secreto");
        }
    }

    #[test]
    fn ordinary_names_are_not_secret() {
        for n in [
            "PATH",
            "HOME",
            "PWD",
            "OLDPWD",
            "TZ",
            "LANG",
            "DATABASE_HOST",
            "POSTGRES_USER",
            "POSTGRES_DB",
            "NODE_ENV",
            "AUTHOR",
            "PORT",
            "MYSQL_PASSWORD_FILE",
            "POSTGRES_PASSWORD_FILE",
            "VERSION",
        ] {
            assert!(!is_secret_name(n), "{n} no debería ser secreto");
        }
    }

    #[test]
    fn values_with_credentials_are_hidden_whatever_the_name() {
        assert_eq!(
            redact_value("DATABASE_URL", &format!("postgres://app:{V}@db:5432/app")),
            "postgres://app:********@db:5432/app"
        );
        assert_eq!(
            redact_value("DSN_LIKE", &format!("app:{V}@tcp(db:3306)/app")),
            REDACTED,
            "DSN es secreto por nombre"
        );
        assert_eq!(
            redact_value("MYSQL_URL", &format!("app:{V}@tcp(db:3306)/app")),
            "app:********@tcp(db:3306)/app"
        );
        assert_eq!(
            redact_value("SITE_URL", "https://example.org/a:b"),
            "https://example.org/a:b"
        );
        assert_eq!(
            redact_value("REDIS_URL", "redis://db:6379/0"),
            "redis://db:6379/0"
        );
        assert!(has_credentials(&format!("amqp://u:{V}@mq")));
        assert!(!has_credentials("amqp://mq:5672"));
    }

    #[test]
    fn trivial_values_stay_visible_for_diagnosis() {
        assert_eq!(redact_value("AUTH_ENABLED", "true"), "true");
        assert_eq!(redact_value("REDIS_AUTH", ""), "");
        assert_eq!(redact_value("REDIS_AUTH", V), REDACTED);
    }

    #[test]
    fn docker_create_body_hides_env_values_but_keeps_names() {
        let body = format!(
            r#"{{"Image":"mariadb:11","Env":["MYSQL_PWD={V}","DB_PASS={V} con espacios","TZ=Europe/Madrid","DATABASE_URL=mysql://app:{V}@db/app"],"Labels":{{"app.password":"{V}","equipo":"web"}}}}"#
        );
        let r = redact_text(&body);
        assert!(!r.contains(V), "{r}");
        assert!(r.contains("MYSQL_PWD=********"));
        assert!(r.contains("DB_PASS=********\""));
        assert!(r.contains("TZ=Europe/Madrid"));
        assert!(r.contains("mysql://app:********@db/app"));
        assert!(r.contains(r#""app.password":"********""#));
        assert!(r.contains(r#""equipo":"web""#));
        serde_json::from_str::<serde_json::Value>(&r).expect("sigue siendo JSON válido");
    }

    #[test]
    fn equivalent_commands_hide_values_in_every_quoting_style() {
        let cmd = format!(
            "docker run -e MYSQL_PWD={V} -e 'SMTP_PASS={V}'\\''x' -e \"PGPASS={V}\" --build-arg NPM_TOKEN={V} -e TZ=UTC --password {V} --token={V} mariadb"
        );
        let r = redact_text(&cmd);
        assert!(!r.contains(V), "{r}");
        assert!(
            !r.contains("'\\''x"),
            "sin restos del valor entrecomillado: {r}"
        );
        assert!(r.contains("-e TZ=UTC"));
        assert!(r.contains("mariadb"));
        assert!(r.contains("--password ********"));
    }

    #[test]
    fn process_output_and_errors() {
        let out = format!(
            "services:\n  db:\n    environment:\n      MYSQL_ROOT_PASSWORD: {V}\n      MYSQL_DATABASE: app\n      - REDIS_AUTH={V}\n      AUTH_ENABLED: \"true\"\n"
        );
        let r = redact_text(&out);
        assert!(!r.contains(V), "{r}");
        assert!(r.contains("MYSQL_DATABASE: app"));
        assert!(r.contains("MYSQL_ROOT_PASSWORD: ********\n"));
        assert!(r.contains("AUTH_ENABLED: \"true\""));
        let err = format!("Error response from daemon: Get \"https://u:{V}@registry.example/v2/\": unauthorized; Authorization: Bearer {V}");
        let r = redact_text(&err);
        assert!(!r.contains(V), "{r}");
        assert!(r.contains("registry.example/v2/"), "el host sigue visible");
        assert!(r.contains("unauthorized"));
    }

    #[test]
    fn private_keys_are_hidden() {
        let pem = format!("clave:\n-----BEGIN OPENSSH PRIVATE KEY-----\n{V}\n{V}\n-----END OPENSSH PRIVATE KEY-----\nfin");
        let r = redact_text(&pem);
        assert!(!r.contains(V), "{r}");
        assert!(r.contains("-----BEGIN OPENSSH PRIVATE KEY-----"));
        assert!(r.contains("-----END OPENSSH PRIVATE KEY-----\nfin"));
        let cert = "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----";
        assert_eq!(redact_text(cert), cert, "los certificados no son secretos");
    }

    #[test]
    fn args_are_redacted_one_by_one() {
        let a: Vec<String> = [
            "compose",
            "-e",
            &format!("MYSQL_PWD={V}"),
            "--password",
            V,
            "--token=x1",
            "--password-stdin",
            "--secret",
            "id=npm,src=/home/u/.npmrc",
            "--env-file",
            ".env",
            &format!("http://u:{V}@h"),
            "-p",
            "8080:80",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let r = redact_args(&a);
        assert!(!r.iter().any(|x| x.contains(V)), "{r:?}");
        assert!(r.contains(&"--token=********".to_string()));
        assert!(r.contains(&"--password-stdin".to_string()));
        assert!(r.contains(&"id=npm,src=/home/u/.npmrc".to_string()));
        assert!(r.contains(&".env".to_string()));
        assert!(r.contains(&"8080:80".to_string()));
    }

    #[test]
    fn text_without_secrets_is_untouched() {
        for t in [
            "docker run -d -p 127.0.0.1:8080:80 -v /srv/html:/usr/share/nginx/html:ro nginx:1.27",
            r#"{"Image":"postgres:16","Env":["POSTGRES_USER=app","POSTGRES_DB=app","PGDATA=/var/lib/postgresql/data"]}"#,
            "a == b; x=1 && y=2",
            "",
            "ñandú: ✓ — sin secretos",
        ] {
            assert!(matches!(redact_text(t), Cow::Borrowed(_)), "{t}");
        }
    }

    #[test]
    fn redaction_is_idempotent() {
        let t = format!("-e MYSQL_PWD={V} postgres://a:{V}@h \"token\": \"{V}\"");
        let once = redact_text(&t).into_owned();
        assert_eq!(redact_text(&once), once);
    }

    /// La interfaz usa exactamente las mismas listas (`ui/src/shared/secrets.ts`).
    #[test]
    fn interface_rules_match() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../ui/src/shared/secrets.ts"
        );
        let ts = std::fs::read_to_string(path).expect("ui/src/shared/secrets.ts");
        let list = |name: &str| -> Vec<String> {
            let start = ts
                .find(&format!("export const {name} = ["))
                .unwrap_or_else(|| panic!("{name}"));
            let rest = &ts[start..];
            let body = &rest[rest.find('[').unwrap() + 1..rest.find(']').unwrap()];
            body.split(',')
                .map(|x| x.trim().trim_matches('"').to_owned())
                .filter(|x| !x.is_empty())
                .collect()
        };
        let v = |a: &[&str]| a.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(list("SECRET_NAME_WORDS"), v(&SECRET_NAME_WORDS));
        assert_eq!(
            list("SECRET_NAME_PART_ENDINGS"),
            v(&SECRET_NAME_PART_ENDINGS)
        );
        assert_eq!(list("NOT_SECRET_NAMES"), v(&NOT_SECRET_NAMES));
        assert_eq!(list("PATH_SUFFIXES"), v(&PATH_SUFFIXES));
        assert!(ts.contains(&format!("REDACTED = \"{REDACTED}\"")));
    }
}
