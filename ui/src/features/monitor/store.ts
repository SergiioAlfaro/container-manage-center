import { create } from "zustand";

/** E6 (1.6.0): contenedor que hay que mostrar al llegar desde otra
 *  herramienta (la auditoría). Se consume una vez, por servidor y por id. */
interface MonitorNav {
  focus: { server: string; id: string } | null;
  show: (server: string, id: string) => void;
  /** Devuelve el id pedido para este servidor y lo olvida. */
  take: (server: string) => string | null;
}

export const useMonitorNav = create<MonitorNav>((set, get) => ({
  focus: null,
  show: (server, id) => set({ focus: { server, id } }),
  take: (server) => {
    const f = get().focus;
    if (!f || f.server !== server) return null;
    set({ focus: null });
    return f.id;
  },
}));
