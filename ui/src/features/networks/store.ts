import { create } from "zustand";

export type NetworkTab = "summary" | "containers" | "inspect";

export type NetworksPage = { kind: "list" } | { kind: "detail"; id: string; name: string; tab: NetworkTab } | { kind: "create" };

interface NetworksNav {
  page: NetworksPage;
  showList: () => void;
  showDetail: (id: string, name: string, tab?: NetworkTab) => void;
  setTab: (tab: NetworkTab) => void;
  showCreate: () => void;
}

/** Navegación dentro de la sección Redes. */
export const useNetworksNav = create<NetworksNav>((set, get) => ({
  page: { kind: "list" },
  showList: () => set({ page: { kind: "list" } }),
  showDetail: (id, name, tab = "summary") => set({ page: { kind: "detail", id, name, tab } }),
  setTab: (tab) => {
    const p = get().page;
    if (p.kind === "detail") set({ page: { ...p, tab } });
  },
  showCreate: () => set({ page: { kind: "create" } }),
}));
