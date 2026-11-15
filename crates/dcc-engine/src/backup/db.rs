// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
//! Volcados de bases de datos (D-098).
//!
//! El volcado lo hace la herramienta del propio motor **dentro de su
//! contenedor** (`docker exec`), con un guion fijo que lee las credenciales de
//! las variables del propio contenedor (también las variantes `_FILE`). Las
//! credenciales nunca pasan por la aplicación, ni por argumentos visibles en
//! `ps`, ni por el registro de actividad: el guion solo las exporta en el
//! entorno del proceso (`MYSQL_PWD`, `PGPASSWORD`, `REDISCLI_AUTH`).
//!
//! La salida estándar del guion es el volcado; se guarda tal cual con su
//! SHA-256. El código de salida distinto de 0 o una salida vacía hacen fallar
//! la copia.

use std::collections::BTreeMap;

use super::model::DbEngine;

/// Detecta el motor por la imagen y, si no basta, por sus variables.
pub fn detect(image: &str, env: &[String]) -> Option<DbEngine> {
    let repo = image
        .rsplit('/')
        .next()
        .unwrap_or(image)
        .split(['@', ':'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let by_name = [
        ("mariadb", DbEngine::Mariadb),
        ("mysql", DbEngine::Mysql),
        ("percona", DbEngine::Mysql),
        ("postgres", DbEngine::Postgres),
        ("postgis", DbEngine::Postgres),
        ("timescaledb", DbEngine::Postgres),
        ("redis", DbEngine::Redis),
        ("valkey", DbEngine::Redis),
        ("mongo", DbEngine::Mongo),
    ];
    for (k, e) in by_name {
        if repo.contains(k) {
            return Some(e);
        }
    }
    let has = |p: &str| env.iter().any(|e| e.starts_with(p));
    if has("MARIADB_") {
        Some(DbEngine::Mariadb)
    } else if has("MYSQL_ROOT_PASSWORD") || has("MYSQL_DATABASE") {
        Some(DbEngine::Mysql)
    } else if has("POSTGRES_") || has("PGDATA=") {
        Some(DbEngine::Postgres)
    } else if has("MONGO_INITDB_") {
        Some(DbEngine::Mongo)
    } else {
        None
    }
}

/// Lee la contraseña de root de MariaDB/MySQL de las variables del contenedor.
const MY_CREDS: &str = r#"dcc_pw="${MARIADB_ROOT_PASSWORD:-${MYSQL_ROOT_PASSWORD:-}}"
dcc_pwf="${MARIADB_ROOT_PASSWORD_FILE:-${MYSQL_ROOT_PASSWORD_FILE:-}}"
if [ -z "$dcc_pw" ] && [ -n "$dcc_pwf" ] && [ -r "$dcc_pwf" ]; then dcc_pw="$(cat "$dcc_pwf")"; fi
if [ -n "$dcc_pw" ]; then MYSQL_PWD="$dcc_pw"; export MYSQL_PWD
elif [ -z "${MARIADB_ALLOW_EMPTY_ROOT_PASSWORD:-${MYSQL_ALLOW_EMPTY_PASSWORD:-}}" ]; then
  echo "DCC_NO_CREDENTIALS" >&2; exit 3
fi
unset dcc_pw dcc_pwf
"#;

const PG_CREDS: &str = r#"dcc_user="${POSTGRES_USER:-postgres}"
if [ -z "${PGPASSWORD:-}" ]; then
  if [ -n "${POSTGRES_PASSWORD:-}" ]; then PGPASSWORD="$POSTGRES_PASSWORD"; export PGPASSWORD
  elif [ -n "${POSTGRES_PASSWORD_FILE:-}" ] && [ -r "$POSTGRES_PASSWORD_FILE" ]; then
    PGPASSWORD="$(cat "$POSTGRES_PASSWORD_FILE")"; export PGPASSWORD
  fi
fi
"#;

const REDIS_CREDS: &str = r#"dcc_pw="${REDIS_PASSWORD:-}"
if [ -z "$dcc_pw" ] && [ -n "${REDIS_PASSWORD_FILE:-}" ] && [ -r "$REDIS_PASSWORD_FILE" ]; then dcc_pw="$(cat "$REDIS_PASSWORD_FILE")"; fi
if [ -n "$dcc_pw" ]; then REDISCLI_AUTH="$dcc_pw"; export REDISCLI_AUTH; fi
unset dcc_pw
"#;

/// Crea un archivo de opciones de mongodump/mongorestore (0600, en /tmp) con
/// la contraseña, para que no aparezca en la línea de órdenes.
const MONGO_CREDS: &str = r#"umask 077
dcc_cfg=""
if [ -n "${MONGO_INITDB_ROOT_USERNAME:-}" ]; then
  dcc_pw="${MONGO_INITDB_ROOT_PASSWORD:-}"
  if [ -z "$dcc_pw" ] && [ -n "${MONGO_INITDB_ROOT_PASSWORD_FILE:-}" ] && [ -r "$MONGO_INITDB_ROOT_PASSWORD_FILE" ]; then dcc_pw="$(cat "$MONGO_INITDB_ROOT_PASSWORD_FILE")"; fi
  dcc_cfg="$(mktemp /tmp/dcc-mongo.XXXXXX)"
  trap 'rm -f "$dcc_cfg" ${dcc_file:+"$dcc_file"}' EXIT
  dcc_esc="$(printf '%s' "$dcc_pw" | sed 's/\\/\\\\/g; s/"/\\"/g')"
  printf 'password: "%s"\n' "$dcc_esc" > "$dcc_cfg"
  set -- --username "$MONGO_INITDB_ROOT_USERNAME" --authenticationDatabase admin --config "$dcc_cfg" "$@"
  unset dcc_pw dcc_esc
fi
"#;

fn script(parts: &[&str]) -> String {
    let mut s = String::from("set -eu\n");
    for p in parts {
        s.push_str(p);
    }
    s
}

/// Guion de volcado: escribe el volcado en la salida estándar.
pub fn dump_script(e: DbEngine) -> String {
    match e {
        DbEngine::Mariadb | DbEngine::Mysql => script(&[
            MY_CREDS,
            r#"o="--all-databases --single-transaction --routines --events --triggers --hex-blob --default-character-set=utf8mb4"
if command -v mariadb-dump >/dev/null 2>&1; then exec mariadb-dump -uroot $o; fi
exec mysqldump -uroot $o
"#,
        ]),
        DbEngine::Postgres => script(&[
            PG_CREDS,
            "exec pg_dumpall -U \"$dcc_user\" --clean --if-exists --quote-all-identifiers\n",
        ]),
        DbEngine::Redis => script(&[
            REDIS_CREDS,
            r#"t="$(mktemp /tmp/dcc-backup.XXXXXX)"
trap 'rm -f "$t"' EXIT
redis-cli --no-auth-warning --rdb "$t" >&2
cat "$t"
"#,
        ]),
        DbEngine::Mongo => script(&[MONGO_CREDS, "mongodump --archive --quiet \"$@\"\n"]),
    }
}

/// Guion de restauración. `$1` es el archivo del volcado, ya subido al
/// contenedor; se borra al terminar pase lo que pase.
pub fn restore_script(e: DbEngine) -> Option<String> {
    let cleanup = "dcc_file=\"$1\"\nshift\ntrap 'rm -f \"$dcc_file\"' EXIT\n";
    Some(match e {
        DbEngine::Mariadb | DbEngine::Mysql => script(&[
            cleanup,
            MY_CREDS,
            r#"if command -v mariadb >/dev/null 2>&1; then mariadb -uroot --default-character-set=utf8mb4 < "$dcc_file"; else mysql -uroot --default-character-set=utf8mb4 < "$dcc_file"; fi
"#,
        ]),
        // Sin ON_ERROR_STOP: `--clean` intenta borrar el usuario con el que se
        // conecta y crear roles que ya existen; esos errores son esperables.
        // Se cuentan y se muestran como aviso.
        DbEngine::Postgres => script(&[
            cleanup,
            PG_CREDS,
            "psql -X -q -U \"$dcc_user\" -d postgres -f \"$dcc_file\"\n",
        ]),
        DbEngine::Mongo => script(&[
            cleanup,
            MONGO_CREDS,
            "mongorestore --archive=\"$dcc_file\" --drop --quiet \"$@\"\n",
        ]),
        // Redis se restaura sustituyendo su archivo RDB con el contenedor
        // detenido (ver restore.rs).
        DbEngine::Redis => return None,
    })
}

/// Guion que da `dir`, `dbfilename` y `appendonly` de Redis, uno por línea.
pub fn redis_config_script() -> String {
    script(&[
        REDIS_CREDS,
        r#"for k in dir dbfilename appendonly; do redis-cli --no-auth-warning --raw CONFIG GET "$k" | sed -n 2p; done
"#,
    ])
}

/// Detalles que se guardan en el manifiesto para poder restaurar.
pub fn details(e: DbEngine, image: &str) -> BTreeMap<String, String> {
    let mut d = BTreeMap::new();
    d.insert("engine".into(), e.as_str().into());
    d.insert("image".into(), image.into());
    d
}

/// Errores de psql que no impiden una restauración correcta con `--clean`.
pub fn pg_expected_error(line: &str) -> bool {
    line.contains("cannot be dropped because")
        || line.contains("current user cannot be dropped")
        || (line.contains("role \"") && line.contains("already exists"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engines_are_detected_by_image_and_env() {
        assert_eq!(detect("mariadb:11", &[]), Some(DbEngine::Mariadb));
        assert_eq!(
            detect("docker.io/library/mysql:8.4", &[]),
            Some(DbEngine::Mysql)
        );
        assert_eq!(detect("postgres:16-alpine", &[]), Some(DbEngine::Postgres));
        assert_eq!(
            detect("postgis/postgis:16-3.4", &[]),
            Some(DbEngine::Postgres)
        );
        assert_eq!(detect("redis:7", &[]), Some(DbEngine::Redis));
        assert_eq!(detect("valkey/valkey:8", &[]), Some(DbEngine::Redis));
        assert_eq!(detect("mongo@sha256:abc", &[]), Some(DbEngine::Mongo));
        assert_eq!(detect("dcc-test/mariadb:1", &[]), Some(DbEngine::Mariadb));
        assert_eq!(
            detect("miapp:1", &["POSTGRES_PASSWORD=x".into()]),
            Some(DbEngine::Postgres)
        );
        assert_eq!(
            detect("miapp:1", &["MARIADB_DATABASE=x".into()]),
            Some(DbEngine::Mariadb)
        );
        assert_eq!(detect("nginx", &["PATH=/bin".into()]), None);
        // El registro con puerto no confunde la detección.
        assert_eq!(
            detect("registro:5000/equipo/redis-cache:2", &[]),
            Some(DbEngine::Redis)
        );
    }

    /// Los guiones son constantes: ningún dato del usuario se concatena, y
    /// ninguna credencial aparece como argumento de una orden.
    #[test]
    fn scripts_are_constant_and_keep_credentials_out_of_argv() {
        for e in [
            DbEngine::Mariadb,
            DbEngine::Mysql,
            DbEngine::Postgres,
            DbEngine::Redis,
            DbEngine::Mongo,
        ] {
            let d = dump_script(e);
            assert!(d.starts_with("set -eu\n"));
            assert!(!d.contains("-p$"), "{d}");
            assert!(!d.contains("--password"), "{d}");
            assert!(!d.contains("-a \""), "{d}");
            if let Some(r) = restore_script(e) {
                assert!(r.contains("trap 'rm -f \"$dcc_file\"' EXIT"));
                assert!(!r.contains("--password"));
            }
        }
        assert!(dump_script(DbEngine::Mariadb).contains("export MYSQL_PWD"));
        assert!(dump_script(DbEngine::Postgres).contains("export PGPASSWORD"));
        assert!(dump_script(DbEngine::Redis).contains("export REDISCLI_AUTH"));
        assert!(dump_script(DbEngine::Mongo).contains("--config \"$dcc_cfg\""));
        assert!(restore_script(DbEngine::Redis).is_none());
    }

    /// Los guiones funcionan en un `sh` real (sin Docker): con herramientas
    /// simuladas que imprimen su entorno y sus argumentos.
    #[test]
    fn scripts_run_in_a_real_shell_with_fake_tools() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::Builder::new()
            .prefix("dcc-test")
            .tempdir()
            .unwrap();
        let fake = |name: &str, body: &str| {
            let p = dir.path().join(name);
            std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        };
        fake("mariadb-dump", "echo \"PWD=$MYSQL_PWD ARGS=$*\"");
        fake("pg_dumpall", "echo \"PGPASSWORD=$PGPASSWORD ARGS=$*\"");
        let run = |script: &str, env: &[(&str, &str)]| {
            let mut c = std::process::Command::new("/bin/sh");
            c.arg("-c").arg(script).arg("sh").env_clear();
            c.env("PATH", format!("{}:/usr/bin:/bin", dir.path().display()));
            for (k, v) in env {
                c.env(k, v);
            }
            c.output().unwrap()
        };
        let o = run(
            &dump_script(DbEngine::Mariadb),
            &[("MARIADB_ROOT_PASSWORD", "s3cr'et\"x")],
        );
        assert!(o.status.success());
        let out = String::from_utf8_lossy(&o.stdout);
        assert!(
            out.starts_with("PWD=s3cr'et\"x ARGS=-uroot --all-databases --single-transaction"),
            "{out}"
        );
        // Contraseña desde un archivo (_FILE).
        let pwf = dir.path().join("pw");
        std::fs::write(&pwf, "desde-archivo").unwrap();
        let o = run(
            &dump_script(DbEngine::Mariadb),
            &[("MYSQL_ROOT_PASSWORD_FILE", pwf.to_str().unwrap())],
        );
        assert!(String::from_utf8_lossy(&o.stdout).starts_with("PWD=desde-archivo "));
        // Sin credenciales: falla con un código reconocible.
        let o = run(&dump_script(DbEngine::Mariadb), &[]);
        assert_eq!(o.status.code(), Some(3));
        assert!(String::from_utf8_lossy(&o.stderr).contains("DCC_NO_CREDENTIALS"));
        let o = run(
            &dump_script(DbEngine::Postgres),
            &[("POSTGRES_USER", "app"), ("POSTGRES_PASSWORD", "pw")],
        );
        let out = String::from_utf8_lossy(&o.stdout);
        assert!(
            out.starts_with("PGPASSWORD=pw ARGS=-U app --clean --if-exists"),
            "{out}"
        );
    }
}
