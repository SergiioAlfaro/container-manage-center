// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * Comando masivo en curso (1.11.0). Como las operaciones masivas, vive fuera
 * del apartado para que el progreso siga llegando aunque se cambie de
 * apartado. Una ejecución a la vez desde la interfaz; el backend además
 * reserva los contenedores (`bulk.busy`).
 *
 * Dos cancelaciones distintas (D-122): «cancelar pendientes» no lanza nada más
 * (`stream_close`); «interrumpir activas» además deja de esperar a las que se
 * están ejecutando (`bulk_exec_interrupt`). Docker Exec no permite detener un
 * proceso ya lanzado: puede seguir dentro del contenedor.
 */
import { create } from "zustand";
import { api } from "@/shared/ipc/client";
import type { BulkCommand } from "@/shared/ipc/bindings/BulkCommand";
import type { BulkExecRequest } from "@/shared/ipc/bindings/BulkExecRequest";
import type { ExecEvent } from "@/shared/ipc/bindings/ExecEvent";
import type { ExecItemResult } from "@/shared/ipc/bindings/ExecItemResult";
import type { ExecSummary } from "@/shared/ipc/bindings/ExecSummary";
import type { BulkTarget } from "@/shared/ipc/bindings/BulkTarget";
import type { ExportMeta } from "./execModel";

export type ExecPhase = "idle" | "starting" | "running" | "done";

interface ExecState {
  phase: ExecPhase;
  sid: number | null;
  batch: string | null;
  command: BulkCommand | null;
  /** Texto del comando como se mostró en la confirmación. */
  commandText: string;
  targets: BulkTarget[];
  /** Imagen y servidor de cada destino (para resultados y exportación). */
  rows: ExportMeta["rows"];
  concurrency: number;
  active: number[];
  results: Array<ExecItemResult | undefined>;
  summary: ExecSummary | null;
  error: string | null;
  cancelling: boolean;
  interrupting: boolean;
  onFinish: (() => void) | null;
  run: (request: BulkExecRequest, meta: { commandText: string; rows: ExportMeta["rows"] }, onFinish?: () => void) => Promise<boolean>;
  cancelPending: () => Promise<void>;
  interrupt: () => Promise<void>;
  dismiss: () => void;
}

const IDLE = {
  phase: "idle" as ExecPhase,
  sid: null,
  batch: null,
  command: null,
  commandText: "",
  targets: [],
  rows: [],
  concurrency: 0,
  active: [],
  results: [],
  summary: null,
  error: null,
  cancelling: false,
  interrupting: false,
  onFinish: null,
};

export const useBulkExec = create<ExecState>((set, get) => ({
  ...IDLE,
  run: async (request, meta, onFinish) => {
    // Nunca dos a la vez desde aquí (dobles clics, repeticiones).
    if (get().phase === "starting" || get().phase === "running") return false;
    set({
      ...IDLE,
      phase: "starting",
      command: request.command,
      commandText: meta.commandText,
      targets: request.targets,
      rows: meta.rows,
      concurrency: request.concurrency,
      results: request.targets.map(() => undefined),
      onFinish: onFinish ?? null,
    });
    const handle = (e: ExecEvent) => {
      switch (e.type) {
        case "started":
          set({ batch: e.batch, concurrency: e.concurrency, phase: "running" });
          break;
        case "running":
          set((s) => ({ active: s.active.includes(e.index) ? s.active : [...s.active, e.index] }));
          break;
        case "item":
          set((s) => {
            const results = s.results.slice();
            results[e.result.index] = e.result;
            return { results, active: s.active.filter((i) => i !== e.result.index) };
          });
          break;
        case "finished":
          set({ phase: "done", summary: e.summary, results: e.summary.results.slice(), active: [], sid: null, cancelling: false, interrupting: false, batch: e.summary.batch });
          get().onFinish?.();
          break;
      }
    };
    try {
      const sid = await api.bulkExecOpen(request, handle);
      if (get().phase !== "done") set({ sid, phase: "running" });
      return true;
    } catch (e) {
      set({ ...IDLE, error: (e as { message: string }).message });
      return false;
    }
  },
  cancelPending: async () => {
    const { sid, phase } = get();
    if (sid === null || phase !== "running") return;
    set({ cancelling: true });
    await api.streamClose(sid).catch(() => false);
  },
  interrupt: async () => {
    const { sid, phase } = get();
    if (sid === null || phase !== "running") return;
    set({ interrupting: true, cancelling: true });
    await api.bulkExecInterrupt(sid).catch(() => false);
  },
  dismiss: () => {
    if (get().phase === "starting" || get().phase === "running") return;
    set({ ...IDLE });
  },
}));
