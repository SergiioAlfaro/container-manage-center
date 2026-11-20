// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * Apartados «Resumen», «Configuración masiva» e «Historial» de la gestión
 * masiva (1.11.0).
 */
import { Fragment, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ChevronDown, ChevronRight, RefreshCw, RotateCw, ScrollText, Tags } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApp } from "@/app/store";
import { useActivity } from "@/features/activity/store";
import { formatDuration } from "@/features/activity/logic";
import type { BulkEnvironment } from "@/shared/ipc/bindings/BulkEnvironment";
import type { HistoryEntry } from "@/shared/ipc/bindings/HistoryEntry";
import { api } from "@/shared/ipc/client";
import { Button, cx } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { useBulkExec } from "./execStore";
import { envLabel, kpis, policyText, type BulkRow, type OpKind } from "./model";
import { useBulkRun } from "./store";
import { operationText } from "./dialogs";

export type BulkTab = "summary" | "containers" | "operations" | "config" | "commands" | "history";
export const TABS: BulkTab[] = ["summary", "containers", "operations", "config", "commands", "history"];

// ------------------------------------------------------------------ resumen

export function SummaryTab({ envs, rows, onTab }: { envs: BulkEnvironment[]; rows: BulkRow[]; onTab: (t: BulkTab) => void }) {
  const { t } = useTranslation();
  const localName = t("servers.local");
  const run = useBulkRun();
  const exec = useBulkExec();
  return (
    <div className="space-y-4" data-testid="bulk-summary">
      <div className="overflow-x-auto rounded-xl border border-line bg-surface">
        <table className="w-full min-w-[640px] text-left text-[13px]" aria-label={t("bulk.summary.envTable")}>
          <thead className="border-b border-line bg-surface-2 text-[12px] text-ink-3">
            <tr>
              <th className="px-3 py-2 font-medium">{t("bulk.col.env")}</th>
              <th className="px-3 py-2 font-medium">{t("bulk.summary.connection")}</th>
              <th className="px-3 py-2 font-medium">{t("bulk.kpi.total")}</th>
              <th className="px-3 py-2 font-medium">{t("bulk.kpi.running")}</th>
              <th className="px-3 py-2 font-medium">{t("bulk.kpi.stopped")}</th>
              <th className="px-3 py-2 font-medium">{t("bulk.kpi.paused")}</th>
              <th className="px-3 py-2 font-medium">{t("bulk.kpi.error")}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-line">
            {envs.map((e) => {
              const k = kpis(rows.filter((r) => r.server === e.server));
              return (
                <tr key={e.server}>
                  <td className="px-3 py-2 font-medium">{envLabel(e, localName)}</td>
                  <td className="px-3 py-2">
                    <span className={cx(e.connection === "connected" ? "text-ok" : "text-ink-3")}>{t(`bulk.summary.state.${e.connection}`, { defaultValue: e.connection })}</span>
                    {e.error && <div className="text-[12px] text-err">{e.error.message}</div>}
                  </td>
                  <td className="px-3 py-2 tabular-nums">{k.total}</td>
                  <td className="px-3 py-2 tabular-nums">{k.running}</td>
                  <td className="px-3 py-2 tabular-nums">{k.stopped}</td>
                  <td className="px-3 py-2 tabular-nums">{k.paused}</td>
                  <td className={cx("px-3 py-2 tabular-nums", k.error > 0 && "text-err")}>{k.error}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <div className="grid gap-3 md:grid-cols-2">
        <div className="rounded-xl border border-line bg-surface p-4">
          <h3 className="mb-1 text-[13px] font-semibold">{t("bulk.summary.lastOperation")}</h3>
          {run.operation ? (
            <p className="text-[12.5px] text-ink-2">
              {operationText(t, run.operation)} · {run.phase === "done" && run.summary ? t("bulk.summary.finishedCounts", { ok: run.summary.counts.succeeded, total: run.summary.total }) : t("bulk.summary.inProgress")}
            </p>
          ) : (
            <p className="text-[12.5px] text-ink-3">{t("bulk.summary.none")}</p>
          )}
          <Button size="sm" variant="ghost" className="mt-2" onClick={() => onTab("operations")}>
            {t("bulk.tabs.operations")}
          </Button>
        </div>
        <div className="rounded-xl border border-line bg-surface p-4">
          <h3 className="mb-1 text-[13px] font-semibold">{t("bulk.summary.lastCommand")}</h3>
          {exec.command ? (
            <p className="text-[12.5px] text-ink-2">
              <code className="font-mono">{exec.commandText}</code> ·{" "}
              {exec.phase === "done" && exec.summary ? t("bulk.summary.finishedCounts", { ok: exec.summary.counts.succeeded, total: exec.summary.total }) : t("bulk.summary.inProgress")}
            </p>
          ) : (
            <p className="text-[12.5px] text-ink-3">{t("bulk.summary.none")}</p>
          )}
          <Button size="sm" variant="ghost" className="mt-2" onClick={() => onTab("commands")}>
            {t("bulk.tabs.commands")}
          </Button>
        </div>
      </div>
    </div>
  );
}

// ------------------------------------------------------- configuración masiva

export function ConfigTab({ selected, busy, onOpen, onLabels, onTab }: { selected: BulkRow[]; busy: boolean; onOpen: (k: OpKind) => void; onLabels: () => void; onTab: (t: BulkTab) => void }) {
  const { t } = useTranslation();
  const policies = new Map<string, number>();
  for (const r of selected) {
    const p = r.c.details ? policyText(r.c.details.restartPolicy) : t("bulk.policy.unknown");
    policies.set(p, (policies.get(p) ?? 0) + 1);
  }
  const autoRemove = selected.filter((r) => r.c.details?.autoRemove).length;
  return (
    <div className="space-y-4" data-testid="bulk-config">
      <p className="max-w-[820px] text-[13px] text-ink-2">{t("bulk.config.intro")}</p>
      <p className="text-[12.5px]">
        {selected.length === 0 ? (
          <>
            {t("bulk.config.noneSelected")}{" "}
            <button type="button" className="text-beacon-strong underline" onClick={() => onTab("containers")}>
              {t("bulk.tabs.containers")}
            </button>
          </>
        ) : (
          t("bulk.bar.selected", { count: selected.length })
        )}
      </p>
      <div className="grid gap-4 lg:grid-cols-2">
        <section className="rounded-xl border border-line bg-surface p-4" aria-label={t("bulk.action.restartPolicy")}>
          <h2 className="mb-1 flex items-center gap-2 text-[14px] font-semibold">
            <RotateCw size={15} /> {t("bulk.action.restartPolicy")}
          </h2>
          <p className="mb-2 text-[12.5px] text-ink-2">{t("bulk.policy.live")}</p>
          {selected.length > 0 && (
            <ul className="mb-3 text-[12.5px]" data-testid="config-policies">
              {[...policies.entries()].map(([p, n]) => (
                <li key={p}>
                  <code className="font-mono">{p}</code>: {n}
                </li>
              ))}
              {autoRemove > 0 && <li className="text-ink-3">{t("bulk.config.autoRemove", { count: autoRemove })}</li>}
            </ul>
          )}
          <Button variant="primary" disabled={busy || selected.length === 0 || selected.length > 1000} onClick={() => onOpen("restartPolicy")}>
            {t("bulk.config.changePolicy")}
          </Button>
        </section>
        <section className="rounded-xl border border-line bg-surface p-4" aria-label={t("bulk.action.labels")}>
          <h2 className="mb-1 flex items-center gap-2 text-[14px] font-semibold">
            <Tags size={15} /> {t("bulk.action.labels")}
          </h2>
          <Banner tone="info">{t("bulk.labels.limit")}</Banner>
          <div className="mt-3 flex flex-wrap gap-2">
            {(["add", "modify", "remove"] as const).map((k) => (
              <Button key={k} size="sm" disabled title={t("bulk.labels.limitShort")}>
                {t(`bulk.config.label.${k}`)}
              </Button>
            ))}
            <Button size="sm" disabled={selected.length === 0} onClick={onLabels}>
              {t("bulk.config.viewLabels")}
            </Button>
          </div>
          <p className="mt-2 text-[12px] text-ink-3">{t("bulk.labels.future")}</p>
        </section>
      </div>
    </div>
  );
}

// ------------------------------------------------------------------ historial

const KINDS = ["container.bulk", "container.exec.bulk"];

export function HistoryTab() {
  const { t, i18n } = useTranslation();
  const setFilter = useActivity((x) => x.setFilter);
  const setActivityOpen = useApp((x) => x.setActivityOpen);
  const [open, setOpen] = useState<string | null>(null);
  const q = useQuery({
    queryKey: ["bulkHistory"],
    queryFn: async () => {
      const pages = await Promise.all(KINDS.map((k) => api.historyList({ text: k, limit: 200 })));
      const all = pages.flatMap((p) => p.entries).filter((e) => KINDS.includes(e.kind) && e.batch);
      all.sort((a, b) => b.startedAt.localeCompare(a.startedAt));
      return { entries: all, persistent: pages[0]?.persistent ?? false };
    },
  });
  return (
    <div className="space-y-3" data-testid="bulk-history">
      <div className="flex flex-wrap items-center gap-2">
        <p className="max-w-[760px] text-[13px] text-ink-2">{t("bulk.history.intro")}</p>
        <Button size="sm" className="ml-auto" onClick={() => void q.refetch()} disabled={q.isFetching}>
          {q.isFetching ? <Spinner size={13} /> : <RefreshCw size={13} />} {t("bulk.refresh")}
        </Button>
      </div>
      {q.data && !q.data.persistent && <Banner tone="info">{t("bulk.history.notPersistent")}</Banner>}
      {q.error ? (
        <Banner tone="error">{t("bulk.loadError", { message: (q.error as { message: string }).message })}</Banner>
      ) : q.isPending ? (
        <p className="flex items-center gap-2 text-[13px] text-ink-3">
          <Spinner size={14} /> {t("common.loading")}
        </p>
      ) : q.data.entries.length === 0 ? (
        <p className="py-8 text-center text-[13px] text-ink-3">{t("bulk.history.empty")}</p>
      ) : (
        <div className="overflow-x-auto rounded-xl border border-line bg-surface">
          <table className="w-full min-w-[760px] text-left text-[13px]" aria-label={t("bulk.tabs.history")}>
            <thead className="border-b border-line bg-surface-2 text-[12px] text-ink-3">
              <tr>
                <th className="w-[28px]" />
                <th className="px-3 py-2 font-medium">{t("bulk.history.date")}</th>
                <th className="px-3 py-2 font-medium">{t("bulk.history.type")}</th>
                <th className="px-3 py-2 font-medium">{t("bulk.history.what")}</th>
                <th className="px-3 py-2 font-medium">{t("bulk.history.result")}</th>
                <th className="px-3 py-2 font-medium">{t("bulk.exec.export.duration")}</th>
                <th className="px-3 py-2" />
              </tr>
            </thead>
            <tbody className="divide-y divide-line">
              {q.data.entries.map((e) => (
                <Fragment key={e.id}>
                  <tr data-testid="bulk-history-row">
                    <td className="pl-2">
                      <button type="button" onClick={() => setOpen(open === e.batch ? null : e.batch)} aria-expanded={open === e.batch} aria-label={t("bulk.history.details")} title={t("bulk.history.details")} className="text-ink-3 hover:text-ink">
                        {open === e.batch ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                      </button>
                    </td>
                    <td className="px-3 py-2 whitespace-nowrap">{new Date(e.startedAt).toLocaleString(i18n.language)}</td>
                    <td className="px-3 py-2">{t(e.kind === "container.exec.bulk" ? "bulk.history.command" : "bulk.history.operation")}</td>
                    <td className="max-w-[420px] px-3 py-2 [overflow-wrap:anywhere]">{e.transport.via === "app" ? e.transport.action : e.target}</td>
                    <td className={cx("px-3 py-2", e.status === "succeeded" ? "text-ok" : e.status === "failed" ? "text-err" : "text-ink-3")}>{(e.status === "failed" ? (e.error ?? e.summary) : (e.summary ?? e.error)) ?? t(`activity.status.${e.status}`, { defaultValue: e.status })}</td>
                    <td className="px-3 py-2 tabular-nums whitespace-nowrap">{e.durationMs !== null ? formatDuration(e.durationMs) : "—"}</td>
                    <td className="px-3 py-2">
                      <Button
                        size="sm"
                        variant="ghost"
                        onClick={() => {
                          setFilter({ batch: e.batch });
                          setActivityOpen(true);
                        }}
                        title={t("bulk.history.activityHint")}
                      >
                        <ScrollText size={13} /> {t("bulk.run.activity")}
                      </Button>
                    </td>
                  </tr>
                  {open === e.batch && e.batch && (
                    <tr>
                      <td colSpan={7} className="bg-surface-2 px-6 py-2">
                        <BatchDetail batch={e.batch} />
                      </td>
                    </tr>
                  )}
                </Fragment>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

/** Órdenes reales de un lote guardadas en el historial: contenedor,
 *  resultado, código de salida y duración. */
function BatchDetail({ batch }: { batch: string }) {
  const { t } = useTranslation();
  const q = useQuery({
    queryKey: ["bulkHistoryBatch", batch],
    queryFn: async () => (await api.historyList({ categories: ["container"], limit: 2000 })).entries.filter((e: HistoryEntry) => e.batch === batch && !KINDS.includes(e.kind) && e.kind !== "container.exec.prepare"),
  });
  if (q.isPending) return <Spinner size={13} />;
  if (q.error) return <p className="text-[12.5px] text-err">{(q.error as { message: string }).message}</p>;
  if (q.data.length === 0) return <p className="text-[12.5px] text-ink-3">{t("bulk.history.noOrders")}</p>;
  return (
    <ul className="space-y-0.5 text-[12.5px]" data-testid="bulk-history-orders">
      {q.data.map((e) => (
        <li key={e.id} className="flex flex-wrap gap-x-3">
          <span className="font-medium">{e.target}</span>
          <span className="font-mono text-[11.5px] text-ink-3">{e.kind}</span>
          <span className={e.status === "succeeded" ? "text-ok" : "text-err"}>{e.status === "failed" ? (e.error ?? e.summary) : e.summary}</span>
          {e.exitCode !== null && <span className="font-mono text-[11.5px]">{t("bulk.exec.exitShort", { code: e.exitCode })}</span>}
          {e.durationMs !== null && <span className="text-ink-3">{formatDuration(e.durationMs)}</span>}
        </li>
      ))}
    </ul>
  );
}
