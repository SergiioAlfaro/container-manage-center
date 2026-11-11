/** Formateadores de datos de Docker para la interfaz. */

/** Idioma de la aplicación ("es", "en") o configuración regional ("es-ES") → configuración regional. */
export function localeOf(lang: string) {
  if (lang.startsWith("es")) return "es-ES";
  if (lang.startsWith("fr")) return "fr-FR";
  if (lang.startsWith("it")) return "it-IT";
  // 0.9.2. Portugués de Portugal (las traducciones usan la norma europea).
  if (lang.startsWith("ca")) return "ca-ES";
  if (lang.startsWith("gl")) return "gl-ES";
  if (lang.startsWith("eu")) return "eu-ES";
  if (lang.startsWith("pt")) return "pt-PT";
  return "en-US";
}

// Crear un formateador de `Intl` es caro (cientos de microsegundos en WebKit) y
// las listas formatean varias fechas por fila: se crean una vez y se reutilizan.
const formatters = new Map<string, unknown>();
function cached<T>(key: string, make: () => T): T {
  let f = formatters.get(key) as T | undefined;
  if (f === undefined) {
    f = make();
    formatters.set(key, f);
  }
  return f;
}

export function numberFormat(lang: string, opts: Intl.NumberFormatOptions = {}): Intl.NumberFormat {
  const locale = localeOf(lang);
  return cached(`n|${locale}|${JSON.stringify(opts)}`, () => new Intl.NumberFormat(locale, opts));
}

export function dateTimeFormat(lang: string, opts: Intl.DateTimeFormatOptions): Intl.DateTimeFormat {
  const locale = localeOf(lang);
  return cached(`d|${locale}|${JSON.stringify(opts)}`, () => new Intl.DateTimeFormat(locale, opts));
}

function relativeFormat(lang: string): Intl.RelativeTimeFormat {
  const locale = localeOf(lang);
  return cached(`r|${locale}`, () => new Intl.RelativeTimeFormat(locale, { numeric: "auto" }));
}

const DATE: Intl.DateTimeFormatOptions = { day: "2-digit", month: "2-digit", year: "numeric" };
const TIME: Intl.DateTimeFormatOptions = { hour: "2-digit", minute: "2-digit", second: "2-digit", hourCycle: "h23" };

/**
 * Marca de tiempo de registros (panel de actividad, eventos, historial, logs):
 * siempre 24 horas, con fecha y milisegundos opcionales. Mismo formato en toda
 * la aplicación: "07/11/2026 18:08:33.904" (es) / "11/07/2026 18:08:33.904" (en).
 */
export function formatTimestamp(d: Date, lang: string, { date = false, ms = false }: { date?: boolean; ms?: boolean } = {}): string {
  if (Number.isNaN(d.getTime())) return "—";
  let s = dateTimeFormat(lang, TIME).format(d);
  if (ms) s += `.${String(d.getMilliseconds()).padStart(3, "0")}`;
  return date ? `${dateTimeFormat(lang, DATE).format(d)} ${s}` : s;
}

/** Fecha y hora de un objeto de Docker (creación de una imagen, una red…). */
export function formatDate(d: Date, lang: string): string {
  return formatTimestamp(d, lang, { date: true });
}

/** Tamaño en unidades binarias, como `docker images` (que usa base 1000) no: aquí base 1024, explícita. */
export function formatBytes(bytes: number | null | undefined, lang: string): string {
  if (bytes === null || bytes === undefined || bytes < 0) return "";
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${numberFormat(lang, { maximumFractionDigits: v < 10 && i > 0 ? 1 : 0 }).format(v)} ${units[i]}`;
}

/**
 * Docker devuelve la fecha cero de Go (0001-01-01, -62135596800 s) cuando un
 * objeto no tiene fecha; el CLI la muestra como "N/A". Aquí se muestra "—".
 */
export function hasDate(unixSeconds: number): boolean {
  return Number.isFinite(unixSeconds) && unixSeconds >= 0;
}

/** "hace 3 horas" a partir de una marca Unix en segundos; "—" sin fecha. */
export function formatRelative(unixSeconds: number, lang: string, now = Date.now()): string {
  if (!hasDate(unixSeconds)) return "—";
  const diff = unixSeconds * 1000 - now;
  const rtf = relativeFormat(lang);
  const abs = Math.abs(diff);
  const units: Array<[Intl.RelativeTimeFormatUnit, number]> = [
    ["year", 365 * 86_400_000],
    ["month", 30 * 86_400_000],
    ["day", 86_400_000],
    ["hour", 3_600_000],
    ["minute", 60_000],
  ];
  for (const [unit, ms] of units) {
    if (abs >= ms) return rtf.format(Math.round(diff / ms), unit);
  }
  return rtf.format(Math.round(diff / 1000), "second");
}

export function formatAbsolute(unixSeconds: number, lang: string): string {
  if (!hasDate(unixSeconds)) return "—";
  return formatDate(new Date(unixSeconds * 1000), lang);
}

/** 12 primeros caracteres del ID, sin el prefijo `sha256:`. */
export function shortId(id: string): string {
  return id.replace(/^sha256:/, "").slice(0, 12);
}

export function formatPort(p: { ip: string | null; privatePort: number; publicPort: number | null; protocol: string }): string {
  const proto = p.protocol || "tcp";
  if (p.publicPort === null) return `${p.privatePort}/${proto}`;
  const ip = p.ip && p.ip !== "0.0.0.0" && p.ip !== "::" ? `${p.ip.includes(":") ? `[${p.ip}]` : p.ip}:` : "";
  return `${ip}${p.publicPort}→${p.privatePort}/${proto}`;
}

/** Docker publica cada puerto dos veces (IPv4 e IPv6); se muestran una sola vez. */
export function uniquePorts<T extends { ip: string | null; privatePort: number; publicPort: number | null; protocol: string }>(ports: T[]): string[] {
  return [...new Set(ports.map((p) => formatPort({ ...p, ip: p.ip === "::" ? "0.0.0.0" : p.ip })))].sort();
}
