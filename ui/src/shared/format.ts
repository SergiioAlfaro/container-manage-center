/** Formateadores de datos de Docker para la interfaz. */

export function localeOf(lang: string) {
  return lang === "es" ? "es-ES" : "en-US";
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
  const nf = new Intl.NumberFormat(localeOf(lang), { maximumFractionDigits: v < 10 && i > 0 ? 1 : 0 });
  return `${nf.format(v)} ${units[i]}`;
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
  const rtf = new Intl.RelativeTimeFormat(localeOf(lang), { numeric: "auto" });
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
  return new Date(unixSeconds * 1000).toLocaleString(localeOf(lang));
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
