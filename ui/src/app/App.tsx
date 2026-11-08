import { Suspense, lazy, useEffect, useMemo, useState, type ComponentType } from "react";
import { PanelRight, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { ActivityPanel } from "@/features/activity/ActivityPanel";
import { useActivity } from "@/features/activity/store";
import { DashboardView } from "@/features/dashboard/DashboardView";
import { ConnectionBadge } from "@/features/engine/ConnectionBadge";
import { useEngine } from "@/features/engine/store";
import { PendingSection } from "@/features/pending/PendingSection";
import { useLiveRefresh } from "@/features/events/liveRefresh";
import { useQueryClient } from "@tanstack/react-query";
import { api } from "@/shared/ipc/client";
import { cx } from "@/shared/ui/controls";
import { Spinner } from "@/shared/ui/feedback";
import { Sidebar } from "./Sidebar";
import { useApp } from "./store";

// Fase 9: cada apartado (y su código) se carga la primera vez que se abre. El
// panel, que es lo primero que se ve, va en el paquete inicial.
const load = {
  ContainersView: () => import("@/features/containers/ContainersView"),
  ImagesView: () => import("@/features/images/ImagesView"),
  NetworksView: () => import("@/features/networks/NetworksView"),
  VolumesView: () => import("@/features/volumes/VolumesView"),
  BindsView: () => import("@/features/binds/BindsView"),
  ComposeView: () => import("@/features/compose/ComposeView"),
  EventsView: () => import("@/features/events/EventsView"),
  SettingsView: () => import("@/features/settings/SettingsView"),
};
type ViewName = keyof typeof load;
/** Apartados ya cargados: se pintan directamente, sin pasar por `React.lazy`,
 *  que suspende siempre la primera vez (y mostraría «Cargando…» un instante)
 *  aunque el código ya esté descargado. */
const ready: Partial<Record<ViewName, ComponentType>> = {};
function remember(name: ViewName, m: unknown): ComponentType {
  const C = (m as Record<string, ComponentType>)[name]!;
  ready[name] = C;
  return C;
}
const lazyViews = Object.fromEntries(
  (Object.keys(load) as ViewName[]).map((n) => [n, lazy(() => load[n]().then((m) => ({ default: remember(n, m) })))]),
) as unknown as Record<ViewName, ComponentType>;

function View({ name }: { name: ViewName }) {
  // Se decide una vez por visita: cambiar de componente a mitad la reiniciaría.
  const C = useMemo(() => ready[name] ?? lazyViews[name], [name]);
  return <C />;
}

/** Precarga el código de los apartados cuando la aplicación ya está en marcha,
 *  para que la primera visita a cada uno sea inmediata. */
export const PRELOAD_DELAY_MS = 2000;
function usePreloadSections() {
  useEffect(() => {
    const id = window.setTimeout(() => {
      for (const n of Object.keys(load) as ViewName[]) {
        void load[n]()
          .then((m) => remember(n, m))
          .catch(() => undefined);
      }
    }, PRELOAD_DELAY_MS);
    return () => window.clearTimeout(id);
  }, []);
}

/** Conecta el registro de actividad con el backend y aplica la apertura automática ante errores. */
function useActivityBridge() {
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const { reload, apply } = useActivity.getState();
    void api
      .onActivity(
        (e) => {
          const rec = apply(e);
          const { config, setActivityOpen } = useApp.getState();
          if (rec?.status === "failed" && config.activity.openOnError) setActivityOpen(true);
        },
        () => void reload(),
      )
      .then((un) => {
        if (disposed) un();
        else unlisten = un;
      });
    void reload();
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const max = useApp((s) => s.config.activity.maxRecords);
  useEffect(() => useActivity.getState().setMax(max), [max]);
}

/** Estado de la conexión: valor inicial y cambios en tiempo real. Al (re)conectar se
 *  descartan los datos en caché para volver a pedirlos a Docker. */
function useEngineBridge() {
  const qc = useQueryClient();
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const { set, init } = useEngine.getState();
    void api
      .onEngineState((s) => {
        const prev = useEngine.getState().state;
        set(s);
        if (s.state === "connected" && prev?.state !== "connected") void qc.invalidateQueries();
        if (s.state !== "connected") qc.removeQueries();
      })
      .then((un) => (disposed ? un() : (unlisten = un)));
    void init();
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [qc]);
}

function Section({ id }: { id: ReturnType<typeof useApp.getState>["section"] }) {
  switch (id) {
    case "dashboard":
      return <DashboardView />;
    case "containers":
      return <View name="ContainersView" />;
    case "images":
      return <View name="ImagesView" />;
    case "networks":
      return <View name="NetworksView" />;
    case "volumes":
      return <View name="VolumesView" />;
    case "binds":
      return <View name="BindsView" />;
    case "compose":
      return <View name="ComposeView" />;
    case "events":
      return <View name="EventsView" />;
    case "settings":
      return <View name="SettingsView" />;
    default:
      return <PendingSection id={id} />;
  }
}

