// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * E4 (1.4.0): lógica pura del apartado Copias (sin React), para probarla aparte.
 */
import type { BackupEntry } from "@/shared/ipc/bindings/BackupEntry";
import type { BackupItem } from "@/shared/ipc/bindings/BackupItem";
import type { BackupJob } from "@/shared/ipc/bindings/BackupJob";
import type { BackupRequest } from "@/shared/ipc/bindings/BackupRequest";
import type { BackupSchedule } from "@/shared/ipc/bindings/BackupSchedule";
import type { BackupTimerStatus } from "@/shared/ipc/bindings/BackupTimerStatus";
import type { CommandError } from "@/shared/ipc/bindings/CommandError";
import type { ContainerSummary } from "@/shared/ipc/bindings/ContainerSummary";
import type { DbEngine } from "@/shared/ipc/bindings/DbEngine";
import type { VolumeSummary } from "@/shared/ipc/bindings/VolumeSummary";
import type { Retention } from "@/shared/ipc/bindings/Retention";
import type { RestoreItem } from "@/shared/ipc/bindings/RestoreItem";

type T = (k: string, o?: Record<string, unknown>) => string;
type Exists = (k: string) => boolean;

export const EMPTY_REQUEST: BackupRequest = { volumes: [], databases: [], containers: [], compose: [], appConfig: false };
export const DEFAULT_RETENTION: Retention = { daily: 7, weekly: 4, monthly: 3 };

export function isEmptyRequest(r: BackupRequest): boolean {
  return r.volumes.length + r.databases.length + r.containers.length + r.compose.length === 0 && !r.appConfig;
}

/** Misma detección que el motor (por la imagen): solo para proponer qué volcar. */
export function dbEngineOf(image: string): DbEngine | null {
  const repo = (image.split("/").pop() ?? image).split(/[@:]/)[0]!.toLowerCase();
  const table: [string, DbEngine][] = [
    ["mariadb", "mariadb"],
    ["mysql", "mysql"],
    ["percona", "mysql"],
    ["postgres", "postgres"],
    ["postgis", "postgres"],
    ["timescaledb", "postgres"],
    ["redis", "redis"],
    ["valkey", "redis"],
    ["mongo", "mongo"],
  ];
  return table.find(([k]) => repo.includes(k))?.[1] ?? null;
}

/** Contenedores en marcha con una base de datos reconocible. */
export function databaseContainers(list: ContainerSummary[]): (ContainerSummary & { engine: DbEngine })[] {
  return list.flatMap((c) => {
    const engine = dbEngineOf(c.image);
    return engine && c.state === "running" ? [{ ...c, engine }] : [];
  });
}

/** E6 (1.6.0): selección para proteger un contenedor, buscado por id (si se
 *  ha vuelto a crear, por nombre): su configuración, su base de datos si es
 *  una en marcha y los volúmenes con nombre que monta (en caliente; el
 *  formulario avisa y deja elegir detenerlo). `null` si ya no existe. */
export function prefillRequest(id: string, name: string, containers: ContainerSummary[], volumes: VolumeSummary[]): BackupRequest | null {
  const c = containers.find((x) => x.id === id) ?? containers.find((x) => x.name === name);
  if (!c) return null;
  const isDb = databaseContainers([c]).length > 0;
  const vols = volumes.filter((v) => !v.anonymous && v.containers.some((u) => u.id === c.id));
  return {
    ...EMPTY_REQUEST,
    containers: [c.name],
    databases: isDb ? [c.name] : [],
    volumes: vols.map((v) => ({ name: v.name, mode: "hot" as const })),
  };
}

/** Código (`restore.volumeNotEmpty`, `backup.warn.hot:web`…) → texto. Los
 *  códigos llevan, tras «:», un dato que se pasa como `value`. */
export function codeText(t: T, exists: Exists, code: string): string {
  const [key, ...rest] = code.split(":");
  const value = rest.join(":");
  const k = `backup.codes.${key!.replace(/\./g, "_")}`;
  return exists(k) ? t(k, { value }) : code;
}

