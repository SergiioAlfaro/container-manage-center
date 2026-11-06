import { create } from "zustand";

export type VolumeTab = "summary" | "containers" | "inspect";

export type VolumesPage = { kind: "list" } | { kind: "detail"; name: string; tab: VolumeTab } | { kind: "create" };

interface VolumesNav {
  page: VolumesPage;
  showList: () => void;
  showDetail: (name: string, tab?: VolumeTab) => void;
  setTab: (tab: VolumeTab) => void;
  showCreate: () => void;
}

/** Navegación dentro de la sección Volúmenes. */
export const useVolumesNav = create<VolumesNav>((set, get) => ({
  page: { kind: "list" },
  showList: () => set({ page: { kind: "list" } }),
  showDetail: (name, tab = "summary") => set({ page: { kind: "detail", name, tab } }),
  setTab: (tab) => {
    const p = get().page;
    if (p.kind === "detail") set({ page: { ...p, tab } });
  },
  showCreate: () => set({ page: { kind: "create" } }),
}));
