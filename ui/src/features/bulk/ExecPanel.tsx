// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * Resultados de un comando masivo (1.11.0): resumen global, progreso,
 * cancelación de lo pendiente, interrupción de lo activo, exportación y, por
 * contenedor, estado, código de salida, duración, salida estándar y de error
 * (que se pueden copiar) y el error técnico.
 *
 * La salida se muestra tal cual, como en las terminales (D-085): son datos del
 * usuario. La exportación y el registro sí ocultan los secretos.
 */
import { useState } from "react";
import { ChevronDown, ChevronRight, Download, ScrollText, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApp } from "@/app/store";
import { useActivity } from "@/features/activity/store";
import { formatDuration } from "@/features/activity/logic";
import { shortId } from "@/shared/format";
import type { ExecItemResult } from "@/shared/ipc/bindings/ExecItemResult";
import { api } from "@/shared/ipc/client";
import { Button, CopyButton, cx } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { skipText } from "./dialogs";
import { exportJson, exportMarkdown, shownStatus, type ShownStatus } from "./execModel";
import { useBulkExec } from "./execStore";

const TONE: Record<ShownStatus, string> = {
  pending: "text-ink-3",
  running: "text-beacon-strong",
  succeeded: "text-ok",
  failed: "text-err",
  timedOut: "text-warn",
  interrupted: "text-warn",
  notRun: "text-ink-3",
  unknown: "text-warn",
};

