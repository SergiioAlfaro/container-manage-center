// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useEffect, useMemo, useState } from "react";
import { useApi, useServerId } from "@/app/server";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Eraser, Pause, Play, RefreshCw, Search, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useEngine } from "@/features/engine/store";
import type { EventQuery } from "@/shared/ipc/bindings/EventQuery";
import { Button, CopyButton, Segmented, cx } from "@/shared/ui/controls";
import { Banner, Dialog, Spinner } from "@/shared/ui/feedback";
import { EventList } from "./EventRow";
import { Chips, DateField, SearchBox } from "./filters";
import { HistoryTab } from "./HistoryTab";
import { EVENT_TYPES, fromLocalInput, matchesLocal, toLocalInput, type EventType } from "./model";
import { useEvents, type EventsTab } from "./store";

const TABS: EventsTab[] = ["live", "journal", "docker", "history"];
const RESOURCE_TYPES = ["container", "image", "network", "volume"];

function useTypeLabel() {
  const { t } = useTranslation();
  return (v: string) => t(`events.type.${v}`, { defaultValue: v });
}

function LiveTab() {
  const { t } = useTranslation();
  const typeLabel = useTypeLabel();
  const { live, pending, paused, setPaused, load, clearView } = useEvents();
  const connected = useEngine((s) => s.state?.state === "connected");
  const [types, setTypes] = useState<EventType[]>([]);
  const [text, setText] = useState("");
  useEffect(() => void load(), [load]);
  // Solo los eventos del servidor de este ámbito (0.10.0).
  const server = useServerId();
  const mine = (e: { server: { id: string } | null }) => (e.server?.id ?? "local") === server;
  const visible = useMemo(() => live.filter((e) => mine(e) && matchesLocal(e, { types, text })), [live, types, text, server]); // eslint-disable-line react-hooks/exhaustive-deps
  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-3">
        <Chips label={t("events.filterType")} items={EVENT_TYPES} selected={types} onChange={setTypes} render={typeLabel} />
        <SearchBox value={text} onChange={setText} placeholder={t("events.search")} />
        <div className="ml-auto flex items-center gap-1.5">
          {paused ? (
            <Button size="sm" onClick={() => setPaused(false)}>
              <Play size={13} />
              {pending.filter(mine).length > 0 ? t("events.resumeN", { count: pending.filter(mine).length }) : t("events.resume")}
            </Button>
          ) : (
            <Button size="sm" onClick={() => setPaused(true)}>
              <Pause size={13} />
              {t("events.pause")}
            </Button>
          )}
          <Button size="sm" variant="ghost" onClick={clearView} title={t("events.clearViewHint")}>
            <Eraser size={13} />
            {t("events.clearView")}
          </Button>
        </div>
      </div>
      <p className="flex items-center gap-2 text-[12px] text-ink-3" data-testid="events-live-status">
        <span className={cx("inline-block h-2 w-2 rounded-full", connected && !paused ? "bg-ok" : "bg-off")} aria-hidden />
        {!connected ? t("events.liveOff") : paused ? t("events.livePaused") : t("events.liveOn")} · {t("events.count", { count: visible.length })}
      </p>
      <EventList events={visible} empty={live.length === 0 ? t("events.liveEmpty") : t("events.noMatches")} />
    </div>
  );
}

