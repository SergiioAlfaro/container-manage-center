import { create } from "zustand";
import type { ContainerSpec } from "@/shared/ipc/bindings/ContainerSpec";

export type DetailTab = "summary" | "logs" | "terminal" | "stats" | "inspect";

export interface Notice {
  tone: "info" | "warn" | "error";
  text: string;
}

export type ContainersPage =
  | { kind: "list" }
  | { kind: "detail"; id: string; name: string; tab: DetailTab; notices: Notice[] }
  | { kind: "create"; spec: ContainerSpec | null; notes: string[]; sourceName: string | null }
  /** «Editar configuración» (0.9.2). */
  | { kind: "edit"; id: string; name: string };

interface ContainersNav {
  page: ContainersPage;
  showList: () => void;
  showDetail: (id: string, name: string, tab?: DetailTab, notices?: Notice[]) => void;
  dismissNotices: () => void;
  setTab: (tab: DetailTab) => void;
  showCreate: (prefill?: { spec: ContainerSpec; notes: string[]; sourceName: string | null }) => void;
  showEdit: (id: string, name: string) => void;
}

/** Navegación dentro de la sección Contenedores (lista, detalle, asistente). */
export const useContainersNav = create<ContainersNav>((set, get) => ({
  page: { kind: "list" },
  showList: () => set({ page: { kind: "list" } }),
  showDetail: (id, name, tab = "summary", notices = []) => set({ page: { kind: "detail", id, name, tab, notices } }),
  dismissNotices: () => {
    const p = get().page;
    if (p.kind === "detail") set({ page: { ...p, notices: [] } });
  },
  setTab: (tab) => {
    const p = get().page;
    if (p.kind === "detail") set({ page: { ...p, tab } });
  },
  showEdit: (id, name) => set({ page: { kind: "edit", id, name } }),
  showCreate: (prefill) =>
    set({ page: { kind: "create", spec: prefill?.spec ?? null, notes: prefill?.notes ?? [], sourceName: prefill?.sourceName ?? null } }),
}));
