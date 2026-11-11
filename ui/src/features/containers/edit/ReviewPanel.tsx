/**
 * Revisión antes de aplicar (2.4.3) y asistente de recreación segura (2.4.2).
 * La comparación la hace el backend con el estado real del contenedor; aquí
 * se muestra y se recoge la confirmación expresa del usuario.
 */
import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, CheckCircle2, CircleAlert, CircleCheck, CircleX, RefreshCcw, ShieldCheck } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api } from "@/shared/ipc/client";
import type { Change } from "@/shared/ipc/bindings/Change";
import type { EditableConfig } from "@/shared/ipc/bindings/EditableConfig";
import type { EditOutcome } from "@/shared/ipc/bindings/EditOutcome";
import type { EditPlan } from "@/shared/ipc/bindings/EditPlan";
import type { EditRequest } from "@/shared/ipc/bindings/EditRequest";
import type { Risk } from "@/shared/ipc/bindings/Risk";
import type { SpecIssue } from "@/shared/ipc/bindings/SpecIssue";
import { keys } from "@/shared/query";
import { formatBytes } from "@/shared/format";
import { Button, CopyButton, cx } from "@/shared/ui/controls";
import { Banner, Checkbox, Spinner } from "@/shared/ui/feedback";
import { IssueList } from "../create/fields";
import { useContainersNav } from "../store";
import { OutcomeBanner, editKey } from "./shared";