export function errorText(t: T, exists: Exists, e: CommandError | null | undefined): string {
  if (!e) return "";
  const k = `backup.codes.${e.code.replace(/\./g, "_")}`;
  if (exists(k)) return t(k, { value: "" });
  return e.message;
}

const WEEKDAYS = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"] as const;

function hhmm(h: number, m: number): string {
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;
}

export function scheduleText(t: T, s: BackupSchedule): string {
  switch (s.kind) {
    case "daily":
      return t("backup.schedule.dailyAt", { time: hhmm(s.hour, s.minute) });
    case "weekly":
      return t("backup.schedule.weeklyAt", { day: t(`backup.weekday.${WEEKDAYS[s.weekday - 1] ?? "mon"}`), time: hhmm(s.hour, s.minute) });
    case "monthly":
      return t("backup.schedule.monthlyAt", { day: s.day, time: hhmm(s.hour, s.minute) });
  }
}

export function retentionText(t: T, r: Retention): string {
  return t("backup.retentionText", { daily: r.daily, weekly: r.weekly, monthly: r.monthly });
}

/** Estado del temporizador en una frase. */
export function timerText(t: T, s: BackupTimerStatus): { text: string; tone: "ok" | "warn" | "muted" } {
  if (s.foreign) return { text: t("backup.timer.foreign"), tone: "warn" };
  if (!s.installed) return { text: t("backup.timer.none"), tone: "muted" };
  if (s.outdated) return { text: t("backup.timer.outdated"), tone: "warn" };
  if (s.problem) return { text: t("backup.timer.problem", { problem: s.problem }), tone: "warn" };
  if (s.unitFileState === "disabled" || s.activeState === "inactive") return { text: t("backup.timer.disabled"), tone: "muted" };
  return { text: s.nextRun ? t("backup.timer.next", { when: s.nextRun }) : t("backup.timer.active"), tone: "ok" };
}

/** Fecha de una copia: la del manifiesto o, si no hay, la de su id. */
export function backupDate(e: BackupEntry): string {
  if (e.manifest) return e.manifest.createdAt;
  const m = /^(\d{4})(\d{2})(\d{2})T(\d{2})(\d{2})(\d{2})Z/.exec(e.id);
  return m ? `${m[1]}-${m[2]}-${m[3]}T${m[4]}:${m[5]}:${m[6]}Z` : "";
}

export function itemWarnings(e: BackupEntry): number {
  return (e.manifest?.items ?? []).reduce((n, i) => n + i.warnings.length, 0);
}

/** Destino propuesto para restaurar un elemento: siempre algo nuevo, salvo
 *  las bases de datos (su propio contenedor). */
export function defaultTarget(item: BackupItem, folder: string | null): string {
  switch (item.kind) {
    case "volume":
      return `${item.name}-restaurado`;
    case "container":
      return `${item.name}-restaurado`;
    case "database":
      return item.details["container"] ?? item.name;
    case "compose":
    case "appConfig":
      return folder ? `${folder.replace(/\/+$/, "")}/${item.kind === "compose" ? item.name : "configuracion"}-restaurado` : "";
  }
}

export function restoreItems(items: BackupItem[], selected: Record<string, boolean>, targets: Record<string, string>): RestoreItem[] {
  return items.filter((i) => selected[itemKey(i)]).map((i) => ({ kind: i.kind, name: i.name, target: targets[itemKey(i)] ?? "" }));
}

export function itemKey(i: { kind: string; name: string }): string {
  return `${i.kind}:${i.name}`;
}

/** Una tarea nueva con lo elegido en «Nueva copia». */
export function newJob(server: string, destination: string, request: BackupRequest): BackupJob {
  return {
    id: "",
    name: "",
    server,
    destination,
    request,
    retention: { ...DEFAULT_RETENTION },
    schedule: { kind: "daily", hour: 3, minute: 0 },
  };
}

/** Palabra que hay que escribir para confirmar una restauración con pérdida de datos. */
export function confirmWord(t: T): string {
  return t("backup.restore.confirmWord");
}
