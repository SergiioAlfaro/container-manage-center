// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useEffect, useMemo, useRef, useState } from "react";
import { useApi } from "@/app/server";
import { RotateCw, Square } from "lucide-react";
import { useTranslation } from "react-i18next";
import { formatTimestamp, numberFormat } from "@/shared/format";
import type { LogEvent } from "@/shared/ipc/bindings/LogEvent";
import type { LogLine } from "@/shared/ipc/bindings/LogLine";
import { Button, CopyButton, Switch, cx } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { useStream } from "./useStream";
import { StreamStatusLine } from "./StreamStatusLine";

export const MAX_LINES = 10_000;
const TAILS = [100, 500, 1000, 5000, null] as const;
type Tail = (typeof TAILS)[number];

/** Añade líneas manteniendo como máximo `MAX_LINES` (se descartan las más antiguas). */
export function appendCapped(prev: LogLine[], next: LogLine[], max = MAX_LINES): LogLine[] {
  const all = prev.length + next.length > max ? prev.concat(next).slice(-max) : prev.concat(next);
  return all;
}

export function formatTs(ts: string, lang: string): string {
  const d = new Date(ts);
  if (Number.isNaN(d.getTime())) return ts;
  return formatTimestamp(d, lang, { date: true, ms: true });
}

export function LogsPanel({ id, name }: { id: string; name: string }) {
  const api = useApi();
  const { t, i18n } = useTranslation();
  const [lines, setLines] = useState<LogLine[]>([]);
  const [dropped, setDropped] = useState(false);
  const [tail, setTail] = useState<Tail>(500);
  const [session, setSession] = useState(0);
  const [timestamps, setTimestamps] = useState(false);
  const [follow, setFollow] = useState(true);
  const [filter, setFilter] = useState("");
  const box = useRef<HTMLDivElement>(null);

  const stream = useStream<LogEvent>(
    (h) => api.logsOpen(id, name, tail, h),
    (e) => {
      if (e.event === "lines")
        setLines((prev) => {
          if (prev.length + e.lines.length > MAX_LINES) setDropped(true);
          return appendCapped(prev, e.lines);
        });
    },
    `${id}|${tail}|${session}`,
  );

  const reopen = () => {
    setLines([]);
    setDropped(false);
    setSession((s) => s + 1);
  };

  const visible = useMemo(() => {
    const q = filter.trim().toLowerCase();
    return q ? lines.filter((l) => l.text.toLowerCase().includes(q)) : lines;
  }, [lines, filter]);

  useEffect(() => {
    if (follow && box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [visible, follow]);

  const text = visible.map((l) => (timestamps && l.ts ? `${l.ts} ${l.text}` : l.text)).join("\n");

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-3">
        <label className="flex items-center gap-2 text-[12.5px] text-ink-2">
          {t("logs.tail")}
          <select
            value={tail === null ? "all" : String(tail)}
            onChange={(e) => {
              setLines([]);
              setDropped(false);
              setTail(e.target.value === "all" ? null : (Number(e.target.value) as Tail));
            }}
            className="h-7 rounded-md border border-line bg-surface px-1.5 text-[12.5px]"
          >
            {TAILS.map((n) => (
              <option key={String(n)} value={n === null ? "all" : n}>
                {n === null ? t("logs.all") : t("logs.lastN", { count: n })}
              </option>
            ))}
          </select>
        </label>
        <input
          type="search"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          placeholder={t("logs.filter")}
          aria-label={t("logs.filter")}
          className="h-7 min-w-[180px] flex-1 rounded-md border border-line bg-surface px-2 text-[12.5px] outline-none placeholder:text-ink-3 focus:border-beacon"
        />
        <span className="flex items-center gap-1.5 text-[12.5px] text-ink-2">
          <Switch checked={timestamps} onChange={setTimestamps} label={t("logs.timestamps")} />
          {t("logs.timestamps")}
        </span>
        <span className="flex items-center gap-1.5 text-[12.5px] text-ink-2">
          <Switch checked={follow} onChange={setFollow} label={t("logs.follow")} />
          {t("logs.follow")}
        </span>
        <CopyButton text={text} />
        {stream.status === "open" || stream.status === "opening" ? (
          <Button size="sm" onClick={stream.close} disabled={stream.status !== "open"}>
            <Square size={13} />
            {t("logs.stop")}
          </Button>
        ) : (
          <Button size="sm" onClick={reopen}>
            <RotateCw size={13} />
            {t("logs.reopen")}
          </Button>
        )}
      </div>

      <StreamStatusLine status={stream.status} closed={stream.closed} openError={stream.openError} live={t("logs.live")} extra={t("logs.count", { count: lines.length })} />
      {dropped && <Banner tone="info">{t("logs.dropped", { max: numberFormat(i18n.language).format(MAX_LINES) })}</Banner>}

      <div
        ref={box}
        data-testid="log-box"
        onScroll={(e) => {
          const el = e.currentTarget;
          const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
          if (!atBottom && follow) setFollow(false);
        }}
        className="selectable h-[calc(100vh-290px)] min-h-[260px] overflow-auto rounded-lg border border-line bg-log py-2 font-mono text-[12px] leading-[1.55]"
      >
        {stream.status === "opening" && lines.length === 0 ? (
          <div className="flex items-center gap-2 px-3 text-ink-3">
            <Spinner size={13} /> {t("logs.opening")}
          </div>
        ) : visible.length === 0 ? (
          <p className="px-3 text-ink-3">{lines.length === 0 ? t("logs.empty") : t("logs.noMatches")}</p>
        ) : (
          visible.map((l, i) => (
            <div key={i} className={cx("flex gap-3 px-3 whitespace-pre-wrap [overflow-wrap:anywhere] [content-visibility:auto]", l.stream === "stderr" && "bg-err/6 text-err")}>
              {timestamps && l.ts && <span className="shrink-0 text-ink-3">{formatTs(l.ts, i18n.language)}</span>}
              <span className="min-w-0">{l.text}</span>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