function JournalTab() {
  const api = useApi();
  const server = api.server;
  const { t } = useTranslation();
  const qc = useQueryClient();
  const typeLabel = useTypeLabel();
  const [since, setSince] = useState(() => toLocalInput(Math.floor(Date.now() / 1000) - 24 * 3600));
  const [until, setUntil] = useState("");
  const [types, setTypes] = useState<EventType[]>([]);
  const [actions, setActions] = useState("");
  const [text, setText] = useState("");
  const [confirm, setConfirm] = useState(false);
  const filter = {
    since: fromLocalInput(since),
    until: fromLocalInput(until),
    types,
    actions: actions.split(",").map((a) => a.trim()).filter(Boolean),
    text,
    limit: 1000,
    servers: [server],
  };
  const q = useQuery({ queryKey: ["eventsJournal", filter], queryFn: () => api.eventsJournal(filter) });
  const clear = useMutation({
    mutationFn: api.eventsJournalClear,
    onSuccess: () => {
      setConfirm(false);
      useEvents.getState().clearView();
      void qc.invalidateQueries({ queryKey: ["eventsJournal"] });
    },
  });
  return (
    <div className="space-y-3">
      <p className="max-w-[820px] text-[12.5px] text-ink-3">{t("events.journalIntro")}</p>
      {q.data && !q.data.persistent && <Banner tone="warn">{t("events.journalOff")}</Banner>}
      <div className="flex flex-wrap items-center gap-3">
        <DateField label={t("events.since")} value={since} onChange={setSince} />
        <DateField label={t("events.until")} value={until} onChange={setUntil} />
        <label className="flex items-center gap-1.5 text-[12.5px] text-ink-2">
          {t("events.actions")}
          <input value={actions} onChange={(e) => setActions(e.target.value)} placeholder="start, die" className="h-8 w-[150px] rounded-md border border-line bg-surface px-2 font-mono text-[12px] outline-none hover:border-line-strong focus:border-beacon" />
        </label>
        <SearchBox value={text} onChange={setText} placeholder={t("events.search")} />
      </div>
      <div className="flex flex-wrap items-center gap-3">
        <Chips label={t("events.filterType")} items={EVENT_TYPES} selected={types} onChange={setTypes} render={typeLabel} />
        <div className="ml-auto flex items-center gap-1.5">
          <Button size="sm" onClick={() => void q.refetch()} disabled={q.isFetching}>
            {q.isFetching ? <Spinner size={13} /> : <RefreshCw size={13} />}
            {t("events.refresh")}
          </Button>
          <Button size="sm" variant="ghost" onClick={() => setConfirm(true)} disabled={!q.data || q.data.stored === 0}>
            <Trash2 size={13} />
            {t("events.journalClear")}
          </Button>
        </div>
      </div>
      {q.data && (
        <p className="text-[12px] text-ink-3" data-testid="journal-count">
          {t("events.journalCount", { shown: q.data.events.length, total: q.data.total, stored: q.data.stored })}
        </p>
      )}
      {q.error && <Banner tone="error">{(q.error as { message: string }).message}</Banner>}
      {q.isPending ? (
        <p className="flex items-center gap-2 py-10 text-[13px] text-ink-3">
          <Spinner /> {t("common.loading")}
        </p>
      ) : (
        <EventList events={q.data?.events ?? []} withDate empty={t("events.noMatches")} />
      )}
      {confirm && (
        <Dialog
          target={false}
          title={t("events.journalClearTitle")}
          onClose={() => setConfirm(false)}
          footer={
            <>
              <Button onClick={() => setConfirm(false)}>{t("containers.cancel")}</Button>
              <Button variant="danger" onClick={() => clear.mutate()} disabled={clear.isPending}>
                <Trash2 size={14} />
                {t("events.journalClear")}
              </Button>
            </>
          }
        >
          <p className="text-[13px]">{t("events.journalClearBody", { count: q.data?.stored ?? 0 })}</p>
          {clear.error && <Banner tone="error">{(clear.error as { message: string }).message}</Banner>}
        </Dialog>
      )}
    </div>
  );
}

