// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * Gestión masiva de contenedores (1.10.0, etapa 1). Reúne los contenedores de
 * todos los servidores conectados (este equipo y los remotos) en una tabla con
 * filtros, orden y selección, y aplica una misma operación a muchos a la vez.
 * Todo pasa por el backend (`bulk_inventory`, `bulk_run_open`): la vista no
 * habla con Docker.
 */
import { useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ArrowDown, ArrowUp, ChevronLeft, ChevronRight, Pause, Play, RefreshCw, RotateCw, Search, Square, Tags, Trash2, Undo2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApp } from "@/app/store";
import { formatAbsolute, formatRelative, formatTimestamp, shortId, uniquePorts } from "@/shared/format";
import type { BulkRequest } from "@/shared/ipc/bindings/BulkRequest";
import { api } from "@/shared/ipc/client";
import { Button, cx, Switch } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { STATE_DOT } from "@/features/containers/containerUi";
import { ConfirmDialog, LabelsDialog } from "./dialogs";
import {
  EMPTY_FILTER,
  LIFECYCLE,
  MAX_TARGETS,
  PAGE_SIZES,
  applicableCount,
  envLabel,
  imagesOf,
  isFiltered,
  kpis,
  matches,
  pageCount,
  pageOf,
  policyText,
  pruneSelection,
  relevantLabels,
  rowsOf,
  selectAll,
  selectedRows,
  sortRows,
  type BulkFilter,
  type BulkRow,
  type OpKind,
  type Sort,
  type SortKey,
  type StateGroup,
} from "./model";
import { RunPanel } from "./RunPanel";
import { useBulkRun } from "./store";

export const BULK_INVENTORY_KEY = ["bulkInventory"] as const;

const OP_ICON = { start: Play, stop: Square, restart: RotateCw, pause: Pause, unpause: Undo2 } as const;
const GROUPS: StateGroup[] = ["running", "stopped", "paused", "error"];
const REFRESH_CHOICES = [5, 15, 30, 60, 120, 300];

