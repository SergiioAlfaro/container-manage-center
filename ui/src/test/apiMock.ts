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
import type { ContainerDetails } from "@/shared/ipc/bindings/ContainerDetails";
import type { ContainerSpec } from "@/shared/ipc/bindings/ContainerSpec";
import type { CreateResult } from "@/shared/ipc/bindings/CreateResult";
import type { DuplicateSpec } from "@/shared/ipc/bindings/DuplicateSpec";
import type { ImageDefaults } from "@/shared/ipc/bindings/ImageDefaults";
import type { LogEvent } from "@/shared/ipc/bindings/LogEvent";
import type { NetworkBrief } from "@/shared/ipc/bindings/NetworkBrief";
import type { SpecPreview } from "@/shared/ipc/bindings/SpecPreview";
import type { StatsEvent } from "@/shared/ipc/bindings/StatsEvent";
import type { TerminalEvent } from "@/shared/ipc/bindings/TerminalEvent";
import type { VolumeBrief } from "@/shared/ipc/bindings/VolumeBrief";
import type { BuildEvent } from "@/shared/ipc/bindings/BuildEvent";
import type { BuildPreview } from "@/shared/ipc/bindings/BuildPreview";
import type { BuildSpec } from "@/shared/ipc/bindings/BuildSpec";
import type { ImageDetails } from "@/shared/ipc/bindings/ImageDetails";
import type { ImageLayer } from "@/shared/ipc/bindings/ImageLayer";
import type { ImageRef } from "@/shared/ipc/bindings/ImageRef";
import type { PruneResult } from "@/shared/ipc/bindings/PruneResult";
import type { PullAuth } from "@/shared/ipc/bindings/PullAuth";
import type { PullEvent } from "@/shared/ipc/bindings/PullEvent";
import type { RemoveResult } from "@/shared/ipc/bindings/RemoveResult";
import type { SearchResult } from "@/shared/ipc/bindings/SearchResult";
import type { NetworkDetails } from "@/shared/ipc/bindings/NetworkDetails";
import type { NetworkPreview } from "@/shared/ipc/bindings/NetworkPreview";
import type { NetworkSpec } from "@/shared/ipc/bindings/NetworkSpec";
import type { NetworkSummary } from "@/shared/ipc/bindings/NetworkSummary";
import type { ConnectOptions } from "@/shared/ipc/bindings/ConnectOptions";
import type { CreatedNetwork } from "@/shared/ipc/bindings/CreatedNetwork";
import type { BindSummary } from "@/shared/ipc/bindings/BindSummary";
import type { VolumeDetails } from "@/shared/ipc/bindings/VolumeDetails";
import type { VolumePreview } from "@/shared/ipc/bindings/VolumePreview";
import type { VolumePruneResult } from "@/shared/ipc/bindings/VolumePruneResult";
import type { VolumeSize } from "@/shared/ipc/bindings/VolumeSize";
import type { VolumeSpec } from "@/shared/ipc/bindings/VolumeSpec";
import type { VolumeSummary } from "@/shared/ipc/bindings/VolumeSummary";
import { emptySpec } from "@/features/containers/specModel";
import type { ComposeAction } from "@/shared/ipc/bindings/ComposeAction";
import type { ComposeConfig } from "@/shared/ipc/bindings/ComposeConfig";
import type { ComposeEvent } from "@/shared/ipc/bindings/ComposeEvent";
import type { ComposeFile } from "@/shared/ipc/bindings/ComposeFile";
import type { ComposeProject } from "@/shared/ipc/bindings/ComposeProject";
import type { ProjectModel } from "@/shared/ipc/bindings/ProjectModel";
import type { ProjectRef } from "@/shared/ipc/bindings/ProjectRef";
import type { RegisterResult } from "@/shared/ipc/bindings/RegisterResult";
import type { UnsavedFile } from "@/shared/ipc/bindings/UnsavedFile";

// Copia literal de DEFAULT_CONFIG: importar app/store crearía un ciclo con este doble.
const DEFAULT_CONFIG: AppConfig = {
  version: 1,
  ui: { theme: "system", language: "es", sidebarCollapsed: false },
  activity: { panelOpen: false, openOnError: true, maxRecords: 500 },
  docker: { mode: "auto", customSocket: "" },
  compose: { projects: [] },
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
    mountPoints: [],
    composeProject: null,
    labels: {},
    ...over,
  };
}

