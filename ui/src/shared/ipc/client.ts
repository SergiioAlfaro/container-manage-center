/**
 * Único punto de contacto entre la interfaz y el backend Rust.
 * Los componentes nunca llaman a `invoke` directamente; así las pruebas pueden
 * sustituir este módulo y los tipos se comparten con Rust (bindings/, ts-rs).
 */
import { Channel, invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { ActivityEvent } from "./bindings/ActivityEvent";
import type { AppConfig } from "./bindings/AppConfig";
import type { AppInfo } from "./bindings/AppInfo";
import type { CommandError } from "./bindings/CommandError";
import type { OperationRecord } from "./bindings/OperationRecord";
import type { ComposeStatus } from "./bindings/ComposeStatus";
import type { ConnectionState } from "./bindings/ConnectionState";
import type { ConnectionTest } from "./bindings/ConnectionTest";
import type { ContainerAction } from "./bindings/ContainerAction";
import type { ContainerState } from "./bindings/ContainerState";
import type { ContainerSummary } from "./bindings/ContainerSummary";
import type { DockerConfig } from "./bindings/DockerConfig";
import type { ImageSummary } from "./bindings/ImageSummary";
import type { SystemOverview } from "./bindings/SystemOverview";
import type { ContainerDetails } from "./bindings/ContainerDetails";
import type { ContainerSpec } from "./bindings/ContainerSpec";
import type { CreateResult } from "./bindings/CreateResult";
import type { DuplicateSpec } from "./bindings/DuplicateSpec";
import type { ImageDefaults } from "./bindings/ImageDefaults";
import type { LogEvent } from "./bindings/LogEvent";
import type { NetworkBrief } from "./bindings/NetworkBrief";
import type { ShellChoice } from "./bindings/ShellChoice";
import type { SpecPreview } from "./bindings/SpecPreview";
import type { StatsEvent } from "./bindings/StatsEvent";
import type { TerminalEvent } from "./bindings/TerminalEvent";
import type { VolumeBrief } from "./bindings/VolumeBrief";
import type { BuildEvent } from "./bindings/BuildEvent";
import type { BuildPreview } from "./bindings/BuildPreview";
import type { BuildSpec } from "./bindings/BuildSpec";
import type { ImageDetails } from "./bindings/ImageDetails";
import type { ImageLayer } from "./bindings/ImageLayer";
import type { ConnectOptions } from "./bindings/ConnectOptions";
import type { CreatedNetwork } from "./bindings/CreatedNetwork";
import type { NetworkDetails } from "./bindings/NetworkDetails";
import type { NetworkPreview } from "./bindings/NetworkPreview";
import type { NetworkSpec } from "./bindings/NetworkSpec";
import type { NetworkSummary } from "./bindings/NetworkSummary";
import type { ImageRef } from "./bindings/ImageRef";
import type { PruneResult } from "./bindings/PruneResult";
import type { PullAuth } from "./bindings/PullAuth";
import type { PullEvent } from "./bindings/PullEvent";
import type { RemoveResult } from "./bindings/RemoveResult";
import type { SearchResult } from "./bindings/SearchResult";

export const ACTIVITY_EVENT = "activity://event";
export const ACTIVITY_RESYNC = "activity://resync";
export const ENGINE_STATE = "engine://state";

/** Normaliza cualquier error de `invoke` a `CommandError`. */
export function toCommandError(e: unknown): CommandError {
  if (e && typeof e === "object" && "code" in e && "message" in e) {
    return e as CommandError;
  }
  return { code: "unknown", message: e instanceof Error ? e.message : String(e), httpStatus: null, issues: null };
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    throw { code: "ipc.unavailable", message: "La interfaz se está ejecutando fuera de la aplicación de escritorio.", httpStatus: null, issues: null } satisfies CommandError;
  }
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw toCommandError(e);
  }
}

function channel<T>(handler: (e: T) => void): Channel<T> {
  const ch = new Channel<T>();
  ch.onmessage = handler;
  return ch;
}