export function ReviewPanel({ cfg, request, onBack }: { cfg: EditableConfig; request: EditRequest; onBack: (issues?: SpecIssue[]) => void }) {
  const { t } = useTranslation();
  const qc = useQueryClient();
  // Una sola comparación al abrir la revisión (cada consulta queda en la actividad).
  const plan = useQuery({ queryKey: ["editPlan", cfg.id, JSON.stringify(request)], queryFn: () => api.containerEditPlan(cfg.id, request), gcTime: 0, staleTime: Infinity, refetchOnWindowFocus: false });
  const [accepted, setAccepted] = useState<string[]>([]);
  const [keepMac, setKeepMac] = useState(false);
  const [confirmed, setConfirmed] = useState(false);
  const apply = useMutation({
    mutationFn: (p: EditPlan) => api.containerEditApply(cfg.id, request, { confirmRecreate: p.needsRecreate && confirmed, acceptedRisks: accepted, keepMac }),
    onSettled: () => {
      void qc.invalidateQueries({ queryKey: keys.containers });
      void qc.invalidateQueries({ queryKey: keys.overview });
      void qc.invalidateQueries({ queryKey: keys.networks });
      void qc.invalidateQueries({ queryKey: ["container"] });
    },
  });

  if (apply.data) {
    const out = apply.data;
    return (
      <Result
        out={out}
        plan={plan.data!}
        onRetry={() => {
          // Tras recrear, el contenedor es otro: se edita el nuevo, nunca la copia.
          void qc.invalidateQueries({ queryKey: editKey(out.containerId) });
          if (out.containerId !== cfg.id) useContainersNav.getState().showEdit(out.containerId, out.name);
          else onBack();
        }}
      />
    );
  }

  if (plan.isPending) {
    return (
      <div className="flex items-center justify-center gap-2 py-20 text-[13px] text-ink-3">
        <Spinner /> {t("edit.comparing")}
      </div>
    );
  }
  if (plan.error || !plan.data) {
    return (
      <div className="space-y-3 px-8 py-6">
        <Banner tone="error">{t("edit.planError", { message: (plan.error as { message: string } | null)?.message ?? "" })}</Banner>
        <Button onClick={() => onBack()}>
          <ArrowLeft size={14} />
          {t("edit.backToEdit")}
        </Button>
      </div>
    );
  }

  const p = plan.data;
  const errors = p.issues.filter((i) => i.severity === "error");
  const risksOk = p.risks.every((r) => accepted.includes(r.code));
  const canApply = errors.length === 0 && p.changes.length > 0 && (!p.needsRecreate || (p.blockers.length === 0 && risksOk && confirmed));
  const macChoice = p.notices.some((n) => n.code === "macChoice");

  return (
    <div className="mx-auto w-full max-w-[920px] space-y-6 px-8 py-6">
      <div className="flex flex-wrap items-center gap-3">
        <h3 className="text-[15px] font-semibold">{t("edit.reviewTitle")}</h3>
        <span className={cx("rounded-md px-2 py-0.5 text-[12px] font-medium", p.needsRecreate ? "bg-warn/15 text-warn" : "bg-beacon-soft text-beacon-strong")}>
          {p.needsRecreate ? t("edit.needsRecreate") : t("edit.directOnly")}
        </span>
      </div>

      {errors.length > 0 && (
        <Banner tone="error">
          <p>{t("edit.hasErrors")}</p>
          <IssueList issues={p.issues} />
        </Banner>
      )}

      <Comparison plan={p} />

      {p.needsRecreate && (
        <section className="space-y-3 rounded-xl border border-line p-4" aria-labelledby="recreate-title">
          <h3 id="recreate-title" className="flex items-center gap-2 text-[14px] font-semibold">
            <ShieldCheck size={16} className="text-beacon-strong" aria-hidden />
            {t("edit.recreateTitle")}
          </h3>
          <ol className="list-decimal space-y-1 pl-5 text-[13px] text-ink-2">
            {p.willStop && <li>{t("edit.plan.stop")}</li>}
            <li>{t("edit.plan.backup", { backup: p.backupName ?? "" })}</li>
            <li>{t("edit.plan.create", { image: p.imageRef ?? "?" })}</li>
            <li>{p.willStop ? t("edit.plan.startVerify") : t("edit.plan.verifyStopped")}</li>
            <li>{t("edit.plan.rollback")}</li>
          </ol>
          <p className="text-[12.5px] text-ink-3">{t("edit.dataPromise")}</p>
          {p.willStop && <Banner tone="warn">{t("edit.interruption")}</Banner>}
          {p.blockers.map((b) => (
            <Banner key={b.code} tone="error">
              {t(`edit.blocker.${b.code}`, riskParams(b, t))}
            </Banner>
          ))}
          {p.blockers.length === 0 && p.risks.length > 0 && (
            <div className="space-y-2">
              <p className="text-[13px] font-medium">{t("edit.risksTitle")}</p>
              {p.risks.map((r) => (
                <div key={r.code} className="rounded-md border border-warn/30 bg-warn/8 px-3 py-2">
                  <Checkbox
                    checked={accepted.includes(r.code)}
                    onChange={(v) => setAccepted((a) => (v ? [...a, r.code] : a.filter((x) => x !== r.code)))}
                    label={t(`edit.risk.${r.code}`, riskParams(r, t))}
                    hint={t(`edit.riskHint.${r.code}`, { defaultValue: "" }) || undefined}
                  />
                </div>
              ))}
            </div>
          )}
          {p.notices.map((n) =>
            n.code === "macChoice" ? null : (
              <Banner key={n.code} tone="info">
                {t(`edit.notice.${n.code}`)}
              </Banner>
            ),
          )}
          {macChoice && <Checkbox checked={keepMac} onChange={setKeepMac} label={t("edit.keepMac")} hint={t("edit.keepMacHint")} />}
          {p.createBody && (
            <details className="rounded-md border border-line bg-surface-2 px-3 py-2 text-[12.5px]">
              <summary className="cursor-pointer text-ink-2">{t("edit.finalConfig")}</summary>
              <div className="mt-2 flex justify-end">
                <CopyButton text={p.createBody} />
              </div>
              <pre className="selectable mt-1 max-h-[320px] overflow-auto rounded bg-code p-2 font-mono text-[11.5px]">{p.createBody}</pre>
            </details>
          )}
          {p.blockers.length === 0 && (
            <div className="rounded-md border border-line bg-surface-2 px-3 py-2.5">
              <Checkbox checked={confirmed} onChange={setConfirmed} label={t("edit.confirmRecreate", { name: p.name })} />
            </div>
          )}
        </section>
      )}

      {p.equivalent.length > 0 && (
        <div className="space-y-1">
          <p className="text-[12.5px] font-medium text-ink-2">{t("edit.operations")}</p>
          <ul className="space-y-0.5">
            {p.equivalent.map((c) => (
              <li key={c}>
                <code className="selectable font-mono text-[12px] text-ink-2">{c}</code>
              </li>
            ))}
          </ul>
        </div>
      )}

      {apply.error && <Banner tone="error">{t("edit.failed", { message: (apply.error as { message: string }).message })}</Banner>}

      <div className="flex flex-wrap items-center gap-2 border-t border-line pt-4">
        <Button onClick={() => onBack(errors.length ? errors : undefined)} disabled={apply.isPending}>
          <ArrowLeft size={14} />
          {t("edit.backToEdit")}
        </Button>
        <span className="flex-1" />
        {apply.isPending && <span className="text-[12.5px] text-ink-3">{p.needsRecreate ? t("edit.recreating") : t("edit.applying")}</span>}
        <Button
          variant="primary"
          loading={apply.isPending}
          disabled={!canApply}
          title={!canApply ? t(p.blockers.length ? "edit.blockedHint" : errors.length ? "edit.fixFirst" : "edit.confirmHint") : undefined}
          onClick={() => apply.mutate(p)}
        >
          {!apply.isPending && (p.needsRecreate ? <RefreshCcw size={14} /> : <CheckCircle2 size={14} />)}
          {p.needsRecreate ? t("edit.recreate") : t("edit.apply")}
        </Button>
      </div>
    </div>
  );
}