export const containers: ContainerSummary[] = [
  container({
    mounts: 2,
    mountPoints: [
      { kind: "volume", name: "datos", source: "/var/lib/docker/volumes/datos/_data", destination: "/var/lib/app", rw: true, mode: "z", propagation: "" },
      { kind: "bind", name: null, source: "/srv/web", destination: "/usr/share/nginx/html", rw: false, mode: "ro", propagation: "rprivate" },
    ],
  }),
  container({
    id: "b2b2b2b2b2b2",
    name: "db",
    image: "postgres:17",
    state: "exited",
    status: "Exited (0) 2 hours ago",
    ports: [],
    imageId: "sha256:2222",
    mounts: 1,
    mountPoints: [{ kind: "volume", name: "datos", source: "/var/lib/docker/volumes/datos/_data", destination: "/backup", rw: false, mode: "ro", propagation: "" }],
  }),
  container({ id: "c3c3c3c3c3c3", name: "cache", image: "redis:8", state: "paused", status: "Up 3 hours (Paused)", ports: [], composeProject: "blog" }),
];

export const images: ImageSummary[] = [
  { id: "sha256:1111aaaabbbbcccc", repoTags: ["nginx:1.27", "nginx:latest"], repoDigests: [], created: 1_760_000_000, size: 192_000_000, dangling: false, labels: {}, usedBy: [{ id: "a1b2c3d4e5f6a7b8c9d0", name: "web", state: "running" }] },
  { id: "sha256:9999ddddeeeeffff", repoTags: [], repoDigests: [], created: 1_750_000_000, size: 5_000_000, dangling: true, labels: {}, usedBy: [] },
];

/**
 * Flujos simulados: las pruebas empujan eventos con `streams.emit(id, evento)`
 * igual que lo haría el backend por el canal de Tauri.
 */
export const streams = {
  next: 0,
  handlers: new Map<number, (e: never) => void>(),
  endEvents: new Map<number, unknown>(),
  closed: [] as number[],
  /** `end` es el evento con el que el backend responde al cierre (por defecto, el de los flujos de la Fase 3). */
  add(h: (e: never) => void, end?: unknown): number {
    const id = ++this.next;
    this.handlers.set(id, h);
    if (end) this.endEvents.set(id, end);
    return id;
  },
  emit<E>(id: number, e: E) {
    (this.handlers.get(id) as ((e: E) => void) | undefined)?.(e);
  },
  /** Igual que el backend: cerrar provoca un evento `end` con el resumen. */
  close(id: number): boolean {
    const h = this.handlers.get(id) as ((e: unknown) => void) | undefined;
    if (!h) return false;
    this.handlers.delete(id);
    this.closed.push(id);
    h(this.endEvents.get(id) ?? { event: "end", closed: { end: { reason: "closed", summary: "Flujo cerrado por el usuario", exitCode: null }, error: null } });
    return true;
  },
  reset() {
    this.next = 0;
    this.handlers.clear();
    this.endEvents.clear();
    this.closed = [];
  },
};

/** Vista previa mínima: lo justo para las pruebas de interfaz; la real está en Rust. */
export function previewOf(spec: ContainerSpec): SpecPreview {
  const issues: SpecPreview["issues"] = [];
  if (!spec.image.trim()) issues.push({ severity: "error", field: "image", code: "image.required", params: {} });
  if (spec.name && !/^[a-zA-Z0-9][a-zA-Z0-9_.-]+$/.test(spec.name)) issues.push({ severity: "error", field: "name", code: "name.invalid", params: {} });
  spec.ports.forEach((p, i) => p.containerPort === 0 && issues.push({ severity: "error", field: `ports.${i}.containerPort`, code: "port.invalid", params: {} }));
  if (spec.privileged) issues.push({ severity: "warning", field: "privileged", code: "privileged", params: {} });
  const args = [
    ...(spec.name ? ["--name", spec.name] : []),
    ...spec.env.map((e) => `--env ${e.key}=${e.value}`),
    ...spec.ports.map((p) => `--publish ${p.hostPort ?? ""}:${p.containerPort}/${p.protocol}`),
    ...(spec.memoryBytes ? [`--memory ${spec.memoryBytes / 1024 ** 2}m`] : []),
    ...(spec.privileged ? ["--privileged"] : []),
  ];
  const tail = [spec.image, ...spec.command].join(" ");
  return { issues, runCommand: ["docker run --detach", ...args, tail].join(" "), createCommand: ["docker create", ...args, tail].join(" "), body: JSON.stringify({ Image: spec.image }), secretsHidden: 0 };
}

