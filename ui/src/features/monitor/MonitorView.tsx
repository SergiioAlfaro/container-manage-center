// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Activity, BellRing, Pause, Play } from "lucide-react";
import { useTranslation } from "react-i18next";
import { serverName, useApi, useServerId, useServers } from "@/app/server";
import { api } from "@/shared/ipc/client";
import type { Alert } from "@/shared/ipc/bindings/Alert";
import type { AlertRules } from "@/shared/ipc/bindings/AlertRules";
import type { CommandError } from "@/shared/ipc/bindings/CommandError";
import type { MonitorContainerSample } from "@/shared/ipc/bindings/MonitorContainerSample";
import type { MonitorSettings } from "@/shared/ipc/bindings/MonitorSettings";
import type { ThresholdRule } from "@/shared/ipc/bindings/ThresholdRule";
import { formatBytes, formatTimestamp, numberFormat } from "@/shared/format";
import { Button, cx, Segmented } from "@/shared/ui/controls";
import { Banner, Checkbox, Spinner } from "@/shared/ui/feedback";
import { LineChart, type ChartSeries } from "@/shared/ui/LineChart";
import { useMonitorNav } from "./store";
import {
  alertText,
  INTERVALS,
  liveGaps,
  liveSeries,
  MAX_COMPARE,
  METRICS,
  minuteGaps,
  minuteSeries,
  rangeOf,
  RANGES,
  serverLive,
  serverMinutes,
  uptime,
  type Metric,
  type RangeKey,
} from "./model";

type Tab = "summary" | "compare" | "alerts";

function useFormats() {
  const { i18n } = useTranslation();
  const lang = i18n.language;
  const nf = numberFormat(lang, { maximumFractionDigits: 1, minimumFractionDigits: 1 });
  const pct = (v: number) => `${nf.format(v)} %`;
  const bytes = (v: number) => formatBytes(v, lang);
  const rate = (v: number) => `${formatBytes(v, lang)}/s`;
  const int = (v: number) => numberFormat(lang, { maximumFractionDigits: 0 }).format(v);
  const time = (t: number) => new Date(t).toLocaleTimeString(lang, { hour: "2-digit", minute: "2-digit" });
  const metric = (m: Metric) => (m === "cpu" || m === "memPercent" ? pct : m === "mem" ? bytes : m === "pids" ? int : rate);
  return { pct, bytes, rate, int, time, metric, lang };
}

