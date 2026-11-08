import { useState, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Box, Gauge, RefreshCw, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApp } from "@/app/store";
import { useContainersNav } from "@/features/containers/store";
import { Badge } from "@/features/networks/badges";
import { formatBytes, formatDate, shortId } from "@/shared/format";
import { api } from "@/shared/ipc/client";
import type { VolumeDetails } from "@/shared/ipc/bindings/VolumeDetails";
import { Button, CopyButton, cx } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { useVolumeSizes, volumeLabel } from "./common";
import { RemoveVolumeDialog } from "./dialogs";
import { createContainerWith, volumeMount } from "./model";
import { useVolumesNav, type VolumeTab } from "./store";

const TABS: VolumeTab[] = ["summary", "containers", "inspect"];

function Section({ title, children, count }: { title: string; children: ReactNode; count?: number }) {
  return (
    <section className="rounded-xl border border-line bg-surface">
      <h3 className="flex items-center gap-2 border-b border-line px-4 py-2.5 text-[13px] font-semibold">
        {title}
        {count !== undefined && <span className="text-[12px] font-normal text-ink-3 tabular-nums">{count}</span>}
      </h3>
      <div className="px-4 py-3">{children}</div>
    </section>
  );
}

function Rows({ rows }: { rows: Array<[string, ReactNode]> }) {
  return (
    <dl className="grid grid-cols-[minmax(120px,max-content)_minmax(0,1fr)] gap-x-4 gap-y-1.5 text-[13px]">
      {rows.map(([k, v]) => (
        <div key={k} className="contents">
          <dt className="text-ink-3">{k}</dt>
          <dd className="selectable min-w-0 [overflow-wrap:anywhere]">{v}</dd>
        </div>
      ))}
    </dl>
  );
}

const mono = (s: string) => <span className="font-mono text-[12.5px]">{s}</span>;
const none = <span className="text-ink-3">—</span>;

function KvList({ entries, empty }: { entries: Array<[string, string | undefined]>; empty: string }) {
  if (entries.length === 0) return <p className="text-[13px] text-ink-3">{empty}</p>;
  return (
    <ul className="selectable space-y-0.5 font-mono text-[12px] [overflow-wrap:anywhere]">
      {entries.map(([k, v]) => (
        <li key={k}>
          <span className="text-ink-2">{k}</span>
          {v ? `=${v}` : ""}
        </li>
      ))}
    </ul>
  );
}

function Summary({ d }: { d: VolumeDetails }) {
  const { t, i18n } = useTranslation();
  const s = d.summary;
  const sizes = useVolumeSizes();
  const sz = sizes.data?.find((x) => x.name === s.name);
  const created = s.created && !s.created.startsWith("0001-01-01") ? new Date(s.created) : null;
  return (
    <div className="grid gap-4 xl:grid-cols-2">
      <Section title={t("volDetail.s.general")}>
        <Rows
          rows={[
            [t("volDetail.f.name"), mono(s.name)],
            [t("volDetail.f.driver"), mono(s.driver)],
            [t("volDetail.f.scope"), mono(s.scope)],
            [t("volDetail.f.mountpoint"), s.mountpoint ? mono(s.mountpoint) : none],
            [t("volDetail.f.created"), created && !Number.isNaN(created.getTime()) ? formatDate(created, i18n.language) : none],
            [
              t("volDetail.f.size"),
              <span key="size" className="inline-flex flex-wrap items-center gap-2">
                {sz ? (sz.size === null ? <span className="text-ink-3">{t("volumes.sizeUnknown")}</span> : formatBytes(sz.size, i18n.language)) : <span className="text-ink-3">{t("volDetail.sizeNotMeasured")}</span>}
                <Button size="sm" onClick={() => void sizes.refetch()} disabled={sizes.isFetching}>
                  {sizes.isFetching ? <Spinner size={13} /> : <Gauge size={13} />}
                  {t("volDetail.measure")}
                </Button>
              </span>,
            ],
            ...(s.composeProject ? ([[t("netDetail.f.compose"), mono(s.composeProject)]] as Array<[string, ReactNode]>) : []),
          ]}
        />
        {sizes.error && <p className="mt-2 text-[12px] text-err">{(sizes.error as { message: string }).message}</p>}
      </Section>
      <Section title={t("volDetail.s.options")} count={Object.keys(s.options).length}>
        <KvList entries={Object.entries(s.options).sort(([a], [b]) => a.localeCompare(b))} empty={t("volDetail.noOptions")} />
      </Section>
      <Section title={t("netDetail.s.labels")} count={Object.keys(s.labels).length}>
        <KvList entries={Object.entries(s.labels).sort(([a], [b]) => a.localeCompare(b))} empty={t("detail.noLabels")} />
      </Section>
      {Object.keys(d.status).length > 0 && (
        <Section title={t("volDetail.s.status")}>
          <KvList entries={Object.entries(d.status)} empty="" />
        </Section>
      )}
    </div>
  );
}

