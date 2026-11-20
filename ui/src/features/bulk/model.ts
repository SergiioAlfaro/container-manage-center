// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * Lógica pura de la gestión masiva (1.10.0): filas de varios servidores,
 * indicadores, filtros, orden, páginas, selección y qué contenedores admite
 * cada operación. Las reglas de aplicabilidad son las mismas que
 * `cmc_engine::bulk::applicable` (el motor vuelve a comprobarlas con el estado
 * real justo antes de actuar; esto solo sirve para avisar antes).
 */
import type { BulkContainer } from "@/shared/ipc/bindings/BulkContainer";
import type { BulkEnvironment } from "@/shared/ipc/bindings/BulkEnvironment";
import type { BulkInventory } from "@/shared/ipc/bindings/BulkInventory";
import type { BulkOperation } from "@/shared/ipc/bindings/BulkOperation";
import type { BulkTarget } from "@/shared/ipc/bindings/BulkTarget";
import type { RestartPolicy } from "@/shared/ipc/bindings/RestartPolicy";
import type { SkipReason } from "@/shared/ipc/bindings/SkipReason";

/** Igual que `cmc_engine::bulk::MAX_TARGETS`. */
export const MAX_TARGETS = 1000;
export const PAGE_SIZES = [25, 50, 100] as const;

/** Un contenedor de un servidor. La clave une servidor e ID: el mismo ID en
 *  dos servidores son dos contenedores distintos. */
export interface BulkRow {
  key: string;
  server: string;
  envName: string;
  remote: boolean;
  c: BulkContainer;
}

export const rowKey = (server: string, id: string) => `${server}\u0000${id}`;

/** Nombre visible de un servidor (`localName` para este equipo). */
export function envLabel(e: Pick<BulkEnvironment, "remote" | "name">, localName: string): string {
  return e.remote ? e.name : localName;
}

export function rowsOf(inv: BulkInventory | undefined, localName: string): BulkRow[] {
  if (!inv) return [];
  return inv.environments.flatMap((e) =>
    e.containers.map((c) => ({ key: rowKey(e.server, c.id), server: e.server, envName: envLabel(e, localName), remote: e.remote, c })),
  );
}

// ---------------------------------------------------------------- indicadores

export type StateGroup = "running" | "stopped" | "paused" | "error";

export function inGroup(c: BulkContainer, g: StateGroup): boolean {
  switch (g) {
    case "running":
      return c.state === "running";
    case "paused":
      return c.state === "paused";
    case "stopped":
      return c.state === "exited" || c.state === "created" || c.state === "dead";
    case "error":
      return c.error;
  }
}

export interface Kpis {
  total: number;
  running: number;
  stopped: number;
  paused: number;
  error: number;
}

export function kpis(rows: BulkRow[]): Kpis {
  const k: Kpis = { total: rows.length, running: 0, stopped: 0, paused: 0, error: 0 };
  for (const r of rows) {
    if (inGroup(r.c, "running")) k.running++;
    if (inGroup(r.c, "stopped")) k.stopped++;
    if (inGroup(r.c, "paused")) k.paused++;
    if (inGroup(r.c, "error")) k.error++;
  }
  return k;
}

// ---------------------------------------------------------------- filtros

export interface BulkFilter {
  query: string;
  state: StateGroup | "all";
  image: string;
  /** `clave` o `clave=valor`; vacío = sin filtro. */
  label: string;
  /** Servidor o "all". */
  env: string;
}

export const EMPTY_FILTER: BulkFilter = { query: "", state: "all", image: "all", label: "", env: "all" };

export function isFiltered(f: BulkFilter): boolean {
  return f.query.trim() !== "" || f.state !== "all" || f.image !== "all" || f.label.trim() !== "" || f.env !== "all";
}

export function matchesLabel(labels: BulkContainer["labels"], expr: string): boolean {
  const e = expr.trim();
  if (!e) return true;
  const eq = e.indexOf("=");
  if (eq < 0) return Object.prototype.hasOwnProperty.call(labels, e);
  const k = e.slice(0, eq).trim();
  const v = e.slice(eq + 1).trim();
  return Object.prototype.hasOwnProperty.call(labels, k) && labels[k] === v;
}

export function matches(r: BulkRow, f: BulkFilter): boolean {
  if (f.env !== "all" && r.server !== f.env) return false;
  if (f.state !== "all" && !inGroup(r.c, f.state)) return false;
  if (f.image !== "all" && r.c.image !== f.image) return false;
  if (!matchesLabel(r.c.labels, f.label)) return false;
  const q = f.query.trim().toLowerCase();
  if (!q) return true;
  // Nombre o ID (también un prefijo del ID, como el ID corto de Docker).
  return r.c.name.toLowerCase().includes(q) || r.c.id.startsWith(q);
}

export function imagesOf(rows: BulkRow[]): string[] {
  return [...new Set(rows.map((r) => r.c.image))].sort((a, b) => a.localeCompare(b));
}

/** Etiquetas que se muestran en la tabla: las que pone el usuario, no las que
 *  añaden Docker Compose o las imágenes (OCI), que son muchas y repetitivas. */
export function relevantLabels(labels: BulkContainer["labels"]): Array<[string, string]> {
  return Object.entries(labels)
    .filter((e): e is [string, string] => e[1] !== undefined)
    .filter(([k]) => !k.startsWith("com.docker.") && !k.startsWith("org.opencontainers.") && !k.startsWith("desktop.docker."))
    .sort(([a], [b]) => a.localeCompare(b));
}

// ---------------------------------------------------------------- orden y páginas