export function MonitorView() {
  const { t } = useTranslation();
  const bound = useApi();
  const server = useServerId();
  const qc = useQueryClient();
  const f = useFormats();
  const [tab, setTab] = useState<Tab>("summary");
  const [range, setRange] = useState<RangeKey>("15m");
  const status = useQuery({ queryKey: ["monitorStatus", server], queryFn: bound.monitorStatus, refetchInterval: 15_000 });
  const settings = useQuery({ queryKey: ["monitorSettings"], queryFn: api.monitorSettings });
  // Cada muestra nueva refresca lo que se ve.
  useEffect(() => {
    let un: (() => void) | undefined;
    let alive = true;
    void api
      .onMonitorSample((e) => {
        if (e.server !== server) return;
        void qc.invalidateQueries({ queryKey: ["monitorStatus", server] });
        void qc.invalidateQueries({ queryKey: ["monitorLive", server] });
      })
      .then((u) => (alive ? (un = u) : u()));
    return () => {
      alive = false;
      un?.();
    };
  }, [server, qc]);
  const toggle = useMutation({
    mutationFn: (on: boolean) => bound.monitorSetServer(on),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["monitorStatus", server] });
      void qc.invalidateQueries({ queryKey: ["monitorSettings"] });
    },
  });
  const pause = useMutation({
    mutationFn: (p: boolean) => api.monitorPause(p),
    onSuccess: () => void qc.invalidateQueries({ queryKey: ["monitorStatus"] }),
  });
  const saveSettings = useMutation({
    mutationFn: (s: MonitorSettings) => api.monitorSettingsSet(s),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["monitorSettings"] });
      void qc.invalidateQueries({ queryKey: ["monitorStatus"] });
    },
  });
  const st = status.data;
  const err = (toggle.error ?? pause.error ?? saveSettings.error) as unknown as CommandError | null;
  const last = st?.last ?? null;
  return (
    <div className="px-8 py-6">
      <p className="mb-4 flex max-w-[860px] items-start gap-2 text-[12.5px] text-ink-2">
        <Activity size={16} className="mt-px shrink-0 text-beacon-strong" aria-hidden />
        {t("monitor.intro")}
      </p>
      <section aria-label={t("monitor.controls")} className="mb-4 flex flex-wrap items-center gap-3 rounded-xl border border-line bg-surface px-4 py-3">
        <Checkbox checked={!!st?.enabled} onChange={(v) => toggle.mutate(v)} label={t("monitor.enable")} />
        {st?.enabled && (
          <>
            <Button size="sm" onClick={() => pause.mutate(!st.paused)}>
              {st.paused ? <Play size={13} /> : <Pause size={13} />}
              {st.paused ? t("monitor.resume") : t("monitor.pause")}
            </Button>
            <label className="flex items-center gap-1.5 text-[12.5px] text-ink-2">
              {t("monitor.interval")}
              <select
                className="h-7 rounded-md border border-line bg-surface px-2 text-[12.5px] text-ink"
                value={settings.data?.intervalSecs ?? 5}
                onChange={(e) => settings.data && saveSettings.mutate({ ...settings.data, intervalSecs: Number(e.target.value) })}
              >
                {INTERVALS.map((s) => (
                  <option key={s} value={s}>
                    {t("monitor.seconds", { count: s })}
                  </option>
                ))}
              </select>
            </label>
          </>
        )}
        <span role="status" className="ml-auto text-[12px] text-ink-3">
          {!st?.enabled
            ? t("monitor.off")
            : st.paused
              ? t("monitor.paused")
              : st.gap
                ? t("monitor.gap")
                : last
                  ? t("monitor.lastSample", { time: f.time(last.t), count: last.containers.length, ms: last.tookMs })
                  : t("monitor.waiting")}
        </span>
      </section>
      {err && <Banner tone="error">{err.message}</Banner>}
      {st?.lastError && st.gap && <Banner tone="warn">{t("monitor.lastError", { error: st.lastError })}</Banner>}
      {!st?.enabled ? (
        <Banner tone="info">{t("monitor.offHint")}</Banner>
      ) : (
        <>
          <div className="mb-4 flex flex-wrap items-center gap-3">
            <Segmented
              label={t("monitor.tabs")}
              value={tab}
              onChange={setTab}
              options={[
                { value: "summary", label: t("monitor.tab.summary") },
                { value: "compare", label: t("monitor.tab.compare") },
                { value: "alerts", label: t("monitor.tab.alerts") },
              ]}
            />
            {tab !== "alerts" && (
              <Segmented label={t("monitor.range")} value={range} onChange={setRange} options={RANGES.map((r) => ({ value: r.key, label: t(`monitor.ranges.${r.key}`) }))} />
            )}
          </div>
          {tab === "summary" && <Summary range={range} />}
          {tab === "compare" && <Compare range={range} />}
          {tab === "alerts" && settings.data && <Alerts settings={settings.data} onSave={(s) => saveSettings.mutate(s)} saving={saveSettings.isPending} />}
        </>
      )}
      {status.isPending && <Spinner />}
    </div>
  );
}

/** Series de los contenedores pedidos en el rango: de memoria o del disco. */
function useSeries(range: RangeKey, ids: string[], names: Record<string, string>, metric: Metric) {
  const bound = useApi();
  const server = useServerId();
  const r = rangeOf(range);
  const live = useQuery({
    queryKey: ["monitorLive", server, ids, r.minutes],
    queryFn: () => bound.monitorLive(ids, r.minutes),
    enabled: r.live,
  });
  const minutes = useQuery({
    queryKey: ["monitorMinutes", server, r.minutes, ids],
    queryFn: () => bound.monitorMinutes(Math.ceil(r.minutes / 60), ids),
    enabled: !r.live,
    refetchInterval: 60_000,
  });
  const now = Date.now();
  const from = now - r.minutes * 60_000;
  if (r.live) {
    const srv = live.data?.server ?? [];
    return { live: true, from, to: now, series: liveSeries(live.data, ids, names, metric), gaps: liveGaps(srv), server: srv, minutes: [] as never[] };
  }
  const recs = minutes.data ?? [];
  return { live: false, from, to: now, series: minuteSeries(recs, ids, names, metric), gaps: minuteGaps(recs), server: [], minutes: recs };
}

