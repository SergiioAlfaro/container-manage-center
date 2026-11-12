/**
 * Reglas de secretos de la interfaz (E1, DCC-SEC-05, D-085). Son las mismas que
 * `crates/dcc-activity/src/redact.rs`: una prueba de Rust comprueba que estas
 * listas coinciden con las suyas. Solo afectan a lo que se muestra; el valor
 * real se envía a Docker sin cambios.
 */

export const REDACTED = "********";

/** Palabras que, en cualquier parte del nombre, lo hacen secreto. */
export const SECRET_NAME_WORDS = ["PASSWORD", "PASSWD", "SECRET", "TOKEN", "KEY", "CREDENTIAL", "PRIVATE"] as const;
/** Terminaciones de una parte del nombre (`_`, `-`, `.`) que lo hacen secreto. */
export const SECRET_NAME_PART_ENDINGS = ["PASS", "PWD", "AUTH", "DSN", "COOKIE"] as const;
/** Nombres que coinciden con las reglas pero nunca son secretos. */
export const NOT_SECRET_NAMES = ["PWD", "OLDPWD"] as const;
/** Terminaciones de una ruta a un secreto (Docker secrets): se muestran. */
export const PATH_SUFFIXES = ["_FILE", "-FILE"] as const;

export function isSecretName(name: string): boolean {
  const n = name.trim().toUpperCase();
  if (n === "" || (NOT_SECRET_NAMES as readonly string[]).includes(n)) return false;
  if (PATH_SUFFIXES.some((s) => n.endsWith(s))) return false;
  if (SECRET_NAME_WORDS.some((w) => n.includes(w))) return true;
  return n.split(/[_.-]/).some((part) => SECRET_NAME_PART_ENDINGS.some((e) => part.endsWith(e)));
}

const SCHEME_CREDENTIALS = /:\/\/[^\s"'/<>@:]*:[^\s"'/<>@]+@/;
const SCHEMELESS_CREDENTIALS = /^[^\s"'/<>@:]+:[^\s"'/<>@]+@/;

/** ¿Lleva el valor una contraseña incrustada (URL o DSN `usuario:clave@host`)? */
export function hasCredentials(value: string): boolean {
  return value.includes("://") ? SCHEME_CREDENTIALS.test(value) : SCHEMELESS_CREDENTIALS.test(value);
}

const TRIVIAL = new Set(["true", "false", "yes", "no", "on", "off", "none", "null"]);

/** ¿Se oculta por defecto este valor? (nombre secreto o credenciales en el valor). */
export function isSecretEntry(name: string, value: string): boolean {
  if (isSecretName(name) && value !== "" && !TRIVIAL.has(value.toLowerCase())) return true;
  return hasCredentials(value);
}
