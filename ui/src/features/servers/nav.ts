// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { create } from "zustand";

export type ServersPage = { kind: "list" } | { kind: "edit"; id: string | null } | { kind: "diagnose"; id: string };
export type ServersTab = "servers" | "overview";

interface Nav {
  page: ServersPage;
  tab: ServersTab;
  showList: () => void;
  edit: (id: string | null) => void;
  diagnose: (id: string) => void;
  setTab: (t: ServersTab) => void;
}

/** Navegación dentro de «Servidores». */
export const useServersNav = create<Nav>((set) => ({
  page: { kind: "list" },
  tab: "servers",
  showList: () => set({ page: { kind: "list" } }),
  edit: (id) => set({ page: { kind: "edit", id } }),
  diagnose: (id) => set({ page: { kind: "diagnose", id } }),
  setTab: (tab) => set({ tab, page: { kind: "list" } }),
}));
