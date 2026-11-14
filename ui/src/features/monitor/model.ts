/**
 * E5 (1.5.0): lógica pura de la monitorización (sin React).
 */
import type { Alert } from "@/shared/ipc/bindings/Alert";
import type { MonitorLive } from "@/shared/ipc/bindings/MonitorLive";
import type { MonitorMinute } from "@/shared/ipc/bindings/MonitorMinute";
import type { MonitorMinuteContainer } from "@/shared/ipc/bindings/MonitorMinuteContainer";
import type { MonitorPoint } from "@/shared/ipc/bindings/MonitorPoint";
import type { MonitorServerPoint } from "@/shared/ipc/bindings/MonitorServerPoint";
import type { ChartSeries, Gap } from "@/shared/ui/LineChart";

type T = (k: string, o?: Record<string, unknown>) => string;

export type RangeKey = "5m" | "15m" | "30m" | "1h" | "6h" | "24h";
export const RANGES: { key: RangeKey; minutes: number; live: boolean }[] = [
  { key: "5m", minutes: 5, live: true },
  { key: "15m", minutes: 15, live: true },
  { key: "30m", minutes: 30, live: true },
  { key: "1h", minutes: 60, live: false },
  { key: "6h", minutes: 360, live: false },
  { key: "24h", minutes: 1440, live: false },
];
export const INTERVALS = [2, 5, 10, 15, 30, 60];
/** Contenedores que se pueden comparar a la vez. */
export const MAX_COMPARE = 4;

export type Metric = "cpu" | "mem" | "memPercent" | "netRx" | "netTx" | "blkRead" | "blkWrite" | "pids";
export const METRICS: Metric[] = ["cpu", "mem", "memPercent", "netRx", "netTx", "blkRead", "blkWrite", "pids"];

export function rangeOf(key: RangeKey) {
  return RANGES.find((r) => r.key === key) ?? RANGES[0]!;
}

/** Valor de una métrica en un punto en vivo. */
export function pointValue(p: MonitorPoint | null, m: Metric): number | null {
  if (!p) return null;
  switch (m) {
    case "cpu":
      return p.cpu;
    case "mem":
      return p.mem;
    case "memPercent":
      return p.memPercent;
    case "netRx":
      return p.netRx;
    case "netTx":
      return p.netTx;
    case "blkRead":
      return p.blkRead;
    case "blkWrite":
      return p.blkWrite;
    case "pids":
      return p.pids;
  }
}

/** Valor de una métrica en un minuto guardado (medias; bytes → bytes/s). */
export function minuteValue(c: MonitorMinuteContainer | undefined, m: Metric): number | null {
  if (!c) return null;
  switch (m) {
    case "cpu":
      return c.cpuAvg;
    case "mem":
      return c.memAvg;
    case "memPercent":
      return c.memLimit > 0 ? (c.memAvg / c.memLimit) * 100 : null;
    case "netRx":
      return c.netRx / 60;
    case "netTx":
      return c.netTx / 60;
    case "blkRead":
      return c.blkRead / 60;
    case "blkWrite":
      return c.blkWrite / 60;
    case "pids":
      return c.pidsMax;
  }
}

export function liveSeries(live: MonitorLive | undefined, ids: string[], names: Record<string, string>, m: Metric): ChartSeries[] {
  return ids.map((id) => {
    const s = live?.containers.find((c) => c.id === id);
    return { name: names[id] ?? id.slice(0, 12), points: (s?.points ?? []).map(([t, p]) => [t, pointValue(p, m)] as [number, number | null]) };
  });
}

/** Series de los minutos guardados. Un minuto que falta es un hueco (`null`). */
export function minuteSeries(records: MonitorMinute[], ids: string[], names: Record<string, string>, m: Metric): ChartSeries[] {
  return ids.map((id) => {
    const pts: Array<[number, number | null]> = [];
    let prev: number | null = null;
    for (const r of records) {
      if (prev !== null && r.t - prev > 90_000) pts.push([prev + 60_000, null]);
      pts.push([r.t, minuteValue(r.containers.find((c) => c.id === id), m)]);
      prev = r.t;
    }
    const name = names[id] ?? records.flatMap((r) => r.containers).find((c) => c.id === id)?.name ?? id.slice(0, 12);
    return { name, points: pts };
  });
}

export type ServerMetric = "cpuTotal" | "memTotal" | "running";

export function serverLive(points: MonitorServerPoint[], m: ServerMetric): Array<[number, number | null]> {
  return points.map((p) => [p.t, !p.connected ? null : m === "cpuTotal" ? p.cpuTotal : m === "memTotal" ? p.memTotal : p.counts.running]);
}

export function serverMinutes(records: MonitorMinute[], m: ServerMetric): Array<[number, number | null]> {
  return records.map((r) => [r.t, r.samples === 0 ? null : m === "cpuTotal" ? r.cpuTotalAvg : m === "memTotal" ? r.memTotalAvg : null]);
}

/** Tramos sin conexión de una serie en vivo del servidor. */
export function liveGaps(points: MonitorServerPoint[]): Gap[] {
  const out: Gap[] = [];
  let start: number | null = null;
  for (let i = 0; i < points.length; i++) {
    const p = points[i]!;
    if (!p.connected && start === null) start = i > 0 ? points[i - 1]!.t : p.t;
    if (p.connected && start !== null) {
      out.push([start, p.t]);
      start = null;
    }
  }
  if (start !== null) out.push([start, points[points.length - 1]!.t]);
  return out;
}

/** Tramos de minutos con huecos o sin registro. */
export function minuteGaps(records: MonitorMinute[]): Gap[] {
  const out: Gap[] = [];
  for (let i = 0; i < records.length; i++) {
    const r = records[i]!;
    if (r.gaps > 0 && r.samples === 0) out.push([r.t, r.t + 60_000]);
    const next = records[i + 1];
    if (next && next.t - r.t > 90_000) out.push([r.t + 60_000, next.t]);
  }
  return out;
}

/** Tiempo en marcha legible («3 d 4 h», «12 min»). */
export function uptime(t: T, startedAt: string | null, now: number): string {
  if (!startedAt) return "—";
  const s = Date.parse(startedAt);
  if (!Number.isFinite(s) || s <= 0) return "—";
  const secs = Math.max(0, Math.floor((now - s) / 1000));
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (d > 0) return t("monitor.uptime.days", { d, h });
  if (h > 0) return t("monitor.uptime.hours", { h, m });
  if (m > 0) return t("monitor.uptime.minutes", { m });
  return t("monitor.uptime.seconds", { s: secs });
}

/** Título y texto de una alerta (lista y notificación de escritorio). */
export function alertText(t: T, a: Alert, server: string, fmtPct: (v: number) => string): { summary: string; body: string } {
  const who = a.container ?? server;
  const value = a.value === null ? "" : a.kind === "exitError" ? String(a.value) : fmtPct(a.value);
  const state = a.state === "fired" ? "fired" : "resolved";
  return {
    summary: t(`monitor.alert.${a.kind}.${state}`, { name: who }),
    body: t(`monitor.alert.${a.kind}.body`, { name: who, value, server }),
  };
}
