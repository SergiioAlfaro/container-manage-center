// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import type { AuditFix } from "@/shared/ipc/bindings/AuditFix";
/**
 * Estado de la pantalla «Editar configuración» (0.9.2): borrador, textos de
 * los campos numéricos y cambios pendientes por pestaña.
 *
 * La clasificación de aquí solo sirve para los indicadores mientras se edita;
 * la comparación que se aplica es la del backend (`container_edit_plan`), que
 * es quien valida y decide (D-070).
 */
import type { EditRequest } from "@/shared/ipc/bindings/EditRequest";
import type { KeyValue } from "@/shared/ipc/bindings/KeyValue";
import type { MountSpec } from "@/shared/ipc/bindings/MountSpec";
import type { PortSpec } from "@/shared/ipc/bindings/PortSpec";
import type { RestartSpec } from "@/shared/ipc/bindings/RestartSpec";
import { bytesFrom, toMemField, type MemUnit } from "../specModel";

export type EditTab = "general" | "network" | "storage" | "env" | "ports" | "resources" | "advanced";
export const EDIT_TABS: EditTab[] = ["general", "network", "storage", "env", "ports", "resources", "advanced"];

export type EditField = "name" | "description" | "restart" | "cpus" | "memory" | "memoryReservation" | "env" | "ports" | "mounts";

/** Pestaña donde se edita cada campo. */
export const FIELD_TAB: Record<EditField, EditTab> = {
  name: "general",
  description: "general",
  restart: "general",
  cpus: "resources",
  memory: "resources",
  memoryReservation: "resources",
  env: "env",
  ports: "ports",
  mounts: "storage",
};

/** Pestaña de un problema de validación (`ports.0.hostPort` → puertos). */
export function tabOfIssue(field: string): EditTab {
  const head = field.split(".")[0] ?? "";
  if (head === "memoryBytes") return "resources";
  if (head === "memoryReservationBytes") return "resources";
  return FIELD_TAB[head as EditField] ?? "general";
}

export interface Texts {
  cpus: string;
  mem: { value: string; unit: MemUnit };
  memRes: { value: string; unit: MemUnit };
}

export function textsOf(r: EditRequest): Texts {
  return {
    cpus: r.cpus === null ? "" : String(Math.round(r.cpus * 1000) / 1000),
    mem: toMemField(r.memoryBytes),
    memRes: toMemField(r.memoryReservationBytes),
  };
}

export type LocalProblem = "cpus" | "memory" | "memoryReservation";

/** Aplica los textos numéricos al borrador. Un texto no válido se señala aquí. */
export function withTexts(r: EditRequest, t: Texts): { request: EditRequest; problems: LocalProblem[] } {
  const problems: LocalProblem[] = [];
  let cpus: number | null = null;
  const c = t.cpus.trim().replace(",", ".");
  if (c !== "") {
    const n = Number(c);
    if (!Number.isFinite(n) || n <= 0) problems.push("cpus");
    else cpus = n;
  }
  const mem = bytesFrom(t.mem.value, t.mem.unit);
  const memRes = bytesFrom(t.memRes.value, t.memRes.unit);
  if (Number.isNaN(mem)) problems.push("memory");
  if (Number.isNaN(memRes)) problems.push("memoryReservation");
  return {
    request: {
      ...r,
      cpus: problems.includes("cpus") ? r.cpus : cpus,
      memoryBytes: Number.isNaN(mem) ? r.memoryBytes : mem,
      memoryReservationBytes: Number.isNaN(memRes) ? r.memoryReservationBytes : memRes,
    },
    problems,
  };
}

const sameRestart = (a: RestartSpec, b: RestartSpec) => a.name === b.name && (a.name !== "onFailure" || a.maxRetries === b.maxRetries);
const sameKv = (a: KeyValue[], b: KeyValue[]) => a.length === b.length && a.every((x, i) => x.key.trim() === b[i]!.key.trim() && x.value === b[i]!.value);
const portKey = (p: PortSpec) => `${p.hostIp.trim()}|${p.hostPort ?? ""}|${p.containerPort}|${p.protocol}`;
const mountKey = (m: MountSpec) => `${m.kind}|${m.source.trim()}|${m.target.trim()}|${m.readOnly}`;
const sameSet = <T>(a: T[], b: T[], key: (x: T) => string) => {
  const ka = a.map(key).sort();
  const kb = b.map(key).sort();
  return ka.length === kb.length && ka.every((x, i) => x === kb[i]);
};

