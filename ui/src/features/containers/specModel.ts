// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/** Utilidades del asistente: especificación vacía, problemas por pestaña y secretos. */
import { isSecretEntry, isSecretName } from "@/shared/secrets";
import type { ContainerSpec } from "@/shared/ipc/bindings/ContainerSpec";
import type { SpecIssue } from "@/shared/ipc/bindings/SpecIssue";

export function emptySpec(): ContainerSpec {
  return {
    name: "",
    image: "",
    command: [],
    entrypoint: [],
    workingDir: "",
    user: "",
    hostname: "",
    env: [],
    labels: [],
    network: "",
    ports: [],
    publishAll: false,
    mounts: [],
    cpus: null,
    memoryBytes: null,
    memoryReservationBytes: null,
    restart: { name: "no", maxRetries: 0 },
    privileged: false,
    autoRemove: false,
    init: false,
    tty: false,
    interactive: false,
    readOnlyRootfs: false,
    capAdd: [],
    capDrop: [],
    extraHosts: [],
    acceptedRisks: [],
  };
}

export type WizardTab = "general" | "env" | "network" | "storage" | "resources" | "advanced";
export const WIZARD_TABS: WizardTab[] = ["general", "env", "network", "storage", "resources", "advanced"];

const FIELD_TAB: Array<[RegExp, WizardTab]> = [
  [/^(name|image|command|entrypoint|workingDir)$/, "general"],
  [/^(env|labels)(\.|$)/, "env"],
  [/^(network|ports|extraHosts|hostname)(\.|$)/, "network"],
  [/^mounts(\.|$)/, "storage"],
  [/^(cpus|memoryBytes|memoryReservationBytes)$/, "resources"],
];

/** Pestaña donde se edita el campo de un problema. */
export function tabOf(field: string): WizardTab {
  for (const [re, tab] of FIELD_TAB) if (re.test(field)) return tab;
  return "advanced";
}

/** Problemas de un campo concreto (`ports.0.hostPort`) o de todo un grupo (`ports`). */
export function issuesFor(issues: SpecIssue[], field: string, exact = true): SpecIssue[] {
  return issues.filter((i) => (exact ? i.field === field : i.field === field || i.field.startsWith(`${field}.`)));
}

/** Variables cuyo valor se oculta por defecto en la interfaz (no en Docker).
 *  Reglas comunes en `@/shared/secrets` (E1, D-085). */
export function isSecretKey(key: string): boolean {
  return isSecretName(key);
}
export { isSecretEntry };

/** Bytes → número + unidad para los campos de memoria. */
export type MemUnit = "m" | "g";
export function bytesFrom(value: string, unit: MemUnit): number | null {
  const v = value.trim().replace(",", ".");
  if (v === "") return null;
  const n = Number(v);
  if (!Number.isFinite(n) || n < 0) return NaN;
  return Math.round(n * (unit === "g" ? 1024 ** 3 : 1024 ** 2));
}
export function toMemField(bytes: number | null): { value: string; unit: MemUnit } {
  if (bytes === null) return { value: "", unit: "m" };
  if (bytes % 1024 ** 3 === 0) return { value: String(bytes / 1024 ** 3), unit: "g" };
  return { value: String(Math.round((bytes / 1024 ** 2) * 100) / 100), unit: "m" };
}