export const api = {
  appInfo: () => call<AppInfo>("app_info"),
  configGet: () => call<AppConfig>("config_get"),
  configSet: (config: AppConfig) => call<AppConfig>("config_set", { config }),
  activityList: () => call<OperationRecord[]>("activity_list"),
  activityClear: () => call<void>("activity_clear"),

  engineState: () => call<ConnectionState>("engine_state"),
  engineReconnect: () => call<ConnectionState>("engine_reconnect"),
  engineTest: (docker: DockerConfig) => call<ConnectionTest>("engine_test", { docker }),
  engineOverview: () => call<SystemOverview>("engine_overview"),
  composeStatus: () => call<ComposeStatus>("compose_status"),
  containersList: () => call<ContainerSummary[]>("containers_list"),
  containerAction: (id: string, name: string | null, action: ContainerAction) => call<ContainerState>("container_action", { id, name, action }),
  containerRemove: (id: string, name: string | null, force: boolean, volumes: boolean) =>
    call<void>("container_remove", { id, name, force, volumes, confirmed: true }),
  imagesList: () => call<ImageSummary[]>("images_list"),

  /** Valida y traduce la especificación sin hablar con Docker. */
  containerPreview: (spec: ContainerSpec) => call<SpecPreview>("container_preview", { spec }),
  containerCreate: (spec: ContainerSpec, start: boolean) => call<CreateResult>("container_create", { spec, start }),
  containerInspect: (id: string, name: string | null) => call<ContainerDetails>("container_inspect", { id, name }),
  containerDuplicateSpec: (id: string, name: string | null) => call<DuplicateSpec>("container_duplicate_spec", { id, name }),
  imageDefaults: (image: string) => call<ImageDefaults>("image_defaults", { image }),
  networksBrief: () => call<NetworkBrief[]>("networks_brief"),
  volumesBrief: () => call<VolumeBrief[]>("volumes_brief"),

  /** Abre un flujo; devuelve su identificador para cerrarlo con `streamClose`. */
  logsOpen: (id: string, name: string | null, tail: number | null, onEvent: (e: LogEvent) => void) =>
    call<number>("logs_open", { id, name, tail, onEvent: channel(onEvent) }),
  statsOpen: (id: string, name: string | null, onEvent: (e: StatsEvent) => void) => call<number>("stats_open", { id, name, onEvent: channel(onEvent) }),
  terminalOpen: (id: string, name: string | null, shell: ShellChoice, cols: number, rows: number, onEvent: (e: TerminalEvent) => void) =>
    call<number>("terminal_open", { id, name, shell, cols, rows, onEvent: channel(onEvent) }),
  terminalInput: (stream: number, data: string) => call<void>("terminal_input", { stream, data }),
  terminalResize: (stream: number, cols: number, rows: number) => call<void>("terminal_resize", { stream, cols, rows }),
  streamClose: (stream: number) => call<boolean>("stream_close", { stream }),

  imageDetails: (reference: string) => call<ImageDetails>("image_details", { reference }),
  imageHistory: (reference: string) => call<ImageLayer[]>("image_history", { reference }),
  imageTag: (source: string, repo: string, tag: string) => call<string>("image_tag", { source, repo, tag }),
  imageRemove: (reference: string, force: boolean) => call<RemoveResult>("image_remove", { reference, force, confirmed: true }),
  imagesPrune: (all: boolean) => call<PruneResult>("images_prune", { all, confirmed: true }),
  imagesSearch: (term: string, limit: number, officialOnly: boolean) => call<SearchResult[]>("images_search", { term, limit, officialOnly }),
  /** Interpreta una referencia como Docker, sin hablar con Docker. */
  imageRefParse: (reference: string) => call<ImageRef>("image_ref_parse", { reference }),
  /** Descarga en flujo. La contraseña (modo manual) solo viaja en esta llamada. */
  imagePullOpen: (reference: string, auth: PullAuth, onEvent: (e: PullEvent) => void) => call<number>("image_pull_open", { reference, auth, onEvent: channel(onEvent) }),
  /** Valida y resume el contexto sin hablar con Docker. */
  buildPreview: (spec: BuildSpec) => call<BuildPreview>("build_preview", { spec }),
  buildOpen: (spec: BuildSpec, onEvent: (e: BuildEvent) => void) => call<number>("build_open", { spec, onEvent: channel(onEvent) }),

  networksList: () => call<NetworkSummary[]>("networks_list"),
  networkDetails: (id: string, name: string | null) => call<NetworkDetails>("network_details", { id, name }),
  /** Valida la red y muestra la petición exacta sin hablar con Docker. */
  networkPreview: (spec: NetworkSpec) => call<NetworkPreview>("network_preview", { spec }),
  networkCreate: (spec: NetworkSpec) => call<CreatedNetwork>("network_create", { spec }),
  networkRemove: (id: string, name: string | null) => call<void>("network_remove", { id, name, confirmed: true }),
  networksPrune: () => call<string[]>("networks_prune", { confirmed: true }),
  networkConnect: (network: string, networkName: string | null, container: string, containerName: string | null, options: ConnectOptions, subnets: string[]) =>
    call<void>("network_connect", { network, networkName, container, containerName, options, subnets }),
  networkDisconnect: (network: string, networkName: string | null, container: string, containerName: string | null, force: boolean) =>
    call<void>("network_disconnect", { network, networkName, container, containerName, force, confirmed: true }),

  /** Selector de archivo del sistema (Dockerfile). */
  pickFile: async (title: string, defaultPath?: string): Promise<string | null> => {
    if (!isTauri()) return null;
    const r = await openDialog({ directory: false, multiple: false, title, defaultPath });
    return typeof r === "string" ? r : null;
  },

  /** Selector de carpeta del sistema (para los bind mounts). */
  pickFolder: async (title: string): Promise<string | null> => {
    if (!isTauri()) return null;
    const r = await openDialog({ directory: true, multiple: false, title });
    return typeof r === "string" ? r : null;
  },

  onEngineState: async (handler: (s: ConnectionState) => void): Promise<UnlistenFn> => {
    if (!isTauri()) return () => {};
    return listen<ConnectionState>(ENGINE_STATE, (e) => handler(e.payload));
  },

  onActivity: async (handler: (e: ActivityEvent) => void, onResync: () => void): Promise<UnlistenFn> => {
    if (!isTauri()) return () => {};
    const un1 = await listen<ActivityEvent>(ACTIVITY_EVENT, (e) => handler(e.payload));
    const un2 = await listen(ACTIVITY_RESYNC, () => onResync());
    return () => {
      un1();
      un2();
    };
  },

  copyText: async (text: string): Promise<void> => {
    if (isTauri()) return writeText(text);
    return navigator.clipboard.writeText(text);
  },
};

export type Api = typeof api;
