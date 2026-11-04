import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { RefreshCw, Search } from "lucide-react";
import { useTranslation } from "react-i18next";
import { RequireConnection } from "@/features/engine/RequireConnection";
import { formatAbsolute, formatBytes, formatRelative, shortId } from "@/shared/format";
import { api } from "@/shared/ipc/client";
import type { ImageSummary } from "@/shared/ipc/bindings/ImageSummary";
import { keys } from "@/shared/query";
import { Button, Segmented } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";

type Filter = "all" | "used" | "unused" | "dangling";

export function matchesImage(i: ImageSummary, filter: Filter, query: string): boolean {
  if (filter === "used" && i.usedBy.length === 0) return false;
  if (filter === "unused" && i.usedBy.length > 0) return false;
  if (filter === "dangling" && !i.dangling) return false;
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return [...i.repoTags, i.id, ...i.repoDigests].some((v) => v.toLowerCase().includes(q));
}

function ImagesTable() {
  const { t, i18n } = useTranslation();
  const q = useQuery({ queryKey: keys.images, queryFn: api.imagesList });
  const [filter, setFilter] = useState<Filter>("all");
  const [query, setQuery] = useState("");
  const all = q.data ?? [];
  const visible = useMemo(() => all.filter((i) => matchesImage(i, filter, query)).sort((a, b) => b.created - a.created), [all, filter, query]);

  return (
    <div className="px-8 py-6">
      <div className="mb-4 flex flex-wrap items-center gap-3">
        <Segmented<Filter>
          label={t("images.columns.usage")}
          value={filter}
          onChange={setFilter}
          options={(["all", "used", "unused", "dangling"] as Filter[]).map((f) => ({ value: f, label: t(`images.filter.${f}`) }))}
        />
        <label className="relative min-w-[220px] flex-1 max-w-[360px]">
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
        <Button className="ml-auto" onClick={() => void q.refetch()} disabled={q.isFetching}>
          {q.isFetching ? <Spinner size={14} /> : <RefreshCw size={14} />}
          {t("images.refresh")}
        </Button>
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
          <table className="w-full min-w-[720px] table-fixed text-left text-[13px]">
            <thead className="border-b border-line bg-surface-2 text-[12px] text-ink-3">
              <tr>
                <th className="w-[38%] px-4 py-2 font-medium">{t("images.columns.tags")}</th>
                <th className="w-[15%] px-3 py-2 font-medium">{t("images.columns.id")}</th>
                <th className="w-[96px] px-3 py-2 text-right font-medium">{t("images.columns.size")}</th>
                <th className="w-[120px] px-3 py-2 font-medium">{t("images.columns.created")}</th>
                <th className="px-3 py-2 font-medium">{t("images.columns.usage")}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-line">
              {visible.map((i) => (
                <tr key={i.id} className="align-top">
                  <td className="px-4 py-2.5">
                    {i.dangling ? (
                      <span className="rounded bg-surface-sunk px-1.5 py-0.5 text-[12px] text-ink-3">{t("images.dangling")}</span>
                    ) : (
                      i.repoTags.map((tag) => (
                        <div key={tag} className="selectable truncate font-mono text-[12.5px]" title={tag}>
                          {tag}
                        </div>
                      ))
                    )}
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
                </tr>
              ))}
            </tbody>
          </table>
          {visible.length === 0 && <p className="px-4 py-10 text-center text-[13px] text-ink-2">{t("images.noMatches")}</p>}
        </div>
      )}
      <p className="mt-3 text-[12px] text-ink-3">{t("images.later")}</p>
    </div>
  );
}

export function ImagesView() {
  return (
    <RequireConnection>
      <ImagesTable />
    </RequireConnection>
  );
}
