import { formatTimestamp } from "@/shared/format";
import type { QueryKey } from "@tanstack/react-query";
import type { DockerEvent } from "@/shared/ipc/bindings/DockerEvent";
import { keys } from "@/shared/query";

/** Tipos de objeto de los eventos de Docker que se ofrecen para filtrar. */
export const EVENT_TYPES = ["container", "image", "volume", "network", "daemon", "plugin", "builder"] as const;
export type EventType = (typeof EVENT_TYPES)[number];

/** Acciones de contenedor que no cambian nada visible en la aplicación
 *  (comprobaciones de salud, terminales, copias…): no provocan recargas. */
const QUIET_CONTAINER_ACTIONS = ["exec_create", "exec_start", "exec_die", "exec_detach", "attach", "detach", "resize", "top", "export", "archive-path", "extract-to-dir"];

/** Datos que hay que volver a pedir a Docker cuando llega un evento. */
export function invalidationsFor(e: Pick<DockerEvent, "kind" | "action">): QueryKey[] {
  const base = e.action.split(":")[0]!.trim();
  switch (e.kind) {
    case "container":
      if (QUIET_CONTAINER_ACTIONS.includes(base)) return [];
      return [keys.containers, ["container"], keys.overview, keys.composeProjects, keys.binds, keys.volumes, keys.networks, ["network"], ["volume"]];
    case "image":
      return [keys.images, ["image"], keys.overview];
    case "volume":
      return [keys.volumes, ["volume"], keys.volumeSizes, keys.overview];
    case "network":
      return [keys.networks, ["network"], keys.containers, ["container"], keys.overview];
    case "daemon":
      return [keys.overview];
    default:
      return [];
  }
}

/** Nombre legible del objeto: nombre, o ID corto si es un hash. */
export function eventResource(e: Pick<DockerEvent, "name" | "actorId" | "kind" | "attributes">): string {
  if (e.name) return e.name;
  if (/^(sha256:)?[0-9a-f]{64}$/.test(e.actorId)) return e.actorId.replace(/^sha256:/, "").slice(0, 12);
  return e.actorId;
}

export function eventSeconds(e: Pick<DockerEvent, "timeNano">): number {
  return Math.floor((e.timeNano + 1000) / 1e9);
}

/** `timeNano` (~1,8·10¹⁸) supera los enteros exactos de JavaScript (2⁵³): llega
 *  con un error de unos cientos de nanosegundos. Se suma 1 µs antes de truncar
 *  a milisegundos para no mostrar el milisegundo anterior. */
export function eventDate(e: Pick<DockerEvent, "timeNano">): Date {
  return new Date(Math.floor((e.timeNano + 1000) / 1e6));
}

/** Hora con milisegundos (los eventos llegan muy seguidos). */
export function formatEventTime(e: Pick<DockerEvent, "timeNano">, lang: string, withDate = false): string {
  return formatTimestamp(eventDate(e), lang, { date: withDate, ms: true });
}

/** Valor de un `<input type="datetime-local">` para unos segundos (hora local). */
export function toLocalInput(secs: number): string {
  const d = new Date(secs * 1000);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${p(d.getHours())}:${p(d.getMinutes())}`;
}

/** Segundos desde un `<input type="datetime-local">`; `null` si está vacío o no es válido. */
export function fromLocalInput(v: string): number | null {
  if (!v) return null;
  const t = new Date(v).getTime();
  return Number.isNaN(t) ? null : Math.floor(t / 1000);
}

export interface LocalFilter {
  types: string[];
  text: string;
}

export function matchesLocal(e: DockerEvent, f: LocalFilter): boolean {
  if (f.types.length > 0 && !f.types.includes(e.kind)) return false;
  const q = f.text.trim().toLowerCase();
  if (!q) return true;
  return [e.kind, e.action, e.actorId, e.name ?? "", e.attributes.image ?? ""].some((x) => x.toLowerCase().includes(q));
}

/** Clave estable de un evento (para listas). */
export function eventKey(e: DockerEvent, i: number): string {
  return `${e.timeNano}-${e.kind}-${e.action}-${e.actorId}-${i}`;
}