export function details(over: Partial<ContainerDetails> = {}): ContainerDetails {
  const base: ContainerDetails = {
    id: "a1b2c3d4e5f6a7b8c9d0",
    name: "web",
    image: "nginx:1.27",
    imageId: "sha256:1111aaaabbbbcccc",
    created: "2026-11-04T09:00:00.123Z",
    status: "running",
    running: true,
    paused: false,
    restarting: false,
    oomKilled: false,
    exitCode: 0,
    error: null,
    startedAt: "2026-11-04T09:00:01Z",
    finishedAt: "0001-01-01T00:00:00Z",
    health: null,
    restartCount: 0,
    pid: 4242,
    command: ["nginx", "-g", "daemon off;"],
    entrypoint: ["/docker-entrypoint.sh"],
    workingDir: "",
    user: "",
    hostname: "a1b2c3d4e5f6",
    tty: false,
    openStdin: false,
    env: [
      { key: "PATH", value: "/usr/bin" },
      { key: "DB_PASSWORD", value: "s3creto" },
    ],
    labels: { equipo: "web" },
    networkMode: "bridge",
    networks: [{ name: "bridge", ipAddress: "172.17.0.2", gateway: "172.17.0.1", macAddress: "02:42:ac:11:00:02", aliases: [] }],
    ports: [
      { ip: "0.0.0.0", privatePort: 80, publicPort: 8080, protocol: "tcp" },
      { ip: "::", privatePort: 80, publicPort: 8080, protocol: "tcp" },
      { ip: null, privatePort: 443, publicPort: null, protocol: "tcp" },
    ],
    mounts: [{ kind: "volume", name: "datos", source: "/var/lib/docker/volumes/datos/_data", destination: "/data", readOnly: false, driver: "local" }],
    restart: { name: "onFailure", maxRetries: 3 },
    cpus: 1.5,
    memoryBytes: 512 * 1024 ** 2,
    memoryReservationBytes: null,
    privileged: false,
    autoRemove: false,
    readOnlyRootfs: false,
    init: false,
    composeProject: null,
    raw: "",
    ...over,
  };
  return { ...base, raw: over.raw ?? JSON.stringify({ Id: base.id, Name: `/${base.name}` }, null, 2) };
}

/** Análisis simplificado para las pruebas de interfaz (el real, y probado, está en Rust). */
export function parseRef(input: string): ImageRef {
  const s = input.trim();
  const fail = (code: string) => {
    throw { code: `ref.${code}`, message: code, httpStatus: null, issues: null };
  };
  if (!s) fail("empty");
  const [rest, digest] = s.split("@") as [string, string | undefined];
  const slash = rest.lastIndexOf("/");
  const colon = rest.lastIndexOf(":");
  const hasTag = colon > slash;
  const name = hasTag ? rest.slice(0, colon) : rest;
  const tag = hasTag ? rest.slice(colon + 1) : null;
  if (tag !== null && !/^[\w][\w.-]{0,127}$/.test(tag)) fail("invalidTag");
  const first = name.split("/")[0]!;
  const isReg = name.includes("/") && (/[.:]/.test(first) || first === "localhost");
  const registry = isReg ? first : "docker.io";
  let repo = isReg ? name.slice(first.length + 1) : name;
  if (!/^[a-z0-9]+(?:(?:[._]|__|-+)[a-z0-9]+)*(?:\/[a-z0-9]+(?:(?:[._]|__|-+)[a-z0-9]+)*)*$/.test(repo)) fail("invalidRepository");
  if (registry === "docker.io" && !repo.includes("/")) repo = `library/${repo}`;
  return { registry, repository: repo, tag, digest: digest ?? null };
}

export function imageDetails(reference = "nginx:1.27", over: Partial<ImageDetails> = {}): ImageDetails {
  return {
    id: "sha256:1111aaaabbbbcccc",
    repoTags: ["nginx:1.27", "nginx:latest"],
    repoDigests: ["nginx@sha256:" + "a".repeat(64)],
    created: "2026-10-01T10:00:00Z",
    size: 192_000_000,
    architecture: "amd64",
    os: "linux",
    variant: null,
    author: "",
    comment: "",
    command: ["nginx", "-g", "daemon off;"],
    entrypoint: ["/docker-entrypoint.sh"],
    env: [
      { key: "PATH", value: "/usr/local/sbin:/usr/bin" },
      { key: "API_TOKEN", value: "t0ken" },
    ],
    exposedPorts: ["80/tcp"],
    volumes: [],
    workingDir: "",
    user: "",
    labels: { maintainer: "NGINX" },
    stopSignal: "SIGQUIT",
    healthcheck: null,
    layers: 7,
    raw: JSON.stringify({ Id: "sha256:1111aaaabbbbcccc", RepoTags: [reference] }, null, 2),
    ...over,
  };
}

