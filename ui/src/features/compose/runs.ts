import { create } from "zustand";
import { useQueryClient } from "@tanstack/react-query";
import { api } from "@/shared/ipc/client";
import type { ComposeAction } from "@/shared/ipc/bindings/ComposeAction";
import type { OutputLine } from "@/shared/ipc/bindings/OutputLine";
import type { ProjectRef } from "@/shared/ipc/bindings/ProjectRef";
import type { RunClosed } from "@/shared/ipc/bindings/RunClosed";
import { keys } from "@/shared/query";

/** Líneas conservadas por operación (las más recientes). */
export const MAX_LINES = 5000;

export type RunStatus = "running" | "succeeded" | "failed" | "stopped" | "error";

export interface Run {
  project: string;
  action: ComposeAction;
  command: string | null;
  lines: OutputLine[];
  dropped: number;
  status: RunStatus;
  closed: RunClosed | null;
  /** Error al abrir (validación, confirmación, conexión…): no se ejecutó nada. */
  error: string | null;
  stream: number | null;
  startedAt: number;
}

export function statusOf(c: RunClosed): RunStatus {
  if (c.error) return "error";
  if (!c.end) return "error";
  if (c.end.end === "stopped") return "stopped";
  return c.end.code === 0 ? "succeeded" : "failed";
}

interface RunsState {
  runs: Record<string, Run>;
  start: (project: ProjectRef, action: ComposeAction, confirmed: boolean, onEnd?: (r: Run) => void) => Promise<void>;
  stop: (project: string) => void;
  dismiss: (project: string) => void;
}

/** Última operación de cada proyecto (sigue viva aunque se cambie de pantalla). */
export const useComposeRuns = create<RunsState>((set, get) => ({
  runs: {},
  start: async (project, action, confirmed, onEnd) => {
    const name = project.name;
    if (get().runs[name]?.status === "running") return;
    const run: Run = { project: name, action, command: null, lines: [], dropped: 0, status: "running", closed: null, error: null, stream: null, startedAt: Date.now() };
    set((s) => ({ runs: { ...s.runs, [name]: run } }));
    const patch = (p: Partial<Run> | ((r: Run) => Partial<Run>)) =>
      set((s) => {
        const cur = s.runs[name];
        if (!cur || cur.startedAt !== run.startedAt) return s;
        const next = { ...cur, ...(typeof p === "function" ? p(cur) : p) };
        return { runs: { ...s.runs, [name]: next } };
      });
    try {
      const preview = await api.composePreview(project, action);
      patch({ command: preview.command });
      const id = await api.composeRunOpen(project, action, confirmed, (e) => {
        if (e.event === "lines") {
          patch((r) => {
            const lines = r.lines.concat(e.lines);
            const extra = Math.max(0, lines.length - MAX_LINES);
            return { lines: extra ? lines.slice(extra) : lines, dropped: r.dropped + extra };
          });
        } else {
          patch({ closed: e.closed, status: statusOf(e.closed), stream: null });
          const done = get().runs[name];
          if (done && onEnd) onEnd(done);
        }
      });
      patch((r) => (r.status === "running" ? { stream: id } : {}));
    } catch (e) {
      patch({ status: "error", error: (e as { message: string }).message, stream: null });
      const done = get().runs[name];
      if (done && onEnd) onEnd(done);
    }
  },
  stop: (project) => {
    const id = get().runs[project]?.stream;
    if (id !== null && id !== undefined) void api.streamClose(id).catch(() => {});
  },
  dismiss: (project) =>
    set((s) => {
      if (s.runs[project]?.status === "running") return s;
      const { [project]: _gone, ...rest } = s.runs;
      return { runs: rest };
    }),
}));

/** Lanza una acción y, al terminar, recarga todo lo que puede haber cambiado. */
export function useRunCompose() {
  const qc = useQueryClient();
  const start = useComposeRuns((s) => s.start);
  return (project: ProjectRef, action: ComposeAction, confirmed = false) =>
    start(project, action, confirmed, () => {
      for (const k of [keys.composeProjects, keys.containers, keys.overview, keys.volumes, keys.networks, keys.images, keys.binds]) {
        void qc.invalidateQueries({ queryKey: k });
      }
      void qc.invalidateQueries({ queryKey: ["composeConfig", project.name] });
    });
}
