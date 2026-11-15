// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/** Lógica de la sección Redes sin dependencias de la interfaz (probada aparte). */
import type { ConnectOptions } from "@/shared/ipc/bindings/ConnectOptions";
import type { ContainerSummary } from "@/shared/ipc/bindings/ContainerSummary";
import type { NetworkMember } from "@/shared/ipc/bindings/NetworkMember";
import type { NetworkSpec } from "@/shared/ipc/bindings/NetworkSpec";
import type { NetworkSummary } from "@/shared/ipc/bindings/NetworkSummary";
import { joinArgs } from "@/features/containers/shellwords";

export type NetworkFilter = "all" | "inUse" | "unused" | "custom";

/** Estados en los que el contenedor tiene la red activa (Docker no deja eliminarla). */
export function isActive(state: string): boolean {
  return state === "running" || state === "paused" || state === "restarting";
}

export function activeMembers(n: Pick<NetworkSummary, "containers">): NetworkMember[] {
  return n.containers.filter((m) => isActive(m.state));
}

export function stoppedMembers(n: Pick<NetworkSummary, "containers">): NetworkMember[] {
  return n.containers.filter((m) => !isActive(m.state));
}

export function matchesNetwork(n: NetworkSummary, filter: NetworkFilter, query: string): boolean {
  if (filter === "inUse" && n.containers.length === 0) return false;
  if (filter === "unused" && n.containers.length > 0) return false;
  if (filter === "custom" && n.predefined) return false;
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return [n.name, n.id, n.driver, ...n.subnets, ...n.containers.map((c) => c.name)].some((v) => v.toLowerCase().includes(q));
}

/**
 * Redes que eliminaría `docker network prune`: las creadas por el usuario sin
 * contenedores en marcha. Docker elimina también las que solo tienen
 * configuradas contenedores detenidos (comprobado con Docker 29 y 24, D-047).
 */
export function pruneCandidates(list: NetworkSummary[]): NetworkSummary[] {
  return list.filter((n) => !n.predefined && n.scope !== "swarm" && activeMembers(n).length === 0);
}

export function emptyNetworkSpec(): NetworkSpec {
  return {
    name: "",
    driver: "bridge",
    internal: false,
    attachable: false,
    enableIpv6: false,
    ipv4: { subnet: "", gateway: "", ipRange: "" },
    ipv6: { subnet: "", gateway: "", ipRange: "" },
    labels: [],
    options: [],
  };
}

export type ConnectBlock = "host" | "none" | "already" | null;

/**
 * Motivo por el que Docker rechazará conectar el contenedor a la red, si lo hay:
 * la red `host`/`none`, un contenedor en modo `host`/`none`, o ya conectado.
 */
export function connectBlock(n: Pick<NetworkSummary, "name" | "driver" | "predefined" | "containers">, c: Pick<ContainerSummary, "id" | "networks">): ConnectBlock {
  if (n.containers.some((m) => m.id === c.id)) return "already";
  if (n.predefined && n.driver === "host") return "host";
  if (n.predefined && n.driver === "null") return "none";
  if (c.networks.includes("host")) return "host";
  if (c.networks.includes("none")) return "none";
  return null;
}

/** Los alias de red solo se admiten en redes creadas por el usuario. */
export function aliasesAllowed(n: Pick<NetworkSummary, "predefined">): boolean {
  return !n.predefined;
}

export function splitAliases(text: string): string[] {
  return text
    .split(/[\s,]+/)
    .map((a) => a.trim())
    .filter(Boolean);
}

/** `docker network connect …` tal como lo registrará el backend. */
export function connectCommand(network: string, container: string, o: ConnectOptions): string {
  const a = ["docker", "network", "connect"];
  for (const al of [...new Set(o.aliases.map((x) => x.trim()).filter(Boolean))]) a.push("--alias", al);
  if (o.ipv4.trim()) a.push("--ip", o.ipv4.trim());
  if (o.ipv6.trim()) a.push("--ip6", o.ipv6.trim());
  a.push(network, container);
  return joinArgs(a);
}

export function disconnectCommand(network: string, container: string, force: boolean): string {
  return joinArgs(["docker", "network", "disconnect", ...(force ? ["--force"] : []), network, container]);
}