export function buildPreviewOf(spec: BuildSpec): BuildPreview {
  const issues: BuildPreview["issues"] = [];
  if (!spec.context) issues.push({ severity: "error", field: "context", code: "context.required", params: {} });
  if (spec.tags.length === 0) issues.push({ severity: "warning", field: "tags", code: "tags.none", params: {} });
  spec.tags.forEach((tg, i) => !/^[a-z0-9./:_-]+$/.test(tg) && issues.push({ severity: "error", field: `tags.${i}`, code: "tag.invalid", params: { tag: tg } }));
  const args = ["docker build", ...spec.tags.map((x) => `--tag ${x}`), ...spec.buildArgs.map((a) => `--build-arg ${a.key}=${/TOKEN|PASSWORD/i.test(a.key) ? "********" : a.value}`), spec.context].join(" ");
  return {
    issues,
    command: spec.builder === "classic" ? `DOCKER_BUILDKIT=0 ${args}` : args,
    request: "POST /build?dockerfile=Dockerfile",
    secretsHidden: spec.buildArgs.filter((a) => /TOKEN|PASSWORD/i.test(a.key)).length,
    context: spec.context ? { files: 12, dirs: 3, symlinks: 0, bytes: 40_960, excluded: 4, skippedSpecial: 0, dockerignore: true, ignorePatterns: ["node_modules", "*.log"], invalidIgnoreLines: [], largest: [{ path: "src/app.js", size: 20_480 }], unreadable: [] } : null,
    contextError: null,
  };
}

export function network(over: Partial<NetworkSummary> = {}): NetworkSummary {
  return {
    id: "n1n1n1n1n1n1n1n1",
    name: "backend",
    driver: "bridge",
    scope: "local",
    created: "2026-11-01T10:00:00Z",
    internal: false,
    attachable: false,
    ingress: false,
    enableIpv4: true,
    enableIpv6: false,
    subnets: ["172.30.0.0/24"],
    gateways: ["172.30.0.1"],
    labels: { "com.example.app": "tienda" },
    predefined: false,
    composeProject: null,
    containers: [
      { id: "a1b2c3d4e5f6a7b8c9d0", name: "web", state: "running", ipv4: null, ipv6: null, mac: null },
      { id: "b2b2b2b2b2b2", name: "db", state: "exited", ipv4: null, ipv6: null, mac: null },
    ],
    ...over,
  };
}

export const networks: NetworkSummary[] = [
  network({ id: "b0b0b0b0b0b0b0b0", name: "bridge", predefined: true, subnets: ["172.17.0.0/16"], gateways: ["172.17.0.1"], labels: {}, containers: [{ id: "c3c3c3c3c3c3", name: "cache", state: "paused", ipv4: null, ipv6: null, mac: null }] }),
  network({ id: "h0h0h0h0h0h0h0h0", name: "host", driver: "host", predefined: true, subnets: [], gateways: [], labels: {}, containers: [] }),
  network({ id: "e0e0e0e0e0e0e0e0", name: "none", driver: "null", predefined: true, subnets: [], gateways: [], labels: {}, containers: [] }),
  network(),
  network({ id: "v1v1v1v1v1v1v1v1", name: "vieja", subnets: ["172.31.0.0/24"], gateways: [], labels: {}, containers: [{ id: "b2b2b2b2b2b2", name: "db", state: "exited", ipv4: null, ipv6: null, mac: null }] }),
  network({ id: "s1s1s1s1s1s1s1s1", name: "aislada", internal: true, subnets: ["172.32.0.0/24"], gateways: [], labels: {}, containers: [], composeProject: "blog" }),
];

export function networkDetails(id: string): NetworkDetails {
  const n = networks.find((x) => x.id === id) ?? network({ id });
  return {
    summary: {
      ...n,
      containers: n.containers.map((m) => (m.state === "running" ? { ...m, ipv4: "172.30.0.2/24", mac: "02:42:ac:1e:00:02" } : m)),
    },
    ipamDriver: "default",
    pools: n.subnets.map((s, i) => ({ subnet: s, gateway: n.gateways[i] ?? "", ipRange: "", auxiliary: {} })),
    options: { "com.docker.network.driver.mtu": "1500" },
    raw: JSON.stringify({ Name: n.name, Id: n.id }, null, 2),
  };
}

/** Imita lo esencial de la validación del backend (las reglas reales se prueban en Rust). */
export function networkPreviewOf(spec: NetworkSpec): NetworkPreview {
  const issues: NetworkPreview["issues"] = [];
  if (!spec.name.trim()) issues.push({ severity: "error", field: "name", code: "name.required", params: {} });
  if (spec.ipv4.subnet === "10.0.0.5/24") issues.push({ severity: "error", field: "ipv4.subnet", code: "subnet.hostBits", params: { value: "10.0.0.5/24", fixed: "10.0.0.0/24" } });
  if (spec.driver === "overlay") issues.push({ severity: "warning", field: "driver", code: "driver.swarm", params: {} });
  const parts = ["docker", "network", "create", "--driver", spec.driver];
  if (spec.ipv4.subnet) parts.push("--subnet", spec.ipv4.subnet);
  if (spec.internal) parts.push("--internal");
  parts.push(spec.name.trim());
  return { issues, command: parts.join(" "), request: "POST /networks/create", body: JSON.stringify({ Name: spec.name.trim(), Driver: spec.driver }, null, 2) };
}

