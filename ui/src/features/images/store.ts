import { create } from "zustand";
import type { BuildSpec } from "@/shared/ipc/bindings/BuildSpec";

export type ImageTab = "summary" | "layers" | "containers" | "inspect";

export type ImagesPage =
  | { kind: "list" }
  | { kind: "detail"; id: string; name: string; tab: ImageTab }
  | { kind: "search" }
  | { kind: "build"; prefill: BuildSpec | null };

interface ImagesNav {
  page: ImagesPage;
  showList: () => void;
  showDetail: (id: string, name: string, tab?: ImageTab) => void;
  setTab: (tab: ImageTab) => void;
  showSearch: () => void;
  showBuild: (prefill?: BuildSpec) => void;
}

/** Navegación dentro de la sección Imágenes. */
export const useImagesNav = create<ImagesNav>((set, get) => ({
  page: { kind: "list" },
  showList: () => set({ page: { kind: "list" } }),
  showDetail: (id, name, tab = "summary") => set({ page: { kind: "detail", id, name, tab } }),
  setTab: (tab) => {
    const p = get().page;
    if (p.kind === "detail") set({ page: { ...p, tab } });
  },
  showSearch: () => set({ page: { kind: "search" } }),
  showBuild: (prefill) => set({ page: { kind: "build", prefill: prefill ?? null } }),
}));
