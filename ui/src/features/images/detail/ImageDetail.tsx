import { useState, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Box, Download, Eye, EyeOff, RefreshCw, Tag, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApp } from "@/app/store";
import { useContainersNav } from "@/features/containers/store";
import { isSecretKey } from "@/features/containers/specModel";
import { joinArgs } from "@/features/containers/shellwords";
import { formatBytes, shortId } from "@/shared/format";
import { api } from "@/shared/ipc/client";
import type { ImageDetails } from "@/shared/ipc/bindings/ImageDetails";
import type { ImageLayer } from "@/shared/ipc/bindings/ImageLayer";
import { keys } from "@/shared/query";
import { Button, CopyButton, cx } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { createContainerFrom } from "../createFrom";
import { RemoveImageDialog, TagDialog } from "../dialogs";
import { PullDialog } from "../pull/PullDialog";
import { PullsPanel } from "../pull/PullsPanel";
import { useImagesNav, type ImageTab } from "../store";

const TABS: ImageTab[] = ["summary", "layers", "containers", "inspect"];

/** Instrucción legible de una capa (`docker history` antepone la shell). */
export function formatInstruction(createdBy: string): string {
  const s = createdBy.trim();
  const nop = s.match(/^\/bin\/sh -c #\(nop\)\s+(.*)$/s);
  if (nop) return nop[1]!.trim();
  const sh = s.match(/^\/bin\/sh -c (.*)$/s);
  if (sh) return `RUN ${sh[1]!.trim()}`;
  return s.replace(/\s+# buildkit$/, "");
}

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

function Summary({ d }: { d: ImageDetails }) {
  const { t, i18n } = useTranslation();
  const [reveal, setReveal] = useState(false);
  const secrets = d.env.filter((e) => isSecretKey(e.key)).length;
  const labels = Object.entries(d.labels).sort(([a], [b]) => a.localeCompare(b));
  const created = d.created ? new Date(d.created) : null;
  return (
    <div className="grid gap-4 xl:grid-cols-2">
      <Section title={t("imageDetail.s.general")}>
        <Rows
          rows={[
            [t("imageDetail.f.id"), mono(d.id)],
            [t("imageDetail.f.tags"), d.repoTags.length ? <div className="space-y-0.5">{d.repoTags.map((x) => <div key={x}>{mono(x)}</div>)}</div> : <span className="text-ink-3">{t("images.dangling")}</span>],
            [t("imageDetail.f.digests"), d.repoDigests.length ? <div className="space-y-0.5">{d.repoDigests.map((x) => <div key={x}>{mono(x)}</div>)}</div> : <span className="text-ink-3">{t("imageDetail.localOnly")}</span>],
            [t("imageDetail.f.created"), created && !Number.isNaN(created.getTime()) ? created.toLocaleString(i18n.language === "es" ? "es-ES" : "en-US") : none],
            [t("imageDetail.f.size"), formatBytes(d.size, i18n.language)],
            [t("imageDetail.f.platform"), mono(`${d.os}/${d.architecture}${d.variant ? `/${d.variant}` : ""}`)],
            [t("imageDetail.f.layers"), mono(String(d.layers))],
            ...(d.author ? ([[t("imageDetail.f.author"), d.author]] as Array<[string, ReactNode]>) : []),
          ]}
        />
      </Section>
      <Section title={t("imageDetail.s.config")}>
        <Rows
          rows={[
            [t("detail.f.entrypoint"), d.entrypoint.length ? mono(joinArgs(d.entrypoint)) : none],
            [t("detail.f.command"), d.command.length ? mono(joinArgs(d.command)) : none],
            [t("detail.f.workingDir"), d.workingDir ? mono(d.workingDir) : none],
            [t("detail.f.user"), d.user ? mono(d.user) : <span className="text-ink-3">root</span>],
            [t("imageDetail.f.ports"), d.exposedPorts.length ? mono(d.exposedPorts.join(", ")) : none],
            [t("imageDetail.f.volumes"), d.volumes.length ? mono(d.volumes.join(", ")) : none],
            ...(d.stopSignal ? ([[t("imageDetail.f.stopSignal"), mono(d.stopSignal)]] as Array<[string, ReactNode]>) : []),
            ...(d.healthcheck ? ([[t("imageDetail.f.healthcheck"), mono(d.healthcheck.join(" "))]] as Array<[string, ReactNode]>) : []),
          ]}
        />
      </Section>
      <Section title={t("detail.s.env")} count={d.env.length}>
        {secrets > 0 && (
          <button type="button" onClick={() => setReveal((r) => !r)} className="mb-2 inline-flex items-center gap-1.5 text-[12.5px] text-beacon-strong hover:underline">
            {reveal ? <EyeOff size={14} /> : <Eye size={14} />}
            {reveal ? t("detail.hideSecrets") : t("detail.revealSecrets", { count: secrets })}
          </button>
        )}
        {d.env.length === 0 ? (
          <p className="text-[13px] text-ink-3">{t("detail.noEnv")}</p>
        ) : (
          <ul className="selectable space-y-0.5 font-mono text-[12px] [overflow-wrap:anywhere]">
            {d.env.map((e, i) => (
              <li key={`${e.key}-${i}`}>
                <span className="text-ink-2">{e.key}</span>={isSecretKey(e.key) && !reveal ? <span className="text-ink-3">••••••••</span> : e.value}
              </li>
            ))}
          </ul>
        )}
      </Section>
      <Section title={t("detail.s.labels")} count={labels.length}>
        {labels.length === 0 ? (
          <p className="text-[13px] text-ink-3">{t("detail.noLabels")}</p>
        ) : (
          <ul className="selectable space-y-0.5 font-mono text-[12px] [overflow-wrap:anywhere]">
            {labels.map(([k, v]) => (
              <li key={k}>
                <span className="text-ink-2">{k}</span>={v}
              </li>
            ))}
          </ul>
        )}
      </Section>
    </div>
  );
}

function Layers({ reference }: { reference: string }) {
  const { t, i18n } = useTranslation();
  const q = useQuery({ queryKey: ["image", reference, "history"], queryFn: () => api.imageHistory(reference) });
  if (q.isPending)
    return (
      <div className="flex items-center gap-2 py-10 text-[13px] text-ink-3">
        <Spinner /> {t("common.loading")}
      </div>
    );
  if (q.error) return <Banner tone="error">{t("imageDetail.layersError", { message: (q.error as { message: string }).message })}</Banner>;
  const layers: ImageLayer[] = q.data;
  const total = layers.reduce((a, l) => a + l.size, 0);
  const withData = layers.filter((l) => !l.empty && l.size > 0).length;
  return (
    <div className="space-y-2">
      <p className="text-[12.5px] text-ink-3">{t("imageDetail.layersSummary", { count: layers.length, data: withData, size: formatBytes(total, i18n.language) })}</p>
      <div className="overflow-x-auto rounded-xl border border-line bg-surface">
        <table className="w-full min-w-[640px] table-fixed text-left text-[12.5px]">
          <thead className="border-b border-line bg-surface-2 text-[12px] text-ink-3">
            <tr>
              <th className="w-[44px] px-3 py-2 text-right font-medium">#</th>
              <th className="px-3 py-2 font-medium">{t("imageDetail.instruction")}</th>
              <th className="w-[96px] px-3 py-2 text-right font-medium">{t("images.columns.size")}</th>
              <th className="w-[150px] px-3 py-2 font-medium">{t("images.columns.created")}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-line">
            {[...layers].reverse().map((l, i) => (
              <tr key={i} className={cx("align-top", l.empty && "text-ink-3")}>
                <td className="px-3 py-2 text-right tabular-nums">{i + 1}</td>
                <td className="px-3 py-2">
                  <code className="selectable font-mono text-[12px] break-all whitespace-pre-wrap">{formatInstruction(l.createdBy) || "—"}</code>
                  {l.comment && <span className="block text-[11.5px] text-ink-3">{l.comment}</span>}
                  {l.tags.length > 0 && <span className="block font-mono text-[11.5px] text-beacon-strong">{l.tags.join(", ")}</span>}
                </td>
                <td className="px-3 py-2 text-right whitespace-nowrap tabular-nums">{l.size > 0 ? formatBytes(l.size, i18n.language) : "0 B"}</td>
                <td className="px-3 py-2 text-[12px] whitespace-nowrap">{l.created > 0 ? new Date(l.created * 1000).toLocaleString(i18n.language === "es" ? "es-ES" : "en-US") : "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

export function ImageDetail({ id, name, tab }: { id: string; name: string; tab: ImageTab }) {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const nav = useImagesNav();
  const q = useQuery({ queryKey: ["image", id], queryFn: () => api.imageDetails(id) });
  const list = useQuery({ queryKey: keys.images, queryFn: api.imagesList });
  const summary = list.data?.find((i) => i.id === id);
  const d = q.data;
  const [dialog, setDialog] = useState<"tag" | "remove" | "pull" | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const title = d?.repoTags[0] ?? name;
  const users = summary?.usedBy ?? [];
  const notFound = (q.error as { httpStatus?: number | null } | null)?.httpStatus === 404;

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ["image", id] });
    void qc.invalidateQueries({ queryKey: keys.images });
  };

  return (
    <div className="flex min-h-full flex-col">
      <div className="flex flex-wrap items-center gap-3 border-b border-line px-8 py-3">
        <Button variant="ghost" size="sm" onClick={nav.showList}>
          <ArrowLeft size={15} />
          {t("imageDetail.back")}
        </Button>
        <h2 className="selectable min-w-0 truncate font-mono text-[14.5px] font-semibold">{title}</h2>
        <span className="font-mono text-[12px] text-ink-3">{shortId(id)}</span>
        <div className="ml-auto flex flex-wrap items-center gap-1.5">
          {d && (
            <>
              {d.repoTags.length > 0 && (
                <Button size="sm" onClick={() => createContainerFrom(d.repoTags[0]!)}>
                  <Box size={14} />
                  {t("imageDetail.createContainer")}
                </Button>
              )}
              <Button size="sm" onClick={() => setDialog("tag")}>
                <Tag size={14} />
                {t("imageOps.tag")}
              </Button>
              {d.repoTags.length > 0 && d.repoDigests.length > 0 && (
                <Button size="sm" onClick={() => setDialog("pull")}>
                  <Download size={14} />
                  {t("imageDetail.pullAgain")}
                </Button>
              )}
              <Button size="sm" onClick={() => setDialog("remove")} className="hover:border-err/50 hover:text-err">
                <Trash2 size={14} />
                {t("containers.action.remove")}
              </Button>
            </>
          )}
          <Button size="sm" variant="ghost" onClick={refresh} aria-label={t("images.refresh")} title={t("images.refresh")} disabled={q.isFetching}>
            {q.isFetching ? <Spinner size={14} /> : <RefreshCw size={14} />}
          </Button>
        </div>
      </div>

      <div className="space-y-3 px-8 pt-4 empty:hidden">
        {notice && (
          <Banner tone="info" onDismiss={() => setNotice(null)} dismissLabel={t("common.dismiss")}>
            {notice}
          </Banner>
        )}
      </div>

      {q.isPending ? (
        <div className="flex items-center justify-center gap-2 py-20 text-[13px] text-ink-3">
          <Spinner /> {t("common.loading")}
        </div>
      ) : q.error && !d ? (
        <div className="px-8 py-6">
          <Banner tone="error">{notFound ? t("imageDetail.notFound", { name }) : t("imageDetail.loadError", { message: (q.error as { message: string }).message })}</Banner>
        </div>
      ) : d ? (
        <div className="flex min-h-0 flex-1 flex-col px-8 py-4">
          <PullsPanel />
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
                {t(`imageDetail.tabs.${tb}`)}
                {tb === "containers" && users.length > 0 && <span className="ml-1.5 text-[12px] text-ink-3 tabular-nums">{users.length}</span>}
              </button>
            ))}
          </div>
          {tab === "summary" && <Summary d={d} />}
          {tab === "layers" && <Layers reference={id} />}
          {tab === "containers" &&
            (users.length === 0 ? (
              <p className="text-[13px] text-ink-3">{t("imageDetail.noContainers")}</p>
            ) : (
              <ul className="divide-y divide-line rounded-xl border border-line bg-surface">
                {users.map((u) => (
                  <li key={u.id} className="flex items-center gap-3 px-4 py-2.5 text-[13px]">
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
                    <span className="font-mono text-[12px] text-ink-3">{shortId(u.id)}</span>
                    <span className="ml-auto text-ink-2">{t(`containers.state.${u.state}`, { defaultValue: u.state })}</span>
                  </li>
                ))}
              </ul>
            ))}
          {tab === "inspect" && (
            <div className="flex min-h-0 flex-1 flex-col gap-2">
              <div className="flex items-center gap-2">
                <p className="flex-1 text-[12.5px] text-ink-3">{t("imageDetail.inspectHint", { name: title })}</p>
                <CopyButton text={d.raw} />
              </div>
              <pre data-testid="image-json" className="selectable min-h-[400px] flex-1 overflow-auto rounded-lg border border-line bg-log p-3 font-mono text-[12px] leading-[1.5]">
                {d.raw}
              </pre>
            </div>
          )}
        </div>
      ) : null}

      {dialog === "tag" && d && <TagDialog image={{ id: d.id, repoTags: d.repoTags, usedBy: users, size: d.size }} onClose={() => setDialog(null)} onDone={setNotice} />}
      {dialog === "pull" && d && <PullDialog initial={d.repoTags[0]} onClose={() => setDialog(null)} />}
      {dialog === "remove" && d && (
        <RemoveImageDialog
          image={{ id: d.id, repoTags: d.repoTags, usedBy: users, size: d.size }}
          onClose={() => setDialog(null)}
          onDone={(r) => {
            // Si la imagen ya no existe, se vuelve a la lista.
            if (r.deleted.length > 0 || r.untagged.length >= d.repoTags.length) {
              qc.removeQueries({ queryKey: ["image", id] });
              nav.showList();
            } else {
              setNotice(t("imageOps.removed", { untagged: r.untagged.length, deleted: r.deleted.length }));
              refresh();
            }
          }}
        />
      )}
    </div>
  );
}
