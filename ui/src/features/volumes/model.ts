// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/** Lógica de volúmenes y directorios vinculados sin dependencias de la interfaz. */
import { useApp } from "@/app/store";
import { emptySpec } from "@/features/containers/specModel";
import { useContainersNav } from "@/features/containers/store";
import type { MountSpec } from "@/shared/ipc/bindings/MountSpec";
import type { VolumeSpec } from "@/shared/ipc/bindings/VolumeSpec";
import type { VolumeSummary } from "@/shared/ipc/bindings/VolumeSummary";

export type VolumeFilter = "all" | "inUse" | "unused" | "anonymous";

export function matchesVolume(v: VolumeSummary, filter: VolumeFilter, query: string): boolean {
  if (filter === "inUse" && v.containers.length === 0) return false;
  if (filter === "unused" && v.containers.length > 0) return false;
  if (filter === "anonymous" && !v.anonymous) return false;
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return [v.name, v.driver, ...v.containers.map((c) => c.name), ...Object.keys(v.labels)].some((x) => x.toLowerCase().includes(q));
}

/**
 * Antes de la API 1.42 (Docker 23), `docker volume prune` elimina también los
 * volúmenes con nombre y no admite `--all` (D-051).
 */
export function legacyPrune(apiVersion: string | null | undefined): boolean {
  if (!apiVersion) return false;
  const [maj, min] = apiVersion.split(".").map((n) => Number.parseInt(n, 10));
  return (maj ?? 0) < 1 || ((maj ?? 0) === 1 && (min ?? 0) < 42);
}

/** Lo que eliminaría `docker volume prune [--all]`: volúmenes sin ningún contenedor (anónimos, o todos con --all). */
export function pruneCandidates(list: VolumeSummary[], all: boolean, apiVersion?: string | null): VolumeSummary[] {
  const everything = all || legacyPrune(apiVersion);
  return list.filter((v) => v.containers.length === 0 && v.scope !== "global" && (everything || v.anonymous));
}

export function emptyVolumeSpec(): VolumeSpec {
  return { name: "", driver: "local", driverOpts: [], labels: [] };
}

/** Abre el asistente de contenedores con un montaje ya puesto. */
export function createContainerWith(mount: MountSpec) {
  useContainersNav.getState().showCreate({ spec: { ...emptySpec(), mounts: [mount] }, notes: [], sourceName: null });
  useApp.getState().navigate("containers");
}

export function volumeMount(name: string): MountSpec {
  return { kind: "volume", source: name, target: "/data", readOnly: false };
}

/** Destino sugerido para una carpeta: /mnt/<nombre de la carpeta>. */
export function bindMount(path: string): MountSpec {
  const base = path.replace(/\/+$/, "").split("/").pop() || "carpeta";
  const safe = base.replace(/[^A-Za-z0-9_.-]/g, "_");
  return { kind: "bind", source: path, target: `/mnt/${safe}`, readOnly: false };
}

/** `0o755` → `rwxr-xr-x`. */
export function modeString(mode: number): string {
  const bits = ["r", "w", "x"];
  let s = "";
  for (let i = 8; i >= 0; i--) s += mode & (1 << i) ? bits[(8 - i) % 3] : "-";
  return s;
}
