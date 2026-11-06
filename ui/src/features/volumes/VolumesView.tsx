import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Box, Gauge, Plus, RefreshCw, Search, Sparkles, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { RequireConnection } from "@/features/engine/RequireConnection";
import { Badge } from "@/features/networks/badges";
import { formatBytes } from "@/shared/format";
import { api } from "@/shared/ipc/client";
import type { VolumeSummary } from "@/shared/ipc/bindings/VolumeSummary";
import { keys } from "@/shared/query";
import { Button, Segmented } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { CreateVolume } from "./CreateVolume";
import { PruneVolumesDialog, RemoveVolumeDialog } from "./dialogs";
import { createContainerWith, matchesVolume, volumeMount, type VolumeFilter } from "./model";
import { useVolumesNav } from "./store";
import { useVolumeSizes, volumeLabel } from "./common";
import { VolumeDetail } from "./VolumeDetail";

type Dialogs = { kind: "prune" } | { kind: "remove"; volume: VolumeSummary } | null;

function IconAction({ label, onClick, children, danger = false }: { label: string; onClick: () => void; children: React.ReactNode; danger?: boolean }) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={label}
      title={label}
      className={`grid h-7 w-7 place-items-center rounded-md text-ink-2 hover:bg-surface-sunk ${danger ? "hover:text-err" : "hover:text-ink"}`}
    >
      {children}
    </button>
  );
}