export function VolumeDetail({ name, tab }: { name: string; tab: VolumeTab }) {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const nav = useVolumesNav();
  const q = useQuery({ queryKey: ["volume", name], queryFn: () => api.volumeDetails(name) });
  const d = q.data;
  const [removing, setRemoving] = useState(false);
  const notFound = (q.error as { httpStatus?: number | null } | null)?.httpStatus === 404;
  const title = d ? volumeLabel(d.summary) : name;
  const users = d?.summary.containers ?? [];

  return (
    <div className="flex min-h-full flex-col">
      <div className="flex flex-wrap items-center gap-3 border-b border-line px-8 py-3">
        <Button variant="ghost" size="sm" onClick={nav.showList}>
          <ArrowLeft size={15} />
          {t("volDetail.back")}
        </Button>
        <h2 className="selectable min-w-0 truncate font-mono text-[14.5px] font-semibold" title={name}>
          {title}
        </h2>
        {d?.summary.anonymous && <Badge>{t("volumes.anonymous")}</Badge>}
        <div className="ml-auto flex flex-wrap items-center gap-1.5">
          {d && (
            <>
              <Button size="sm" onClick={() => createContainerWith(volumeMount(d.summary.name))}>
                <Box size={14} />
                {t("volDetail.createContainer")}
              </Button>
              <Button size="sm" onClick={() => setRemoving(true)} className="hover:border-err/50 hover:text-err">
                <Trash2 size={14} />
                {t("volOps.remove")}
              </Button>
            </>
          )}
          <Button size="sm" variant="ghost" onClick={() => void qc.invalidateQueries({ queryKey: ["volume", name] })} aria-label={t("volumes.refresh")} title={t("volumes.refresh")} disabled={q.isFetching}>
            {q.isFetching ? <Spinner size={14} /> : <RefreshCw size={14} />}
          </Button>
        </div>
      </div>

      {q.isPending ? (
        <div className="flex items-center justify-center gap-2 py-20 text-[13px] text-ink-3">
          <Spinner /> {t("common.loading")}
        </div>
      ) : q.error && !d ? (
        <div className="px-8 py-6">
          <Banner tone="error">{notFound ? t("volDetail.notFound", { name }) : t("volDetail.loadError", { message: (q.error as { message: string }).message })}</Banner>
        </div>
      ) : d ? (
        <div className="flex min-h-0 flex-1 flex-col px-8 py-4">
          <div role="tablist" aria-label={title} className="mb-4 flex flex-wrap gap-1 border-b border-line">
            {TABS.map((tb) => (
              <button
                key={tb}
                type="button"
                role="tab"
                aria-selected={tab === tb}
                onClick={() => nav.setTab(tb)}
                className={cx("-mb-px h-9 border-b-2 px-3 text-[13px]", tab === tb ? "border-beacon font-medium text-ink" : "border-transparent text-ink-2 hover:text-ink")}
              >
                {t(`volDetail.tabs.${tb}`)}
                {tb === "containers" && users.length > 0 && <span className="ml-1.5 text-[12px] text-ink-3 tabular-nums">{users.length}</span>}
              </button>
            ))}
          </div>
          {tab === "summary" && <Summary d={d} />}
          {tab === "containers" &&
            (users.length === 0 ? (
              <p className="text-[13px] text-ink-3">{t("volDetail.noContainers")}</p>
            ) : (
              <div className="overflow-x-auto rounded-xl border border-line bg-surface">
                <table className="w-full min-w-[560px] table-fixed text-left text-[13px]">
                  <thead className="border-b border-line bg-surface-2 text-[12px] text-ink-3">
                    <tr>
                      <th className="w-[32%] px-4 py-2 font-medium">{t("netDetail.c.name")}</th>
                      <th className="w-[18%] px-3 py-2 font-medium">{t("netDetail.c.state")}</th>
                      <th className="px-3 py-2 font-medium">{t("volDetail.c.destination")}</th>
                      <th className="w-[130px] px-3 py-2 font-medium">{t("volDetail.c.access")}</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-line">
                    {users.map((u) => (
                      <tr key={`${u.id}-${u.destination}`}>
                        <td className="px-4 py-2.5">
                          <button
                            type="button"
                            className="font-medium hover:text-beacon-strong hover:underline"
                            onClick={() => {
                              useContainersNav.getState().showDetail(u.id, u.name);
                              useApp.getState().navigate("containers");
                            }}
                          >
                            {u.name}
                          </button>
                          <span className="block font-mono text-[11.5px] text-ink-3">{shortId(u.id)}</span>
                        </td>
                        <td className="px-3 py-2.5 text-[12.5px]">{t(`containers.state.${u.state}`, { defaultValue: u.state })}</td>
                        <td className="selectable px-3 py-2.5 font-mono text-[12px]">{u.destination}</td>
                        <td className="px-3 py-2.5 text-[12.5px]">{u.rw ? t("volDetail.rw") : t("volDetail.ro")}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ))}
          {tab === "inspect" && (
            <div className="flex min-h-0 flex-1 flex-col gap-2">
              <div className="flex items-center gap-2">
                <p className="flex-1 text-[12.5px] text-ink-3">{t("volDetail.inspectHint", { name: d.summary.name })}</p>
                <CopyButton text={d.raw} />
              </div>
              <pre data-testid="volume-json" className="selectable min-h-[400px] flex-1 overflow-auto rounded-lg border border-line bg-log p-3 font-mono text-[12px] leading-[1.5]">
                {d.raw}
              </pre>
            </div>
          )}
        </div>
      ) : null}

      {removing && d && (
        <RemoveVolumeDialog
          volume={d.summary}
          onClose={() => setRemoving(false)}
          onDone={() => {
            qc.removeQueries({ queryKey: ["volume", name] });
            nav.showList();
          }}
        />
      )}
    </div>
  );
}
