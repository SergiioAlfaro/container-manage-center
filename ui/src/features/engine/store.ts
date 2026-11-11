import { useServerId, useServers } from "@/app/server";
import type { ConnectionState } from "@/shared/ipc/bindings/ConnectionState";

/** Conexión del servidor del ámbito actual (0.10.0: uno por servidor). */
export interface EngineView {
  /** `null` hasta recibir el primer estado del backend. */
  state: ConnectionState | null;
  reconnecting: boolean;
  error: string | null;
  reconnect: () => Promise<void>;
}

/** Estado de la conexión con Docker del servidor de este ámbito. */
export function useEngine(): EngineView;
export function useEngine<T>(select: (v: EngineView) => T): T;
export function useEngine<T>(select?: (v: EngineView) => T): T | EngineView {
  const id = useServerId();
  const state = useServers((s) => s.states[id] ?? null);
  const reconnecting = useServers((s) => !!s.busy[id]);
  const error = useServers((s) => s.error);
  const connect = useServers((s) => s.connect);
  const view: EngineView = {
    state,
    reconnecting,
    error,
    reconnect: async () => {
      await connect(id);
    },
  };
  return select ? select(view) : view;
}

export const isConnected = (s: ConnectionState | null): s is Extract<ConnectionState, { state: "connected" }> =>
  s?.state === "connected";
