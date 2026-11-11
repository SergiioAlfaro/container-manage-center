/** Piezas comunes de «Servidores» (0.10.0). */
import { Check, Fingerprint, Laptop, Minus, Server, ShieldAlert, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { CheckStep } from "@/shared/ipc/bindings/CheckStep";
import type { HostKeyInfo } from "@/shared/ipc/bindings/HostKeyInfo";
import type { RemoteProblem } from "@/shared/ipc/bindings/RemoteProblem";
import type { ServerConfig } from "@/shared/ipc/bindings/ServerConfig";
import { CopyButton, cx } from "@/shared/ui/controls";
import { Spinner } from "@/shared/ui/feedback";
import { explainRemote, statusTone, type ServerStatus } from "./remote";

const TONE: Record<ReturnType<typeof statusTone>, string> = {
  ok: "bg-ok shadow-[0_0_0_3px_color-mix(in_srgb,var(--signal-ok)_20%,transparent)]",
  err: "bg-err",
  warn: "bg-warn",
  idle: "bg-ink-3/60",
  busy: "",
};

/** Punto de color del estado (con texto accesible). */
export function StatusDot({ status, className }: { status: ServerStatus; className?: string }) {
  const { t } = useTranslation();
  const tone = statusTone(status);
  if (tone === "busy") return <Spinner size={11} className={cx("shrink-0 text-ink-3", className)} />;
  return <span role="img" aria-label={t(`servers.status.${status}`)} className={cx("inline-block h-2 w-2 shrink-0 rounded-full", TONE[tone], className)} />;
}

export function StatusLabel({ status }: { status: ServerStatus }) {
  const { t } = useTranslation();
  return (
    <span className="inline-flex items-center gap-1.5 whitespace-nowrap">
      <StatusDot status={status} />
      <span>{t(`servers.status.${status}`)}</span>
    </span>
  );
}

/** Icono de equipo local o servidor remoto. */
export function KindIcon({ remote, size = 14, className }: { remote: boolean; size?: number; className?: string }) {
  const Icon = remote ? Server : Laptop;
  return <Icon size={size} aria-hidden className={cx("shrink-0", className)} />;
}

/** Huella de la identidad del servidor, para compararla carácter a carácter. */
export function HostKeyBox({ info, tone = "neutral" }: { info: HostKeyInfo; tone?: "neutral" | "danger" }) {
  const { t } = useTranslation();
  return (
    <div className={cx("rounded-lg border px-3 py-2.5", tone === "danger" ? "border-err/40 bg-err/[0.06]" : "border-line bg-surface-2")}>
      <div className="flex items-center gap-2 text-[12px] text-ink-3">
        <Fingerprint size={14} aria-hidden />
        <span>{t("servers.hostKey.type", { type: info.keyType })}</span>
      </div>
      <div className="mt-1 flex items-center gap-2">
        <code data-testid="fingerprint" className="selectable min-w-0 flex-1 font-mono text-[13px] font-medium break-all">
          {info.fingerprint}
        </code>
        <CopyButton text={info.fingerprint} compact />
      </div>
    </div>
  );
}

const STEP_ICON = {
  ok: <Check size={14} className="text-ok" aria-hidden />,
  failed: <X size={14} className="text-err" aria-hidden />,
  notRun: <Minus size={14} className="text-ink-3" aria-hidden />,
  skipped: <Minus size={14} className="text-ink-3" aria-hidden />,
};

/** Resultado de cada uno de los siete pasos de la prueba (§3.3). */
export function CheckList({ steps, running = false }: { steps: CheckStep[]; running?: boolean }) {
  const { t } = useTranslation();
  return (
    <ol aria-label={t("servers.test.steps")} className="divide-y divide-line rounded-lg border border-line">
      {steps.map((s) => (
        <li key={s.id} data-testid={`check-${s.id}`} data-status={s.status} className="flex items-start gap-3 px-3 py-2">
          <span className="mt-0.5 grid h-4 w-4 shrink-0 place-items-center">
            {running && s.status === "notRun" ? <Spinner size={12} className="text-ink-3" /> : STEP_ICON[s.status]}
          </span>
          <div className="min-w-0 flex-1">
            <p className="text-[13px] font-medium">{t(`servers.checks.${s.id}`)}</p>
            <p className="text-[12px] text-ink-3">
              {t(`servers.checkStatus.${s.status}`)}
              {s.detail && (
                <>
                  {" · "}
                  <span className="selectable font-mono [overflow-wrap:anywhere]">{s.detail}</span>
                </>
              )}
            </p>
          </div>
        </li>
      ))}
    </ol>
  );
}

/** Explicación de un problema remoto y pasos para resolverlo (§3.4, §10). */
export function RemoteProblemView({ problem, config }: { problem: RemoteProblem; config: ServerConfig }) {
  const { t } = useTranslation();
  const ex = explainRemote(problem, config);
  return (
    <div data-testid="remote-problem" className={cx("rounded-lg border px-4 py-3.5", ex.danger ? "border-err/50 bg-err/[0.06]" : "border-line bg-surface")}>
      <h3 className={cx("flex items-center gap-2 text-[14px] font-semibold", ex.danger && "text-err")}>
        {ex.danger && <ShieldAlert size={16} aria-hidden />}
        {t(`servers.problems.${ex.problem}.title`, ex.params)}
      </h3>
      <p className="selectable mt-1 text-[13px] text-ink-2 [overflow-wrap:anywhere]">{t(`servers.problems.${ex.problem}.body`, ex.params)}</p>
      <ol className="mt-3 space-y-2.5">
        {ex.steps.map((s, i) => (
          <li key={`${s.key}-${i}`} className="flex gap-3">
            <span
              aria-hidden
              className="mt-px grid h-5 w-5 shrink-0 place-items-center rounded-full bg-surface-sunk text-[11px] font-semibold text-ink-2 tabular-nums"
            >
              {i + 1}
            </span>
            <div className="min-w-0 flex-1 text-[13px]">
              <p>{t(`servers.fix.${s.key}`, { ...ex.params, ...s.params })}</p>
              {s.command && (
                <div className="mt-1.5">
                  <p className="text-[11.5px] text-ink-3">{t(s.where === "server" ? "servers.fix.onServer" : "servers.fix.onThisComputer")}</p>
                  <div className="mt-0.5 flex items-center gap-2 rounded-md bg-code px-2.5 py-1.5">
                    <code className="selectable min-w-0 flex-1 font-mono text-[12.5px] [overflow-wrap:anywhere]">{s.command}</code>
                    <CopyButton text={s.command} compact />
                  </div>
                </div>
              )}
            </div>
          </li>
        ))}
      </ol>
      {ex.groupWarning && (
        <p className="mt-3 flex gap-2 rounded-md border border-warn/30 bg-warn/8 px-3 py-2 text-[12.5px]">
          <ShieldAlert size={15} className="mt-px shrink-0 text-warn" />
          {t("servers.groupWarning")}
        </p>
      )}
      <p className="mt-3 text-[12px] text-ink-3">{t("servers.noAutoFix")}</p>
    </div>
  );
}
