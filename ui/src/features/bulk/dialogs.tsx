// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * Diálogos de la gestión masiva (1.10.0): confirmación con vista previa de
 * los contenedores afectados, política de reinicio y etiquetas (solo
 * explicación: Docker no permite cambiarlas sin recrear el contenedor).
 */
import { useState, type ReactNode } from "react";
import { AlertTriangle, ArrowRight } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApp } from "@/app/store";
import { shortId } from "@/shared/format";
import type { BulkOperation } from "@/shared/ipc/bindings/BulkOperation";
import type { BulkRequest } from "@/shared/ipc/bindings/BulkRequest";
import type { RestartPolicyName } from "@/shared/ipc/bindings/RestartPolicyName";
import type { SkipReason } from "@/shared/ipc/bindings/SkipReason";
import { Button, cx, Segmented } from "@/shared/ui/controls";
import { Banner, Checkbox, Dialog } from "@/shared/ui/feedback";
import { STATE_DOT } from "@/features/containers/containerUi";
import { envsOf, plan, policyText, relevantLabels, targetsOf, validPolicy, type BulkRow, type OpKind } from "./model";

type T = (k: string, o?: Record<string, unknown>) => string;

export function skipText(t: T, r: SkipReason): string {
  switch (r.reason) {
    case "state":
      return t("bulk.skip.state", { state: t(`containers.state.${r.state}`, { defaultValue: r.state }) });
    case "needsForce":
      return t("bulk.skip.needsForce", { state: t(`containers.state.${r.state}`, { defaultValue: r.state }) });
    case "autoRemove":
      return t("bulk.skip.autoRemove");
    case "unchanged":
      return t("bulk.skip.unchanged");
    case "gone":
      return t("bulk.skip.gone");
  }
}

export function operationText(t: T, op: BulkOperation): string {
  if (op.kind === "restartPolicy") return t("bulk.op.restartPolicyTo", { policy: policyText(op.policy) });
  if (op.kind === "remove" && op.force) return t("bulk.op.removeForce");
  return t(`bulk.op.${op.kind}`);
}

const POLICIES: RestartPolicyName[] = ["no", "always", "unless-stopped", "on-failure"];

function RowLine({ r, extra }: { r: BulkRow; extra?: ReactNode }) {
  const { t } = useTranslation();
  return (
    <li className="flex items-center gap-2 px-3 py-1.5 text-[12.5px]">
      <span aria-hidden className={cx("h-2 w-2 shrink-0 rounded-full", STATE_DOT[r.c.state] ?? "bg-off")} />
      <span className="min-w-0 truncate font-medium" title={r.c.name}>
        {r.c.name}
      </span>
      <span className="selectable shrink-0 font-mono text-[11.5px] text-ink-3">{shortId(r.c.id)}</span>
      <span className="shrink-0 rounded bg-surface-sunk px-1.5 text-[11px] text-ink-2">{r.envName}</span>
      <span className="ml-auto min-w-0 truncate text-right text-[12px] text-ink-3">{extra ?? t(`containers.state.${r.c.state}`, { defaultValue: r.c.state })}</span>
    </li>
  );
}

/**
 * Confirmación de una operación masiva. Muestra qué se hará, en qué
 * servidores, a qué contenedores (y cuáles se omiten y por qué), las
 * consecuencias y el límite de operaciones a la vez. Eliminar pide además
 * escribir cuántos contenedores se eliminarán.
 */
