// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { RefreshCw, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useServers } from "@/app/server";
import { opKey } from "@/features/activity/ActivityItem";
import { describeTransport, formatDateTime, formatDuration } from "@/features/activity/logic";
import { api } from "@/shared/ipc/client";
import type { Category } from "@/shared/ipc/bindings/Category";
import type { HistoryEntry } from "@/shared/ipc/bindings/HistoryEntry";
import type { OpStatus } from "@/shared/ipc/bindings/OpStatus";
import { Button, cx } from "@/shared/ui/controls";
import { Banner, Dialog, Spinner } from "@/shared/ui/feedback";
import { Chips, DateField, SearchBox } from "./filters";
import { fromLocalInput, toLocalInput } from "./model";

const CATEGORIES: Category[] = ["container", "image", "network", "volume", "compose", "system"];
const STATUSES: OpStatus[] = ["succeeded", "failed", "cancelled"];
const STATUS_TONE: Record<OpStatus, string> = { running: "text-run", succeeded: "text-ok", failed: "text-err", cancelled: "text-off" };

function Row({ h, previous }: { h: HistoryEntry; previous: boolean }) {
  const { t, i18n } = useTranslation();
  const [open, setOpen] = useState(false);
  return (
    <li className="border-b border-line last:border-0">
      <button type="button" aria-expanded={open} onClick={() => setOpen(!open)} className="flex w-full items-center gap-3 px-4 py-2 text-left text-[12.5px] hover:bg-surface-2">
        <time className="w-[178px] shrink-0 font-mono text-[11.5px] text-ink-3 tabular-nums">{formatDateTime(h.startedAt, i18n.language)}</time>
        <span className={cx("w-[82px] shrink-0 text-[12px] font-medium", STATUS_TONE[h.status])}>{t(`activity.status.${h.status}`)}</span>
        <span className="min-w-0 flex-1 truncate">
          {t(opKey(h.kind), { defaultValue: h.kind })}
          {h.target && <span className="ml-2 font-mono text-[12px] text-ink-2">{h.target}</span>}
        </span>
        {h.server?.remote && (
          <span title={h.server.address} className="max-w-[160px] shrink-0 truncate rounded bg-beacon/15 px-1.5 text-[11px] font-semibold text-beacon-strong">
            {h.server.name}
          </span>
        )}
        {previous && <span className="shrink-0 rounded bg-surface-sunk px-1.5 text-[11px] text-ink-3">{t("history.previous")}</span>}
        <span className="w-[72px] shrink-0 text-right text-[11.5px] text-ink-3 tabular-nums">{formatDuration(h.durationMs)}</span>
      </button>
      {open && (
        <dl className="selectable mx-4 mb-2 grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1 rounded-md bg-surface-2 px-3 py-2 text-[12px]">
          <dt className="text-ink-3">{t("servers.target")}</dt>
          <dd>{h.server?.remote ? `${h.server.name} (${h.server.address})` : t("servers.local")}</dd>
          <dt className="text-ink-3">{t("activity.executed")}</dt>
          <dd className="font-mono text-[11.5px] [overflow-wrap:anywhere]">{describeTransport(h.transport)}</dd>
          {h.summary && (
            <>
              <dt className="text-ink-3">{t("activity.summary")}</dt>
              <dd>{h.summary}</dd>
            </>
          )}
          {h.error && (
            <>
              <dt className="text-ink-3">{t("activity.error")}</dt>
              <dd className="text-err [overflow-wrap:anywhere]">{h.error}</dd>
            </>
          )}
          {h.httpStatus !== null && (
            <>
              <dt className="text-ink-3">{t("activity.httpStatus")}</dt>
              <dd className="font-mono">{h.httpStatus}</dd>
            </>
          )}
          {h.exitCode !== null && (
            <>
              <dt className="text-ink-3">{t("activity.exitCode")}</dt>
              <dd className="font-mono">{h.exitCode}</dd>
            </>
          )}
          <dt className="text-ink-3">{t("history.notKept")}</dt>
          <dd className="text-ink-3">{t("history.notKeptBody")}</dd>
        </dl>
      )}
    </li>
  );
}

