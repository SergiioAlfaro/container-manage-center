// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { create } from "zustand";
import { api } from "@/shared/ipc/client";
import type { ActivityEvent } from "@/shared/ipc/bindings/ActivityEvent";
import type { OperationRecord } from "@/shared/ipc/bindings/OperationRecord";
import { EMPTY_FILTER, upsert, type ActivityFilter } from "./logic";

interface ActivityState {
  records: OperationRecord[];
  loadError: string | null;
  filter: ActivityFilter;
  max: number;
  setFilter: (patch: Partial<ActivityFilter>) => void;
  resetFilter: () => void;
  setMax: (max: number) => void;
  reload: () => Promise<void>;
  apply: (e: ActivityEvent) => OperationRecord | null;
  clear: () => Promise<void>;
}

export const useActivity = create<ActivityState>((set, get) => ({
  records: [],
  loadError: null,
  filter: EMPTY_FILTER,
  max: 500,
  setFilter: (patch) => set((s) => ({ filter: { ...s.filter, ...patch } })),
  resetFilter: () => set({ filter: EMPTY_FILTER }),
  setMax: (max) => set((s) => ({ max, records: s.records.slice(0, max) })),
  reload: async () => {
    try {
      const records = await api.activityList();
      set({ records: records.slice(0, get().max), loadError: null });
    } catch (e) {
      set({ loadError: (e as { message: string }).message });
    }
  },
  /** Aplica un evento del backend y devuelve el registro afectado (si lo hay). */
  apply: (e) => {
    if (e.type === "cleared") {
      set((s) => ({ records: s.records.filter((r) => r.status === "running") }));
      return null;
    }
    set((s) => ({ records: upsert(s.records, e.record, s.max) }));
    return e.record;
  },
  clear: async () => {
    await api.activityClear();
  },
}));
