/**
 * E6 (1.6.0, D-108): los contenedores auditados en las demás herramientas.
 * Se reconocen por su id; desde aquí se abre la monitorización de ese
 * contenedor o se prepara una copia (que el usuario revisa antes de copiar).
 */
import { useQuery } from "@tanstack/react-query";
import { Activity, BellRing, DatabaseBackup } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApi, useServerId } from "@/app/server";
import { useApp } from "@/app/store";
import { useBackupNav } from "@/features/backup/store";
import { useMonitorNav } from "@/features/monitor/store";
import type { AuditReport } from "@/shared/ipc/bindings/AuditReport";
import type { CommandError } from "@/shared/ipc/bindings/CommandError";
import type { RiskLevel } from "@/shared/ipc/bindings/RiskLevel";
import type { ToolLinks } from "@/shared/ipc/bindings/ToolLinks";
import { Button, cx } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { RiskLevelBadge } from "@/shared/ui/HostRisks";
import { containerTargets, worstByTarget } from "./model";

function when(iso: string, lang: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString(lang, { dateStyle: "short", timeStyle: "short" });
}

export function ToolLinksSection({ report }: { report: AuditReport }) {
  const { t, i18n } = useTranslation();
  const bound = useApi();
  const server = useServerId();
  const targets = containerTargets(report);
  const worst = worstByTarget(report);
  const links = useQuery({
    queryKey: ["toolLinks", server, report.id],
    queryFn: () => bound.toolsLinks(targets),
    enabled: targets.length > 0,
    // Las alertas y las copias cambian mientras se mira el informe.
    staleTime: 10_000,
    refetchInterval: 15_000,
  });
  if (targets.length === 0) return null;
  const byId = new Map<string, ToolLinks>((links.data ?? []).map((l) => [l.id, l]));
  const openMonitor = (id: string) => {
    useMonitorNav.getState().show(server, id);
    useApp.getState().navigate("monitor");
  };
  const prepareBackup = (id: string, name: string) => {
    useBackupNav.getState().prepare({ server, id, name });
    useApp.getState().navigate("backup");
  };
  return (
    <section aria-labelledby="audit-links" className="mt-6">
      <h3 id="audit-links" className="mb-1 text-[13.5px] font-semibold">
        {t("audit.links.title")}
      </h3>
      <p className="mb-2 max-w-[860px] text-[12px] text-ink-3">{t("audit.links.hint")}</p>
      {links.error && <Banner tone="error">{t("audit.links.error", { error: (links.error as unknown as CommandError).message })}</Banner>}
      {links.isPending && <Spinner />}
      {links.data && (
        <table className="w-full rounded-xl border border-line bg-surface text-[12.5px]">
          <caption className="sr-only">{t("audit.links.title")}</caption>
          <thead className="text-left text-[12px] text-ink-3">
            <tr>
              <th className="px-3 py-2 font-medium">{t("audit.links.col.container")}</th>
              <th className="px-3 py-2 font-medium">{t("audit.links.col.findings")}</th>
              <th className="px-3 py-2 font-medium">{t("audit.links.col.monitor")}</th>
              <th className="px-3 py-2 font-medium">{t("audit.links.col.backup")}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-line">
            {targets.map((x) => {
              const l = byId.get(x.id);
              const level: RiskLevel | undefined = worst.get(x.id);
              const m = l?.monitor;
              const b = l?.backup ?? null;
              return (
                <tr key={x.id} data-container={x.id}>
                  <td className="px-3 py-2 align-top">
                    <div className="font-medium">{x.name}</div>
                    <div className="selectable font-mono text-[11px] text-ink-3" title={x.id}>
                      {x.id.slice(0, 12)}
                    </div>
                  </td>
                  <td className="px-3 py-2 align-top">{level ? <RiskLevelBadge level={level} /> : <span className="text-ink-3">{t("audit.links.noFindings")}</span>}</td>
                  <td className="px-3 py-2 align-top">
                    {!m?.enabled ? (
                      <span className="text-ink-3">{t("audit.links.monitorOff")}</span>
                    ) : m.sampled ? (
                      <span className="text-ok">{t("audit.links.monitored", { state: m.state ?? "" })}</span>
                    ) : (
                      <span className="text-ink-3">{t("audit.links.notSampled")}</span>
                    )}
                    {m && m.firing.length > 0 && (
                      <div className="mt-0.5 inline-flex items-center gap-1 rounded bg-err/12 px-1.5 text-[11px] text-err">
                        <BellRing size={11} aria-hidden />
                        {t("audit.links.alerts", { count: m.firing.length, kinds: m.firing.map((k) => t(`audit.links.alertKind.${k}`)).join(", ") })}
                      </div>
                    )}
                    <div className="mt-1">
                      <Button size="sm" onClick={() => openMonitor(x.id)}>
                        <Activity size={13} />
                        {t("audit.links.openMonitor")}
                      </Button>
                    </div>
                  </td>
                  <td className="px-3 py-2 align-top">
                    {b ? (
                      <>
                        <div className={cx(b.match === "id" ? "text-ok" : "text-warn")}>{t("audit.links.backup", { date: when(b.createdAt, i18n.language) })}</div>
                        <div className="text-[11.5px] text-ink-2">{b.kinds.map((k) => t(`backup.kind.${k}`)).join(", ")}</div>
                        <div className="text-[11.5px] text-ink-3">{t(`audit.links.match.${b.match}`)}</div>
                        {b.hot && <div className="text-[11.5px] text-warn">{t("audit.links.hot")}</div>}
                      </>
                    ) : (
                      <span className="text-ink-3">{t("audit.links.noBackup")}</span>
                    )}
                    {l && l.jobs.length > 0 && <div className="text-[11.5px] text-ink-2">{t("audit.links.jobs", { count: l.jobs.length, names: l.jobs.map((j) => `«${j.name}»`).join(", ") })}</div>}
                    <div className="mt-1">
                      <Button size="sm" onClick={() => prepareBackup(x.id, x.name)}>
                        <DatabaseBackup size={13} />
                        {t("audit.links.newBackup")}
                      </Button>
                    </div>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </section>
  );
}