function Summary({ range }: { range: RangeKey }) {
  const { t } = useTranslation();
  const bound = useApi();
  const server = useServerId();
  const f = useFormats();
  const st = useQuery({ queryKey: ["monitorStatus", server], queryFn: bound.monitorStatus }).data;
  // E6: al llegar desde la auditoría se abre el detalle de ese contenedor.
  const [selected, setSelected] = useState<string | null>(() => useMonitorNav.getState().take(server));
  const containers = useMemo(() => st?.last?.containers ?? [], [st?.last?.containers]);
  const names = useMemo(() => Object.fromEntries(containers.map((c) => [c.meta.id, c.meta.name])), [containers]);
  const sel = selected && containers.some((c) => c.meta.id === selected) ? selected : null;
  const srv = useSeries(range, sel ? [sel] : [], names, "cpu");
  const now = Date.now();
  const host = st?.host;
  const firing = new Set((st?.firing ?? []).map((a) => a.containerId));
  const serverCpu: ChartSeries[] = [{ name: t("monitor.cpuTotal"), points: srv.live ? serverLive(srv.server, "cpuTotal") : serverMinutes(srv.minutes, "cpuTotal") }];
  const serverMem: ChartSeries[] = [{ name: t("monitor.memTotal"), points: srv.live ? serverLive(srv.server, "memTotal") : serverMinutes(srv.minutes, "memTotal") }];
  return (
    <div className="space-y-4">
      <div className="grid gap-3 md:grid-cols-3">
        <div className="rounded-xl border border-line bg-surface px-4 py-3">
          <p className="text-[12px] text-ink-3">{t("monitor.host")}</p>
          <p className="mt-0.5 text-[13px] tabular-nums">
            {host ? t("monitor.hostValue", { cpus: host.ncpu ?? "—", mem: host.memTotal ? f.bytes(host.memTotal) : "—" }) : "—"}
          </p>
          <p className="text-[11.5px] text-ink-3">{host ? `Docker ${host.serverVersion ?? "?"} · ${host.operatingSystem ?? ""}` : ""}</p>
          <p className="mt-1 text-[11px] text-ink-3">{t("monitor.hostHint")}</p>
        </div>
        <div className="rounded-xl border border-line bg-surface px-4 py-3">
          <p className="text-[12px] text-ink-3">{t("monitor.counts")}</p>
          <p className="mt-0.5 text-[13px] tabular-nums">
            {st?.last ? t("monitor.countsValue", { running: st.last.counts.running, paused: st.last.counts.paused, stopped: st.last.counts.stopped }) : "—"}
          </p>
        </div>
        <div className="rounded-xl border border-line bg-surface px-4 py-3">
          <p className="text-[12px] text-ink-3">{t("monitor.usage")}</p>
          <p className="mt-0.5 text-[13px] tabular-nums">
            {st?.last ? t("monitor.usageValue", { cpu: st.last.cpuTotal === null ? "—" : f.pct(st.last.cpuTotal), mem: f.bytes(st.last.memTotal) }) : "—"}
          </p>
        </div>
      </div>
      <div className="grid gap-3 lg:grid-cols-2">
        <LineChart label={t("monitor.cpuTotal")} series={serverCpu} from={srv.from} to={srv.to} gaps={srv.gaps} format={f.pct} timeFormat={f.time} emptyText={t("monitor.noData")} />
        <LineChart label={t("monitor.memTotal")} series={serverMem} from={srv.from} to={srv.to} gaps={srv.gaps} format={f.bytes} timeFormat={f.time} emptyText={t("monitor.noData")} />
      </div>
      <table className="w-full rounded-xl border border-line bg-surface text-[12.5px]">
        <caption className="sr-only">{t("monitor.containers")}</caption>
        <thead className="text-left text-[12px] text-ink-3">
          <tr>
            <th className="px-3 py-2 font-medium">{t("monitor.col.name")}</th>
            <th className="px-3 py-2 font-medium">{t("monitor.col.cpu")}</th>
            <th className="px-3 py-2 font-medium">{t("monitor.col.mem")}</th>
            <th className="px-3 py-2 font-medium">{t("monitor.col.net")}</th>
            <th className="px-3 py-2 font-medium">{t("monitor.col.disk")}</th>
            <th className="px-3 py-2 font-medium">{t("monitor.col.pids")}</th>
            <th className="px-3 py-2 font-medium">{t("monitor.col.uptime")}</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-line">
          {containers.map((c) => (
            <Row key={c.meta.id} c={c} selected={sel === c.meta.id} alert={firing.has(c.meta.id)} now={now} onSelect={() => setSelected(sel === c.meta.id ? null : c.meta.id)} />
          ))}
        </tbody>
      </table>
      {sel && <Detail range={range} id={sel} name={names[sel] ?? sel} />}
    </div>
  );
}