export function ExecPanel() {
  const { t, i18n } = useTranslation();
  const s = useBulkExec();
  const setFilter = useActivity((x) => x.setFilter);
  const setActivityOpen = useApp((x) => x.setActivityOpen);
  const [onlyProblems, setOnlyProblems] = useState(false);
  const [open, setOpen] = useState<number | null>(null);
  const [exportMsg, setExportMsg] = useState<{ tone: "info" | "error"; text: string } | null>(null);
  if (s.phase === "idle" || !s.command) return null;

  const total = s.targets.length;
  const done = s.results.filter(Boolean).length;
  const finished = s.phase === "done";
  const counts = s.summary?.counts;
  const items = s.targets.map((target, index) => ({ target, index, result: s.results[index], running: s.active.includes(index) }));
  const pending = items.filter((x) => !x.result && !x.running).length;
  const visible = onlyProblems ? items.filter((x) => x.result && x.result.status !== "succeeded") : items;
  const problems = counts ? counts.failed + counts.timedOut + counts.interrupted + counts.unknown + counts.notRun : 0;

  const meta = () => ({
    rows: s.rows,
    command: s.commandText,
    user: s.command?.user ?? null,
    workdir: s.command?.workdir ?? null,
    timeoutSecs: s.command?.timeoutSecs ?? 0,
    batch: s.batch,
    startedAt: s.summary?.startedAt ?? null,
    durationMs: s.summary?.durationMs ?? null,
  });
  const doExport = async (format: "markdown" | "json") => {
    setExportMsg(null);
    const results = s.results.filter((r): r is ExecItemResult => !!r);
    const content = format === "markdown" ? exportMarkdown(t, results, meta()) : exportJson(results, meta());
    try {
      // Se reutiliza el guardado de informes (E3): pide la ruta, oculta los
      // secretos otra vez, escribe con permisos 0600 y de forma atómica.
      const path = await api.auditExportSave(format, content, `comando-masivo-${(s.batch ?? "").slice(0, 8)}`);
      if (path) setExportMsg({ tone: "info", text: t("bulk.exec.exported", { path }) });
    } catch (e) {
      setExportMsg({ tone: "error", text: (e as { message: string }).message });
    }
  };

  return (
    <section aria-label={t("bulk.exec.results")} data-testid="exec-run" className="rounded-xl border border-line bg-surface">
      <header className="flex flex-wrap items-center gap-3 border-b border-line px-4 py-3">
        <div className="min-w-0">
          <h2 className="text-[14px] font-semibold">{t("bulk.exec.results")}</h2>
          <code className="block max-h-[4.5em] overflow-hidden font-mono text-[12px] whitespace-pre-wrap text-ink-2 [overflow-wrap:anywhere]" title={s.commandText}>
            {s.commandText}
          </code>
          <p className="text-[12px] text-ink-3" aria-live="polite">
            {finished
              ? t("bulk.run.finished", { duration: formatDuration(s.summary?.durationMs ?? 0) })
              : s.interrupting
                ? t("bulk.exec.interrupting")
                : s.cancelling
                  ? t("bulk.exec.cancelling")
                  : t("bulk.exec.progress", { done, total, active: s.active.length, pending, concurrency: s.concurrency })}
          </p>
        </div>
        <div className="ml-auto flex flex-wrap items-center gap-2">
          {!finished && (
            <>
              <Button size="sm" onClick={() => void s.cancelPending()} disabled={s.cancelling || s.sid === null} title={t("bulk.exec.cancelPendingHint")}>
                {t("bulk.exec.cancelPending")}
              </Button>
              <Button size="sm" onClick={() => void s.interrupt()} disabled={s.interrupting || s.sid === null} title={t("bulk.exec.interruptHint")}>
                {t("bulk.exec.interrupt")}
              </Button>
            </>
          )}
          {finished && (
            <>
              <Button size="sm" onClick={() => void doExport("markdown")}>
                <Download size={14} /> {t("bulk.exec.exportMd")}
              </Button>
              <Button size="sm" onClick={() => void doExport("json")}>
                <Download size={14} /> {t("bulk.exec.exportJson")}
              </Button>
            </>
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
        <div className="h-1.5 overflow-hidden rounded-full bg-surface-sunk" role="progressbar" aria-valuemin={0} aria-valuemax={total} aria-valuenow={done} aria-label={t("bulk.exec.results")}>
          <div className={cx("h-full transition-[width] duration-300", problems > 0 ? "bg-warn" : "bg-beacon")} style={{ width: `${total ? Math.round((done / total) * 100) : 0}%` }} />
        </div>
        {!finished && <p className="text-[12px] text-ink-3">{t("bulk.exec.interruptNote")}</p>}
        {counts && (
          <div data-testid="exec-summary">
            <Banner tone={problems === 0 ? "info" : counts.succeeded === 0 ? "error" : "warn"}>
              {t("bulk.exec.summary", {
                ok: counts.succeeded,
                failed: counts.failed,
                timedOut: counts.timedOut,
                interrupted: counts.interrupted,
                unknown: counts.unknown,
                notRun: counts.notRun,
              })}
              {counts.unknown > 0 && <span className="mt-1 block text-[12px]">{t("bulk.exec.unknownHint")}</span>}
            </Banner>
          </div>
        )}
        {exportMsg && <Banner tone={exportMsg.tone}>{exportMsg.text}</Banner>}
        {finished && (
          <label className="flex items-center gap-2 text-[12.5px] text-ink-2">
            <input type="checkbox" checked={onlyProblems} onChange={(e) => setOnlyProblems(e.target.checked)} className="h-4 w-4 accent-[var(--beacon)]" />
            {t("bulk.run.onlyProblems")}
          </label>
        )}
        <ul className="divide-y divide-line rounded-md border border-line" data-testid="exec-items">
          {visible.map(({ target, index, result, running }) => {
            const st = shownStatus(result, running);
            const row = s.rows[index];
            const expanded = open === index;
            return (
              <li key={index} data-testid="exec-item" data-status={st}>
                <button
                  type="button"
                  onClick={() => setOpen(expanded ? null : index)}
                  aria-expanded={expanded}
                  className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-[12.5px] hover:bg-surface-2"
                >
                  {expanded ? <ChevronDown size={14} className="shrink-0" /> : <ChevronRight size={14} className="shrink-0" />}
                  <span className="truncate font-medium">{target.name}</span>
                  <span className="shrink-0 font-mono text-[11px] text-ink-3">{shortId(target.id)}</span>
                  {row && <span className="shrink-0 rounded bg-surface-sunk px-1.5 text-[11px] text-ink-2">{row.envName}</span>}
                  <span className={cx("ml-auto flex shrink-0 items-center gap-1 font-medium", TONE[st])}>
                    {st === "running" && <Spinner size={12} />}
                    {t(`bulk.exec.status.${st}`)}
                  </span>
                  {result?.output?.exitCode !== null && result?.output?.exitCode !== undefined && (
                    <span className="shrink-0 font-mono text-[11.5px] text-ink-2">{t("bulk.exec.exitShort", { code: result.output.exitCode })}</span>
                  )}
                  {result && result.durationMs > 0 && <span className="shrink-0 tabular-nums text-ink-3">{formatDuration(result.durationMs)}</span>}
                </button>
                {expanded && <ItemDetail result={result} index={index} name={target.name} id={target.id} image={row?.image ?? ""} env={row?.envName ?? target.server} user={s.command?.user ?? null} command={s.commandText} lang={i18n.language} />}
              </li>
            );
          })}
        </ul>
      </div>
    </section>
  );
}

function ItemDetail({ result, name, id, image, env, user, command, lang }: { result: ExecItemResult | undefined; index: number; name: string; id: string; image: string; env: string; user: string | null; command: string; lang: string }) {
  const { t } = useTranslation();
  const o = result?.output;
  return (
    <div className="space-y-2 border-t border-line bg-surface-2 px-4 py-3 text-[12.5px]" data-testid="exec-detail">
      <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-0.5">
        <dt className="text-ink-3">{t("bulk.col.name")}</dt>
        <dd>{name}</dd>
        <dt className="text-ink-3">ID</dt>
        <dd className="selectable font-mono text-[11.5px] [overflow-wrap:anywhere]">{id}</dd>
        <dt className="text-ink-3">{t("bulk.col.image")}</dt>
        <dd className="font-mono text-[11.5px]">{image}</dd>
        <dt className="text-ink-3">{t("bulk.col.env")}</dt>
        <dd>{env}</dd>
        <dt className="text-ink-3">{t("bulk.exec.export.command")}</dt>
        <dd className="selectable font-mono text-[11.5px] whitespace-pre-wrap [overflow-wrap:anywhere]">{command}</dd>
        <dt className="text-ink-3">{t("bulk.exec.user")}</dt>
        <dd>{user ?? t("bulk.exec.imageDefault")}</dd>
        {o && (
          <>
            <dt className="text-ink-3">{t("bulk.exec.export.started")}</dt>
            <dd>{new Date(o.startedAt).toLocaleString(lang)}</dd>
          </>
        )}
        {result && (
          <>
            <dt className="text-ink-3">{t("bulk.exec.export.duration")}</dt>
            <dd>{formatDuration(result.durationMs)}</dd>
          </>
        )}
        <dt className="text-ink-3">{t("bulk.exec.exitCode")}</dt>
        <dd>{o?.exitCode ?? "—"}</dd>
      </dl>
      {!result && <p className="text-ink-3">{t("bulk.exec.noResultYet")}</p>}
      {result?.cancelled && <p className="text-ink-3">{t("bulk.exec.cancelledBefore")}</p>}
      {result?.skip && <p className="text-ink-3">{skipText(t, result.skip)}</p>}
      {o?.launchFailed && <p className="text-err">{t("bulk.exec.launchFailed")}</p>}
      {o && o.end !== "exited" && (
        <p className="text-warn">
          {t(o.end === "timedOut" ? "bulk.exec.timedOutNote" : "bulk.exec.interruptedNote")}{" "}
          {o.stillRunning === true ? t("bulk.exec.stillRunning") : o.stillRunning === false ? t("bulk.exec.notRunning") : t("bulk.exec.runningUnknown")}
        </p>
      )}
      {result?.error && (
        <div>
          <div className="text-ink-3">{t("bulk.exec.error")}</div>
          <p className="selectable text-err [overflow-wrap:anywhere]">{result.error.message}</p>
        </div>
      )}
      {o && (
        <>
          <Output label="stdout" text={o.stdout} truncated={o.stdoutTruncated} />
          <Output label="stderr" text={o.stderr} truncated={o.stderrTruncated} />
        </>
      )}
    </div>
  );
}

function Output({ label, text, truncated }: { label: string; text: string; truncated: boolean }) {
  const { t } = useTranslation();
  return (
    <div>
      <div className="mb-1 flex items-center gap-2">
        <span className="font-mono text-[11.5px] font-semibold">{label}</span>
        {truncated && <span className="text-[11.5px] text-warn">{t("bulk.exec.truncated")}</span>}
        {text && <CopyButton text={text} compact />}
      </div>
      {text ? (
        <pre className="selectable max-h-[260px] overflow-auto rounded-md border border-line bg-surface px-2 py-1.5 font-mono text-[12px] whitespace-pre-wrap [overflow-wrap:anywhere]" data-testid={`exec-${label}`}>
          {text}
        </pre>
      ) : (
        <p className="text-[12px] text-ink-3">{t("bulk.exec.empty")}</p>
      )}
    </div>
  );
}