function DockerTab() {
  const api = useApi();
  const { t } = useTranslation();
  const typeLabel = useTypeLabel();
  const now = () => Math.floor(Date.now() / 1000);
  const [since, setSince] = useState(() => toLocalInput(now() - 3600));
  const [until, setUntil] = useState("");
  const [type, setType] = useState<string>("all");
  const [actions, setActions] = useState("");
  const [resource, setResource] = useState("");
  const canResource = RESOURCE_TYPES.includes(type);
  const m = useMutation({
    mutationFn: () => {
      const q: EventQuery = {
        since: fromLocalInput(since) ?? 0,
        until: fromLocalInput(until) ?? now(),
        types: type === "all" ? [] : [type],
        actions: actions.split(",").map((a) => a.trim()).filter(Boolean),
        resource: canResource && resource.trim() ? resource.trim() : null,
      };
      return api.eventsQuery(q);
    },
  });
  return (
    <div className="space-y-3">
      <Banner tone="info">{t("events.dockerIntro")}</Banner>
      <div className="flex flex-wrap items-center gap-3">
        <DateField label={t("events.since")} value={since} onChange={setSince} />
        <DateField label={t("events.until")} value={until} onChange={setUntil} />
        <label className="flex items-center gap-1.5 text-[12.5px] text-ink-2">
          {t("events.filterType")}
          <select value={type} onChange={(e) => setType(e.target.value)} className="h-8 rounded-md border border-line bg-surface px-2 text-[12.5px] text-ink outline-none focus:border-beacon">
            <option value="all">{t("events.allTypes")}</option>
            {EVENT_TYPES.map((x) => (
              <option key={x} value={x}>
                {typeLabel(x)}
              </option>
            ))}
          </select>
        </label>
        <label className="flex items-center gap-1.5 text-[12.5px] text-ink-2">
          {t("events.actions")}
          <input value={actions} onChange={(e) => setActions(e.target.value)} placeholder="start, die" className="h-8 w-[140px] rounded-md border border-line bg-surface px-2 font-mono text-[12px] outline-none hover:border-line-strong focus:border-beacon" />
        </label>
        <label className="flex items-center gap-1.5 text-[12.5px] text-ink-2">
          {t("events.resource")}
          <input
            value={resource}
            onChange={(e) => setResource(e.target.value)}
            disabled={!canResource}
            placeholder={canResource ? t("events.resourcePh") : t("events.resourceNeedsType")}
            className="h-8 w-[200px] rounded-md border border-line bg-surface px-2 font-mono text-[12px] outline-none hover:border-line-strong focus:border-beacon disabled:opacity-50"
          />
        </label>
        <Button variant="primary" size="sm" onClick={() => m.mutate()} disabled={m.isPending}>
          {m.isPending ? <Spinner size={13} /> : <Search size={13} />}
          {t("events.query")}
        </Button>
      </div>
      {m.error && <Banner tone="error">{(m.error as { message: string }).message}</Banner>}
      {m.data && (
        <>
          <div className="flex items-start gap-2 rounded-md bg-code px-2.5 py-1.5">
            <code data-testid="events-query-command" className="selectable flex-1 font-mono text-[11.5px] [overflow-wrap:anywhere]">
              {m.data.command}
            </code>
            <CopyButton text={m.data.command} compact />
          </div>
          {m.data.truncated && <Banner tone="warn">{t("events.truncated")}</Banner>}
          <p className="text-[12px] text-ink-3">{t("events.count", { count: m.data.events.length })}</p>
          <EventList events={[...m.data.events].reverse()} withDate empty={t("events.dockerEmpty")} />
        </>
      )}
    </div>
  );
}

export function EventsView() {
  const { t } = useTranslation();
  const tab = useEvents((s) => s.tab);
  const setTab = useEvents((s) => s.setTab);
  return (
    <div className="px-8 py-6">
      <div className="mb-4">
        <Segmented<EventsTab> label={t("events.tabsLabel")} value={tab} onChange={setTab} options={TABS.map((x) => ({ value: x, label: t(`events.tabs.${x}`) }))} />
      </div>
      {tab === "live" && <LiveTab />}
      {tab === "journal" && <JournalTab />}
      {tab === "docker" && <DockerTab />}
      {tab === "history" && <HistoryTab />}
    </div>
  );
}