function riskParams(r: Risk, t: (k: string) => string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(r.params)) if (v !== undefined) out[k] = v;
  if (r.params.bytes) out.size = formatBytes(Number(r.params.bytes), document.documentElement.lang || "es");
  if (r.params.status) out.status = t(`containers.state.${r.params.status}`);
  return out;
}

/** Actual / nuevo / tipo de cada cambio, y lo que se mantiene. */
function Comparison({ plan }: { plan: EditPlan }) {
  const { t } = useTranslation();
  return (
    <div className="space-y-3">
      <div className="overflow-x-auto rounded-lg border border-line">
        <table className="w-full min-w-[640px] text-left text-[12.5px]" aria-label={t("edit.comparison")}>
          <thead className="border-b border-line bg-surface-2 text-[12px] text-ink-3">
            <tr>
              <th className="w-[22%] px-3 py-1.5 font-medium">{t("edit.col.setting")}</th>
              <th className="px-3 py-1.5 font-medium">{t("edit.col.current")}</th>
              <th className="px-3 py-1.5 font-medium">{t("edit.col.new")}</th>
              <th className="w-[20%] px-3 py-1.5 font-medium">{t("edit.col.kind")}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-line">
            {plan.changes.map((c) => (
              <ChangeRow key={c.field} c={c} />
            ))}
          </tbody>
        </table>
      </div>
      {plan.kept.length > 0 && (
        <p className="text-[12.5px] text-ink-2">
          <span className="font-medium">{t("edit.kept")}</span> {plan.kept.map((f) => t(`edit.field.${f}`)).join(", ")}. {t("edit.keptRest")}
        </p>
      )}
    </div>
  );
}