export function BulkView() {
  const { t, i18n } = useTranslation();
  const bulk = useApp((s) => s.config.bulk);
  const updateConfig = useApp((s) => s.updateConfig);
  const phase = useBulkRun((s) => s.phase);
  const runBulk = useBulkRun((s) => s.run);
  const runError = useBulkRun((s) => s.error);
  const busy = phase === "starting" || phase === "running";

  // «Actualizar» vuelve a pedir también los datos de inspect (política de reinicio).
  const force = useRef(false);
  const q = useQuery({
    queryKey: BULK_INVENTORY_KEY,
    queryFn: () => {
      const f = force.current;
      force.current = false;
      return api.bulkInventory(f);
    },
    // Sin actualización automática mientras se ejecuta una operación: al terminar se recarga.
    refetchInterval: bulk.autoRefresh && !busy ? bulk.refreshSecs * 1000 : false,
  });
  const refresh = () => {
    force.current = true;
    void q.refetch();
  };

  const localName = t("servers.local");
  const rows = useMemo(() => rowsOf(q.data, localName), [q.data, localName]);
  const envs = q.data?.environments ?? [];
  const envName = (server: string) => {
    const e = envs.find((x) => x.server === server);
    return e ? envLabel(e, localName) : server;
  };

  const [filter, setFilterState] = useState<BulkFilter>(EMPTY_FILTER);
  const [sort, setSort] = useState<Sort>({ key: "name", dir: "asc" });
  const [page, setPage] = useState(0);
  const [size, setSize] = useState<number>(50);
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [dialog, setDialog] = useState<OpKind | "labels" | null>(null);

  const setFilter = (patch: Partial<BulkFilter>) => {
    setFilterState((f) => ({ ...f, ...patch }));
    setPage(0);
  };

  // Lo que ya no existe deja de estar seleccionado.
  useEffect(() => {
    if (!q.data) return;
    setSel((s) => {
      const next = pruneSelection(s, rows);
      return next.size === s.size ? s : next;
    });
  }, [q.data, rows]);

  const filtered = useMemo(() => sortRows(rows.filter((r) => matches(r, filter)), sort), [rows, filter, sort]);
  const pages = pageCount(filtered.length, size);
  const current = pageOf(filtered, page, size);
  const pageIndex = Math.min(page, pages - 1);
  const chosen = useMemo(() => selectedRows(rows, sel), [rows, sel]);
  const k = kpis(rows);
  const images = useMemo(() => imagesOf(rows), [rows]);
  const tooMany = chosen.length > MAX_TARGETS;

  const pageAllSelected = current.length > 0 && current.every((r) => sel.has(r.key));
  const toggle = (key: string) =>
    setSel((s) => {
      const next = new Set(s);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  const togglePage = () =>
    setSel((s) => {
      if (pageAllSelected) {
        const next = new Set(s);
        for (const r of current) next.delete(r.key);
        return next;
      }
      return selectAll(s, current);
    });

  const sortBy = (key: SortKey) => setSort((s) => (s.key === key ? { key, dir: s.dir === "asc" ? "desc" : "asc" } : { key, dir: key === "created" ? "desc" : "asc" }));

  const run = async (req: BulkRequest) => {
    setDialog(null);
    const ok = await runBulk(req, () => void q.refetch());
    if (ok) setSel(new Set());
  };

  const disconnected = envs.filter((e) => e.connection !== "connected");
  const failedEnvs = envs.filter((e) => e.error);

  return (
    <div className="px-8 py-6">
      <p className="mb-4 max-w-[760px] text-[13px] text-ink-2">{t("bulk.intro")}</p>

      {/* Panel general */}
      <div className="mb-4 grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-5" data-testid="bulk-kpis">
        <Kpi label={t("bulk.kpi.total")} value={k.total} active={filter.state === "all"} onClick={() => setFilter({ state: "all" })} />
        {GROUPS.map((g) => (
          <Kpi key={g} label={t(`bulk.kpi.${g}`)} value={k[g]} tone={g} active={filter.state === g} onClick={() => setFilter({ state: filter.state === g ? "all" : g })} />
        ))}
      </div>
      <div className="mb-4 flex flex-wrap items-center gap-x-4 gap-y-2 text-[12.5px] text-ink-3">
        <span data-testid="bulk-envs">
          {t("bulk.envSummary", { count: envs.length, connected: envs.length - disconnected.length })}
        </span>
        {q.data && <span data-testid="bulk-updated">{t("bulk.updated", { time: formatTimestamp(new Date(q.data.fetchedAt), i18n.language) })}</span>}
        <span className="ml-auto flex items-center gap-2">
          <Switch checked={bulk.autoRefresh} onChange={(v) => void updateConfig({ bulk: { autoRefresh: v } })} label={t("bulk.autoRefresh")} />
          <span>{t("bulk.autoRefresh")}</span>
          <select
            aria-label={t("bulk.refreshEvery")}
            value={bulk.refreshSecs}
            disabled={!bulk.autoRefresh}
            onChange={(e) => void updateConfig({ bulk: { refreshSecs: Number(e.target.value) } })}
            className="h-7 rounded-md border border-line bg-surface px-1.5 text-[12.5px] disabled:opacity-50"
          >
            {[...new Set([...REFRESH_CHOICES, bulk.refreshSecs])].sort((a, b) => a - b).map((s) => (
              <option key={s} value={s}>
                {t("bulk.everySecs", { count: s })}
              </option>
            ))}
          </select>
          <Button onClick={refresh} disabled={q.isFetching}>
            {q.isFetching ? <Spinner size={14} /> : <RefreshCw size={14} />}
            {t("bulk.refresh")}
          </Button>
        </span>
      </div>

      {disconnected.length > 0 && (
        <div className="mb-3">
          <Banner tone="info">{t("bulk.disconnected", { names: disconnected.map((e) => envLabel(e, localName)).join(", ") })}</Banner>
        </div>
      )}
      {failedEnvs.map((e) => (
        <div key={e.server} className="mb-3">
          <Banner tone="error">{t("bulk.envError", { name: envLabel(e, localName), message: e.error!.message })}</Banner>
        </div>
      ))}
      {q.error && (
        <div className="mb-3">
          <Banner tone="error">{t("bulk.loadError", { message: (q.error as { message: string }).message })}</Banner>
        </div>
      )}
      {runError && (
        <div className="mb-3">
          <Banner tone="error" onDismiss={() => useBulkRun.setState({ error: null })} dismissLabel={t("containers.dismiss")}>
            {t("bulk.runError", { message: runError })}
          </Banner>
        </div>
      )}

      <RunPanel envName={envName} />

      {/* Filtros */}
      <div className="mb-3 flex flex-wrap items-center gap-2">
        <label className="relative min-w-[200px] max-w-[320px] flex-1">
          <span className="sr-only">{t("bulk.search")}</span>
          <Search size={15} className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-ink-3" />
          <input
            type="search"
            value={filter.query}
            onChange={(e) => setFilter({ query: e.target.value })}
            placeholder={t("bulk.search")}
            className="h-8 w-full rounded-md border border-line bg-surface pr-2 pl-8 text-[13px] outline-none placeholder:text-ink-3 hover:border-line-strong focus:border-beacon"
          />
        </label>
        <select aria-label={t("bulk.filter.state")} value={filter.state} onChange={(e) => setFilter({ state: e.target.value as BulkFilter["state"] })} className={SELECT}>
          <option value="all">
            {t("bulk.filter.state")}: {t("bulk.filter.all")}
          </option>
          {GROUPS.map((g) => (
            <option key={g} value={g}>
              {t(`bulk.kpi.${g}`)}
            </option>
          ))}
        </select>
        <select aria-label={t("bulk.filter.image")} value={filter.image} onChange={(e) => setFilter({ image: e.target.value })} className={cx(SELECT, "max-w-[220px]")}>
          <option value="all">
            {t("bulk.filter.image")}: {t("bulk.filter.all")}
          </option>
          {images.map((im) => (
            <option key={im} value={im}>
              {im}
            </option>
          ))}
        </select>
        <select aria-label={t("bulk.filter.env")} value={filter.env} onChange={(e) => setFilter({ env: e.target.value })} className={SELECT}>
          <option value="all">
            {t("bulk.filter.env")}: {t("bulk.filter.all")}
          </option>
          {envs.map((e) => (
            <option key={e.server} value={e.server}>
              {envLabel(e, localName)}
            </option>
          ))}
        </select>
        <input
          value={filter.label}
          onChange={(e) => setFilter({ label: e.target.value })}
          placeholder={t("bulk.filter.labelPlaceholder")}
          aria-label={t("bulk.filter.label")}
          className="h-8 w-[200px] rounded-md border border-line bg-surface px-2 font-mono text-[12.5px] outline-none placeholder:font-sans placeholder:text-ink-3 focus:border-beacon"
        />
        {isFiltered(filter) && (
          <Button variant="ghost" onClick={() => setFilter(EMPTY_FILTER)}>
            {t("bulk.filter.clear")}
          </Button>
        )}
        <span className="ml-auto text-[12.5px] text-ink-3 tabular-nums" data-testid="bulk-count">
          {t("bulk.shown", { count: filtered.length, total: rows.length })}
        </span>
      </div>

      {/* Barra contextual */}
      {chosen.length > 0 && (
        <div role="toolbar" aria-label={t("bulk.bar.label")} data-testid="bulk-bar" className="sticky top-0 z-10 mb-3 flex flex-wrap items-center gap-2 rounded-xl border border-beacon/40 bg-beacon-soft px-3 py-2">
          <span className="text-[13px] font-medium text-beacon-strong" data-testid="bulk-selected">
            {t("bulk.bar.selected", { count: chosen.length })}
          </span>
          {filtered.length > 0 && filtered.some((r) => !sel.has(r.key)) && (
            <Button size="sm" variant="ghost" onClick={() => setSel((s) => selectAll(s, filtered))}>
              {t("bulk.bar.selectFiltered", { count: filtered.length })}
            </Button>
          )}
          <Button size="sm" variant="ghost" onClick={() => setSel(new Set())}>
            {t("bulk.bar.clear")}
          </Button>
          <span className="mx-1 h-5 w-px bg-line" aria-hidden />
          {LIFECYCLE.map((op) => {
            const n = applicableCount(op as Exclude<OpKind, "remove" | "restartPolicy">, chosen);
            const Icon = OP_ICON[op as keyof typeof OP_ICON];
            return (
              <Button
                key={op}
                size="sm"
                disabled={busy || tooMany || n === 0}
                onClick={() => setDialog(op)}
                title={n === 0 ? t("bulk.bar.none", { op: t(`bulk.op.${op}`) }) : t("bulk.bar.applies", { count: n, total: chosen.length })}
              >
                <Icon size={13} /> {t(`bulk.action.${op}`)}
                <span className="tabular-nums text-ink-3">{n}</span>
              </Button>
            );
          })}
          <Button size="sm" disabled={busy || tooMany} onClick={() => setDialog("restartPolicy")}>
            <RotateCw size={13} /> {t("bulk.action.restartPolicy")}
          </Button>
          <Button size="sm" onClick={() => setDialog("labels")} title={t("bulk.labels.limitShort")}>
            <Tags size={13} /> {t("bulk.action.labels")}
          </Button>
          <Button size="sm" disabled={busy || tooMany} onClick={() => setDialog("remove")} className="text-err">
            <Trash2 size={13} /> {t("bulk.action.remove")}
          </Button>
          {busy && <span className="text-[12px] text-ink-3">{t("bulk.bar.busy")}</span>}
          {tooMany && <span className="text-[12px] text-err">{t("bulk.bar.tooMany", { max: MAX_TARGETS })}</span>}
        </div>
      )}

      {/* Tabla */}
      {q.isPending ? (
        <div className="flex items-center justify-center gap-2 py-20 text-[13px] text-ink-3">
          <Spinner /> {t("common.loading")}
        </div>
      ) : rows.length === 0 && !q.error ? (
        <div className="py-16 text-center">
          <h2 className="text-[15px] font-semibold">{t("bulk.empty")}</h2>
          <p className="mx-auto mt-1.5 max-w-[440px] text-[13px] text-ink-2">{t("bulk.emptyBody")}</p>
        </div>
      ) : (
        <div className="overflow-x-auto rounded-xl border border-line bg-surface">
          <table className="w-full min-w-[1080px] table-fixed text-left text-[13px]" aria-label={t("bulk.tableLabel")}>
            <thead className="border-b border-line bg-surface-2 text-[12px] text-ink-3">
              <tr>
                <th className="w-[40px] px-3 py-2">
                  <input type="checkbox" checked={pageAllSelected} onChange={togglePage} aria-label={t("bulk.selectPage")} className="h-4 w-4 accent-[var(--beacon)]" />
                </th>
                <Th k="name" sort={sort} onSort={sortBy} className="w-[20%]">
                  {t("bulk.col.name")}
                </Th>
                <Th k="image" sort={sort} onSort={sortBy} className="w-[17%]">
                  {t("bulk.col.image")}
                </Th>
                <Th k="state" sort={sort} onSort={sortBy} className="w-[13%]">
                  {t("bulk.col.state")}
                </Th>
                <Th k="created" sort={sort} onSort={sortBy} className="w-[100px]">
                  {t("bulk.col.created")}
                </Th>
                <th className="w-[12%] px-3 py-2 font-medium">{t("bulk.col.ports")}</th>
                <Th k="restart" sort={sort} onSort={sortBy} className="w-[110px]">
                  {t("bulk.col.restart")}
                </Th>
                <Th k="env" sort={sort} onSort={sortBy} className="w-[110px]">
                  {t("bulk.col.env")}
                </Th>
                <th className="w-[15%] px-3 py-2 font-medium">{t("bulk.col.labels")}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-line">
              {current.map((r) => (
                <Row key={r.key} r={r} selected={sel.has(r.key)} onToggle={toggle} />
              ))}
            </tbody>
          </table>
          {filtered.length === 0 ? (
            <p className="px-4 py-10 text-center text-[13px] text-ink-2">{t("bulk.noMatches")}</p>
          ) : (
            <div className="flex flex-wrap items-center gap-3 border-t border-line px-4 py-2 text-[12.5px] text-ink-3">
              <label className="flex items-center gap-1.5">
                {t("bulk.pageSize")}
                <select
                  value={size}
                  onChange={(e) => {
                    setSize(Number(e.target.value));
                    setPage(0);
                  }}
                  className="h-7 rounded-md border border-line bg-surface px-1.5"
                >
                  {PAGE_SIZES.map((s) => (
                    <option key={s} value={s}>
                      {s}
                    </option>
                  ))}
                </select>
              </label>
              <span className="ml-auto tabular-nums" data-testid="bulk-range">
                {t("bulk.range", { from: pageIndex * size + 1, to: Math.min(filtered.length, (pageIndex + 1) * size), total: filtered.length })}
              </span>
              <Button size="sm" variant="ghost" disabled={pageIndex === 0} onClick={() => setPage(pageIndex - 1)} aria-label={t("bulk.prev")} title={t("bulk.prev")}>
                <ChevronLeft size={15} />
              </Button>
              <Button size="sm" variant="ghost" disabled={pageIndex >= pages - 1} onClick={() => setPage(pageIndex + 1)} aria-label={t("bulk.next")} title={t("bulk.next")}>
                <ChevronRight size={15} />
              </Button>
            </div>
          )}
        </div>
      )}

      {dialog === "labels" && <LabelsDialog rows={chosen} onClose={() => setDialog(null)} />}
      {dialog && dialog !== "labels" && <ConfirmDialog kind={dialog} rows={chosen} onClose={() => setDialog(null)} onConfirm={(req) => void run(req)} />}
    </div>
  );
}

const SELECT = "h-8 rounded-md border border-line bg-surface px-2 text-[12.5px] outline-none hover:border-line-strong focus:border-beacon";

function Kpi({ label, value, tone, active, onClick }: { label: string; value: number; tone?: StateGroup; active: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={active}
      className={cx(
        "rounded-xl border bg-surface px-4 py-3 text-left transition-colors hover:border-line-strong",
        active ? "border-beacon ring-1 ring-beacon/40" : "border-line",
      )}
    >
      <div className="flex items-center gap-1.5 text-[12px] text-ink-3">
        {tone && <span aria-hidden className={cx("h-2 w-2 rounded-full", tone === "running" ? "bg-ok" : tone === "paused" ? "bg-warn" : tone === "error" ? "bg-err" : "bg-off")} />}
        {label}
      </div>
      <div className="mt-1 text-[22px] font-semibold tabular-nums">{value}</div>
    </button>
  );
}

function Th({ k, sort, onSort, className, children }: { k: SortKey; sort: Sort; onSort: (k: SortKey) => void; className?: string; children: React.ReactNode }) {
  const active = sort.key === k;
  return (
    <th className={cx("px-3 py-2 font-medium", className)} aria-sort={active ? (sort.dir === "asc" ? "ascending" : "descending") : "none"}>
      <button type="button" onClick={() => onSort(k)} className={cx("inline-flex items-center gap-1 hover:text-ink", active && "text-ink")}>
        {children}
        {active && (sort.dir === "asc" ? <ArrowUp size={12} /> : <ArrowDown size={12} />)}
      </button>
    </th>
  );
}

function Row({ r, selected, onToggle }: { r: BulkRow; selected: boolean; onToggle: (key: string) => void }) {
  const { t, i18n } = useTranslation();
  const c = r.c;
  const ports = uniquePorts(c.ports);
  const labels = relevantLabels(c.labels);
  return (
    <tr className={cx("align-top", selected && "bg-beacon-soft/50")} data-testid="bulk-row">
      <td className="px-3 py-2.5">
        <input type="checkbox" checked={selected} onChange={() => onToggle(r.key)} aria-label={t("bulk.selectOne", { name: c.name, env: r.envName })} className="h-4 w-4 accent-[var(--beacon)]" />
      </td>
      <td className="px-3 py-2.5">
        <div className="flex items-center gap-2">
          <span aria-hidden className={cx("h-2 w-2 shrink-0 rounded-full", STATE_DOT[c.state] ?? "bg-off")} />
          <span className="truncate font-medium" title={c.name}>
            {c.name}
          </span>
        </div>
        <div className="mt-0.5 pl-4 font-mono text-[11.5px] text-ink-3 selectable">{shortId(c.id)}</div>
      </td>
      <td className="selectable truncate px-3 py-2.5 font-mono text-[12px]" title={c.image}>
        {c.image}
      </td>
      <td className="px-3 py-2.5">
        <div className={cx(c.error && "text-err")}>
          {t(`containers.state.${c.state}`, { defaultValue: c.state })}
          {c.health && <span className="ml-1 text-[11.5px] text-ink-3">({t(`bulk.health.${c.health}`, { defaultValue: c.health })})</span>}
        </div>
        <div className="truncate text-[11.5px] text-ink-3" title={c.status}>
          {c.status}
        </div>
      </td>
      <td className="truncate px-3 py-2.5 text-[12px] whitespace-nowrap text-ink-2" title={formatAbsolute(c.created, i18n.language)}>
        {formatRelative(c.created, i18n.language)}
      </td>
      <td className="selectable px-3 py-2.5 font-mono text-[11.5px] text-ink-2">
        {ports.length === 0 ? <span className="text-ink-3">—</span> : ports.slice(0, 3).map((p) => <div key={p}>{p}</div>)}
        {ports.length > 3 && <div className="text-ink-3">+{ports.length - 3}</div>}
      </td>
      <td className="px-3 py-2.5 font-mono text-[12px] text-ink-2">
        {c.details ? policyText(c.details.restartPolicy) : <span className="text-ink-3">—</span>}
        {c.details?.autoRemove && <div className="font-sans text-[11px] text-ink-3">--rm</div>}
      </td>
      <td className="px-3 py-2.5">
        <span className={cx("inline-block max-w-full truncate rounded px-1.5 text-[11.5px]", r.remote ? "bg-beacon/15 font-semibold text-beacon-strong" : "bg-surface-sunk text-ink-2")} title={r.envName}>
          {r.envName}
        </span>
      </td>
      <td className="selectable px-3 py-2.5 font-mono text-[11px] text-ink-3" title={labels.map(([k, v]) => `${k}=${v}`).join("\n")}>
        {c.composeProject && <div className="truncate">compose: {c.composeProject}</div>}
        {labels.slice(0, 2).map(([k, v]) => (
          <div key={k} className="truncate">
            {k}={v}
          </div>
        ))}
        {labels.length > 2 && <div>+{labels.length - 2}</div>}
        {!c.composeProject && labels.length === 0 && "—"}
      </td>
    </tr>
  );
}
