import { create } from "zustand";

export type ProjectTab = "services" | "logs" | "file" | "config" | "output";

export type ComposePage = { kind: "list" } | { kind: "detail"; name: string; tab: ProjectTab; logServices?: string[] } | { kind: "new" };

interface ComposeNav {
  page: ComposePage;
  showList: () => void;
  showDetail: (name: string, tab?: ProjectTab) => void;
  setTab: (tab: ProjectTab) => void;
  /** Abre los registros filtrados por servicios. */
  showLogs: (services: string[]) => void;
  showNew: () => void;
}

/** Navegación dentro de la sección Docker Compose. */
export const useComposeNav = create<ComposeNav>((set, get) => ({
  page: { kind: "list" },
  showList: () => set({ page: { kind: "list" } }),
  showDetail: (name, tab = "services") => set({ page: { kind: "detail", name, tab } }),
  setTab: (tab) => {
    const p = get().page;
    if (p.kind === "detail") set({ page: { ...p, tab } });
  },
  showLogs: (services) => {
    const p = get().page;
    if (p.kind === "detail") set({ page: { ...p, tab: "logs", logServices: services } });
  },
  showNew: () => set({ page: { kind: "new" } }),
}));
