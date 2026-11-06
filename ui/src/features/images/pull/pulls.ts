/**
 * Descargas en curso y recientes. Viven fuera de los componentes para que una
 * descarga continúe aunque el usuario cambie de apartado; cada una es una
 * operación real (y un registro en el panel de actividad).
 */
import { create } from "zustand";
import { api } from "@/shared/ipc/client";
import type { LayerProgress } from "@/shared/ipc/bindings/LayerProgress";
import type { PullAuth } from "@/shared/ipc/bindings/PullAuth";
import type { PullEvent } from "@/shared/ipc/bindings/PullEvent";

export type PullStatus = "starting" | "running" | "done" | "failed" | "cancelled";

export interface PullState {
  key: number;
  reference: string;
  stream: number | null;
  status: PullStatus;
  layers: LayerProgress[];
  messages: string[];
  summary: string | null;
  error: string | null;
  cancelRequested: boolean;
}

interface Pulls {
  items: PullState[];
  start: (reference: string, auth: PullAuth, onFinished?: (ok: boolean) => void) => Promise<number>;
  cancel: (key: number) => void;
  dismiss: (key: number) => void;
  clearFinished: () => void;
}

let nextKey = 0;
const MAX_MESSAGES = 200;

const DONE = ["Pull complete", "Download complete", "Already exists", "Exists"];
const DOWNLOADED = ["Verifying Checksum", "Download complete", "Extracting"];

/** Avance de una capa (0–1): la descarga es la primera mitad y la extracción la segunda. */
export function layerProgress(l: LayerProgress): number {
  if (l.status === "Pull complete" || l.status === "Already exists" || l.status === "Exists") return 1;
  const frac = l.total ? Math.min(1, (l.current ?? 0) / l.total) : 0;
  if (l.status === "Downloading") return 0.5 * frac;
  if (l.status === "Extracting") return 0.5 + 0.5 * frac;
  if (DOWNLOADED.includes(l.status)) return 0.5;
  return 0;
}

/** Avance total (0–1), ponderado por el tamaño conocido de cada capa; `null` sin capas. */
export function overallProgress(layers: LayerProgress[]): number | null {
  if (layers.length === 0) return null;
  // Con el almacén de containerd, "Download complete" es el estado final.
  if (layers.every((l) => DONE.includes(l.status))) return 1;
  const sizes = layers.map((l) => l.total ?? 0).filter((n) => n > 0);
  const avg = sizes.length ? sizes.reduce((a, b) => a + b, 0) / sizes.length : 1;
  let cur = 0;
  let tot = 0;
  for (const l of layers) {
    const w = l.total ?? avg;
    cur += layerProgress(l) * w;
    tot += w;
  }
  return tot > 0 ? cur / tot : null;
}

export const usePulls = create<Pulls>((set, get) => {
  const patch = (key: number, p: Partial<PullState> | ((s: PullState) => Partial<PullState>)) =>
    set((st) => ({ items: st.items.map((i) => (i.key === key ? { ...i, ...(typeof p === "function" ? p(i) : p) } : i)) }));

  return {
    items: [],
    start: async (reference, auth, onFinished) => {
      const key = ++nextKey;
      set((st) => ({
        items: [{ key, reference, stream: null, status: "starting", layers: [], messages: [], summary: null, error: null, cancelRequested: false }, ...st.items],
      }));
      const handler = (e: PullEvent) => {
        if (e.event === "progress") {
          patch(key, (s) => ({ status: "running", layers: e.update.layers.length ? e.update.layers : s.layers, messages: [...s.messages, ...e.update.messages].slice(-MAX_MESSAGES) }));
        } else {
          const c = e.closed;
          const status: PullStatus = c.error ? "failed" : c.end?.reason === "closed" ? "cancelled" : "done";
          patch(key, { status, stream: null, summary: c.end?.summary ?? null, error: c.error?.message ?? null });
          onFinished?.(status === "done");
        }
      };
      try {
        const stream = await api.imagePullOpen(reference, auth, handler);
        const cur = get().items.find((i) => i.key === key);
        if (cur && (cur.status === "starting" || cur.status === "running")) {
          patch(key, { stream });
          if (cur.cancelRequested) void api.streamClose(stream);
        }
      } catch (e) {
        patch(key, { status: "failed", error: (e as { message: string }).message });
        onFinished?.(false);
      }
      return key;
    },
    cancel: (key) => {
      const p = get().items.find((i) => i.key === key);
      if (!p) return;
      patch(key, { cancelRequested: true });
      if (p.stream !== null) void api.streamClose(p.stream);
    },
    dismiss: (key) => set((st) => ({ items: st.items.filter((i) => i.key !== key || i.status === "running" || i.status === "starting") })),
    clearFinished: () => set((st) => ({ items: st.items.filter((i) => i.status === "running" || i.status === "starting") })),
  };
});