function ChangeRow({ c }: { c: Change }) {
  const { t } = useTranslation();
  const empty = <span className="font-sans text-ink-3">{t("edit.none")}</span>;
  const kind = (
    <td className="px-3 py-2 align-top">
      <span className={cx("rounded px-1.5 py-px text-[11.5px] font-medium", c.kind === "direct" ? "bg-beacon-soft text-beacon-strong" : "bg-warn/15 text-warn")}>{t(`edit.kind.${c.kind}`)}</span>
      {c.reason && <div className="mt-1 text-[11.5px] text-ink-3">{t(`edit.reason.${c.reason}`)}</div>}
    </td>
  );
  if (c.items.length === 0) {
    const show = (v: string) => (v === "" ? empty : c.field === "restart" ? t(`edit.restartValue.${v.split(":")[0]}`, { n: v.split(":")[1] ?? "" }) : v);
    return (
      <tr>
        <th scope="row" className="px-3 py-2 align-top font-medium">
          {t(`edit.field.${c.field}`)}
        </th>
        <td className="selectable px-3 py-2 align-top font-mono text-[12px] [overflow-wrap:anywhere]">{show(c.before)}</td>
        <td className="selectable px-3 py-2 align-top font-mono text-[12px] [overflow-wrap:anywhere]">{show(c.after)}</td>
        {kind}
      </tr>
    );
  }
  return (
    <tr>
      <th scope="row" className="px-3 py-2 align-top font-medium">
        {t(`edit.field.${c.field}`)}
      </th>
      <td colSpan={2} className="px-3 py-2 align-top">
        <ul className="space-y-1">
          {c.items.map((i) => (
            <li key={`${i.op}-${i.label}`} className="grid grid-cols-[86px_minmax(0,1fr)] gap-2">
              <span className={cx("h-fit rounded px-1.5 py-px text-center text-[11px] font-medium", i.op === "added" ? "bg-ok/15 text-ok" : i.op === "removed" ? "bg-err/12 text-err" : "bg-surface-sunk text-ink-2")}>{t(`edit.op.${i.op}`)}</span>
              <span className="selectable font-mono text-[12px] [overflow-wrap:anywhere]">
                <span className="font-semibold">{i.label}</span>
                {i.before !== null && (
                  <span className="text-ink-3">
                    {" "}
                    {i.before}
                  </span>
                )}
                {i.before !== null && i.after !== null && <span className="text-ink-3"> → </span>}
                {i.after !== null && <span> {i.after}</span>}
              </span>
            </li>
          ))}
        </ul>
      </td>
      {kind}
    </tr>
  );
}

function Result({ out, plan, onRetry }: { out: EditOutcome; plan: EditPlan; onRetry: () => void }) {
  const { t } = useTranslation();
  const nav = useContainersNav();
  const ok = out.error === null;
  return (
    <div className="mx-auto w-full max-w-[920px] space-y-4 px-8 py-6" role="status">
      {ok ? (
        <Banner tone="info">
          <p className="flex items-center gap-1.5 font-medium text-ink">
            <CircleCheck size={16} className="text-ok" aria-hidden />
            {out.recreated ? t("edit.doneRecreated", { name: out.name }) : t("edit.doneDirect", { name: out.name })}
          </p>
          {out.recreated && out.backupName && <p className="mt-1">{t("edit.backupKept", { backup: out.backupName })}</p>}
        </Banner>
      ) : (
        <OutcomeBanner error={out.error ?? ""} rolledBack={out.rolledBack} rollbackFailed={out.rollbackFailed} />
      )}
      <div>
        <p className="mb-1 text-[12.5px] font-medium text-ink-2">{t("edit.stepsTitle")}</p>
        <ol className="space-y-1 text-[13px]">
          {out.steps.map((s, i) => (
            <li key={i} className="flex items-start gap-2">
              {s.ok ? <CircleCheck size={15} className="mt-0.5 shrink-0 text-ok" aria-label={t("edit.stepOk")} /> : <CircleX size={15} className="mt-0.5 shrink-0 text-err" aria-label={t("edit.stepFailed")} />}
              <span>
                {t(`edit.step.${s.code}`, { backup: plan.backupName ?? out.backupName ?? "" })}
                {s.detail && <span className="block text-[12px] text-ink-3 [overflow-wrap:anywhere]">{s.detail}</span>}
              </span>
            </li>
          ))}
        </ol>
      </div>
      {out.rollbackFailed && (
        <Banner tone="error">
          <CircleAlert size={14} className="mr-1 inline" aria-hidden />
          {t("edit.rollbackFailedHelp")}
        </Banner>
      )}
      <div className="flex flex-wrap gap-2 border-t border-line pt-4">
        <Button onClick={onRetry}>
          <ArrowLeft size={14} />
          {t("edit.backToEdit")}
        </Button>
        <Button variant="primary" onClick={() => nav.showDetail(out.containerId, out.name)}>
          {t("edit.openContainer")}
        </Button>
      </div>
    </div>
  );
}
