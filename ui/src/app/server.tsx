/**
 * Servidores en la interfaz (0.10.0).
 *
 * - `useServers`: lista de servidores, estado de cada conexión y servidor
 *   seleccionado en el área principal.
 * - `ServerScope`: todo lo que cuelga de él trabaja con UN servidor. Da un
 *   cliente con las órdenes de Docker ligadas a ese servidor (`useApi`) y su
 *   propia caché de consultas. Al cambiar de servidor se monta de nuevo
 *   (`key`), así que nada de un servidor puede quedar mostrado ni usarse en
 *   otro, y una operación ya iniciada sigue yendo a su servidor.
 */
import { createContext, useContext, useMemo, type ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { create } from "zustand";
import { api as appApi, dockerApi, LOCAL_SERVER, toCommandError, type DockerApi } from "@/shared/ipc/client";
import { createQueryClient } from "@/shared/query";
import type { ConnectionState } from "@/shared/ipc/bindings/ConnectionState";
import type { ServerView } from "@/shared/ipc/bindings/ServerView";
import type { ServerStateEvent } from "@/shared/ipc/bindings/ServerStateEvent";
import type { SecretKind } from "@/shared/ipc/bindings/SecretKind";

export { LOCAL_SERVER };

export type BoundApi = typeof appApi & DockerApi & { server: string };

const STORAGE_KEY = "dcc.selectedServer";

function readSelected(): string {
  try {
    return window.localStorage.getItem(STORAGE_KEY) ?? LOCAL_SERVER;
  } catch {
    return LOCAL_SERVER;
  }
}

function writeSelected(id: string) {
  try {
    window.localStorage.setItem(STORAGE_KEY, id);
  } catch {
    /* sin almacenamiento: solo se pierde la preferencia */
  }
}

/** Petición de secreto pendiente (frase de paso o contraseña). */
export interface SecretPrompt {
  server: string;
  kind: SecretKind;
  /** Error de un intento anterior (contraseña incorrecta…). */
  retry: boolean;
}

interface ServersStore {
  servers: ServerView[];
  /** Estado de cada servidor por identificador. */
  states: Record<string, ConnectionState>;
  selected: string;
  loaded: boolean;
  error: string | null;
  /** Servidores con una conexión pedida por el usuario en curso. */
  busy: Record<string, boolean>;
  /** Aviso del keyring de la última conexión (por servidor). */
  keyringNotice: Record<string, string | null>;
  secretPrompt: SecretPrompt | null;
  load: () => Promise<void>;
  select: (id: string) => void;
  applyState: (e: ServerStateEvent) => void;
  /** Conecta (o reconecta) un servidor; pide el secreto si hace falta. */
  connect: (id: string, secret?: string | null, remember?: boolean) => Promise<ConnectionState | null>;
  disconnect: (id: string) => Promise<void>;
  closeSecretPrompt: () => void;
}

export const useServers = create<ServersStore>((set, get) => ({
  servers: [],
  states: {},
  selected: readSelected(),
  loaded: false,
  error: null,
  busy: {},
  keyringNotice: {},
  secretPrompt: null,

  load: async () => {
    try {
      const servers = await appApi.serversList();
      const states = { ...get().states };
      for (const s of servers) if (!(s.id in states)) states[s.id] = s.state;
      // Los que ya no existen se olvidan.
      for (const id of Object.keys(states)) if (!servers.some((s) => s.id === id)) delete states[id];
      const selected = servers.some((s) => s.id === get().selected) ? get().selected : LOCAL_SERVER;
      if (selected !== get().selected) writeSelected(selected);
      set({ servers, states, selected, loaded: true, error: null });
    } catch (e) {
      set({ loaded: true, error: toCommandError(e).message });
    }
  },

  select: (id) => {
    if (!get().servers.some((s) => s.id === id) && id !== LOCAL_SERVER) return;
    writeSelected(id);
    set({ selected: id });
  },

  applyState: ({ server, state }) => set((s) => ({ states: { ...s.states, [server]: state } })),

  connect: async (id, secret = null, remember = false) => {
    set((s) => ({ busy: { ...s.busy, [id]: true }, secretPrompt: null }));
    try {
      if (id === LOCAL_SERVER) {
        const st = await dockerApi(LOCAL_SERVER).engineReconnect();
        get().applyState({ server: id, state: st });
        return st;
      }
      const out = await appApi.serverConnect(id, secret, remember);
      get().applyState({ server: id, state: out.state });
      set((s) => ({ keyringNotice: { ...s.keyringNotice, [id]: out.keyringNotice } }));
      const p = out.state.state === "disconnected" && out.state.diagnosis.problem.kind === "remote" ? out.state.diagnosis.problem.remote : null;
      if (p?.kind === "secretRequired") set({ secretPrompt: { server: id, kind: p.secret, retry: false } });
      else if (p?.kind === "authFailed" && secret !== null) {
        const kind = get().servers.find((x) => x.id === id)?.config?.auth === "password" ? "password" : "passphrase";
        set({ secretPrompt: { server: id, kind, retry: true } });
      }
      void get().load();
      return out.state;
    } catch (e) {
      set({ error: toCommandError(e).message });
      return null;
    } finally {
      set((s) => ({ busy: { ...s.busy, [id]: false } }));
    }
  },

  disconnect: async (id) => {
    await appApi.serverDisconnect(id);
    void get().load();
  },

  closeSecretPrompt: () => set({ secretPrompt: null }),
}));

/** Nombre a mostrar de un servidor (el local lo traduce la interfaz). */
export function serverName(s: ServerView | undefined, localName: string): string {
  if (!s) return "";
  return s.remote ? s.name : localName;
}

// --------------------------------------------------------------- caché por servidor

const clients = new Map<string, QueryClient>();

/** Caché de consultas propia de cada servidor. */
export function queryClientFor(server: string): QueryClient {
  let c = clients.get(server);
  if (!c) {
    c = createQueryClient();
    clients.set(server, c);
  }
  return c;
}

/** Para las pruebas: olvida las cachés. */
export function resetQueryClients() {
  for (const c of clients.values()) c.clear();
  clients.clear();
}

// --------------------------------------------------------------- ámbito de servidor

interface Scope {
  server: string;
  api: BoundApi;
}

const ScopeContext = createContext<Scope | null>(null);

export function bindApi(server: string): BoundApi {
  return { ...appApi, ...dockerApi(server), server };
}

/**
 * Ámbito de un servidor. `client`: caché a usar (por defecto, la del servidor).
 * Quien lo use con varios servidores debe ponerle `key={server}`.
 */
export function ServerScope({ server, client, children }: { server: string; client?: QueryClient; children: ReactNode }) {
  const scope = useMemo(() => ({ server, api: bindApi(server) }), [server]);
  const qc = client ?? queryClientFor(server);
  return (
    <ScopeContext.Provider value={scope}>
      <QueryClientProvider client={qc}>{children}</QueryClientProvider>
    </ScopeContext.Provider>
  );
}

function useScope(): Scope {
  const s = useContext(ScopeContext);
  if (!s) throw new Error("Este componente necesita un servidor (ServerScope).");
  return s;
}

/** Cliente con las órdenes de Docker del servidor de este ámbito. */
export function useApi(): BoundApi {
  return useScope().api;
}

/** Identificador del servidor de este ámbito. */
export function useServerId(): string {
  return useScope().server;
}

/** Servidor de este ámbito (datos del perfil y estado). */
export function useServerView(): ServerView | undefined {
  const id = useServerId();
  return useServers((s) => s.servers.find((x) => x.id === id));
}

/** Estado de la conexión del servidor de este ámbito. */
export function useServerState(): ConnectionState | null {
  const id = useServerId();
  return useServers((s) => s.states[id] ?? null);
}
