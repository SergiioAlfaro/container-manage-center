/**
 * Único punto de contacto entre la interfaz y el backend Rust.
 * Los componentes nunca llaman a `invoke` directamente; así las pruebas pueden
 * sustituir este módulo y los tipos se comparten con Rust (bindings/, ts-rs).
 */
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
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

export const ACTIVITY_EVENT = "activity://event";
export const ACTIVITY_RESYNC = "activity://resync";
export const ENGINE_STATE = "engine://state";

/** Normaliza cualquier error de `invoke` a `CommandError`. */
export function toCommandError(e: unknown): CommandError {
  if (e && typeof e === "object" && "code" in e && "message" in e) {
    return e as CommandError;
  }
  return { code: "unknown", message: e instanceof Error ? e.message : String(e), httpStatus: null };
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    throw { code: "ipc.unavailable", message: "La interfaz se está ejecutando fuera de la aplicación de escritorio.", httpStatus: null } satisfies CommandError;
  }
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw toCommandError(e);
  }
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