export interface Pending {
  field: EditField;
  /** Docker lo aplica sobre el contenedor (rename, update) o hay que recrearlo. */
  kind: "direct" | "recreate";
}

/** Cambios pendientes, con la misma clasificación que el backend. */
export function pendingChanges(cur: EditRequest, next: EditRequest): Pending[] {
  const out: Pending[] = [];
  if (cur.name !== next.name.trim()) out.push({ field: "name", kind: "direct" });
  if (cur.description !== next.description.trim()) out.push({ field: "description", kind: "recreate" });
  if (!sameRestart(cur.restart, next.restart)) out.push({ field: "restart", kind: "direct" });
  // `docker update` no sabe quitar un límite: quitarlo exige recrear.
  const limit = (field: EditField, a: number | null, b: number | null) => {
    if (a === b || (a !== null && b !== null && Math.abs(a - b) < 1e-6)) return;
    out.push({ field, kind: b === null ? "recreate" : "direct" });
  };
  limit("cpus", cur.cpus, next.cpus);
  limit("memory", cur.memoryBytes, next.memoryBytes);
  limit("memoryReservation", cur.memoryReservationBytes, next.memoryReservationBytes);
  if (!sameKv(cur.env, next.env)) out.push({ field: "env", kind: "recreate" });
  if (!sameSet(cur.ports, next.ports, portKey)) out.push({ field: "ports", kind: "recreate" });
  if (!sameSet(cur.mounts, next.mounts, mountKey)) out.push({ field: "mounts", kind: "recreate" });
  return out;
}

/** Pestañas con cambios pendientes y si alguno de ellos exige recrear. */
export function pendingByTab(p: Pending[]): Partial<Record<EditTab, "direct" | "recreate">> {
  const out: Partial<Record<EditTab, "direct" | "recreate">> = {};
  for (const c of p) {
    const tab = FIELD_TAB[c.field];
    out[tab] = out[tab] === "recreate" || c.kind === "recreate" ? "recreate" : "direct";
  }
  return out;
}

/**
 * Correcciones propuestas por la auditoría (E3, D-095) aplicadas al borrador:
 * puertos solo en 127.0.0.1 y carpetas en solo lectura. Los límites de CPU y
 * memoria los elige el usuario: se abre la pestaña de recursos. Nada se aplica
 * a Docker hasta revisar y confirmar.
 */
export function applyAuditFixes(r: EditRequest, fixes: AuditFix[]): { request: EditRequest; tab: EditTab | null; applied: number; resources: boolean } {
  const request: EditRequest = structuredClone(r);
  let applied = 0;
  let tab: EditTab | null = null;
  let resources = false;
  for (const f of fixes) {
    if (f.kind === "portLocalhost") {
      for (const p of request.ports) {
        const sameHost = f.hostPort === "" || String(p.hostPort ?? "") === f.hostPort;
        if (p.containerPort === f.containerPort && p.protocol === f.protocol && sameHost && (p.hostIp === "" || p.hostIp === "0.0.0.0" || p.hostIp === "::")) {
          p.hostIp = "127.0.0.1";
          applied++;
          tab ??= "ports";
        }
      }
    } else if (f.kind === "mountReadOnly") {
      for (const m of request.mounts) {
        if (m.target === f.target && !m.readOnly) {
          m.readOnly = true;
          applied++;
          tab ??= "storage";
        }
      }
    } else {
      resources = true;
    }
  }
  // Se abre la pestaña de un cambio ya hecho; si solo faltan límites, Recursos.
  return { request, tab: tab ?? (resources ? "resources" : null), applied, resources };
}
