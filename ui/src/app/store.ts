import { create } from "zustand";
import { api } from "@/shared/ipc/client";
import { setLanguage } from "@/shared/i18n";
import type { AppConfig } from "@/shared/ipc/bindings/AppConfig";
import type { AppInfo } from "@/shared/ipc/bindings/AppInfo";
import type { Theme } from "@/shared/ipc/bindings/Theme";
import type { SectionId } from "./sections";

import { refreshTerminalThemes } from "@/shared/ui/terminalTheme";

export const DEFAULT_CONFIG: AppConfig = {
  version: 1,
  ui: { theme: "system", language: "es", accent: "teal", sidebarCollapsed: false },
  activity: { panelOpen: false, openOnError: true, maxRecords: 500, keepHistory: true },
  docker: { mode: "auto", customSocket: "" },
  compose: { projects: [] },
  updates: { checkOnStart: false, source: "" },
};

type DeepPartial<T> = { [K in keyof T]?: T[K] extends object ? DeepPartial<T[K]> : T[K] };

interface AppState {
  section: SectionId;
  config: AppConfig;
  info: AppInfo | null;
  /** Último error al guardar la configuración, para mostrarlo en pantalla. */
  saveError: string | null;
  ready: boolean;
  navigate: (s: SectionId) => void;
  init: () => Promise<void>;
  /** Cambia la configuración de forma optimista y la persiste; revierte si falla. */
  updateConfig: (patch: DeepPartial<AppConfig>) => Promise<boolean>;
  toggleSidebar: () => void;
  toggleActivity: () => void;
  setActivityOpen: (open: boolean) => void;
  dismissSaveError: () => void;
}

function merge(base: AppConfig, patch: DeepPartial<AppConfig>): AppConfig {
  return {
    version: base.version,
    ui: { ...base.ui, ...patch.ui },
    activity: { ...base.activity, ...patch.activity },
    docker: { ...base.docker, ...patch.docker },
    // La lista de proyectos Compose solo la cambia el backend (compose_register/unregister).
    compose: base.compose,
    updates: { ...base.updates, ...patch.updates },
  };
}

const media = typeof window !== "undefined" && window.matchMedia ? window.matchMedia("(prefers-color-scheme: dark)") : null;

export function resolveTheme(theme: Theme): "light" | "dark" {
  if (theme === "system") return media?.matches ? "dark" : "light";
  return theme;
}

export function applyAppearance(config: AppConfig) {
  document.documentElement.dataset.theme = resolveTheme(config.ui.theme);
  document.documentElement.dataset.accent = config.ui.accent;
  refreshTerminalThemes();
  setLanguage(config.ui.language);
}

export const useApp = create<AppState>((set, get) => ({
  section: "dashboard",
  config: DEFAULT_CONFIG,
  info: null,
  saveError: null,
  ready: false,
  navigate: (section) => set({ section }),

  init: async () => {
    try {
      const [config, info] = await Promise.all([api.configGet(), api.appInfo()]);
      applyAppearance(config);
      set({ config, info, ready: true });
    } catch (e) {
      applyAppearance(get().config);
      set({ ready: true, saveError: (e as { message: string }).message });
    }
    media?.addEventListener("change", () => applyAppearance(get().config));
  },

  updateConfig: async (patch) => {
    const previous = get().config;
    const next = merge(previous, patch);
    set({ config: next, saveError: null });
    applyAppearance(next);
    try {
      const saved = await api.configSet(next);
      set({ config: saved });
      applyAppearance(saved);
      return true;
    } catch (e) {
      set({ config: previous, saveError: (e as { message: string }).message });
      applyAppearance(previous);
      return false;
    }
  },

  toggleSidebar: () => void get().updateConfig({ ui: { sidebarCollapsed: !get().config.ui.sidebarCollapsed } }),
  toggleActivity: () => void get().updateConfig({ activity: { panelOpen: !get().config.activity.panelOpen } }),
  setActivityOpen: (open) => {
    if (get().config.activity.panelOpen !== open) void get().updateConfig({ activity: { panelOpen: open } });
  },
  dismissSaveError: () => set({ saveError: null }),
}));
