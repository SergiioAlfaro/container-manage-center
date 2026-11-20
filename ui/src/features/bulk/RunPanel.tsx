// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * Progreso y resultado de una operación masiva (1.10.0): un resultado por
 * contenedor (correcto, fallido, omitido, cancelado o desconocido), con el
 * error comprensible y la duración. Nada se da por bueno sin la confirmación
 * de Docker; si se cortó la conexión a mitad, el resultado es «desconocido».
 */
import { useState } from "react";
import { Ban, CheckCircle2, CircleSlash, HelpCircle, ScrollText, X, XCircle } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApp } from "@/app/store";
import { useActivity } from "@/features/activity/store";
import { formatDuration } from "@/features/activity/logic";
import { shortId } from "@/shared/format";
import type { BulkItemResult } from "@/shared/ipc/bindings/BulkItemResult";
import type { BulkItemStatus } from "@/shared/ipc/bindings/BulkItemStatus";
import { Button, cx } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { operationText, skipText } from "./dialogs";
import { doneCount, useBulkRun } from "./store";

const STATUS_ICON: Record<BulkItemStatus, typeof CheckCircle2> = {
  succeeded: CheckCircle2,
  failed: XCircle,
  skipped: CircleSlash,
  cancelled: Ban,
  unknown: HelpCircle,
};
const STATUS_TONE: Record<BulkItemStatus, string> = {
  succeeded: "text-ok",
  failed: "text-err",
  skipped: "text-ink-3",
  cancelled: "text-ink-3",
  unknown: "text-warn",
};

export function RunPanel({ envName }: { envName: (server: string) => string }) {
  const { t } = useTranslation();
  const s = useBulkRun();
  const [onlyProblems, setOnlyProblems] = useState(false);
  const setFilter = useActivity((x) => x.setFilter);
  const setActivityOpen = useApp((x) => x.setActivityOpen);
  if (s.phase === "idle" || !s.operation) return null;

  const total = s.targets.length;
  const done = doneCount(s.results);
  const finished = s.phase === "done";
  const counts = s.summary?.counts;
  const pct = total === 0 ? 0 : Math.round((done / total) * 100);
  const rows = s.targets.map((target, index) => ({ target, index, result: s.results[index] }));
  const visible = onlyProblems ? rows.filter((r) => r.result && r.result.status !== "succeeded") : rows;
  const tone = !counts ? "info" : counts.failed + counts.unknown > 0 ? "error" : counts.cancelled > 0 ? "warn" : "ok";

  return (
    <section aria-label={t("bulk.run.title")} data-testid="bulk-run" className="mb-4 rounded-xl border border-line bg-surface">
      <header className="flex flex-wrap items-center gap-3 border-b border-line px-4 py-3">
        <div className="min-w-0">
          <h2 className="text-[14px] font-semibold">{operationText(t, s.operation)}</h2>
          <p className="text-[12px] text-ink-3" aria-live="polite">
            {finished
              ? t("bulk.run.finished", { duration: formatDuration(s.summary?.durationMs ?? 0) })
              : s.cancelling
                ? t("bulk.run.cancelling")
                : t("bulk.run.progress", { done, total, concurrency: s.concurrency })}
          </p>
        </div>
        <div className="ml-auto flex items-center gap-2">
          {!finished && (
            <Button size="sm" onClick={() => void s.cancel()} disabled={s.cancelling || s.sid === null} title={t("bulk.run.cancelHint")}>
              {t("bulk.run.cancel")}
            </Button>
          )}
          {s.batch && (
            <Button
              size="sm"
              onClick={() => {
                setFilter({ batch: s.batch });
                setActivityOpen(true);
              }}
            >
              <ScrollText size={14} /> {t("bulk.run.activity")}
            </Button>
          )}
          {finished && (
            <Button size="sm" variant="ghost" onClick={s.dismiss} aria-label={t("bulk.run.close")} title={t("bulk.run.close")}>
              <X size={15} />
            </Button>
          )}
        </div>
      </header>

      <div className="space-y-3 px-4 py-3">
        <div className="h-1.5 overflow-hidden rounded-full bg-surface-sunk" role="progressbar" aria-valuemin={0} aria-valuemax={total} aria-valuenow={done} aria-label={t("bulk.run.title")}>
          <div className={cx("h-full transition-[width] duration-300", tone === "error" ? "bg-err" : tone === "warn" ? "bg-warn" : "bg-beacon")} style={{ width: `${pct}%` }} />
        </div>

        {counts && (
          <div data-testid="bulk-run-summary">
            <Banner tone={tone === "ok" ? "info" : tone}>
              {counts.failed + counts.unknown === 0 && counts.cancelled === 0
                ? t("bulk.run.allOk", { count: counts.succeeded, skipped: counts.skipped })
                : t("bulk.run.partial", { ok: counts.succeeded, failed: counts.failed, unknown: counts.unknown, skipped: counts.skipped, cancelled: counts.cancelled })}
              {counts.unknown > 0 && <span className="mt-1 block text-[12px]">{t("bulk.run.unknownHint")}</span>}
            </Banner>
          </div>
        )}

        {finished && (
          <label className="flex items-center gap-2 text-[12.5px] text-ink-2">
            <input type="checkbox" checked={onlyProblems} onChange={(e) => setOnlyProblems(e.target.checked)} className="h-4 w-4 accent-[var(--beacon)]" />
            {t("bulk.run.onlyProblems")}
          </label>
        )}

        <ul className="max-h-[280px] divide-y divide-line overflow-y-auto rounded-md border border-line" data-testid="bulk-run-items">
          {visible.map(({ target, index, result }) => (
            <ItemRow key={index} name={target.name} id={target.id} env={envName(target.server)} result={result} running={s.active.includes(index)} />
          ))}
        </ul>
      </div>
    </section>
  );
}

