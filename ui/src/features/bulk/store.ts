// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * Operación masiva en curso (1.10.0). Vive fuera del apartado para que el
 * progreso siga llegando aunque se cambie de apartado y se vea al volver.
 * Solo hay una a la vez desde la interfaz; el backend además reserva los
 * contenedores (`bulk.busy`) para que nunca se apliquen dos órdenes a la vez.
 */
import { create } from "zustand";
import { api } from "@/shared/ipc/client";
import type { BulkEvent } from "@/shared/ipc/bindings/BulkEvent";
import type { BulkItemResult } from "@/shared/ipc/bindings/BulkItemResult";
import type { BulkOperation } from "@/shared/ipc/bindings/BulkOperation";
import type { BulkRequest } from "@/shared/ipc/bindings/BulkRequest";
import type { BulkSummary } from "@/shared/ipc/bindings/BulkSummary";
import type { BulkTarget } from "@/shared/ipc/bindings/BulkTarget";

export type RunPhase = "idle" | "starting" | "running" | "done";

interface RunState {
  phase: RunPhase;
  sid: number | null;
  batch: string | null;
  operation: BulkOperation | null;
  targets: BulkTarget[];
  concurrency: number;
  /** Índices que se están aplicando ahora. */
  active: number[];
  results: Array<BulkItemResult | undefined>;
  summary: BulkSummary | null;
  /** Error al empezar (validación, reserva…): no se envió nada. */
  error: string | null;
  cancelling: boolean;
  /** Se ejecuta cuando termina (la vista recarga la tabla). */
  onFinish: (() => void) | null;
  run: (request: BulkRequest, onFinish?: () => void) => Promise<boolean>;
  cancel: () => Promise<void>;
  dismiss: () => void;
}

const IDLE = {
  phase: "idle" as RunPhase,
  sid: null,
  batch: null,
  operation: null,
  targets: [],
  concurrency: 0,
  active: [],
  results: [],
  summary: null,
  error: null,
  cancelling: false,
  onFinish: null,
};

export const useBulkRun = create<RunState>((set, get) => ({
  ...IDLE,
  run: async (request, onFinish) => {
    // Nunca dos a la vez desde aquí (evita dobles clics y repeticiones).
    if (get().phase === "starting" || get().phase === "running") return false;
    set({ ...IDLE, phase: "starting", operation: request.operation, targets: request.targets, results: request.targets.map(() => undefined), onFinish: onFinish ?? null });
    const handle = (e: BulkEvent) => {
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
        case "finished": {
          const results = e.summary.results.slice();
          set({ phase: "done", summary: e.summary, results, active: [], sid: null, cancelling: false, batch: e.summary.batch });
          get().onFinish?.();
          break;
        }
      }
    };
    try {
      const sid = await api.bulkRunOpen(request, handle);
      // El evento «finished» puede haber llegado ya (operaciones muy rápidas).
      if (get().phase !== "done") set({ sid, phase: "running" });
      return true;
    } catch (e) {
      set({ ...IDLE, error: (e as { message: string }).message });
      return false;
    }
  },
  cancel: async () => {
    const { sid, phase } = get();
    if (sid === null || phase !== "running") return;
    set({ cancelling: true });
    // Solo se cancela lo pendiente; lo que ya se envió a Docker termina y se informa.
    await api.streamClose(sid).catch(() => false);
  },
  dismiss: () => {
    if (get().phase === "starting" || get().phase === "running") return;
    set({ ...IDLE });
  },
}));

export function doneCount(results: Array<BulkItemResult | undefined>): number {
  return results.filter(Boolean).length;
}