/** Historial de operaciones guardado entre sesiones (D-058). */
export function HistoryTab() {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const [since, setSince] = useState(() => toLocalInput(Math.floor(Date.now() / 1000) - 7 * 24 * 3600));
  const [until, setUntil] = useState("");
  const [categories, setCategories] = useState<Category[]>([]);
  const [statuses, setStatuses] = useState<OpStatus[]>([]);
  const [text, setText] = useState("");
  const [confirm, setConfirm] = useState(false);
  // 0.10.0: el historial es de todos los servidores; se puede filtrar por uno.
  const [server, setServer] = useState("all");
  const known = useServers((s) => s.servers);
  const filter = { since: fromLocalInput(since), until: fromLocalInput(until), categories, statuses, text, limit: 1000, servers: server === "all" ? [] : [server] };
  const q = useQuery({ queryKey: ["history", filter], queryFn: () => api.historyList(filter) });
  const clear = useMutation({
    mutationFn: api.historyClear,
    onSuccess: () => {
      setConfirm(false);
      void qc.invalidateQueries({ queryKey: ["history"] });
    },
  });
  const sessionStart = q.data ? new Date(q.data.sessionStart).getTime() : 0;
  return (
    <div className="space-y-3">
      <p className="max-w-[820px] text-[12.5px] text-ink-3">{t("history.intro")}</p>
      {q.data && !q.data.persistent && <Banner tone="warn">{t("history.off")}</Banner>}
      <div className="flex flex-wrap items-center gap-3">
        <DateField label={t("events.since")} value={since} onChange={setSince} />
        <DateField label={t("events.until")} value={until} onChange={setUntil} />
        <SearchBox value={text} onChange={setText} placeholder={t("history.search")} />
        {known.some((x) => x.remote) && (
          <label className="flex items-center gap-1.5 text-[12.5px] text-ink-2">
            {t("activity.filterServer")}
            <select value={server} onChange={(e) => setServer(e.target.value)} className="h-8 rounded-md border border-line bg-surface px-2 text-[12.5px]">
              <option value="all">{t("activity.allServers")}</option>
              {known.map((x) => (
                <option key={x.id} value={x.id}>
                  {x.remote ? x.name : t("servers.local")}
                </option>
              ))}
            </select>
          </label>
        )}
      </div>
      <div className="flex flex-wrap items-center gap-3">
        <Chips label={t("activity.filterCategory")} items={CATEGORIES} selected={categories} onChange={setCategories} render={(c) => t(`activity.category.${c}`)} />
        <Chips label={t("activity.filterStatus")} items={STATUSES} selected={statuses} onChange={setStatuses} render={(s) => t(`activity.status.${s}`)} />
        <div className="ml-auto flex items-center gap-1.5">
          <Button size="sm" onClick={() => void q.refetch()} disabled={q.isFetching}>
            {q.isFetching ? <Spinner size={13} /> : <RefreshCw size={13} />}
            {t("events.refresh")}
          </Button>
          <Button size="sm" variant="ghost" onClick={() => setConfirm(true)} disabled={!q.data || q.data.stored === 0}>
            <Trash2 size={13} />
            {t("history.clear")}
          </Button>
        </div>
      </div>
      {q.data && (
        <p className="text-[12px] text-ink-3" data-testid="history-count">
          {t("history.count", { shown: q.data.entries.length, total: q.data.total, stored: q.data.stored })}
          {q.data.location && <span className="ml-2 font-mono">{q.data.location}</span>}
        </p>
      )}
      {q.error && <Banner tone="error">{(q.error as { message: string }).message}</Banner>}
      {q.isPending ? (
        <p className="flex items-center gap-2 py-10 text-[13px] text-ink-3">
          <Spinner /> {t("common.loading")}
        </p>
      ) : (q.data?.entries.length ?? 0) === 0 ? (
        <p className="rounded-xl border border-line bg-surface px-4 py-10 text-center text-[13px] text-ink-2">{q.data?.stored ? t("history.noMatches") : t("history.empty")}</p>
      ) : (
        <ul data-testid="history-list" className="overflow-hidden rounded-xl border border-line bg-surface">
          {q.data!.entries.map((h) => (
            <Row key={h.id} h={h} previous={new Date(h.startedAt).getTime() < sessionStart} />
          ))}
        </ul>
      )}
      {confirm && (
        <Dialog
          target={false}
          title={t("history.clearTitle")}
          onClose={() => setConfirm(false)}
          footer={
            <>
              <Button onClick={() => setConfirm(false)}>{t("containers.cancel")}</Button>
              <Button variant="danger" onClick={() => clear.mutate()} disabled={clear.isPending}>
                <Trash2 size={14} />
                {t("history.clear")}
              </Button>
            </>
          }
        >
          <p className="text-[13px]">{t("history.clearBody", { count: q.data?.stored ?? 0 })}</p>
          {clear.error && <Banner tone="error">{(clear.error as { message: string }).message}</Banner>}
        </Dialog>
      )}
    </div>
  );
}