function ItemRow({ name, id, env, result, running }: { name: string; id: string; env: string; result: BulkItemResult | undefined; running: boolean }) {
  const { t } = useTranslation();
  const Icon = result ? STATUS_ICON[result.status] : null;
  const detail = !result
    ? running
      ? t("bulk.status.running")
      : t("bulk.status.pending")
    : result.status === "skipped" && result.skip
      ? skipText(t, result.skip)
      : result.error
        ? result.error.message
        : result.status === "succeeded"
          ? result.state
            ? t("bulk.run.nowState", { state: t(`containers.state.${result.state}`, { defaultValue: result.state }) })
            : t("bulk.run.removed")
          : "";
  return (
    <li data-testid="bulk-run-item" data-status={result?.status ?? (running ? "running" : "pending")} className="flex items-start gap-2 px-3 py-1.5 text-[12.5px]">
      <span className="mt-0.5 shrink-0">{Icon ? <Icon size={14} className={STATUS_TONE[result!.status]} /> : running ? <Spinner size={14} /> : <span className="block h-3.5 w-3.5 rounded-full border border-line-strong" />}</span>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate font-medium">{name}</span>
          <span className="shrink-0 font-mono text-[11px] text-ink-3">{shortId(id)}</span>
          <span className="shrink-0 rounded bg-surface-sunk px-1.5 text-[11px] text-ink-2">{env}</span>
          <span className={cx("ml-auto shrink-0 font-medium", result ? STATUS_TONE[result.status] : "text-ink-3")}>{result ? t(`bulk.status.${result.status}`) : ""}</span>
          {result && result.durationMs > 0 && <span className="shrink-0 tabular-nums text-ink-3">{formatDuration(result.durationMs)}</span>}
        </div>
        {detail && <p className={cx("selectable mt-0.5 [overflow-wrap:anywhere]", result?.status === "failed" ? "text-err" : "text-ink-3")}>{detail}</p>}
      </div>
    </li>
  );
}
