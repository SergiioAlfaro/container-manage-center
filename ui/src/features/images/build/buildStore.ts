/**
 * Construcción en curso (una a la vez). Vive fuera del componente para que la
 * construcción siga si el usuario cambia de apartado.
 */
import { create } from "zustand";
import { api } from "@/shared/ipc/client";
import type { BuildEnd } from "@/shared/ipc/bindings/BuildEnd";
import type { BuildEvent } from "@/shared/ipc/bindings/BuildEvent";
import type { BuildSpec } from "@/shared/ipc/bindings/BuildSpec";
import type { BuildStep } from "@/shared/ipc/bindings/BuildStep";
import type { CommandError } from "@/shared/ipc/bindings/CommandError";

export type BuildStatus = "idle" | "starting" | "running" | "done" | "failed" | "cancelled";
export const MAX_BUILD_LINES = 20_000;

interface BuildSession {
  status: BuildStatus;
  spec: BuildSpec | null;
  stream: number | null;
  steps: BuildStep[];
  lines: string[];
  droppedLines: number;
  result: BuildEnd | null;
  error: CommandError | null;
  cancelRequested: boolean;
  start: (spec: BuildSpec, onFinished?: (ok: boolean) => void) => Promise<void>;
  cancel: () => void;
  reset: () => void;
}

const empty = { status: "idle" as BuildStatus, spec: null, stream: null, steps: [], lines: [], droppedLines: 0, result: null, error: null, cancelRequested: false };

/** Sustituye los pasos actualizados y mantiene el orden por número. */
export function mergeSteps(prev: BuildStep[], next: BuildStep[]): BuildStep[] {
  const map = new Map(prev.map((s) => [s.number, s]));
  for (const s of next) map.set(s.number, s);
  return [...map.values()].sort((a, b) => a.number - b.number);
}

export const useBuild = create<BuildSession>((set, get) => ({
  ...empty,
  start: async (spec, onFinished) => {
    if (get().status === "starting" || get().status === "running") return;
    set({ ...empty, status: "starting", spec });
    const handler = (e: BuildEvent) => {
      if (e.event === "progress") {
        set((s) => {
          const all = s.lines.concat(e.update.lines);
          const drop = Math.max(0, all.length - MAX_BUILD_LINES);
          return { status: "running", steps: mergeSteps(s.steps, e.update.steps), lines: drop ? all.slice(drop) : all, droppedLines: s.droppedLines + drop };
        });
      } else {
        const c = e.closed;
        const status: BuildStatus = c.error ? "failed" : c.result?.end.reason === "closed" ? "cancelled" : "done";
        set({ status, stream: null, result: c.result, error: c.error });
        onFinished?.(status === "done");
      }
    };
    try {
      const stream = await api.buildOpen(spec, handler);
      const s = get();
      if (s.status === "starting" || s.status === "running") {
        set({ stream });
        if (s.cancelRequested) void api.streamClose(stream);
      }
    } catch (e) {
      set({ status: "failed", error: e as CommandError });
      onFinished?.(false);
    }
  },
  cancel: () => {
    set({ cancelRequested: true });
    const id = get().stream;
    if (id !== null) void api.streamClose(id);
  },
  reset: () => {
    const s = get().status;
    if (s !== "starting" && s !== "running") set({ ...empty });
  },
}));