export function volume(over: Partial<VolumeSummary> = {}): VolumeSummary {
  return {
    name: "datos",
    driver: "local",
    scope: "local",
    mountpoint: "/var/lib/docker/volumes/datos/_data",
    created: "2026-11-01T10:00:00Z",
    labels: { "com.example.app": "tienda" },
    options: {},
    anonymous: false,
    composeProject: null,
    containers: [
      { id: "b2b2b2b2b2b2", name: "db", state: "exited", destination: "/backup", rw: false },
      { id: "a1b2c3d4e5f6a7b8c9d0", name: "web", state: "running", destination: "/var/lib/app", rw: true },
    ],
    ...over,
  };
}

const ANON = "3f1c9e2b7a6d5c4b3a291807f6e5d4c3b2a19087f6e5d4c3b2a1908f7e6d5c4b";

export const volumes: VolumeSummary[] = [
  volume(),
  volume({ name: "copias", labels: {}, containers: [{ id: "b2b2b2b2b2b2", name: "db", state: "exited", destination: "/dump", rw: true }] }),
  volume({ name: "huerfano", labels: {}, containers: [], composeProject: "blog" }),
  volume({ name: ANON, labels: { "com.docker.volume.anonymous": "" }, anonymous: true, containers: [] }),
];

export const volumeSizes: VolumeSize[] = [
  { name: "datos", size: 52_428_800, refCount: 2 },
  { name: "copias", size: 1_048_576, refCount: 1 },
  { name: "huerfano", size: 0, refCount: 0 },
  { name: ANON, size: 4096, refCount: 0 },
];

export function volumeDetails(name: string): VolumeDetails {
  const v = volumes.find((x) => x.name === name) ?? volume({ name });
  return { summary: v, status: {}, raw: JSON.stringify({ Name: v.name, Driver: v.driver, Mountpoint: v.mountpoint }, null, 2) };
}

/** Imita lo esencial de la validación del backend (las reglas reales se prueban en Rust). */
export function volumePreviewOf(spec: VolumeSpec): VolumePreview {
  const issues: VolumePreview["issues"] = [];
  const n = spec.name.trim();
  if (n.length === 1) issues.push({ severity: "error", field: "name", code: "volumeName.short", params: { value: n } });
  else if (n && !/^[A-Za-z0-9][A-Za-z0-9_.-]+$/.test(n)) issues.push({ severity: "error", field: "name", code: "volumeName.invalid", params: { value: n } });
  const parts = ["docker", "volume", "create", "--driver", spec.driver];
  for (const o of spec.driverOpts.filter((x) => x.key)) parts.push("--opt", `${o.key}=${o.value}`);
  if (n) parts.push(n);
  return { issues, command: parts.join(" "), request: "POST /volumes/create", body: JSON.stringify({ Name: n || undefined, Driver: spec.driver }, null, 2) };
}

export const binds: BindSummary[] = [
  {
    source: "/srv/web",
    host: { kind: "directory", uid: 1000, mode: 0o755, readable: true },
    risk: null,
    writable: true,
    containers: [
      { id: "b2b2b2b2b2b2", name: "db", state: "exited", destination: "/work", rw: true, propagation: "rprivate" },
      { id: "a1b2c3d4e5f6a7b8c9d0", name: "web", state: "running", destination: "/usr/share/nginx/html", rw: false, propagation: "rprivate" },
    ],
  },
  {
    source: "/var/run/docker.sock",
    host: { kind: "socket", uid: 0, mode: 0o660, readable: false },
    risk: "dockerSocket",
    writable: true,
    containers: [{ id: "c3c3c3c3c3c3", name: "cache", state: "paused", destination: "/var/run/docker.sock", rw: true, propagation: "rprivate" }],
  },
  {
    source: "/home/u/borrado",
    host: { kind: "missing", uid: null, mode: null, readable: false },
    risk: null,
    writable: false,
    containers: [{ id: "b2b2b2b2b2b2", name: "db", state: "exited", destination: "/import", rw: false, propagation: "rprivate" }],
  },
];

// ------------------------------------------------------------------ Fase 7

