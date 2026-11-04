import { create } from "zustand";
import { api } from "@/shared/ipc/client";
import type { ConnectionState } from "@/shared/ipc/bindings/ConnectionState";

interface EngineStore {
  /** `null` hasta recibir el primer estado del backend. */
  state: ConnectionState | null;
  reconnecting: boolean;
  error: string | null;
  set: (s: ConnectionState) => void;
  init: () => Promise<void>;
  reconnect: () => Promise<void>;
}

export const useEngine = create<EngineStore>((set, get) => ({
  state: null,
  reconnecting: false,
  error: null,
  set: (state) => set({ state }),
  init: async () => {
    try {
      const s = await api.engineState();
      // Un evento pudo llegar antes que la respuesta: no se pisa un estado más reciente.
      if (get().state === null) set({ state: s });
    } catch (e) {
      set({ error: (e as { message: string }).message });
    }
  },
  reconnect: async () => {
    set({ reconnecting: true, error: null });
    try {
      set({ state: await api.engineReconnect() });
    } catch (e) {
      set({ error: (e as { message: string }).message });
    } finally {
      set({ reconnecting: false });
    }
  },
}));

export const isConnected = (s: ConnectionState | null): s is Extract<ConnectionState, { state: "connected" }> =>
  s?.state === "connected";
