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

export const ACTIVITY_EVENT = "activity://event";
export const ACTIVITY_RESYNC = "activity://resync";

/** Normaliza cualquier error de `invoke` a `CommandError`. */
export function toCommandError(e: unknown): CommandError {
  if (e && typeof e === "object" && "code" in e && "message" in e) {
    return e as CommandError;
  }
  return { code: "unknown", message: e instanceof Error ? e.message : String(e) };
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    throw { code: "ipc.unavailable", message: "La interfaz se está ejecutando fuera de la aplicación de escritorio." } satisfies CommandError;
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