export function project(over: Partial<ComposeProject> = {}): ComposeProject {
  return {
    name: "tienda",
    files: ["/home/u/tienda/compose.yaml"],
    workingDir: "/home/u/tienda",
    envFiles: [],
    registered: true,
    missingFiles: [],
    containers: [
      { id: "c0ffee01", name: "tienda-db-1", service: "db", number: 1, state: "running", status: "Up 2 hours", image: "tienda-db", oneoff: false },
      { id: "c0ffee02", name: "tienda-web-1", service: "web", number: 1, state: "running", status: "Up 2 hours", image: "nginx:1.27", oneoff: false },
    ],
    services: ["db", "web"],
    state: "running",
    running: 2,
    total: 2,
    createdBy: "2.40.0",
    ...over,
  };
}

export const composeProjects: ComposeProject[] = [
  project(),
  project({
    name: "blog",
    files: ["/srv/blog/compose.yaml"],
    workingDir: "/srv/blog",
    registered: false,
    missingFiles: ["/srv/blog/compose.yaml"],
    containers: [{ id: "b10g0001", name: "blog-app-1", service: "app", number: 1, state: "exited", status: "Exited (0) 1 day ago", image: "ghost:5", oneoff: false }],
    services: ["app"],
    state: "stopped",
    running: 0,
    total: 1,
  }),
  project({ name: "nuevo", files: ["/home/u/nuevo/compose.yaml"], workingDir: null, containers: [], services: [], state: "notDeployed", running: 0, total: 0, createdBy: null }),
];

export const tiendaModel: ProjectModel = {
  name: "tienda",
  services: [
    { name: "db", image: null, build: "/home/u/tienda/db", ports: [], mounts: [{ kind: "volume", source: "datos", target: "/var/lib/db", readOnly: false }], dependsOn: [], profiles: [], envKeys: ["DB_PASSWORD"], replicas: null, containerName: null },
    { name: "extra", image: "busybox", build: null, ports: [], mounts: [], dependsOn: [], profiles: ["debug"], envKeys: [], replicas: null, containerName: null },
    { name: "web", image: "nginx:1.27", build: null, ports: [{ published: "8080", target: "80", protocol: "tcp", hostIp: null }], mounts: [], dependsOn: ["db"], profiles: [], envKeys: ["MODE"], replicas: null, containerName: null },
  ],
  networks: [{ key: "default", name: "tienda_default", external: false }],
  volumes: [
    { key: "datos", name: "tienda_datos", external: false },
    { key: "compartido", name: "compartido", external: true },
  ],
  profiles: ["debug"],
};

export const TIENDA_YAML = "services:\n  web:\n    image: nginx:1.27\n";

