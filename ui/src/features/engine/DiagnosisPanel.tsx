import { RefreshCw, ShieldAlert, Unplug } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { Diagnosis } from "@/shared/ipc/bindings/Diagnosis";
import type { ProbeResult } from "@/shared/ipc/bindings/ProbeResult";
import { Button, CopyButton, cx } from "@/shared/ui/controls";
import { Spinner } from "@/shared/ui/feedback";
import { explain } from "./diagnosis";
import { SourceLabel } from "./SourceLabel";

const PROBE_TONE: Record<ProbeResult["result"], string> = {
  reachable: "text-ok",
  missing: "text-ink-3",
  notASocket: "text-warn",
  permissionDenied: "text-err",
  refused: "text-warn",
  error: "text-err",
};

export function DiagnosisPanel({
  diagnosis,
  onReconnect,
  reconnecting,
  onConfigure,
}: {
  diagnosis: Diagnosis;
  onReconnect: () => void;
  reconnecting: boolean;
  onConfigure: () => void;
}) {
  const { t } = useTranslation();
  const ex = explain(diagnosis);

  return (
    <section aria-labelledby="diag-title" className="overflow-hidden rounded-xl border border-err/25 bg-surface">
      <div className="flex flex-wrap items-start gap-4 border-b border-line bg-err/[0.05] px-6 py-5">
        <div className="grid h-10 w-10 shrink-0 place-items-center rounded-lg bg-err/12 text-err">
          <Unplug size={20} strokeWidth={1.75} />
        </div>
        <div className="min-w-[240px] flex-1 basis-[320px]">
          <p className="text-[12.5px] font-medium text-err">{t("diag.title")}</p>
          <h2 id="diag-title" className="mt-0.5 text-[17px] font-semibold tracking-[-0.01em]">
            {t(`diag.problems.${ex.problem}.title`)}
          </h2>
          <p className="selectable mt-1.5 text-[13.5px] text-ink-2 [overflow-wrap:anywhere]">{t(`diag.problems.${ex.problem}.body`, ex.params)}</p>
        </div>
        <div className="flex shrink-0 flex-wrap gap-2">
          <Button onClick={onConfigure}>{t("conn.configure")}</Button>
          <Button variant="primary" onClick={onReconnect} disabled={reconnecting}>
            {reconnecting ? <Spinner size={14} /> : <RefreshCw size={14} />}
            {reconnecting ? t("conn.reconnecting") : t("conn.reconnect")}
          </Button>
        </div>
      </div>

      <div className="grid gap-x-10 gap-y-6 px-6 py-5 [grid-template-columns:repeat(auto-fit,minmax(min(100%,300px),1fr))]">
        <div>
          <h3 className="mb-3 text-[13px] font-semibold text-ink-2">{t("diag.steps")}</h3>
          <ol className="space-y-3">
            {ex.steps.map((s, i) => (
              <li key={`${s.key}-${i}`} className="flex gap-3">
                <span aria-hidden className="mt-px grid h-5 w-5 shrink-0 place-items-center rounded-full bg-surface-sunk text-[11px] font-semibold text-ink-2 tabular-nums">
                  {i + 1}
                </span>
                <div className="min-w-0 flex-1 text-[13.5px]">
                  <p>{t(`diag.stepsText.${s.key}`, s.params)}</p>
                  {s.command && (
                    <div className="mt-1.5 flex items-center gap-2 rounded-md bg-code px-2.5 py-1.5">
                      <code className="selectable min-w-0 flex-1 font-mono text-[12.5px] [overflow-wrap:anywhere]">{s.command}</code>
                      <CopyButton text={s.command} compact />
                    </div>
                  )}
                </div>
              </li>
            ))}
          </ol>
          {ex.groupWarning && (
            <p className="mt-4 flex gap-2 rounded-md border border-warn/30 bg-warn/8 px-3 py-2 text-[12.5px]">
              <ShieldAlert size={15} className="mt-px shrink-0 text-warn" />
              {t("diag.rootGroupWarning", { group: ex.groupWarning })}
            </p>
          )}
          <p className="mt-4 text-[12px] text-ink-3">{t("diag.noAutoFix")}</p>
        </div>

        <div className="space-y-5">
          <div>
            <h3 className="text-[13px] font-semibold text-ink-2">{t("diag.candidates")}</h3>
            <p className="mb-2 text-[12px] text-ink-3">{t("diag.candidatesHint")}</p>
            {diagnosis.candidates.length === 0 ? (
              <p className="text-[12.5px] text-ink-3">—</p>
            ) : (
              <ul className="divide-y divide-line rounded-lg border border-line">
                {diagnosis.candidates.map((c) => (
                  <li key={c.endpoint.socketPath} className="px-3 py-2">
                    <div className="flex items-baseline justify-between gap-3 text-[11.5px]">
                      <span className="text-ink-3">
                        <SourceLabel source={c.endpoint.source} />
                      </span>
                      <span className={cx("shrink-0 font-medium", PROBE_TONE[c.result.result])}>
                        {t(`diag.probe.${c.result.result === "error" ? "errorLabel" : c.result.result}`)}
                      </span>
                    </div>
                    <code className="selectable block font-mono text-[12px] break-all">{c.endpoint.socketPath}</code>
                    {c.result.result === "error" && <p className="selectable mt-0.5 text-[11.5px] text-err [overflow-wrap:anywhere]">{c.result.message}</p>}
                  </li>
                ))}
              </ul>
            )}
          </div>
          <div>
            <h3 className="mb-2 text-[13px] font-semibold text-ink-2">{t("diag.installation")}</h3>
            <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-[12.5px]">
              <dt className="text-ink-3">{t("diag.cli")}</dt>
              <dd className="selectable font-mono text-[12px] break-all">{diagnosis.installation.cliPath ?? t("diag.notFound")}</dd>
              <dt className="text-ink-3">{t("diag.daemon")}</dt>
              <dd className="selectable font-mono text-[12px] break-all">{diagnosis.installation.daemonPath ?? t("diag.notFound")}</dd>
              <dt className="text-ink-3">{t("diag.rootlessTools")}</dt>
              <dd>{diagnosis.installation.rootlessTools ? t("diag.yes") : t("diag.no")}</dd>
              <dt className="text-ink-3">{t("diag.user")}</dt>
              <dd className="selectable font-mono text-[12px]">
                {diagnosis.user.name} ({diagnosis.user.uid})
              </dd>
            </dl>
          </div>
        </div>
      </div>
      <p className="border-t border-line px-6 py-2.5 text-[12px] text-ink-3">{t("conn.autoRetry")}</p>
    </section>
  );
}
