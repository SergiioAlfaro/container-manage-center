// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { formatTimestamp } from "@/shared/format";
import type { Category } from "@/shared/ipc/bindings/Category";
import type { OpStatus } from "@/shared/ipc/bindings/OpStatus";
import type { OperationRecord } from "@/shared/ipc/bindings/OperationRecord";
import type { Transport } from "@/shared/ipc/bindings/Transport";

export const CATEGORIES: Category[] = ["system", "container", "image", "network", "volume", "bind", "compose", "backup"];
export const STATUSES: OpStatus[] = ["running", "succeeded", "failed", "cancelled"];

/** Misma regla que `dcc_activity::quote` en Rust (probado en ambos lados). */
const SAFE = /^[A-Za-z0-9\-_./:=@%+,]+$/;

export function quoteArg(arg: string): string {
  if (arg.length > 0 && SAFE.test(arg)) return arg;
  return `'${arg.replaceAll("'", "'\\''")}'`;
}

export function renderArgv(program: string, args: string[]): string {
  return [program, ...args].map(quoteArg).join(" ");
}

/** Texto de la operación realmente ejecutada. */
export function describeTransport(t: Transport): string {
  switch (t.via) {
    case "api":
      return `${t.method} ${t.path}`;
    case "cli":
      return renderArgv(t.program, t.args);
    case "app":
      // Operación propia de la aplicación (0.9.2): sin petición ni proceso.
      return t.action;
  }
}

/**
 * Comando que se muestra en la línea principal del registro:
 * para CLI es el comando ejecutado; para API, el equivalente (si existe).
 */
export function headlineCommand(r: OperationRecord): string | null {
  if (r.transport.via === "cli") return describeTransport(r.transport);
  return r.equivalentCommand;
}

export interface ActivityFilter {
  query: string;
  category: Category | "all";
  status: OpStatus | "all";
  /** Servidor (0.10.0): identificador o "all". */
  server: string;
}

export const EMPTY_FILTER: ActivityFilter = { query: "", category: "all", status: "all", server: "all" };

/** Servidor de un registro; los anteriores a 0.10.0 son del equipo local. */
export function recordServer(r: Pick<OperationRecord, "server">): string {
  return r.server?.id ?? "local";
}

export function isFiltered(f: ActivityFilter): boolean {
  return f.query.trim() !== "" || f.category !== "all" || f.status !== "all" || f.server !== "all";
}

export function matches(r: OperationRecord, f: ActivityFilter): boolean {
  if (f.server !== "all" && recordServer(r) !== f.server) return false;
  if (f.category !== "all" && r.category !== f.category) return false;
  if (f.status !== "all" && r.status !== f.status) return false;
  const q = f.query.trim().toLowerCase();
  if (!q) return true;
  const haystack = [r.kind, r.target ?? "", r.equivalentCommand ?? "", describeTransport(r.transport), r.error ?? "", r.summary ?? "", r.server?.name ?? ""]
    .join("\n")
    .toLowerCase();
  return q.split(/\s+/).every((term) => haystack.includes(term));
}

/** Inserta o reemplaza por id manteniendo el orden: más reciente primero. */
export function upsert(list: OperationRecord[], record: OperationRecord, max: number): OperationRecord[] {
  const idx = list.findIndex((r) => r.id === record.id);
  if (idx >= 0) {
    const next = list.slice();
    next[idx] = record;
    return next;
  }
  return [record, ...list].slice(0, max);
}

export function formatDuration(ms: number | null): string {
  if (ms === null) return "";
  if (ms < 1000) return `${ms} ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(ms < 10_000 ? 2 : 1)} s`;
  const m = Math.floor(ms / 60_000);
  const s = Math.round((ms % 60_000) / 1000);
  return `${m} min ${s} s`;
}

export function formatTime(iso: string, lang: string): string {
  return formatTimestamp(new Date(iso), lang);
}

export function formatDateTime(iso: string, lang: string): string {
  return formatTimestamp(new Date(iso), lang, { date: true, ms: true });
}