/** Comando mostrado (la versión real la calcula Rust). */
export function composeCommand(p: ProjectRef, a: ComposeAction): string {
  const extra: string[] = [];
  if (a.action === "up") {
    extra.push("--detach");
    if (a.build) extra.push("--build");
    if (a.pull !== "missing") extra.push("--pull", a.pull);
    if (a.forceRecreate) extra.push("--force-recreate");
    if (a.removeOrphans) extra.push("--remove-orphans");
  }
  if (a.action === "down") {
    if (a.volumes) extra.push("--volumes");
    if (a.images !== "none") extra.push("--rmi", a.images);
    if (a.removeOrphans) extra.push("--remove-orphans");
  }
  const svcs = "services" in a && a.services.length ? ["--", ...a.services] : [];
  const profiles = a.action === "up" ? a.profiles.flatMap((x) => ["--profile", x]) : ["down", "stop", "start", "restart"].includes(a.action) ? ["--profile", "'*'"] : [];
  return ["/usr/bin/docker", "compose", "--ansi", "never", "-p", p.name, ...p.files.flatMap((f) => ["-f", f]), ...profiles, a.action, ...extra, ...svcs].join(" ");
}

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
  containerAction: vi.fn(async (id: string, n: string | null, a: string) => ({
    id,
    name: n ?? id,
    status: a === "stop" ? "exited" : a === "pause" ? "paused" : "running",
    exitCode: a === "stop" ? 0 : null,
  })),
  containerRemove: vi.fn(async (_id: string, _n: string | null, _f: boolean, _v: boolean) => {}),
  imagesList: vi.fn(async () => images),
  containerPreview: vi.fn(async (spec: ContainerSpec): Promise<SpecPreview> => previewOf(spec)),
  containerCreate: vi.fn(async (spec: ContainerSpec, start: boolean): Promise<CreateResult> => ({
    id: "f00dfeedf00dfeed",
    name: spec.name || "nombre_generado",
    warnings: [],
    state: start ? { id: "f00dfeedf00dfeed", name: spec.name, status: "running", exitCode: null } : null,
    startError: null,
  })),
  containerInspect: vi.fn(async (id: string, _n: string | null): Promise<ContainerDetails> => details({ id })),
  containerDuplicateSpec: vi.fn(async (_id: string, n: string | null): Promise<DuplicateSpec> => ({
    spec: { ...emptySpec(), image: "nginx:1.27", env: [{ key: "MODO", value: "produccion" }], ports: [{ hostIp: "", hostPort: 8080, containerPort: 80, protocol: "tcp" }] },
    notes: ["composeLabels"],
    sourceName: n ?? "web",
  })),
  imageDefaults: vi.fn(async (_r: string): Promise<ImageDefaults> => ({
    id: "sha256:1111aaaabbbbcccc",
    command: ["nginx", "-g", "daemon off;"],
    entrypoint: ["/docker-entrypoint.sh"],
    env: ["PATH=/usr/local/sbin:/usr/bin", "NGINX_VERSION=1.27.5"],
    exposedPorts: ["80/tcp"],
    workingDir: "",
    user: "",
    volumes: [],
  })),
  networksBrief: vi.fn(async (): Promise<NetworkBrief[]> => [
    { name: "bridge", driver: "bridge", scope: "local" },
    { name: "host", driver: "host", scope: "local" },
    { name: "backend", driver: "bridge", scope: "local" },
  ]),
  volumesBrief: vi.fn(async (): Promise<VolumeBrief[]> => [{ name: "datos", driver: "local" }]),
  logsOpen: vi.fn(async (_id: string, _n: string | null, _tail: number | null, h: (e: LogEvent) => void) => streams.add(h)),
  statsOpen: vi.fn(async (_id: string, _n: string | null, h: (e: StatsEvent) => void) => streams.add(h)),
  terminalOpen: vi.fn(async (_id: string, _n: string | null, _s: string, _c: number, _r: number, h: (e: TerminalEvent) => void) => streams.add(h)),
  terminalInput: vi.fn(async (_s: number, _d: string) => {}),
  terminalResize: vi.fn(async (_s: number, _c: number, _r: number) => {}),
  streamClose: vi.fn(async (s: number) => streams.close(s)),
  pickFolder: vi.fn(async (_t: string): Promise<string | null> => "/home/u/datos"),
  pickFile: vi.fn(async (_t: string, _d?: string): Promise<string | null> => "/home/u/proyecto/docker/Dockerfile.prod"),
  imageDetails: vi.fn(async (reference: string): Promise<ImageDetails> => imageDetails(reference)),
  imageHistory: vi.fn(async (_r: string): Promise<ImageLayer[]> => [
    { id: "sha256:1111aaaabbbbcccc", created: 1_760_000_000, createdBy: "/bin/sh -c #(nop)  CMD [\"nginx\" \"-g\" \"daemon off;\"]", size: 0, comment: "", tags: ["nginx:1.27"], empty: true },
    { id: null, created: 1_759_999_000, createdBy: "/bin/sh -c apt-get update && apt-get install -y nginx", size: 60_000_000, comment: "", tags: [], empty: false },
    { id: null, created: 1_759_000_000, createdBy: "/bin/sh -c #(nop) ADD file:abc in / ", size: 74_000_000, comment: "", tags: [], empty: false },
  ]),
  imageTag: vi.fn(async (_s: string, repo: string, tag: string) => `${repo}:${tag}`),
  imageRemove: vi.fn(async (reference: string, _f: boolean): Promise<RemoveResult> => ({ untagged: [reference], deleted: [] })),
  imagesPrune: vi.fn(async (_all: boolean): Promise<PruneResult> => ({ untagged: [], deleted: ["sha256:9999ddddeeeeffff"], spaceReclaimed: 5_000_000 })),
  imagesSearch: vi.fn(async (term: string, _l: number, _o: boolean): Promise<SearchResult[]> => [
    { name: term, description: "Official build of " + term, stars: 21000, official: true },
    { name: `bitnami/${term}`, description: "", stars: 150, official: false },
  ]),
  imageRefParse: vi.fn(async (reference: string): Promise<ImageRef> => parseRef(reference)),
  imagePullOpen: vi.fn(async (_r: string, _a: PullAuth, h: (e: PullEvent) => void) => streams.add(h)),
  buildPreview: vi.fn(async (spec: BuildSpec): Promise<BuildPreview> => buildPreviewOf(spec)),
  buildOpen: vi.fn(async (s: BuildSpec, h: (e: BuildEvent) => void) =>
    streams.add(h, { event: "end", closed: { result: { end: { reason: "closed", summary: "Construcción cancelada por el usuario (2 pasos iniciados)", exitCode: null }, imageId: null, tags: s.tags }, error: null } }),
  ),
  networksList: vi.fn(async (): Promise<NetworkSummary[]> => networks),
  networkDetails: vi.fn(async (id: string, _n: string | null): Promise<NetworkDetails> => networkDetails(id)),
  networkPreview: vi.fn(async (spec: NetworkSpec): Promise<NetworkPreview> => networkPreviewOf(spec)),
  networkCreate: vi.fn(async (_spec: NetworkSpec): Promise<CreatedNetwork> => ({ id: "f00df00df00df00d", warning: null })),
  networkRemove: vi.fn(async (_id: string, _n: string | null) => {}),
  networksPrune: vi.fn(async (): Promise<string[]> => ["vieja", "aislada"]),
  networkConnect: vi.fn(async (_n: string, _nn: string | null, _c: string, _cn: string | null, _o: ConnectOptions, _s: string[]) => {}),
  networkDisconnect: vi.fn(async (_n: string, _nn: string | null, _c: string, _cn: string | null, _f: boolean) => {}),
  volumesList: vi.fn(async (): Promise<VolumeSummary[]> => volumes),
  volumeDetails: vi.fn(async (name: string): Promise<VolumeDetails> => volumeDetails(name)),
  volumeSizes: vi.fn(async (): Promise<VolumeSize[]> => volumeSizes),
  volumePreview: vi.fn(async (spec: VolumeSpec): Promise<VolumePreview> => volumePreviewOf(spec)),
  volumeCreate: vi.fn(async (spec: VolumeSpec): Promise<VolumeSummary> => volume({ name: spec.name || ANON, anonymous: !spec.name, containers: [] })),
  volumeRemove: vi.fn(async (_name: string) => {}),
  volumesPrune: vi.fn(async (all: boolean): Promise<VolumePruneResult> => ({ deleted: all ? [ANON, "huerfano"] : [ANON], spaceReclaimed: 4096 })),
  bindsList: vi.fn(async (): Promise<BindSummary[]> => binds),
  composeProjects: vi.fn(async (): Promise<ComposeProject[]> => composeProjects),
  composeConfig: vi.fn(async (p: ProjectRef, unsaved: UnsavedFile | null = null): Promise<ComposeConfig> => {
    if (unsaved?.content.includes("ports: 12")) {
      return { model: null, resolved: null, error: "validating -: services.web.ports must be a array", warnings: [], command: `docker compose -p ${p.name} -f - config` };
    }
    return { model: p.name === "tienda" ? tiendaModel : { ...tiendaModel, name: p.name, services: [], volumes: [], profiles: [] }, resolved: '{"name": "tienda", "services": {"db": {"environment": {"DB_PASSWORD": "s3creto"}}}}', error: null, warnings: [], command: `/usr/bin/docker compose -p ${p.name} --profile '*' config --format json` };
  }),
  composePreview: vi.fn(async (p: ProjectRef, a: ComposeAction) => ({ command: composeCommand(p, a) })),
  composeRunOpen: vi.fn(async (_p: ProjectRef, _a: ComposeAction, _c: boolean, h: (e: ComposeEvent) => void) =>
    streams.add(h, { event: "end", closed: { end: { end: "stopped" }, error: null, stderrTail: "" } }),
  ),
  composeLogsOpen: vi.fn(async (_p: ProjectRef, _s: string[], _t: number, _ts: boolean, h: (e: ComposeEvent) => void) =>
    streams.add(h, { event: "end", closed: { end: { end: "stopped" }, error: null, stderrTail: "" } }),
  ),
  composeRegister: vi.fn(async (files: string[], name: string | null): Promise<RegisterResult> => ({
    project: { name: name ?? "proyecto", files, workingDir: null, envFiles: [] },
    error: null,
    command: "docker compose config --format json",
  })),
  composeUnregister: vi.fn(async (_n: string) => true),
  composeFileRead: vi.fn(async (path: string): Promise<ComposeFile> => ({ path, content: TIENDA_YAML, version: "v1", writable: true })),
  composeFileSave: vi.fn(async (path: string, content: string, _v: string): Promise<ComposeFile> => ({ path, content, version: "v2", writable: true })),
  composeFileCreate: vi.fn(async (dir: string, content: string): Promise<ComposeFile> => ({ path: `${dir}/compose.yaml`, content, version: "v1", writable: true })),
  composeFindFile: vi.fn(async (_d: string): Promise<string | null> => null),
  pickComposeFiles: vi.fn(async (_t: string, _f: string): Promise<string[]> => ["/home/u/tienda/compose.yaml"]),
  onEngineState: vi.fn(async (_h: (s: ConnectionState) => void) => () => {}),
};

export const module = { api, toCommandError: (e: unknown) => e };
