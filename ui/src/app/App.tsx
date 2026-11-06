import { useEffect } from "react";
import { PanelRight, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { ActivityPanel } from "@/features/activity/ActivityPanel";
import { useActivity } from "@/features/activity/store";
import { ContainersView } from "@/features/containers/ContainersView";
import { DashboardView } from "@/features/dashboard/DashboardView";
import { ConnectionBadge } from "@/features/engine/ConnectionBadge";
import { useEngine } from "@/features/engine/store";
import { ImagesView } from "@/features/images/ImagesView";
import { NetworksView } from "@/features/networks/NetworksView";
import { VolumesView } from "@/features/volumes/VolumesView";
import { BindsView } from "@/features/binds/BindsView";
import { PendingSection } from "@/features/pending/PendingSection";
import { useQueryClient } from "@tanstack/react-query";
import { SettingsView } from "@/features/settings/SettingsView";
import { api } from "@/shared/ipc/client";
import { cx } from "@/shared/ui/controls";
import { Sidebar } from "./Sidebar";
import { useApp } from "./store";

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
      return <ContainersView />;
    case "images":
      return <ImagesView />;
    case "networks":
      return <NetworksView />;
    case "volumes":
      return <VolumesView />;
    case "binds":
      return <BindsView />;
    case "settings":
      return <SettingsView />;
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
  const { section, config, info, saveError, navigate, toggleSidebar, toggleActivity, setActivityOpen, dismissSaveError } = useApp();
  const running = useActivity((s) => s.records.filter((r) => r.status === "running").length);
  const failedRecent = useActivity((s) => s.records.some((r) => r.status === "failed"));
  useActivityBridge();
  useEngineBridge();
  useShortcuts();

  const panelOpen = config.activity.panelOpen;
  const notice = saveError ?? info?.startupWarning ?? null;

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

        <div className="min-h-0 flex-1 overflow-y-auto">
          <Section id={section} />
        </div>
      </main>

      {panelOpen && <ActivityPanel onClose={() => setActivityOpen(false)} />}
    </div>
  );
}
