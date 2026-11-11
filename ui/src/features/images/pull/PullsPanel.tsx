import { CheckCircle2, CircleAlert, CircleSlash, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { formatBytes } from "@/shared/format";
import { Button, cx } from "@/shared/ui/controls";
import { Spinner } from "@/shared/ui/feedback";
import { layerProgress, overallProgress, usePulls, useServerPulls, type PullState } from "./pulls";
import { useServerId } from "@/app/server";

function Bar({ value, label }: { value: number | null; label: string }) {
  return (
    <div role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={value === null ? undefined : Math.round(value * 100)} className="h-1.5 w-full overflow-hidden rounded-full bg-surface-sunk">
      <div className={cx("h-full rounded-full bg-beacon transition-[width] duration-300", value === null && "w-1/3 animate-pulse")} style={value === null ? undefined : { width: `${Math.round(value * 100)}%` }} />
    </div>
  );
}

function PullRow({ p }: { p: PullState }) {
  const { t, i18n } = useTranslation();
  const { cancel, dismiss } = usePulls.getState();
  const total = overallProgress(p.layers);
  const active = p.status === "starting" || p.status === "running";
  const last = p.messages[p.messages.length - 1];
  return (
    <li className="rounded-lg border border-line bg-surface px-4 py-3">
      <div className="flex items-center gap-2">
        {active ? <Spinner size={14} /> : p.status === "done" ? <CheckCircle2 size={15} className="text-ok" /> : p.status === "cancelled" ? <CircleSlash size={15} className="text-ink-3" /> : <CircleAlert size={15} className="text-err" />}
        <span className="selectable min-w-0 flex-1 truncate font-mono text-[12.5px]">{p.reference}</span>
        <span className="text-[12px] text-ink-3">{t(`pull.status.${p.status}`)}</span>
        {active ? (
          <Button size="sm" onClick={() => cancel(p.key)} disabled={p.cancelRequested}>
            {t("pull.cancel")}
          </Button>
        ) : (
          <button type="button" onClick={() => dismiss(p.key)} aria-label={t("common.dismiss")} title={t("common.dismiss")} className="grid h-6 w-6 place-items-center rounded text-ink-3 hover:bg-surface-sunk hover:text-ink">
            <X size={14} />
          </button>
        )}
      </div>
      {active && (
        <div className="mt-2 space-y-1.5">
          <Bar value={total} label={t("pull.progress", { ref: p.reference })} />
          {p.layers.length > 0 && (
            <details className="text-[12px] text-ink-2">
              <summary className="cursor-pointer text-ink-3">{t("pull.layers", { count: p.layers.length })}</summary>
              <ul className="mt-1.5 space-y-1">
                {p.layers.map((l) => (
                  <li key={l.id} className="grid grid-cols-[96px_1fr_150px] items-center gap-2">
                    <span className="font-mono text-[11.5px] text-ink-3">{l.id}</span>
                    <Bar value={layerProgress(l)} label={l.id} />
                    <span className="truncate text-[11.5px]">
                      {l.status}
                      {l.total ? ` · ${formatBytes(l.current ?? 0, i18n.language)} / ${formatBytes(l.total, i18n.language)}` : ""}
                    </span>
                  </li>
                ))}
              </ul>
            </details>
          )}
          {last && <p className="truncate text-[12px] text-ink-3">{last}</p>}
        </div>
      )}
      {!active && (p.summary || p.error) && <p className={cx("selectable mt-1.5 text-[12.5px] [overflow-wrap:anywhere]", p.error ? "text-err" : "text-ink-2")}>{p.error ?? p.summary}</p>}
    </li>
  );
}

/** Descargas en curso y recientes. */
export function PullsPanel() {
  const { t } = useTranslation();
  const items = useServerPulls();
  const server = useServerId();
  const clearFinished = usePulls((s) => s.clearFinished);
  const clear = () => clearFinished(server);
  if (items.length === 0) return null;
  const finished = items.some((i) => i.status !== "running" && i.status !== "starting");
  return (
    <section aria-label={t("pull.panel")} className="mb-5 space-y-2">
      <div className="flex items-center">
        <h2 className="flex-1 text-[13px] font-semibold">{t("pull.panel")}</h2>
        {finished && (
          <Button size="sm" variant="ghost" onClick={clear}>
            {t("pull.clear")}
          </Button>
        )}
      </div>
      <ul className="space-y-2">
        {items.map((p) => (
          <PullRow key={p.key} p={p} />
        ))}
      </ul>
    </section>
  );
}
