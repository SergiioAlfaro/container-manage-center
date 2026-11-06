import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { ArrowLeft, BadgeCheck, Download, Search, Star } from "lucide-react";
import { useTranslation } from "react-i18next";
import { api } from "@/shared/ipc/client";
import { Button } from "@/shared/ui/controls";
import { Banner, Checkbox, Spinner } from "@/shared/ui/feedback";
import { PullDialog } from "./pull/PullDialog";
import { PullsPanel } from "./pull/PullsPanel";
import { useImagesNav } from "./store";

const LIMITS = [10, 25, 50, 100];

/** Búsqueda en Docker Hub a través del daemon (`docker search`). */
export function SearchPage() {
  const { t, i18n } = useTranslation();
  const nav = useImagesNav();
  const [term, setTerm] = useState("");
  const [official, setOfficial] = useState(false);
  const [limit, setLimit] = useState(25);
  const [pulling, setPulling] = useState<string | null>(null);
  const search = useMutation({ mutationFn: () => api.imagesSearch(term.trim(), limit, official) });
  const nf = new Intl.NumberFormat(i18n.language === "es" ? "es-ES" : "en-US");

  return (
    <div className="flex min-h-full flex-col">
      <div className="flex items-center gap-3 border-b border-line px-8 py-3">
        <Button variant="ghost" size="sm" onClick={nav.showList}>
          <ArrowLeft size={15} />
          {t("imageDetail.back")}
        </Button>
        <h2 className="text-[15px] font-semibold">{t("search.title")}</h2>
      </div>
      <div className="px-8 py-5">
        <form
          className="mb-4 flex flex-wrap items-center gap-3"
          onSubmit={(e) => {
            e.preventDefault();
            if (term.trim()) search.mutate();
          }}
        >
          <label className="relative max-w-[420px] min-w-[240px] flex-1">
            <span className="sr-only">{t("search.term")}</span>
            <Search size={15} className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-ink-3" />
            <input
              type="search"
              value={term}
              onChange={(e) => setTerm(e.target.value)}
              placeholder={t("search.placeholder")}
              aria-label={t("search.term")}
              autoFocus
              className="h-8 w-full rounded-md border border-line bg-surface pr-2 pl-8 text-[13px] outline-none placeholder:text-ink-3 hover:border-line-strong focus:border-beacon"
            />
          </label>
          <Checkbox checked={official} onChange={setOfficial} label={t("search.officialOnly")} />
          <label className="flex items-center gap-2 text-[12.5px] text-ink-2">
            {t("search.limit")}
            <select value={limit} onChange={(e) => setLimit(Number(e.target.value))} className="h-7 rounded-md border border-line bg-surface px-1.5 text-[12.5px]">
              {LIMITS.map((n) => (
                <option key={n} value={n}>
                  {n}
                </option>
              ))}
            </select>
          </label>
          <Button type="submit" variant="primary" disabled={!term.trim() || search.isPending}>
            {search.isPending ? <Spinner size={14} /> : <Search size={14} />}
            {t("search.submit")}
          </Button>
        </form>
        <p className="mb-4 text-[12px] text-ink-3">{t("search.hint")}</p>
        <PullsPanel />
        {search.error && (
          <div className="mb-4">
            <Banner tone="error">
              <p>{t("search.failed", { message: (search.error as { message: string }).message })}</p>
              <p className="mt-1 text-[12px]">{t("search.failedHint")}</p>
            </Banner>
          </div>
        )}
        {search.data &&
          (search.data.length === 0 ? (
            <p className="py-10 text-center text-[13px] text-ink-2">{t("search.none")}</p>
          ) : (
            <div className="overflow-x-auto rounded-xl border border-line bg-surface">
              <table className="w-full min-w-[640px] table-fixed text-left text-[13px]">
                <thead className="border-b border-line bg-surface-2 text-[12px] text-ink-3">
                  <tr>
                    <th className="w-[30%] px-4 py-2 font-medium">{t("search.name")}</th>
                    <th className="px-3 py-2 font-medium">{t("search.description")}</th>
                    <th className="w-[90px] px-3 py-2 text-right font-medium">{t("search.stars")}</th>
                    <th className="w-[130px] px-3 py-2" />
                  </tr>
                </thead>
                <tbody className="divide-y divide-line">
                  {search.data.map((r) => (
                    <tr key={r.name}>
                      <td className="px-4 py-2.5">
                        <span className="selectable font-mono text-[12.5px]">{r.name}</span>
                        {r.official && (
                          <span className="ml-2 inline-flex items-center gap-1 rounded bg-ok/10 px-1.5 text-[11.5px] text-ok">
                            <BadgeCheck size={12} />
                            {t("search.official")}
                          </span>
                        )}
                      </td>
                      <td className="px-3 py-2.5 text-[12.5px] text-ink-2">{r.description || "—"}</td>
                      <td className="px-3 py-2.5 text-right whitespace-nowrap tabular-nums">
                        <Star size={12} className="mr-1 inline text-ink-3" />
                        {nf.format(r.stars)}
                      </td>
                      <td className="px-3 py-2 text-right">
                        <Button size="sm" onClick={() => setPulling(r.name)} aria-label={t("search.pullName", { name: r.name })}>
                          <Download size={13} />
                          {t("search.pull")}
                        </Button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ))}
      </div>
      {pulling && <PullDialog initial={pulling} onClose={() => setPulling(null)} />}
    </div>
  );
}