export function ConfirmDialog({ kind, rows, onClose, onConfirm }: { kind: OpKind; rows: BulkRow[]; onClose: () => void; onConfirm: (req: BulkRequest) => void }) {
  const { t } = useTranslation();
  const concurrency = useApp((s) => s.config.bulk.concurrency);
  const updateConfig = useApp((s) => s.updateConfig);
  const [force, setForce] = useState(false);
  const [policyName, setPolicyName] = useState<RestartPolicyName>("unless-stopped");
  const [retries, setRetries] = useState("0");
  const [typed, setTyped] = useState("");
  // El backend lee el límite al empezar: no se confirma hasta que esté guardado.
  const [saving, setSaving] = useState(false);

  const retryCount = Number(retries);
  const policy = { name: policyName, maximumRetryCount: policyName === "on-failure" && /^\d+$/.test(retries) ? retryCount : 0 };
  const policyOk = policyName !== "on-failure" || (/^\d+$/.test(retries) && validPolicy({ name: policyName, maximumRetryCount: retryCount }));

  // Se recalcula en cada cambio (forzar, política): como mucho 1000 filas.
  const op: BulkOperation = kind === "remove" ? { kind, force } : kind === "restartPolicy" ? { kind, policy } : ({ kind } as BulkOperation);
  const p = plan(op, rows);
  const envs = envsOf(p.apply);
  const n = p.apply.length;
  const destructive = kind === "remove";
  const countOk = !destructive || typed.trim() === String(n);
  const remoteInvolved = envs.some((e) => e.remote);
  const canRun = n > 0 && countOk && !saving && (kind !== "restartPolicy" || policyOk);

  const submit = () => {
    if (!canRun) return;
    onConfirm({ operation: op, targets: targetsOf(p.apply), confirmed: true, confirmCount: destructive ? n : null });
  };

  return (
    <Dialog
      title={t("bulk.confirm.title", { op: operationText(t, op), count: n })}
      onClose={onClose}
      wide
      target={false}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant={destructive ? "danger" : "primary"} disabled={!canRun} onClick={submit} data-testid="bulk-confirm-run">
            {t("bulk.confirm.run", { count: n })}
          </Button>
        </>
      }
    >
      {kind === "restartPolicy" && (
        <fieldset className="space-y-2">
          <legend className="mb-1 text-[12.5px] font-medium">{t("bulk.policy.new")}</legend>
          <Segmented<RestartPolicyName> label={t("bulk.policy.new")} value={policyName} onChange={setPolicyName} options={POLICIES.map((v) => ({ value: v, label: t(`bulk.policy.name.${v}`) }))} />
          <p className="text-[12px] text-ink-3">{t(`bulk.policy.hint.${policyName}`)}</p>
          {policyName === "on-failure" && (
            <label className="flex items-center gap-2 text-[12.5px]">
              {t("bulk.policy.retries")}
              <input
                type="number"
                min={0}
                max={1000}
                value={retries}
                onChange={(e) => setRetries(e.target.value)}
                aria-invalid={!policyOk}
                className="h-7 w-24 rounded-md border border-line bg-surface px-2 text-[13px] outline-none focus:border-beacon aria-[invalid=true]:border-err"
              />
              <span className="text-ink-3">{t("bulk.policy.retriesHint")}</span>
            </label>
          )}
          <p className="text-[12px] text-ink-3">{t("bulk.policy.live")}</p>
        </fieldset>
      )}

      <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-[12.5px]">
        <dt className="text-ink-3">{t("bulk.confirm.operation")}</dt>
        <dd className="font-medium">{operationText(t, op)}</dd>
        <dt className="text-ink-3">{t("bulk.confirm.containers")}</dt>
        <dd>{t("bulk.confirm.applyCount", { count: n })}</dd>
        <dt className="text-ink-3">{t("bulk.confirm.environments")}</dt>
        <dd data-testid="bulk-confirm-envs">{envs.length === 0 ? "—" : envs.map((e) => `${e.name} (${e.count})`).join(" · ")}</dd>
      </dl>

      <p className="text-[12.5px] text-ink-2">{t(`bulk.consequence.${kind}`)}</p>
      {remoteInvolved && <Banner tone="info">{t("bulk.confirm.remote")}</Banner>}

      {n > 0 && (
        <section>
          <h3 className="mb-1 text-[12.5px] font-medium">{t("bulk.confirm.affected", { count: n })}</h3>
          <ul data-testid="bulk-confirm-list" className="max-h-[200px] divide-y divide-line overflow-y-auto rounded-md border border-line">
            {p.apply.map((r) => (
              <RowLine
                key={r.key}
                r={r}
                extra={
                  kind === "restartPolicy" ? (
                    <span className="inline-flex items-center gap-1 font-mono">
                      {r.c.details ? policyText(r.c.details.restartPolicy) : t("bulk.policy.unknown")} <ArrowRight size={12} /> {policyText(policy)}
                    </span>
                  ) : undefined
                }
              />
            ))}
          </ul>
        </section>
      )}
      {p.skip.length > 0 && (
        <section>
          <h3 className="mb-1 text-[12.5px] font-medium">{t("bulk.confirm.skipped", { count: p.skip.length })}</h3>
          <ul data-testid="bulk-confirm-skipped" className="max-h-[140px] divide-y divide-line overflow-y-auto rounded-md border border-line bg-surface-2">
            {p.skip.map(({ row, reason }) => (
              <RowLine key={row.key} r={row} extra={skipText(t, reason)} />
            ))}
          </ul>
        </section>
      )}
      {n === 0 && <Banner tone="warn">{t("bulk.confirm.nothing")}</Banner>}

      {destructive && (
        <div className="space-y-2.5 rounded-lg border border-err/30 bg-err/5 p-3">
          <p className="flex items-start gap-2 text-[12.5px]">
            <AlertTriangle size={15} className="mt-0.5 shrink-0 text-err" />
            {t("bulk.remove.risk")}
          </p>
          <Checkbox checked={force} onChange={setForce} label={t("bulk.remove.force")} hint={t("bulk.remove.forceHint")} />
          {n > 0 && (
            <label className="block text-[12.5px]">
              {t("bulk.remove.type", { count: n })}
              <input
                value={typed}
                onChange={(e) => setTyped(e.target.value)}
                inputMode="numeric"
                aria-label={t("bulk.remove.typeLabel")}
                className="mt-1 h-8 w-28 rounded-md border border-line bg-surface px-2 font-mono text-[13px] outline-none focus:border-err"
              />
            </label>
          )}
        </div>
      )}

      <label className="flex items-center gap-2 text-[12.5px] text-ink-2">
        {t("bulk.confirm.concurrency")}
        <select
          value={concurrency}
          aria-label={t("bulk.confirm.concurrency")}
          disabled={saving}
          onChange={(e) => {
            setSaving(true);
            void updateConfig({ bulk: { concurrency: Number(e.target.value) } }).finally(() => setSaving(false));
          }}
          className="h-7 rounded-md border border-line bg-surface px-1.5 text-[12.5px]"
        >
          {Array.from({ length: 16 }, (_, i) => i + 1).map((v) => (
            <option key={v} value={v}>
              {v}
            </option>
          ))}
        </select>
      </label>
      <p className="text-[12px] text-ink-3">{t("bulk.confirm.partial")}</p>
    </Dialog>
  );
}

