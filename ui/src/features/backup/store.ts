import { create } from "zustand";
import { api, dockerApi } from "@/shared/ipc/client";
import type { BackupEntry } from "@/shared/ipc/bindings/BackupEntry";
import type { BackupProgress } from "@/shared/ipc/bindings/BackupProgress";
import type { BackupRequest } from "@/shared/ipc/bindings/BackupRequest";
import type { CommandError } from "@/shared/ipc/bindings/CommandError";

export type RunStatus = "idle" | "running" | "done" | "failed" | "cancelled";

export interface BackupRunState {
  status: RunStatus;
  stream: number | null;
  progress: BackupProgress | null;
  entry: BackupEntry | null;
  error: CommandError | null;
  cancelRequested: boolean;
}

const empty: BackupRunState = { status: "idle", stream: null, progress: null, entry: null, error: null, cancelRequested: false };

interface Backups {
  runs: Record<string, BackupRunState>;
  /** `job`: id de la tarea (usa su petición y su destino). */
  start: (server: string, request: BackupRequest, job: string | null, onFinished?: (ok: boolean) => void) => Promise<void>;
  cancel: (server: string) => void;
  dismiss: (server: string) => void;
}

/** Copia en curso por servidor: sigue al cambiar de apartado. */
export const useBackups = create<Backups>((set, get) => {
  const cur = (server: string) => get().runs[server] ?? empty;
  const put = (server: string, patch: Partial<BackupRunState>) => set((s) => ({ runs: { ...s.runs, [server]: { ...(s.runs[server] ?? empty), ...patch } } }));
  return {
    runs: {},
    start: async (server, request, job, onFinished) => {
      if (cur(server).status === "running") return;
      put(server, { ...empty, status: "running" });
      try {
        const stream = await dockerApi(server).backupRunOpen(request, job, (e) => {
          if (e.event === "progress") {
            put(server, { progress: e.progress });
          } else if (e.event === "backupEnd") {
            const cancelled = cur(server).cancelRequested;
            if (e.entry) put(server, { status: "done", stream: null, entry: e.entry, error: null, progress: null });
            else put(server, { status: cancelled ? "cancelled" : "failed", stream: null, error: cancelled ? null : e.error, progress: null });
            onFinished?.(!!e.entry);
          }
        });
        if (cur(server).status === "running") {
          put(server, { stream });
          if (cur(server).cancelRequested) void api.streamClose(stream);
        }
      } catch (e) {
        put(server, { status: "failed", error: e as CommandError });
        onFinished?.(false);
      }
    },
    cancel: (server) => {
      put(server, { cancelRequested: true });
      const id = cur(server).stream;
      if (id !== null) void api.streamClose(id);
    },
    dismiss: (server) => {
      if (cur(server).status !== "running") put(server, { ...empty });
    },
  };
});

/** E6 (1.6.0): copia preparada desde la auditoría para un contenedor (por
 *  id). El formulario la convierte en una selección que el usuario revisa;
 *  nunca se copia nada sin pulsar «Copiar». */
export interface BackupPrefill {
  server: string;
  id: string;
  name: string;
}

interface BackupNav {
  prefill: BackupPrefill | null;
  prepare: (p: BackupPrefill) => void;
  take: (server: string) => BackupPrefill | null;
}

export const useBackupNav = create<BackupNav>((set, get) => ({
  prefill: null,
  prepare: (p) => set({ prefill: p }),
  take: (server) => {
    const p = get().prefill;
    if (!p || p.server !== server) return null;
    set({ prefill: null });
    return p;
  },
}));