function VolumesTable() {
  const { t, i18n } = useTranslation();
  const nav = useVolumesNav();
  const q = useQuery({ queryKey: keys.volumes, queryFn: api.volumesList });
  const sizes = useVolumeSizes();
  const [filter, setFilter] = useState<VolumeFilter>("all");
  const [query, setQuery] = useState("");
  const [dialog, setDialog] = useState<Dialogs>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const all = q.data ?? [];
  const visible = useMemo(() => all.filter((v) => matchesVolume(v, filter, query)), [all, filter, query]);
  const sizeOf = (name: string) => sizes.data?.find((s) => s.name === name);

  return (
    <div className="px-8 py-6">
      <div className="mb-4 flex flex-wrap items-center gap-2">
        <Button variant="primary" onClick={nav.showCreate}>
          <Plus size={15} />
          {t("volumes.create")}
        </Button>
        <Button onClick={() => setDialog({ kind: "prune" })} disabled={all.length === 0}>
          <Sparkles size={15} />
          {t("volumes.prune")}
        </Button>
        <Button onClick={() => void sizes.refetch()} disabled={sizes.isFetching || all.length === 0}>
          {sizes.isFetching ? <Spinner size={14} /> : <Gauge size={15} />}
          {sizes.data ? t("volumes.sizesAgain") : t("volumes.sizes")}
        </Button>
        <Button className="ml-auto" onClick={() => void q.refetch()} disabled={q.isFetching}>
          {q.isFetching ? <Spinner size={14} /> : <RefreshCw size={14} />}
          {t("volumes.refresh")}
        </Button>
      </div>

      {notice && (
        <div className="mb-4">
          <Banner tone="info" onDismiss={() => setNotice(null)} dismissLabel={t("common.dismiss")}>
            {notice}
          </Banner>
        </div>
      )}
      {sizes.error && (
        <div className="mb-4">
          <Banner tone="error">{t("volumes.sizesError", { message: (sizes.error as { message: string }).message })}</Banner>
        </div>
      )}

      <div className="mb-4 flex flex-wrap items-center gap-3">
        <Segmented<VolumeFilter>
          label={t("volumes.filterLabel")}
          value={filter}
          onChange={setFilter}
          options={(["all", "inUse", "unused", "anonymous"] as VolumeFilter[]).map((f) => ({ value: f, label: t(`volumes.filter.${f}`) }))}
        />
        <label className="relative max-w-[360px] min-w-[220px] flex-1">
          <span className="sr-only">{t("volumes.search")}</span>
          <Search size={15} className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-ink-3" />
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("volumes.search")}
            className="h-8 w-full rounded-md border border-line bg-surface pr-2 pl-8 text-[13px] outline-none placeholder:text-ink-3 hover:border-line-strong focus:border-beacon"
          />
        </label>
        <span className="text-[12.5px] text-ink-3 tabular-nums">{t("volumes.count", { count: visible.length })}</span>
      </div>

      {q.error && (
        <div className="mb-4">
          <Banner tone="error">{t("volumes.loadError", { message: (q.error as { message: string }).message })}</Banner>
        </div>
      )}

      {q.isPending ? (
        <div className="flex items-center justify-center gap-2 py-20 text-[13px] text-ink-3">
          <Spinner /> {t("common.loading")}
        </div>
      ) : all.length === 0 && !q.error ? (
        <div className="py-20 text-center">
          <h2 className="text-[15px] font-semibold">{t("volumes.empty")}</h2>
          <p className="mx-auto mt-1.5 max-w-[440px] text-[13px] text-ink-2">{t("volumes.emptyBody")}</p>
        </div>
      ) : (
        <div className="overflow-x-auto rounded-xl border border-line bg-surface">
          <table className="w-full min-w-[820px] table-fixed text-left text-[13px]">
            <thead className="border-b border-line bg-surface-2 text-[12px] text-ink-3">
              <tr>
                <th className="w-[32%] px-4 py-2 font-medium">{t("volumes.columns.name")}</th>
                <th className="w-[96px] px-3 py-2 font-medium">{t("volumes.columns.driver")}</th>
                <th className="w-[110px] px-3 py-2 text-right font-medium">{t("volumes.columns.size")}</th>
                <th className="px-3 py-2 font-medium">{t("volumes.columns.containers")}</th>
                <th className="w-[84px] px-3 py-2 text-right font-medium">
                  <span className="sr-only">{t("volumes.columns.actions")}</span>
                </th>
              </tr>
            </thead>
            <tbody className="divide-y divide-line">
              {visible.map((v) => {
                const sz = sizeOf(v.name);
                return (
                  <tr key={v.name} className="align-top">
                    <td className="px-4 py-2.5">
                      <button type="button" onClick={() => nav.showDetail(v.name)} className="block max-w-full truncate text-left font-mono text-[12.5px] hover:text-beacon-strong hover:underline" title={v.name}>
                        {volumeLabel(v)}
                      </button>
                      <div className="mt-1 flex flex-wrap gap-1 empty:hidden">
                        {v.anonymous && <Badge>{t("volumes.anonymous")}</Badge>}
                        {v.composeProject && <Badge tone="accent">{t("networks.badge.compose", { project: v.composeProject })}</Badge>}
                      </div>
                    </td>
                    <td className="px-3 py-2.5 font-mono text-[12px]">{v.driver}</td>
                    <td className="px-3 py-2.5 text-right whitespace-nowrap tabular-nums">
                      {sz ? sz.size === null ? <span className="text-ink-3">{t("volumes.sizeUnknown")}</span> : formatBytes(sz.size, i18n.language) : <span className="text-ink-3">—</span>}
                    </td>
                    <td className="px-3 py-2.5 text-[12.5px]">
                      {v.containers.length === 0 ? (
                        <span className="text-ink-3">{t("volumes.unused")}</span>
                      ) : (
                        <span title={v.containers.map((c) => c.name).join(", ")}>
                          {t("volumes.users", { count: v.containers.length })}
                          <span className="block truncate text-[11.5px] text-ink-3">{[...new Set(v.containers.map((c) => c.name))].join(", ")}</span>
                        </span>
                      )}
                    </td>
                    <td className="px-3 py-2">
                      <div className="flex justify-end gap-0.5">
                        <IconAction label={t("volumes.createContainer", { name: volumeLabel(v) })} onClick={() => createContainerWith(volumeMount(v.name))}>
                          <Box size={15} />
                        </IconAction>
                        <IconAction danger label={t("volumes.removeAction", { name: volumeLabel(v) })} onClick={() => setDialog({ kind: "remove", volume: v })}>
                          <Trash2 size={15} />
                        </IconAction>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          {visible.length === 0 && <p className="px-4 py-10 text-center text-[13px] text-ink-2">{t("volumes.noMatches")}</p>}
          {sizes.data && <p className="border-t border-line px-4 py-2 text-[12px] text-ink-3">{t("volumes.sizesNote")}</p>}
        </div>
      )}

      {dialog?.kind === "prune" && <PruneVolumesDialog onClose={() => setDialog(null)} onDone={(r) => setNotice(t("volOps.pruned", { count: r.deleted.length, size: formatBytes(r.spaceReclaimed, i18n.language) }))} />}
      {dialog?.kind === "remove" && <RemoveVolumeDialog volume={dialog.volume} onClose={() => setDialog(null)} onDone={() => setNotice(t("volOps.removed", { name: volumeLabel(dialog.volume) }))} />}
    </div>
  );
}

function VolumesPages() {
  const page = useVolumesNav((s) => s.page);
  switch (page.kind) {
    case "detail":
      return <VolumeDetail key={page.name} name={page.name} tab={page.tab} />;
    case "create":
      return <CreateVolume />;
    default:
      return <VolumesTable />;
  }
}

export function VolumesView() {
  return (
    <RequireConnection>
      <VolumesPages />
    </RequireConnection>
  );
}
