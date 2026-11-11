import { useMemo, useState } from "react";
import { useApi } from "@/app/server";
import { useQuery } from "@tanstack/react-query";
import { Box, Download, Hammer, RefreshCw, Search, Sparkles, Tag, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { RequireConnection } from "@/features/engine/RequireConnection";
import { formatAbsolute, formatBytes, formatRelative, shortId } from "@/shared/format";
import type { ImageSummary } from "@/shared/ipc/bindings/ImageSummary";
import { keys } from "@/shared/query";
import { Button, Segmented } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { BuildPage } from "./build/BuildPage";
import { useBuild } from "./build/buildStore";
import { createContainerFrom } from "./createFrom";
import { ImageDetail } from "./detail/ImageDetail";
import { PruneDialog, RemoveImageDialog, TagDialog } from "./dialogs";
import { PullDialog } from "./pull/PullDialog";
import { PullsPanel } from "./pull/PullsPanel";
import { SearchPage } from "./SearchPage";
import { useImagesNav } from "./store";

type Filter = "all" | "used" | "unused" | "dangling";

export function matchesImage(i: ImageSummary, filter: Filter, query: string): boolean {
  if (filter === "used" && i.usedBy.length === 0) return false;
  if (filter === "unused" && i.usedBy.length > 0) return false;
  if (filter === "dangling" && !i.dangling) return false;
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return [...i.repoTags, i.id, ...i.repoDigests].some((v) => v.toLowerCase().includes(q));
}

type Dialogs = { kind: "pull" } | { kind: "prune" } | { kind: "tag"; image: ImageSummary } | { kind: "remove"; image: ImageSummary } | null;

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

function ImagesTable() {
  const api = useApi();
  const { t, i18n } = useTranslation();
  const nav = useImagesNav();
  const build = useBuild((s) => s.status);
  const q = useQuery({ queryKey: keys.images, queryFn: api.imagesList });
  const [filter, setFilter] = useState<Filter>("all");
  const [query, setQuery] = useState("");
  const [dialog, setDialog] = useState<Dialogs>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const all = q.data ?? [];
  const visible = useMemo(() => all.filter((i) => matchesImage(i, filter, query)).sort((a, b) => b.created - a.created), [all, filter, query]);

  return (
    <div className="px-8 py-6">
      <div className="mb-4 flex flex-wrap items-center gap-2">
        <Button variant="primary" onClick={() => setDialog({ kind: "pull" })}>
          <Download size={15} />
          {t("images.pull")}
        </Button>
        <Button onClick={nav.showSearch}>
          <Search size={15} />
          {t("images.searchHub")}
        </Button>
        <Button onClick={() => nav.showBuild()}>
          <Hammer size={15} />
          {t("images.build")}
        </Button>
        <Button onClick={() => setDialog({ kind: "prune" })} disabled={all.length === 0}>
          <Sparkles size={15} />
          {t("images.prune")}
        </Button>
        <Button className="ml-auto" onClick={() => void q.refetch()} disabled={q.isFetching}>
          {q.isFetching ? <Spinner size={14} /> : <RefreshCw size={14} />}
          {t("images.refresh")}
        </Button>
      </div>

      {(build === "running" || build === "starting") && (
        <div className="mb-4">
          <Banner tone="info">
            {t("images.buildRunning")}{" "}
            <button type="button" className="font-medium underline" onClick={() => nav.showBuild()}>
              {t("images.buildShow")}
            </button>
          </Banner>
        </div>
      )}
      {notice && (
        <div className="mb-4">
          <Banner tone="info" onDismiss={() => setNotice(null)} dismissLabel={t("common.dismiss")}>
            {notice}
          </Banner>
        </div>
      )}
      <PullsPanel />

      <div className="mb-4 flex flex-wrap items-center gap-3">
        <Segmented<Filter>
          label={t("images.columns.usage")}
          value={filter}
          onChange={setFilter}
          options={(["all", "used", "unused", "dangling"] as Filter[]).map((f) => ({ value: f, label: t(`images.filter.${f}`) }))}
        />
        <label className="relative max-w-[360px] min-w-[220px] flex-1">
          <span className="sr-only">{t("images.search")}</span>
          <Search size={15} className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-ink-3" />
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("images.search")}
            className="h-8 w-full rounded-md border border-line bg-surface pr-2 pl-8 text-[13px] outline-none placeholder:text-ink-3 hover:border-line-strong focus:border-beacon"
          />
        </label>
        <span className="text-[12.5px] text-ink-3 tabular-nums">{t("images.count", { count: visible.length })}</span>
      </div>

      {q.error && (
        <div className="mb-4">
          <Banner tone="error">{t("images.loadError", { message: (q.error as { message: string }).message })}</Banner>
        </div>
      )}

      {q.isPending ? (
        <div className="flex items-center justify-center gap-2 py-20 text-[13px] text-ink-3">
          <Spinner /> {t("common.loading")}
        </div>
      ) : all.length === 0 && !q.error ? (
        <div className="py-20 text-center">
          <h2 className="text-[15px] font-semibold">{t("images.empty")}</h2>
          <p className="mx-auto mt-1.5 max-w-[420px] text-[13px] text-ink-2">{t("images.emptyBody")}</p>
        </div>
      ) : (
        <div className="overflow-x-auto rounded-xl border border-line bg-surface">
          <table className="w-full min-w-[800px] table-fixed text-left text-[13px]">
            <thead className="border-b border-line bg-surface-2 text-[12px] text-ink-3">
              <tr>
                <th className="w-[34%] px-4 py-2 font-medium">{t("images.columns.tags")}</th>
                <th className="w-[14%] px-3 py-2 font-medium">{t("images.columns.id")}</th>
                <th className="w-[96px] px-3 py-2 text-right font-medium">{t("images.columns.size")}</th>
                <th className="w-[120px] px-3 py-2 font-medium">{t("images.columns.created")}</th>
                <th className="px-3 py-2 font-medium">{t("images.columns.usage")}</th>
                <th className="w-[112px] px-3 py-2 text-right font-medium">
                  <span className="sr-only">{t("images.columns.actions")}</span>
                </th>
              </tr>
            </thead>
            <tbody className="divide-y divide-line">
              {visible.map((i) => {
                const name = i.repoTags[0] ?? shortId(i.id);
                return (
                  <tr key={i.id} className="align-top">
                    <td className="px-4 py-2.5">
                      <button type="button" onClick={() => nav.showDetail(i.id, name)} className="block max-w-full text-left hover:text-beacon-strong hover:underline" title={t("images.open", { name })}>
                        {i.dangling ? (
                          <span className="rounded bg-surface-sunk px-1.5 py-0.5 text-[12px] text-ink-3">{t("images.dangling")}</span>
                        ) : (
                          i.repoTags.map((tag) => (
                            <span key={tag} className="block truncate font-mono text-[12.5px]">
                              {tag}
                            </span>
                          ))
                        )}
                      </button>
                    </td>
                    <td className="selectable px-3 py-2.5 font-mono text-[12px] text-ink-2" title={i.id}>
                      {shortId(i.id)}
                    </td>
                    <td className="px-3 py-2.5 text-right whitespace-nowrap tabular-nums">{formatBytes(i.size, i18n.language)}</td>
                    <td className="truncate px-3 py-2.5 text-[12px] whitespace-nowrap text-ink-2" title={formatAbsolute(i.created, i18n.language)}>
                      {formatRelative(i.created, i18n.language)}
                    </td>
                    <td className="px-3 py-2.5 text-[12.5px]">
                      {i.usedBy.length === 0 ? (
                        <span className="text-ink-3">{t("images.unused")}</span>
                      ) : (
                        <span title={i.usedBy.map((u) => u.name).join(", ")}>
                          {t("images.usedBy", { count: i.usedBy.length })}
                          <span className="block truncate text-[11.5px] text-ink-3">{i.usedBy.map((u) => u.name).join(", ")}</span>
                        </span>
                      )}
                    </td>
                    <td className="px-3 py-2">
                      <div className="flex justify-end gap-0.5">
                        {!i.dangling && (
                          <IconAction label={t("images.createContainer", { name })} onClick={() => createContainerFrom(i.repoTags[0]!)}>
                            <Box size={15} />
                          </IconAction>
                        )}
                        <IconAction label={t("images.tagAction", { name })} onClick={() => setDialog({ kind: "tag", image: i })}>
                          <Tag size={15} />
                        </IconAction>
                        <IconAction danger label={t("images.removeAction", { name })} onClick={() => setDialog({ kind: "remove", image: i })}>
                          <Trash2 size={15} />
                        </IconAction>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          {visible.length === 0 && <p className="px-4 py-10 text-center text-[13px] text-ink-2">{t("images.noMatches")}</p>}
        </div>
      )}

      {dialog?.kind === "pull" && <PullDialog onClose={() => setDialog(null)} />}
      {dialog?.kind === "prune" && <PruneDialog images={all} onClose={() => setDialog(null)} onDone={(r) => setNotice(t("imageOps.pruned", { count: r.deleted.length + r.untagged.length, size: formatBytes(r.spaceReclaimed, i18n.language) }))} />}
      {dialog?.kind === "tag" && <TagDialog image={dialog.image} onClose={() => setDialog(null)} onDone={setNotice} />}
      {dialog?.kind === "remove" && <RemoveImageDialog image={dialog.image} onClose={() => setDialog(null)} onDone={(r) => setNotice(t("imageOps.removed", { untagged: r.untagged.length, deleted: r.deleted.length }))} />}
    </div>
  );
}

function ImagesPages() {
  const page = useImagesNav((s) => s.page);
  switch (page.kind) {
    case "detail":
      return <ImageDetail key={page.id} id={page.id} name={page.name} tab={page.tab} />;
    case "search":
      return <SearchPage />;
    case "build":
      return <BuildPage prefill={page.prefill} />;
    default:
      return <ImagesTable />;
  }
}

export function ImagesView() {
  return (
    <RequireConnection>
      <ImagesPages />
    </RequireConnection>
  );
}
