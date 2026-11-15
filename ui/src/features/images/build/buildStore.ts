// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * Construcción en curso (una a la vez por servidor, 0.10.0). Vive fuera del
 * componente para que la construcción siga si el usuario cambia de apartado o
 * de servidor; cada una va siempre al servidor en el que se inició.
 */
import { create } from "zustand";
import { api, dockerApi } from "@/shared/ipc/client";
import { useServerId } from "@/app/server";
import type { BuildEnd } from "@/shared/ipc/bindings/BuildEnd";
import type { BuildEvent } from "@/shared/ipc/bindings/BuildEvent";
import type { BuildSpec } from "@/shared/ipc/bindings/BuildSpec";
import type { BuildStep } from "@/shared/ipc/bindings/BuildStep";
import type { CommandError } from "@/shared/ipc/bindings/CommandError";

export type BuildStatus = "idle" | "starting" | "running" | "done" | "failed" | "cancelled";
export const MAX_BUILD_LINES = 20_000;

interface BuildData {
  status: BuildStatus;
  spec: BuildSpec | null;
  stream: number | null;
  steps: BuildStep[];
  lines: string[];
  droppedLines: number;
  result: BuildEnd | null;
  error: CommandError | null;
  cancelRequested: boolean;
}

interface BuildSession extends BuildData {
  start: (spec: BuildSpec, onFinished?: (ok: boolean) => void, shareLocalCredentials?: boolean) => Promise<void>;
  cancel: () => void;
  reset: () => void;
}

const empty: BuildData = { status: "idle" as BuildStatus, spec: null, stream: null, steps: [], lines: [], droppedLines: 0, result: null, error: null, cancelRequested: false };

/** Sustituye los pasos actualizados y mantiene el orden por número. */
export function mergeSteps(prev: BuildStep[], next: BuildStep[]): BuildStep[] {
  const map = new Map(prev.map((s) => [s.number, s]));
  for (const s of next) map.set(s.number, s);
  return [...map.values()].sort((a, b) => a.number - b.number);
}

interface BuildStore {
  byServer: Record<string, BuildData>;
  start: (server: string, spec: BuildSpec, onFinished?: (ok: boolean) => void, shareLocalCredentials?: boolean) => Promise<void>;
  cancel: (server: string) => void;
  reset: (server: string) => void;
}

export const useBuildStore = create<BuildStore>((set, get) => {
  const cur = (server: string): BuildData => get().byServer[server] ?? empty;
  const put = (server: string, p: Partial<BuildData> | ((s: BuildData) => Partial<BuildData>)) =>
    set((st) => {
      const prev = st.byServer[server] ?? empty;
      return { byServer: { ...st.byServer, [server]: { ...prev, ...(typeof p === "function" ? p(prev) : p) } } };
    });
  return {
    byServer: {},
    start: async (server, spec, onFinished, shareLocalCredentials = false) => {
      const st = cur(server).status;
      if (st === "starting" || st === "running") return;
      put(server, { ...empty, status: "starting", spec });
      const handler = (e: BuildEvent) => {
        if (e.event === "progress") {
          put(server, (s) => {
            const all = s.lines.concat(e.update.lines);
            const drop = Math.max(0, all.length - MAX_BUILD_LINES);
            return { status: "running", steps: mergeSteps(s.steps, e.update.steps), lines: drop ? all.slice(drop) : all, droppedLines: s.droppedLines + drop };
          });
        } else {
          const c = e.closed;
          const status: BuildStatus = c.error ? "failed" : c.result?.end.reason === "closed" ? "cancelled" : "done";
          put(server, { status, stream: null, result: c.result, error: c.error });
          onFinished?.(status === "done");
        }
      };
      try {
        const stream = await dockerApi(server).buildOpen(spec, handler, shareLocalCredentials);
        const s = cur(server);
        if (s.status === "starting" || s.status === "running") {
          put(server, { stream });
          if (s.cancelRequested) void api.streamClose(stream);
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
    reset: (server) => {
      const s = cur(server).status;
      if (s !== "starting" && s !== "running") put(server, { ...empty });
    },
  };
});

/** Construcción del servidor de este ámbito. */
export function useBuild(): BuildSession;
export function useBuild<T>(select: (s: BuildSession) => T): T;
export function useBuild<T>(select?: (s: BuildSession) => T): T | BuildSession {
  const server = useServerId();
  const data = useBuildStore((s) => s.byServer[server] ?? empty);
  const store = useBuildStore.getState();
  const session: BuildSession = {
    ...data,
    start: (spec, onFinished, share) => store.start(server, spec, onFinished, share),
    cancel: () => store.cancel(server),
    reset: () => store.reset(server),
  };
  return select ? select(session) : session;
}