function Row({ c, selected, alert, now, onSelect }: { c: MonitorContainerSample; selected: boolean; alert: boolean; now: number; onSelect: () => void }) {
  const { t } = useTranslation();
  const f = useFormats();
  const p = c.point;
  const dash = "—";
  return (
    <tr className={cx(selected && "bg-beacon-soft")}>
      <td className="px-3 py-2">
        <button type="button" className="text-left font-medium hover:underline" aria-pressed={selected} onClick={onSelect}>
          {c.meta.name}
        </button>
        {alert && (
          <span className="ml-2 inline-flex items-center gap-1 rounded bg-err/12 px-1.5 text-[11px] text-err">
            <BellRing size={11} aria-hidden />
            {t("monitor.alerting")}
          </span>
        )}
        <div className="text-[11px] text-ink-3">{c.meta.state === "running" ? c.meta.image : t("monitor.notRunning", { state: c.meta.state })}</div>
      </td>
      <td className="px-3 py-2 tabular-nums">{p?.cpu == null ? dash : f.pct(p.cpu)}</td>
      <td className="px-3 py-2 tabular-nums">
        {p ? `${f.bytes(p.mem)} / ${f.bytes(p.memLimit)}` : dash}
        {p?.memPercent != null && <div className="text-[11px] text-ink-3">{f.pct(p.memPercent)}</div>}
      </td>
      <td className="px-3 py-2 tabular-nums">{p?.netRx == null ? dash : `↓ ${f.rate(p.netRx)} · ↑ ${f.rate(p.netTx ?? 0)}`}</td>
      <td className="px-3 py-2 tabular-nums">{p?.blkRead == null ? dash : `${f.rate(p.blkRead)} · ${f.rate(p.blkWrite ?? 0)}`}</td>
      <td className="px-3 py-2 tabular-nums">{p?.pids ?? dash}</td>
      <td className="px-3 py-2 tabular-nums">
        {c.meta.state === "running" ? uptime(t, c.meta.startedAt, now) : dash}
        {c.meta.restartCount ? <div className="text-[11px] text-warn">{t("monitor.restarts", { count: c.meta.restartCount })}</div> : null}
      </td>
    </tr>
  );
}

function Detail({ range, id, name }: { range: RangeKey; id: string; name: string }) {
  const { t } = useTranslation();
  const f = useFormats();
  const names = { [id]: name };
  const cpu = useSeries(range, [id], names, "cpu");
  const mem = useSeries(range, [id], names, "mem");
  const rx = useSeries(range, [id], names, "netRx");
  const tx = useSeries(range, [id], names, "netTx");
  const br = useSeries(range, [id], names, "blkRead");
  const bw = useSeries(range, [id], names, "blkWrite");
  const pids = useSeries(range, [id], names, "pids");
  const common = { from: cpu.from, to: cpu.to, gaps: cpu.gaps, timeFormat: f.time, emptyText: t("monitor.noData") };
  const two = (a: ChartSeries[], b: ChartSeries[], la: string, lb: string): ChartSeries[] => [
    { name: la, points: a[0]?.points ?? [] },
    { name: lb, points: b[0]?.points ?? [] },
  ];
  return (
    <section aria-label={t("monitor.detail", { name })} className="space-y-3">
      <h2 className="text-[14px] font-semibold">{t("monitor.detail", { name })}</h2>
      <div className="grid gap-3 lg:grid-cols-2">
        <LineChart label={t("monitor.metric.cpu")} series={cpu.series} format={f.pct} {...common} />
        <LineChart label={t("monitor.metric.mem")} series={mem.series} format={f.bytes} {...common} />
        <LineChart label={t("monitor.col.net")} series={two(rx.series, tx.series, t("monitor.metric.netRx"), t("monitor.metric.netTx"))} format={f.rate} {...common} />
        <LineChart label={t("monitor.col.disk")} series={two(br.series, bw.series, t("monitor.metric.blkRead"), t("monitor.metric.blkWrite"))} format={f.rate} {...common} />
        <LineChart label={t("monitor.metric.pids")} series={pids.series} format={f.int} {...common} />
      </div>
      <p className="text-[11.5px] text-ink-3">{t("monitor.diskHint")}</p>
    </section>
  );
}

