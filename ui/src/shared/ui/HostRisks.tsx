import { ShieldAlert } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { HostRisk as Risk } from "@/shared/ipc/bindings/HostRisk";
import type { RiskLevel } from "@/shared/ipc/bindings/RiskLevel";
import { cx } from "./controls";
import { Checkbox } from "./feedback";

/**
 * Riesgos para el equipo (E1, D-086): montajes, modo privilegiado,
 * capacidades y red del equipo. Los altos y críticos se aceptan uno a uno; el
 * motor vuelve a comprobarlo, así que esta lista no es la única barrera.
 */

export function needsAcceptance(r: Risk): boolean {
  return r.level === "high" || r.level === "critical";
}

/** ¿Están aceptados todos los riesgos que lo exigen? */
export function allAccepted(risks: Risk[], accepted: string[]): boolean {
  return risks.filter(needsAcceptance).every((r) => accepted.includes(r.key));
}

/** Claves aceptadas que siguen existiendo (si cambia la configuración, hay que volver a aceptar). */
export function stillAccepted(risks: Risk[], accepted: string[]): string[] {
  return accepted.filter((k) => risks.some((r) => r.key === k));
}

const LEVEL_STYLE: Record<RiskLevel, string> = {
  critical: "bg-err/15 text-err",
  high: "bg-warn/20 text-warn",
  medium: "bg-warn/10 text-warn",
  low: "bg-surface-sunk text-ink-2",
};

export function RiskLevelBadge({ level }: { level: RiskLevel }) {
  const { t } = useTranslation();
  return <span className={cx("inline-block shrink-0 rounded px-1.5 py-px text-[11px] font-semibold uppercase tracking-wide", LEVEL_STYLE[level])}>{t(`hostRisk.level.${level}`)}</span>;
}

export function riskText(t: (k: string, o?: Record<string, unknown>) => string, r: Risk): string {
  return t(`issues.${r.code}`, { ...r.params, defaultValue: r.code });
}

export function HostRisks({ risks, accepted, onChange, showService = false }: { risks: Risk[]; accepted: string[]; onChange: (a: string[]) => void; showService?: boolean }) {
  const { t } = useTranslation();
  if (risks.length === 0) return null;
  const sorted = [...risks].sort((a, b) => order(b.level) - order(a.level));
  const pending = sorted.filter(needsAcceptance).filter((r) => !accepted.includes(r.key)).length;
  return (
    <section aria-labelledby="host-risks-title" className="space-y-2 rounded-xl border border-warn/40 bg-warn/5 p-3.5" data-testid="host-risks">
      <h3 id="host-risks-title" className="flex items-center gap-2 text-[13.5px] font-semibold">
        <ShieldAlert size={16} className="text-warn" aria-hidden />
        {t("hostRisk.title")}
      </h3>
      <p className="text-[12.5px] text-ink-2">{t("hostRisk.intro")}</p>
      <ul className="space-y-2">
        {sorted.map((r) => {
          const text = (showService && r.params.service ? `${r.params.service}: ` : "") + riskText(t, r);
          return (
            <li key={r.key} className="rounded-md border border-line bg-surface px-3 py-2">
              <div className="flex items-start gap-2">
                <RiskLevelBadge level={r.level} />
                {needsAcceptance(r) ? (
                  <Checkbox
                    checked={accepted.includes(r.key)}
                    onChange={(v) => onChange(v ? [...accepted, r.key] : accepted.filter((k) => k !== r.key))}
                    label={text}
                    hint={t("hostRisk.accept")}
                  />
                ) : (
                  <p className="text-[13px] leading-snug">{text}</p>
                )}
              </div>
            </li>
          );
        })}
      </ul>
      {pending > 0 && <p className="text-[12px] font-medium text-warn">{t("hostRisk.pending", { count: pending })}</p>}
    </section>
  );
}

function order(l: RiskLevel): number {
  return { low: 0, medium: 1, high: 2, critical: 3 }[l];
}