/** Etiquetas: Docker no permite cambiarlas en un contenedor que ya existe. */
export function LabelsDialog({ rows, onClose }: { rows: BulkRow[]; onClose: () => void }) {
  const { t } = useTranslation();
  const shown = rows.slice(0, 50);
  return (
    <Dialog title={t("bulk.labels.title")} onClose={onClose} wide target={false} footer={<Button onClick={onClose}>{t("common.close")}</Button>}>
      <Banner tone="info">{t("bulk.labels.limit")}</Banner>
      <p className="text-[12.5px] text-ink-2">{t("bulk.labels.future")}</p>
      <h3 className="text-[12.5px] font-medium">{t("bulk.labels.current", { count: rows.length })}</h3>
      <ul className="max-h-[260px] divide-y divide-line overflow-y-auto rounded-md border border-line">
        {shown.map((r) => {
          const labels = relevantLabels(r.c.labels);
          return (
            <li key={r.key} className="px-3 py-1.5 text-[12.5px]">
              <div className="flex items-center gap-2">
                <span className="truncate font-medium">{r.c.name}</span>
                <span className="rounded bg-surface-sunk px-1.5 text-[11px] text-ink-2">{r.envName}</span>
              </div>
              <div className="selectable mt-0.5 font-mono text-[11.5px] text-ink-3 [overflow-wrap:anywhere]">
                {labels.length === 0 ? t("bulk.labels.none") : labels.map(([k, v]) => `${k}=${v}`).join("  ")}
              </div>
            </li>
          );
        })}
      </ul>
      {rows.length > shown.length && <p className="text-[12px] text-ink-3">{t("bulk.labels.more", { count: rows.length - shown.length })}</p>}
    </Dialog>
  );
}