function Compare({ range }: { range: RangeKey }) {
  const { t } = useTranslation();
  const bound = useApi();
  const server = useServerId();
  const f = useFormats();
  const st = useQuery({ queryKey: ["monitorStatus", server], queryFn: bound.monitorStatus }).data;
  const containers = useMemo(() => (st?.last?.containers ?? []).filter((c) => c.meta.state === "running"), [st?.last?.containers]);
  const [ids, setIds] = useState<string[]>([]);
  const [metric, setMetric] = useState<Metric>("cpu");
  const names = useMemo(() => Object.fromEntries(containers.map((c) => [c.meta.id, c.meta.name])), [containers]);
  const data = useSeries(range, ids, names, metric);
  return (
    <div className="space-y-3">
      <p className="text-[12px] text-ink-3">{t("monitor.compareHint", { count: MAX_COMPARE })}</p>
      <div className="flex flex-wrap gap-x-4 gap-y-1.5">
        {containers.map((c) => (
          <Checkbox
            key={c.meta.id}
            checked={ids.includes(c.meta.id)}
            disabled={!ids.includes(c.meta.id) && ids.length >= MAX_COMPARE}
            onChange={(on) => setIds((v) => (on ? [...v, c.meta.id] : v.filter((x) => x !== c.meta.id)))}
            label={c.meta.name}
          />
        ))}
      </div>
      <label className="flex items-center gap-2 text-[12.5px] text-ink-2">
        {t("monitor.metricLabel")}
        <select className="h-7 rounded-md border border-line bg-surface px-2 text-[12.5px] text-ink" value={metric} onChange={(e) => setMetric(e.target.value as Metric)}>
          {METRICS.map((m) => (
            <option key={m} value={m}>
              {t(`monitor.metric.${m}`)}
            </option>
          ))}
        </select>
      </label>
      {ids.length === 0 ? (
        <p className="text-[13px] text-ink-3">{t("monitor.compareEmpty")}</p>
      ) : (
        <LineChart
          label={t(`monitor.metric.${metric}`)}
          series={data.series}
          from={data.from}
          to={data.to}
          gaps={data.gaps}
          format={f.metric(metric)}
          max={metric === "memPercent" ? 100 : undefined}
          height={220}
          timeFormat={f.time}
          emptyText={t("monitor.noData")}
        />
      )}
    </div>
  );
}

function ThresholdFields({ rule, onChange, label, unit }: { rule: ThresholdRule; onChange: (r: ThresholdRule) => void; label: string; unit: string }) {
  const { t } = useTranslation();
  const num = (k: "above" | "clearBelow" | "samples", text: string, step: number) => (
    <label className="block text-[12px] text-ink-2">
      {text}
      <input
        type="number"
        step={step}
        min={0}
        className="mt-1 h-8 w-full rounded-md border border-line bg-surface px-2 text-[13px] text-ink"
        value={rule[k]}
        disabled={!rule.enabled}
        onChange={(e) => onChange({ ...rule, [k]: Number(e.target.value) })}
      />
    </label>
  );
  return (
    <fieldset className="rounded-lg border border-line p-3">
      <legend className="px-1">
        <Checkbox checked={rule.enabled} onChange={(v) => onChange({ ...rule, enabled: v })} label={label} />
      </legend>
      <div className="grid gap-3 md:grid-cols-3">
        {num("above", t("monitor.rules.above", { unit }), 1)}
        {num("clearBelow", t("monitor.rules.clearBelow", { unit }), 1)}
        {num("samples", t("monitor.rules.samples"), 1)}
      </div>
    </fieldset>
  );
}

