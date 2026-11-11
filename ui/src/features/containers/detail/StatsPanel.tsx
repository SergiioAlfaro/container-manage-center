import { useState } from "react";
import { useApi } from "@/app/server";
import { RotateCw, Square } from "lucide-react";
import { useTranslation } from "react-i18next";
import { formatBytes, numberFormat } from "@/shared/format";
import type { StatsEvent } from "@/shared/ipc/bindings/StatsEvent";
import type { StatsSample } from "@/shared/ipc/bindings/StatsSample";
import { Button } from "@/shared/ui/controls";
import { Banner } from "@/shared/ui/feedback";
import { StreamStatusLine } from "./StreamStatusLine";
import { useStream } from "./useStream";

export const HISTORY = 60;

/** Línea de tendencia mínima en SVG (sin ejes: los valores exactos están al lado). */
export function Sparkline({ values, max, label }: { values: Array<number | null>; max?: number; label: string }) {
  const nums = values.map((v) => v ?? 0);
  const top = Math.max(max ?? 0, ...nums, 1e-9);
  const w = 240;
  const h = 44;
  const step = w / Math.max(HISTORY - 1, 1);
  const offset = (HISTORY - nums.length) * step;
  const pts = nums.map((v, i) => `${(offset + i * step).toFixed(1)},${(h - 2 - (v / top) * (h - 4)).toFixed(1)}`).join(" ");
  return (
    <svg viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none" role="img" aria-label={label} className="h-11 w-full">
      <line x1="0" x2={w} y1={h - 1} y2={h - 1} stroke="var(--log-rule)" />
      {nums.length > 1 && <polyline points={pts} fill="none" stroke="var(--beacon)" strokeWidth="1.6" vectorEffect="non-scaling-stroke" strokeLinejoin="round" />}
    </svg>
  );
}

function Card({ title, value, sub, children }: { title: string; value: string; sub?: string; children?: React.ReactNode }) {
  return (
    <div className="rounded-xl border border-line bg-surface px-4 py-3">
      <p className="text-[12px] text-ink-3">{title}</p>
      <p className="mt-0.5 text-[20px] font-semibold tabular-nums">{value}</p>
      {sub && <p className="text-[12px] text-ink-3 tabular-nums">{sub}</p>}
      {children && <div className="mt-2">{children}</div>}
    </div>
  );
}

export function StatsPanel({ id, name, running }: { id: string; name: string; running: boolean }) {
  const api = useApi();
  const { t, i18n } = useTranslation();
  const lang = i18n.language;
  const [samples, setSamples] = useState<StatsSample[]>([]);
  const [session, setSession] = useState(0);
  const stream = useStream<StatsEvent>(
    running ? (h) => api.statsOpen(id, name, h) : null,
    (e) => {
      if (e.event === "sample") setSamples((s) => [...s.slice(-(HISTORY - 1)), e.sample]);
    },
    `${id}|${session}`,
  );
  const last = samples[samples.length - 1];
  const pct = (v: number | null | undefined) => (v === null || v === undefined ? "—" : `${numberFormat(lang, { maximumFractionDigits: 2, minimumFractionDigits: 2 }).format(v)} %`);

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-3">
        {running && (stream.status === "open" || stream.status === "opening") ? (
          <Button size="sm" onClick={stream.close} disabled={stream.status !== "open"}>
            <Square size={13} />
            {t("stats.stop")}
          </Button>
        ) : (
          <Button
            size="sm"
            disabled={!running}
            onClick={() => {
              setSamples([]);
              setSession((s) => s + 1);
            }}
          >
            <RotateCw size={13} />
            {t("stats.reopen")}
          </Button>
        )}
        <div className="flex-1">
          {running && <StreamStatusLine status={stream.status} closed={stream.closed} openError={stream.openError} live={t("stats.live")} extra={t("stats.samples", { count: samples.length })} />}
        </div>
      </div>
      {!running && <Banner tone="info">{t("stats.notRunning")}</Banner>}
      {last && (
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          <Card title={t("stats.cpu")} value={pct(last.cpuPercent)} sub={last.onlineCpus ? t("stats.cpuSub", { count: last.onlineCpus }) : undefined}>
            <Sparkline values={samples.map((s) => s.cpuPercent)} label={t("stats.cpuTrend")} />
          </Card>
          <Card title={t("stats.memory")} value={formatBytes(last.memoryUsed, lang)} sub={t("stats.memorySub", { limit: formatBytes(last.memoryLimit, lang), pct: pct(last.memoryPercent) })}>
            <Sparkline values={samples.map((s) => s.memoryUsed)} max={last.memoryLimit} label={t("stats.memoryTrend")} />
          </Card>
          <Card title={t("stats.pids")} value={last.pids === null ? "—" : String(last.pids)} />
          <Card title={t("stats.net")} value={`↓ ${formatBytes(last.netRx, lang)}`} sub={`↑ ${formatBytes(last.netTx, lang)}`} />
          <Card title={t("stats.block")} value={`${t("stats.read")} ${formatBytes(last.blockRead, lang)}`} sub={`${t("stats.written")} ${formatBytes(last.blockWrite, lang)}`} />
        </div>
      )}
      {last && <p className="text-[12px] text-ink-3">{t("stats.formulaHint")}</p>}
    </div>
  );
}
