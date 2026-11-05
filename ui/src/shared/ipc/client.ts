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
