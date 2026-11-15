// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import type { ComposeAction } from "@/shared/ipc/bindings/ComposeAction";
import type { ComposeProject } from "@/shared/ipc/bindings/ComposeProject";
import type { ProjectContainer } from "@/shared/ipc/bindings/ProjectContainer";
import type { ProjectModel } from "@/shared/ipc/bindings/ProjectModel";
import type { ProjectRef } from "@/shared/ipc/bindings/ProjectRef";
import type { ServiceModel } from "@/shared/ipc/bindings/ServiceModel";
import type { ServicePort } from "@/shared/ipc/bindings/ServicePort";

export type ProjectFilter = "all" | "running" | "stopped" | "notDeployed";

/** Identidad del proyecto sobre la que se ejecutan las acciones. */
export function refOf(p: Pick<ComposeProject, "name" | "files" | "workingDir" | "envFiles">): ProjectRef {
  return { name: p.name, files: p.files, workingDir: p.workingDir, envFiles: p.envFiles };
}

export function matchesProject(p: ComposeProject, filter: ProjectFilter, query: string): boolean {
  if (filter === "running" && p.state !== "running" && p.state !== "partial") return false;
  if (filter === "stopped" && p.state !== "stopped") return false;
  if (filter === "notDeployed" && p.state !== "notDeployed") return false;
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return [p.name, p.workingDir ?? "", ...p.files, ...p.services].some((x) => x.toLowerCase().includes(q));
}

/** Los archivos existen: se puede desplegar, descargar o construir. */
export function hasFiles(p: Pick<ComposeProject, "files" | "missingFiles">): boolean {
  return p.files.length > 0 && p.missingFiles.length === 0;
}

export type ServiceState = "running" | "partial" | "stopped" | "notCreated";

export interface ServiceRow {
  name: string;
  model: ServiceModel | null;
  containers: ProjectContainer[];
  running: number;
  state: ServiceState;
  /** Tiene contenedores pero ya no está en los archivos. */
  orphan: boolean;
}

/** Une los servicios del archivo con los contenedores reales de cada uno. */
export function serviceRows(model: ProjectModel | null, containers: ProjectContainer[]): ServiceRow[] {
  const byService = new Map<string, ProjectContainer[]>();
  for (const c of containers.filter((c) => !c.oneoff)) {
    byService.set(c.service, [...(byService.get(c.service) ?? []), c]);
  }
  const names = new Set<string>([...(model?.services.map((s) => s.name) ?? []), ...byService.keys()]);
  return [...names]
    .sort((a, b) => a.localeCompare(b))
    .map((name) => {
      const cs = byService.get(name) ?? [];
      const running = cs.filter((c) => c.state === "running").length;
      const state: ServiceState = cs.length === 0 ? "notCreated" : running === cs.length ? "running" : running === 0 ? "stopped" : "partial";
      const m = model?.services.find((s) => s.name === name) ?? null;
      return { name, model: m, containers: cs, running, state, orphan: model !== null && m === null };
    });
}

export function portLabel(p: ServicePort): string {
  const proto = p.protocol && p.protocol !== "tcp" ? `/${p.protocol}` : "";
  if (!p.published) return `${p.target}${proto}`;
  return `${p.hostIp ? `${p.hostIp}:` : ""}${p.published}→${p.target}${proto}`;
}

/** Volúmenes que `down --volumes` eliminaría (los declarados y no externos). */
export function removableVolumes(model: ProjectModel | null): string[] {
  return (model?.volumes ?? []).filter((v) => !v.external).map((v) => v.name);
}

export function upAction(over: Partial<Extract<ComposeAction, { action: "up" }>> = {}): ComposeAction {
  return { action: "up", services: [], build: false, pull: "missing", forceRecreate: false, removeOrphans: false, profiles: [], ...over };
}

export function downAction(over: Partial<Extract<ComposeAction, { action: "down" }>> = {}): ComposeAction {
  return { action: "down", volumes: false, images: "none", removeOrphans: false, ...over };
}

export function simpleAction(action: "start" | "stop" | "restart" | "pull", services: string[] = []): ComposeAction {
  return { action, services };
}

export function buildAction(services: string[] = [], noCache = false): ComposeAction {
  return { action: "build", services, noCache, pull: false };
}

/** Nombre que Compose derivaría de una carpeta (para validar un proyecto nuevo). */
export function nameFromDir(dir: string): string {
  const base = dir.replace(/\/+$/, "").split("/").pop() ?? "";
  return base
    .toLowerCase()
    .replace(/[^a-z0-9_-]/g, "")
    .replace(/^[-_]+/, "");
}

export function validProjectName(name: string): boolean {
  return /^[a-z0-9][a-z0-9_-]*$/.test(name) && name.length <= 255;
}

/** Contenido inicial de un proyecto nuevo. */
export function template(name: string): string {
  const head = name ? `name: ${name}\n\n` : "";
  return `${head}services:\n  web:\n    image: nginx:1.27\n    ports:\n      - "8080:80"\n    restart: unless-stopped\n`;
}

export function dirOf(path: string): string {
  const i = path.lastIndexOf("/");
  return i <= 0 ? "/" : path.slice(0, i);
}

export function baseName(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1);
}

/** Ruta mostrada relativa a la carpeta del proyecto (`./db`, `.`) si está dentro de ella. */
export function relativeTo(path: string, dir: string | null): string {
  if (!dir) return path;
  const d = dir.replace(/\/+$/, "");
  if (path === d) return ".";
  return path.startsWith(`${d}/`) ? `./${path.slice(d.length + 1)}` : path;
}