function Alerts({ settings, onSave, saving }: { settings: MonitorSettings; onSave: (s: MonitorSettings) => void; saving: boolean }) {
  const { t } = useTranslation();
  const bound = useApi();
  const server = useServerId();
  const view = useServers((s) => s.servers.find((x) => x.id === server));
  const serverLabel = serverName(view, t("servers.local")) || server;
  const f = useFormats();
  const qc = useQueryClient();
  const [draft, setDraft] = useState<MonitorSettings>(settings);
  const rules = draft.rules;
  const setRules = (r: AlertRules) => setDraft({ ...draft, rules: r });
  const alerts = useQuery({ queryKey: ["monitorAlerts", server], queryFn: bound.monitorAlerts });
  useEffect(() => {
    let un: (() => void) | undefined;
    let alive = true;
    void api.onMonitorAlert(() => void qc.invalidateQueries({ queryKey: ["monitorAlerts"] })).then((u) => (alive ? (un = u) : u()));
    return () => {
      alive = false;
      un?.();
    };
  }, [qc]);
  const test = useMutation({ mutationFn: () => api.monitorNotify(t("monitor.testTitle"), t("monitor.testBody")) });
  const dirty = JSON.stringify(draft) !== JSON.stringify(settings);
  return (
    <div className="space-y-4">
      <section aria-label={t("monitor.rules.title")} className="space-y-3 rounded-xl border border-line bg-surface p-4">
        <h2 className="text-[13.5px] font-semibold">{t("monitor.rules.title")}</h2>
        <p className="text-[12px] text-ink-3">{t("monitor.rules.hint")}</p>
        <ThresholdFields label={t("monitor.rules.cpu")} unit="%" rule={rules.cpu} onChange={(r) => setRules({ ...rules, cpu: r })} />
        <ThresholdFields label={t("monitor.rules.memory")} unit="%" rule={rules.memory} onChange={(r) => setRules({ ...rules, memory: r })} />
        <div className="grid gap-1.5 md:grid-cols-2">
          {(["restart", "oom", "exitError", "disconnected"] as const).map((k) => (
            <Checkbox key={k} checked={rules[k]} onChange={(v) => setRules({ ...rules, [k]: v })} label={t(`monitor.rules.${k}`)} />
          ))}
        </div>
        <div className="grid gap-3 md:grid-cols-2">
          <label className="block text-[12px] text-ink-2">
            {t("monitor.rules.silence")}
            <input type="number" min={0} max={1440} className="mt-1 h-8 w-full rounded-md border border-line bg-surface px-2 text-[13px] text-ink" value={rules.silenceMinutes} onChange={(e) => setRules({ ...rules, silenceMinutes: Number(e.target.value) })} />
          </label>
          <label className="block text-[12px] text-ink-2">
            {t("monitor.retention")}
            <input type="number" min={1} max={168} className="mt-1 h-8 w-full rounded-md border border-line bg-surface px-2 text-[13px] text-ink" value={draft.retentionHours} onChange={(e) => setDraft({ ...draft, retentionHours: Number(e.target.value) })} />
          </label>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <Checkbox checked={draft.notifications} onChange={(v) => setDraft({ ...draft, notifications: v })} label={t("monitor.notifications")} hint={t("monitor.notificationsHint")} />
          <Button size="sm" onClick={() => test.mutate()} loading={test.isPending}>
            {t("monitor.testNotification")}
          </Button>
          {test.isSuccess && <span className="text-[12px] text-ok">{t("monitor.testSent")}</span>}
          {test.error && <span className="text-[12px] text-err">{t("monitor.testFailed", { error: (test.error as unknown as CommandError).message })}</span>}
        </div>
        <Button variant="primary" disabled={!dirty} loading={saving} onClick={() => onSave(draft)}>
          {t("monitor.save")}
        </Button>
      </section>
      <section aria-label={t("monitor.history")}>
        <h2 className="mb-2 text-[13.5px] font-semibold">{t("monitor.history")}</h2>
        {(alerts.data ?? []).length === 0 ? (
          <p className="text-[13px] text-ink-3">{t("monitor.noAlerts")}</p>
        ) : (
          <ul className="divide-y divide-line rounded-xl border border-line bg-surface">
            {(alerts.data ?? []).slice(0, 200).map((a: Alert, i) => {
              const x = alertText(t, a, serverLabel, f.pct);
              return (
                <li key={`${a.at}-${a.kind}-${a.containerId ?? ""}-${i}`} className="flex flex-wrap items-baseline gap-2 px-4 py-2 text-[12.5px]">
                  <span className="text-ink-3 tabular-nums">{formatTimestamp(new Date(a.at), f.lang, { date: true })}</span>
                  <span className={cx("font-medium", a.state === "fired" ? "text-err" : "text-ok")}>{x.summary}</span>
                  <span className="text-ink-2">{x.body}</span>
                  {a.state === "fired" && !a.notified && <span className="text-[11px] text-ink-3">{t("monitor.silenced")}</span>}
                </li>
              );
            })}
          </ul>
        )}
      </section>
    </div>
  );
}
