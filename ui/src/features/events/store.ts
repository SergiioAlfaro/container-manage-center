// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { create } from "zustand";
import { api } from "@/shared/ipc/client";
import type { DockerEvent } from "@/shared/ipc/bindings/DockerEvent";

export const LIVE_MAX = 2000;
export type EventsTab = "live" | "journal" | "docker" | "history";

interface EventsState {
  /** Eventos de esta sesión, los más recientes primero. */
  live: DockerEvent[];
  /** Llegados mientras la vista estaba en pausa. */
  pending: DockerEvent[];
  paused: boolean;
  loaded: boolean;
  tab: EventsTab;
  setTab: (t: EventsTab) => void;
  add: (e: DockerEvent) => void;
  setPaused: (p: boolean) => void;
  load: () => Promise<void>;
  clearView: () => void;
}

export const useEvents = create<EventsState>((set, get) => ({
  live: [],
  pending: [],
  paused: false,
  loaded: false,
  tab: "live",
  setTab: (tab) => set({ tab }),
  add: (e) =>
    set((s) => (s.paused ? { pending: [e, ...s.pending].slice(0, LIVE_MAX) } : { live: [e, ...s.live].slice(0, LIVE_MAX) })),
  setPaused: (paused) =>
    set((s) => (paused ? { paused } : { paused, live: [...s.pending, ...s.live].slice(0, LIVE_MAX), pending: [] })),
  load: async () => {
    if (get().loaded) return;
    try {
      const page = await api.eventsRecent({ limit: LIVE_MAX });
      set((s) => {
        // Los que ya hayan llegado por el canal no se duplican.
        const key = (e: DockerEvent) => `${e.server?.id ?? "local"}|${e.timeNano}${e.action}${e.actorId}`;
        const seen = new Set(s.live.map(key));
        const older = page.events.filter((e) => !seen.has(key(e)));
        return { live: [...s.live, ...older].slice(0, LIVE_MAX), loaded: true };
      });
    } catch {
      set({ loaded: true });
    }
  },
  clearView: () => set({ live: [], pending: [] }),
}));
