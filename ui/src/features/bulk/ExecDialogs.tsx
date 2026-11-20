// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * Confirmación de un comando masivo (1.11.0): el comando exacto, el usuario,
 * el directorio, el tiempo máximo, la concurrencia, los servidores, en qué
 * contenedores se ejecutará y en cuáles no, el riesgo y sus advertencias.
 * Desde el nivel «administrativo» hay que escribir el número de contenedores
 * (el backend también lo comprueba).
 */
import { useState } from "react";
import { AlertTriangle } from "lucide-react";
import { useTranslation } from "react-i18next";
import { shortId } from "@/shared/format";
import type { BulkCommand } from "@/shared/ipc/bindings/BulkCommand";
import type { BulkExecRequest } from "@/shared/ipc/bindings/BulkExecRequest";
import type { ExecPreview } from "@/shared/ipc/bindings/ExecPreview";
import { Button, cx } from "@/shared/ui/controls";
import { Banner, Dialog } from "@/shared/ui/feedback";
import { envsOf, targetsOf, type BulkRow } from "./model";

export function ExecConfirmDialog({
  preview,
  command,
  run,
  skip,
  concurrency,
  onClose,
  onConfirm,
}: {
  preview: ExecPreview;
  command: BulkCommand;
  run: BulkRow[];
  skip: BulkRow[];
  concurrency: number;
  onClose: () => void;
  onConfirm: (req: BulkExecRequest) => void;
}) {
  const { t } = useTranslation();
  const [typed, setTyped] = useState("");
  const n = run.length;
  const countOk = !preview.needsCount || typed.trim() === String(n);
  const envs = envsOf(run);
  const risky = preview.risk.level === "administrative" || preview.risk.level === "destructive";

  return (
    <Dialog
      title={t("bulk.exec.confirmTitle", { count: n })}
      onClose={onClose}
      wide
      target={false}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button
            variant={risky ? "danger" : "primary"}
            disabled={n === 0 || !countOk}
            data-testid="exec-confirm-run"
            onClick={() => onConfirm({ command, targets: targetsOf(run), concurrency, confirmed: true, confirmCount: preview.needsCount ? n : null })}
          >
            {t("bulk.exec.confirmRun", { count: n })}
          </Button>
        </>
      }
    >
      <div>
        <div className="text-[12px] text-ink-3">{t("bulk.exec.equivalent")}</div>
        <code className="selectable block rounded-md bg-surface-sunk px-2 py-1.5 font-mono text-[12px] [overflow-wrap:anywhere] whitespace-pre-wrap" data-testid="exec-confirm-command">
          {preview.equivalent}
        </code>
      </div>
      <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-[12.5px]">
        <dt className="text-ink-3">{t("bulk.exec.user")}</dt>
        <dd>{command.user ?? t("bulk.exec.imageDefault")}</dd>
        <dt className="text-ink-3">{t("bulk.exec.workdir")}</dt>
        <dd>{command.workdir ?? t("bulk.exec.imageDefault")}</dd>
        <dt className="text-ink-3">{t("bulk.exec.timeout")}</dt>
        <dd>{t("bulk.exec.seconds", { count: command.timeoutSecs })}</dd>
        <dt className="text-ink-3">{t("bulk.exec.concurrency")}</dt>
        <dd>{concurrency}</dd>
        <dt className="text-ink-3">{t("bulk.confirm.environments")}</dt>
        <dd data-testid="exec-confirm-envs">{envs.map((e) => `${e.name} (${e.count})`).join(" · ") || "—"}</dd>
      </dl>

      <RiskBadge p={preview} />
      {risky && (
        <p className="flex items-start gap-2 rounded-lg border border-err/30 bg-err/5 p-3 text-[12.5px]">
          <AlertTriangle size={15} className="mt-0.5 shrink-0 text-err" />
          {t("bulk.exec.riskyWarning")}
        </p>
      )}
      {envs.some((e) => e.remote) && <Banner tone="info">{t("bulk.confirm.remote")}</Banner>}

      <section>
        <h3 className="mb-1 text-[12.5px] font-medium">{t("bulk.exec.willRun", { count: n })}</h3>
        <ul className="max-h-[180px] divide-y divide-line overflow-y-auto rounded-md border border-line" data-testid="exec-confirm-list">
          {run.map((r) => (
            <li key={r.key} className="flex items-center gap-2 px-3 py-1.5 text-[12.5px]">
              <span className="truncate font-medium">{r.c.name}</span>
              <span className="font-mono text-[11px] text-ink-3">{shortId(r.c.id)}</span>
              <span className="min-w-0 truncate font-mono text-[11px] text-ink-3">{r.c.image}</span>
              <span className="ml-auto shrink-0 rounded bg-surface-sunk px-1.5 text-[11px] text-ink-2">{r.envName}</span>
            </li>
          ))}
        </ul>
      </section>
      {skip.length > 0 && (
        <section>
          <h3 className="mb-1 text-[12.5px] font-medium">{t("bulk.exec.willNotRunTitle", { count: skip.length })}</h3>
          <ul className="max-h-[120px] divide-y divide-line overflow-y-auto rounded-md border border-line bg-surface-2" data-testid="exec-confirm-skipped">
            {skip.map((r) => (
              <li key={r.key} className="flex items-center gap-2 px-3 py-1.5 text-[12.5px]">
                <span className="truncate font-medium">{r.c.name}</span>
                <span className="shrink-0 rounded bg-surface-sunk px-1.5 text-[11px] text-ink-2">{r.envName}</span>
                <span className="ml-auto text-ink-3">{t("bulk.exec.willNotRun", { state: t(`containers.state.${r.c.state}`, { defaultValue: r.c.state }) })}</span>
              </li>
            ))}
          </ul>
        </section>
      )}
      <p className="text-[12px] text-ink-3">{t("bulk.exec.confirmNote")}</p>
      {preview.needsCount && (
        <label className="block text-[12.5px]">
          {t("bulk.exec.typeCount", { count: n })}
          <input
            value={typed}
            onChange={(e) => setTyped(e.target.value)}
            inputMode="numeric"
            aria-label={t("bulk.exec.typeCountLabel")}
            className="mt-1 block h-8 w-28 rounded-md border border-line bg-surface px-2 font-mono text-[13px] outline-none focus:border-err"
          />
        </label>
      )}
    </Dialog>
  );
}

export const RISK_TONE: Record<string, string> = {
  informative: "bg-surface-sunk text-ink-2",
  modification: "bg-warn/15 text-warn",
  administrative: "bg-warn/25 text-warn",
  destructive: "bg-err/15 text-err",
};

export function RiskBadge({ p }: { p: ExecPreview }) {
  const { t } = useTranslation();
  return (
    <div className="space-y-1" data-testid="exec-risk">
      <span className={cx("inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[11.5px] font-semibold", RISK_TONE[p.risk.level])}>
        {p.risk.level !== "informative" && <AlertTriangle size={12} />}
        {t(`bulk.exec.risk.${p.risk.level}`)}
      </span>
      {p.risk.reasons.length > 0 && (
        <ul className="list-inside list-disc text-[12px] text-ink-2">
          {p.risk.reasons.map((r) => (
            <li key={r}>{t(`bulk.exec.reason.${r}`, { defaultValue: r })}</li>
          ))}
        </ul>
      )}
      {p.risk.exposesSecrets && <p className="text-[12px] text-warn">{t("bulk.exec.secretsWarning")}</p>}
      <p className="text-[11.5px] text-ink-3">{t("bulk.exec.riskNote")}</p>
    </div>
  );
}