function useShortcuts() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!e.ctrlKey || e.altKey || e.metaKey) return;
      const key = e.key.toLowerCase();
      if (key === "b" && !e.shiftKey) {
        e.preventDefault();
        useApp.getState().toggleSidebar();
      } else if (key === "a" && e.shiftKey) {
        e.preventDefault();
        useApp.getState().toggleActivity();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}

export function App() {
  const { t } = useTranslation();
  usePreloadSections();
  const { section, config, info, saveError, navigate, toggleSidebar, toggleActivity, setActivityOpen, dismissSaveError } = useApp();
  const running = useActivity((s) => s.records.filter((r) => r.status === "running").length);
  const failedRecent = useActivity((s) => s.records.some((r) => r.status === "failed"));
  useActivityBridge();
  useLiveRefresh();
  useEngineBridge();
  useShortcuts();

  const panelOpen = config.activity.panelOpen;
  const notice = saveError ?? info?.startupWarning ?? null;
  const [update, setUpdate] = useState<string | null>(null);
  // Comprobación de versiones al iniciar (si está activada): aviso discreto.
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void api
      .onUpdateResult((o) => {
        if (o.check?.newer) setUpdate(o.check.latest);
      })
      .then((un) => (disposed ? un() : (unlisten = un)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  return (
    <div className="flex h-full">
      <Sidebar active={section} collapsed={config.ui.sidebarCollapsed} onNavigate={navigate} onToggle={toggleSidebar} />

      <main className="flex min-w-0 flex-1 flex-col">
        <header className="flex h-14 shrink-0 items-center gap-4 border-b border-line bg-surface px-6">
          <div className="min-w-0">
            <h1 className="truncate text-[17px] font-semibold leading-tight tracking-[-0.01em]">{t(`sections.${section}.title`)}</h1>
            <p className="truncate text-[12.5px] text-ink-3">{t(`sections.${section}.description`)}</p>
          </div>
          <div className="ml-auto flex shrink-0">
            <ConnectionBadge onOpen={() => navigate("dashboard")} />
          </div>
          <button
            type="button"
            onClick={toggleActivity}
            aria-pressed={panelOpen}
            aria-label={panelOpen ? t("activity.close") : t("activity.open")}
            title={`${panelOpen ? t("activity.close") : t("activity.open")} (${t("activity.shortcut")})`}
            className={cx(
              "relative inline-flex h-8 items-center gap-2 rounded-md border px-3 text-[13px] transition-colors",
              panelOpen ? "border-beacon bg-beacon-soft text-beacon-strong" : "border-line bg-surface text-ink-2 hover:border-line-strong hover:text-ink",
            )}
          >
            <PanelRight size={16} strokeWidth={1.75} />
            <span>{t("activity.title")}</span>
            {running > 0 && <span className="rounded-full bg-run px-1.5 text-[11px] leading-[18px] font-semibold text-white tabular-nums">{running}</span>}
            {running === 0 && failedRecent && <span aria-hidden className="h-2 w-2 rounded-full bg-err" />}
          </button>
        </header>

        {notice && (
          <div role="alert" className="flex items-start gap-3 border-b border-warn/30 bg-warn/10 px-6 py-2.5 text-[13px]">
            <p className="flex-1 text-ink">{saveError ? t("settings.saveError", { message: saveError }) : notice}</p>
            {saveError && (
              <button type="button" onClick={dismissSaveError} aria-label={t("common.dismiss")} className="text-ink-3 hover:text-ink">
                <X size={16} />
              </button>
            )}
          </div>
        )}

        {update && (
          <div role="status" data-testid="update-banner" className="flex items-center gap-3 border-b border-beacon/30 bg-beacon/10 px-6 py-2 text-[13px]">
            <p className="flex-1 text-ink">{t("updates.banner", { latest: update })}</p>
            <button type="button" onClick={() => navigate("settings")} className="font-medium text-beacon-strong hover:underline">
              {t("updates.bannerOpen")}
            </button>
            <button type="button" onClick={() => setUpdate(null)} aria-label={t("common.dismiss")} className="text-ink-3 hover:text-ink">
              <X size={16} />
            </button>
          </div>
        )}

        <div className="min-h-0 flex-1 overflow-y-auto">
          <Suspense
            fallback={
              <div role="status" className="flex items-center justify-center gap-2 py-20 text-[13px] text-ink-3">
                <Spinner /> {t("common.loading")}
              </div>
            }
          >
            <Section id={section} />
          </Suspense>
        </div>
      </main>

      {panelOpen && <ActivityPanel onClose={() => setActivityOpen(false)} />}
    </div>
  );
}