export type SortKey = "name" | "image" | "state" | "created" | "env" | "restart";
export interface Sort {
  key: SortKey;
  dir: "asc" | "desc";
}

export function policyText(p: RestartPolicy | undefined | null): string {
  if (!p) return "";
  return p.name === "on-failure" && p.maximumRetryCount > 0 ? `on-failure:${p.maximumRetryCount}` : p.name;
}

function sortValue(r: BulkRow, k: SortKey): string | number {
  switch (k) {
    case "name":
      return r.c.name.toLowerCase();
    case "image":
      return r.c.image.toLowerCase();
    case "state":
      return r.c.state;
    case "created":
      return r.c.created;
    case "env":
      return r.envName.toLowerCase();
    case "restart":
      return policyText(r.c.details?.restartPolicy);
  }
}

export function sortRows(rows: BulkRow[], s: Sort): BulkRow[] {
  const dir = s.dir === "asc" ? 1 : -1;
  return [...rows].sort((a, b) => {
    const x = sortValue(a, s.key);
    const y = sortValue(b, s.key);
    const c = typeof x === "number" && typeof y === "number" ? x - y : String(x).localeCompare(String(y));
    // Desempate estable: servidor y nombre.
    return c !== 0 ? c * dir : a.key.localeCompare(b.key);
  });
}

export function pageCount(total: number, size: number): number {
  return Math.max(1, Math.ceil(total / size));
}

export function pageOf<T>(rows: T[], page: number, size: number): T[] {
  const p = Math.min(Math.max(0, page), pageCount(rows.length, size) - 1);
  return rows.slice(p * size, p * size + size);
}

// ---------------------------------------------------------------- selección

/** Quita de la selección lo que ya no existe (tras actualizar). */
export function pruneSelection(sel: ReadonlySet<string>, rows: BulkRow[]): Set<string> {
  const keys = new Set(rows.map((r) => r.key));
  return new Set([...sel].filter((k) => keys.has(k)));
}

export function selectAll(sel: ReadonlySet<string>, rows: BulkRow[]): Set<string> {
  const next = new Set(sel);
  for (const r of rows) next.add(r.key);
  return next;
}

export function selectedRows(rows: BulkRow[], sel: ReadonlySet<string>): BulkRow[] {
  return rows.filter((r) => sel.has(r.key));
}

// ---------------------------------------------------------------- operaciones

export type OpKind = BulkOperation["kind"];
export const LIFECYCLE: OpKind[] = ["start", "stop", "restart", "pause", "unpause"];

export function isActive(state: string): boolean {
  return state === "running" || state === "paused" || state === "restarting";
}

/** Igual que `cmc_engine::bulk::applicable`, más lo que ya se sabe de
 *  `inspect` para la política de reinicio (`--rm` y sin cambios). */
export function skipReason(op: BulkOperation, c: BulkContainer): SkipReason | null {
  const s = c.state;
  switch (op.kind) {
    case "start":
      return ["created", "exited", "dead"].includes(s) ? null : { reason: "state", state: s };
    case "stop":
      return ["running", "paused", "restarting"].includes(s) ? null : { reason: "state", state: s };
    case "restart":
    case "pause":
      return s === "running" ? null : { reason: "state", state: s };
    case "unpause":
      return s === "paused" ? null : { reason: "state", state: s };
    case "remove":
      if (s === "removing") return { reason: "state", state: s };
      return isActive(s) && !op.force ? { reason: "needsForce", state: s } : null;
    case "restartPolicy":
      if (s === "removing") return { reason: "state", state: s };
      if (c.details?.autoRemove) return { reason: "autoRemove" };
      if (c.details && policyText(c.details.restartPolicy) === policyText(op.policy)) return { reason: "unchanged" };
      return null;
  }
}

export interface Plan {
  /** Se envían (el motor las vuelve a comprobar con el estado real). */
  apply: BulkRow[];
  /** No se envían: se informa del motivo. */
  skip: Array<{ row: BulkRow; reason: SkipReason }>;
}

export function plan(op: BulkOperation, rows: BulkRow[]): Plan {
  const p: Plan = { apply: [], skip: [] };
  for (const r of rows) {
    const reason = skipReason(op, r.c);
    if (reason) p.skip.push({ row: r, reason });
    else p.apply.push(r);
  }
  return p;
}

export function targetsOf(rows: BulkRow[]): BulkTarget[] {
  return rows.map((r) => ({ server: r.server, id: r.c.id, name: r.c.name }));
}

/** Cuántos de la selección admiten cada operación (para la barra). */
export function applicableCount(kind: Exclude<OpKind, "remove" | "restartPolicy">, rows: BulkRow[]): number {
  const op = { kind } as BulkOperation;
  return rows.filter((r) => skipReason(op, r.c) === null).length;
}

export function isDestructive(op: BulkOperation): boolean {
  return op.kind === "remove";
}

/** Servidores distintos de una lista de filas, con cuántos contenedores. */
export function envsOf(rows: BulkRow[]): Array<{ server: string; name: string; remote: boolean; count: number }> {
  const m = new Map<string, { server: string; name: string; remote: boolean; count: number }>();
  for (const r of rows) {
    const e = m.get(r.server) ?? { server: r.server, name: r.envName, remote: r.remote, count: 0 };
    e.count++;
    m.set(r.server, e);
  }
  return [...m.values()];
}

export function validPolicy(p: RestartPolicy): boolean {
  return Number.isInteger(p.maximumRetryCount) && p.maximumRetryCount >= 0 && p.maximumRetryCount <= 1000 && (p.name === "on-failure" || p.maximumRetryCount === 0);
}
