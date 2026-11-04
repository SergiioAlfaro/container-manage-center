/**
 * Doble del cliente IPC para pruebas de interfaz. Los datos son fixtures de
 * prueba: nunca se importan desde código de la aplicación.
 */
import { vi } from "vitest";
import type { ConnectionState } from "@/shared/ipc/bindings/ConnectionState";
import type { ContainerSummary } from "@/shared/ipc/bindings/ContainerSummary";
import type { Diagnosis } from "@/shared/ipc/bindings/Diagnosis";
import type { ImageSummary } from "@/shared/ipc/bindings/ImageSummary";
import type { SystemOverview } from "@/shared/ipc/bindings/SystemOverview";
import type { AppConfig } from "@/shared/ipc/bindings/AppConfig";

// Copia literal de DEFAULT_CONFIG: importar app/store crearía un ciclo con este doble.
const DEFAULT_CONFIG: AppConfig = {
  version: 1,
  ui: { theme: "system", language: "es", sidebarCollapsed: false },
  activity: { panelOpen: false, openOnError: true, maxRecords: 500 },
  docker: { mode: "auto", customSocket: "" },
};

export const connected: ConnectionState = {
  state: "connected",
  endpoint: { socketPath: "/var/run/docker.sock", source: { kind: "system" } },
  engine: { version: "29.9.2", apiVersion: "1.56", os: "Debian GNU/Linux 13 (trixie)", arch: "amd64", rootless: false },
  since: "2026-11-04T10:00:00Z",
};

export function diagnosis(problem: Diagnosis["problem"], over: Partial<Diagnosis> = {}): Diagnosis {
  return {
    problem,
    candidates: [
      { endpoint: { socketPath: "/run/user/1000/docker.sock", source: { kind: "rootless" } }, result: { result: "missing" } },
      { endpoint: { socketPath: "/var/run/docker.sock", source: { kind: "system" } }, result: { result: "permissionDenied" } },
    ],
    installation: { cliPath: "/usr/bin/docker", daemonPath: "/usr/bin/dockerd", rootlessTools: false },
    user: { name: "sergio", uid: 1000 },
    mode: { mode: "auto" },
    ...over,
  };
}

export const disconnected = (d: Diagnosis): ConnectionState => ({ state: "disconnected", diagnosis: d, since: "2026-11-04T10:00:00Z" });

export const overview: SystemOverview = {
  endpoint: connected.state === "connected" ? connected.endpoint : (null as never),
  version: {
    version: "29.9.2",
    apiVersion: "1.56",
    minApiVersion: "1.40",
    goVersion: "go1.26.10",
    gitCommit: "a82e528",
    buildTime: null,
    os: "linux",
    arch: "amd64",
    kernelVersion: "6.12.115",
    components: [],
  },
  info: {
    name: "portatil",
    serverVersion: "29.9.2",
    operatingSystem: "Debian GNU/Linux 13 (trixie)",
    osType: "linux",
    kernelVersion: "6.12.115+deb13.1-amd64",
    architecture: "x86_64",
    ncpu: 8,
    memTotal: 16_000_000_000,
    dockerRootDir: "/var/lib/docker",
    storageDriver: "overlayfs",
    loggingDriver: "json-file",
    cgroupDriver: "systemd",
    cgroupVersion: "2",
    containers: 3,
    containersRunning: 1,
    containersPaused: 1,
    containersStopped: 1,
    images: 2,
    securityOptions: [],
    warnings: ["WARNING: bridge-nf-call-iptables is disabled"],
    swarmActive: false,
    rootless: false,
  },
};

export function container(over: Partial<ContainerSummary> = {}): ContainerSummary {
  return {
    id: "a1b2c3d4e5f6a7b8c9d0",
    name: "web",
    image: "nginx:1.27",
    imageId: "sha256:1111",
    command: "nginx -g 'daemon off;'",
    created: Math.floor(Date.now() / 1000) - 3600,
    state: "running",
    status: "Up 1 hour",
    ports: [
      { ip: "0.0.0.0", privatePort: 80, publicPort: 8080, protocol: "tcp" },
      { ip: "::", privatePort: 80, publicPort: 8080, protocol: "tcp" },
    ],
    networks: ["bridge"],
    mounts: 0,
    composeProject: null,
    labels: {},
    ...over,
  };
}

export const containers: ContainerSummary[] = [
  container(),
  container({ id: "b2b2b2b2b2b2", name: "db", image: "postgres:17", state: "exited", status: "Exited (0) 2 hours ago", ports: [], imageId: "sha256:2222" }),
  container({ id: "c3c3c3c3c3c3", name: "cache", image: "redis:8", state: "paused", status: "Up 3 hours (Paused)", ports: [], composeProject: "blog" }),
];

export const images: ImageSummary[] = [
  { id: "sha256:1111aaaabbbbcccc", repoTags: ["nginx:1.27", "nginx:latest"], repoDigests: [], created: 1_760_000_000, size: 192_000_000, dangling: false, labels: {}, usedBy: [{ id: "a1b2c3d4e5f6a7b8c9d0", name: "web", state: "running" }] },
  { id: "sha256:9999ddddeeeeffff", repoTags: [], repoDigests: [], created: 1_750_000_000, size: 5_000_000, dangling: true, labels: {}, usedBy: [] },
];

export const api = {
  appInfo: vi.fn(async () => ({ name: "Docker Control Center", version: "0.2.0", configPath: "/home/u/.config/docker-control-center/config.toml", startupWarning: null })),
  configGet: vi.fn(async () => structuredClone(DEFAULT_CONFIG)),
  configSet: vi.fn(async (c: typeof DEFAULT_CONFIG) => c),
  activityList: vi.fn(async () => []),
  activityClear: vi.fn(async () => {}),
  onActivity: vi.fn(async (_h: unknown, _r: unknown) => () => {}),
  copyText: vi.fn(async (_t: string) => {}),
  engineState: vi.fn(async (): Promise<ConnectionState> => connected),
  engineReconnect: vi.fn(async (): Promise<ConnectionState> => connected),
  engineTest: vi.fn(async (_d: unknown) => ({ result: "ok" as const, endpoint: { socketPath: "/var/run/docker.sock", source: { kind: "custom" as const } }, engine: connected.state === "connected" ? connected.engine : (null as never) })),
  engineOverview: vi.fn(async () => overview),
  composeStatus: vi.fn(async (): Promise<import("@/shared/ipc/bindings/ComposeStatus").ComposeStatus> => ({ status: "available", version: "2.40.0", kind: "plugin", program: "/usr/bin/docker" })),
  containersList: vi.fn(async () => containers),
  containerAction: vi.fn(async (id: string, _n: string | null, a: string) => ({
    id,
    status: a === "stop" ? "exited" : a === "pause" ? "paused" : "running",
    exitCode: a === "stop" ? 0 : null,
  })),
  containerRemove: vi.fn(async (_id: string, _n: string | null, _f: boolean, _v: boolean) => {}),
  imagesList: vi.fn(async () => images),
  onEngineState: vi.fn(async (_h: (s: ConnectionState) => void) => () => {}),
};

export const module = { api, toCommandError: (e: unknown) => e };
