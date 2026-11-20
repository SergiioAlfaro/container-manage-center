// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useMemo } from "react";
import { Search, Trash2, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { Category } from "@/shared/ipc/bindings/Category";
import type { OpStatus } from "@/shared/ipc/bindings/OpStatus";
import { Button } from "@/shared/ui/controls";
import { ActivityItem } from "./ActivityItem";
import { CATEGORIES, STATUSES, isFiltered, matches } from "./logic";
import { useServers } from "@/app/server";
import { useActivity } from "./store";

const selectClass =
  "h-8 min-w-0 flex-1 rounded-md border border-line bg-surface px-2 text-[12.5px] text-ink outline-none hover:border-line-strong";

export function ActivityPanel({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const { records, filter, setFilter, resetFilter, clear, loadError } = useActivity();

  const visible = useMemo(() => records.filter((r) => matches(r, filter)), [records, filter]);
  // Servidores conocidos y los que aparecen en los registros (también los ya eliminados).
  const known = useServers((s) => s.servers);
  const servers = useMemo(() => {
    const m = new Map<string, string>();
    for (const s of known) m.set(s.id, s.remote ? s.name : t("servers.local"));
    for (const r of records) if (r.server && !m.has(r.server.id)) m.set(r.server.id, r.server.remote ? r.server.name : t("servers.local"));
    if (!m.has("local")) m.set("local", t("servers.local"));
    return [...m.entries()].map(([id, name]) => ({ id, name }));
  }, [known, records, t]);
  const running = records.filter((r) => r.status === "running").length;
  const failed = records.filter((r) => r.status === "failed").length;

  return (
    <aside
      aria-label={t("activity.title")}
      className="panel-enter flex h-full w-[440px] shrink-0 flex-col border-l border-line bg-log shadow-[var(--shadow-panel)]"
    >
      <header className="flex h-14 shrink-0 items-center gap-3 border-b border-line px-4">
        <div className="min-w-0">
          <h2 className="text-[14px] font-semibold leading-tight">{t("activity.title")}</h2>
          <p className="truncate text-[12px] text-ink-3">{t("activity.subtitle")}</p>
        </div>
        <div className="ml-auto flex items-center gap-1.5 text-[11.5px]">
          {running > 0 && <span className="rounded-full bg-run/12 px-2 py-0.5 font-medium text-run">{t("activity.running", { count: running })}</span>}
          {failed > 0 && <span className="rounded-full bg-err/12 px-2 py-0.5 font-medium text-err">{t("activity.failed", { count: failed })}</span>}
        </div>
        <button
          type="button"
          onClick={onClose}
          aria-label={t("activity.close")}
          title={`${t("activity.close")} (${t("activity.shortcut")})`}
          className="grid h-8 w-8 place-items-center rounded-md text-ink-3 hover:bg-surface-sunk hover:text-ink"
        >
          <X size={17} />
        </button>
      </header>

      <div className="shrink-0 space-y-2 border-b border-line px-4 py-3">
        {filter.batch && (
          // 1.10.0: «Ver en actividad» de una operación masiva muestra solo sus registros.
          <div data-testid="activity-batch-filter" className="flex items-center gap-2 rounded-md bg-beacon-soft px-2.5 py-1.5 text-[12px] text-beacon-strong">
            <span className="min-w-0 flex-1 truncate">
              {t("activity.batchFilter")} <span className="selectable font-mono">{filter.batch.slice(0, 8)}</span>
            </span>
            <button type="button" onClick={() => setFilter({ batch: null })} aria-label={t("activity.batchFilterClear")} title={t("activity.batchFilterClear")} className="shrink-0 hover:text-ink">
              <X size={14} />
            </button>
          </div>
        )}
        <label className="relative block">
          <span className="sr-only">{t("activity.search")}</span>
          <Search size={15} className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-ink-3" />
          <input
            type="search"
            value={filter.query}
            onChange={(e) => setFilter({ query: e.target.value })}
            placeholder={t("activity.search")}
            className="h-8 w-full rounded-md border border-line bg-surface pr-2 pl-8 text-[13px] text-ink outline-none placeholder:text-ink-3 hover:border-line-strong focus:border-beacon"
          />
        </label>
        <div className="flex items-center gap-2">
          <select
            aria-label={t("activity.filterCategory")}
            value={filter.category}
            onChange={(e) => setFilter({ category: e.target.value as Category | "all" })}
            className={selectClass}
          >
            <option value="all">
              {t("activity.filterCategory")}: {t("activity.all")}
            </option>
            {CATEGORIES.map((c) => (
              <option key={c} value={c}>
                {t(`activity.category.${c}`)}
              </option>
            ))}
          </select>
          <select
            aria-label={t("activity.filterStatus")}
            value={filter.status}
            onChange={(e) => setFilter({ status: e.target.value as OpStatus | "all" })}
            className={selectClass}
          >
            <option value="all">
              {t("activity.filterStatus")}: {t("activity.allStatus")}
            </option>
            {STATUSES.map((s) => (
              <option key={s} value={s}>
                {t(`activity.status.${s}`)}
              </option>
            ))}
          </select>
          {servers.length > 1 && (
            <select aria-label={t("activity.filterServer")} value={filter.server} onChange={(e) => setFilter({ server: e.target.value })} className={selectClass}>
              <option value="all">
                {t("activity.filterServer")}: {t("activity.allServers")}
              </option>
              {servers.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </select>
          )}
          <Button
            variant="quiet"
            onClick={() => void clear()}
            disabled={records.length === 0}
            title={t("activity.clearHint")}
            aria-label={t("activity.clear")}
            className="w-8 shrink-0 px-0"
          >
            <Trash2 size={15} strokeWidth={1.75} />
          </Button>
        </div>
      </div>

      {loadError && (
        <p role="alert" className="mx-4 mt-3 rounded-md bg-err/10 px-3 py-2 text-[12.5px] text-err">
          {t("activity.loadError", { message: loadError })}
        </p>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto">
        {records.length === 0 ? (
          <div className="px-8 py-16 text-center">
            <p className="font-mono text-[13px] text-ink-3">$ _</p>
            <h3 className="mt-3 text-[14px] font-medium">{t("activity.emptyTitle")}</h3>
            <p className="mx-auto mt-1.5 max-w-[300px] text-[12.5px] leading-relaxed text-ink-2">{t("activity.emptyBody")}</p>
          </div>
        ) : visible.length === 0 ? (
          <div className="px-8 py-12 text-center text-[13px] text-ink-2">
            <p>{t("activity.noMatches")}</p>
            {isFiltered(filter) && (
              <Button className="mt-3" size="sm" onClick={resetFilter}>
                {t("activity.resetFilters")}
              </Button>
            )}
          </div>
        ) : (
          <ul aria-label={t("activity.title")}>
            {visible.map((r) => (
              <ActivityItem key={r.id} record={r} />
            ))}
          </ul>
        )}
      </div>
    </aside>
  );
}
