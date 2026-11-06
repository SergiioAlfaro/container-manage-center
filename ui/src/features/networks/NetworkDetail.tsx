import { useState, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Link2, RefreshCw, Trash2, Unlink } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApp } from "@/app/store";
import { useContainersNav } from "@/features/containers/store";
import { shortId } from "@/shared/format";
import { api } from "@/shared/ipc/client";
import type { NetworkDetails } from "@/shared/ipc/bindings/NetworkDetails";
import type { NetworkMember } from "@/shared/ipc/bindings/NetworkMember";
import { Button, CopyButton, cx } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { ConnectDialog, DisconnectDialog, RemoveNetworkDialog } from "./dialogs";
import { isActive } from "./model";
import { NetworkBadges } from "./badges";
import { useNetworksNav, type NetworkTab } from "./store";

const TABS: NetworkTab[] = ["summary", "containers", "inspect"];

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
          <span className="text-ink-2">{k}</span>={v ?? ""}
        </li>
      ))}
    </ul>
  );
}

function Summary({ d }: { d: NetworkDetails }) {
  const { t, i18n } = useTranslation();
  const s = d.summary;
  const created = s.created && !s.created.startsWith("0001-01-01") ? new Date(s.created) : null;
  const yes = (b: boolean) => (b ? t("common.yes") : t("common.no"));
  return (
    <div className="grid gap-4 xl:grid-cols-2">
      <Section title={t("netDetail.s.general")}>
        <Rows
          rows={[
            [t("netDetail.f.id"), mono(s.id)],
            [t("netDetail.f.driver"), mono(s.driver)],
            [t("netDetail.f.scope"), mono(s.scope)],
            [t("netDetail.f.created"), created && !Number.isNaN(created.getTime()) ? created.toLocaleString(i18n.language === "es" ? "es-ES" : "en-US") : none],
            [t("netDetail.f.internal"), yes(s.internal)],
            [t("netDetail.f.attachable"), yes(s.attachable)],
            [t("netDetail.f.ipv4"), yes(s.enableIpv4)],
            [t("netDetail.f.ipv6"), yes(s.enableIpv6)],
            ...(s.composeProject ? ([[t("netDetail.f.compose"), mono(s.composeProject)]] as Array<[string, ReactNode]>) : []),
          ]}
        />
      </Section>
      <Section title={t("netDetail.s.ipam")} count={d.pools.length}>
        <p className="mb-2 text-[12.5px] text-ink-3">{t("netDetail.ipamDriver", { driver: d.ipamDriver || "default" })}</p>
        {d.pools.length === 0 ? (
          <p className="text-[13px] text-ink-3">{t("netDetail.noPools")}</p>
        ) : (
          <div className="space-y-3">
            {d.pools.map((p, i) => (
              <Rows
                key={`${p.subnet}-${i}`}
                rows={[
                  [t("netDetail.f.subnet"), p.subnet ? mono(p.subnet) : none],
                  [t("netDetail.f.gateway"), p.gateway ? mono(p.gateway) : none],
                  [t("netDetail.f.ipRange"), p.ipRange ? mono(p.ipRange) : <span className="text-ink-3">{t("netDetail.wholeSubnet")}</span>],
                  ...(Object.keys(p.auxiliary).length
                    ? ([[t("netDetail.f.aux"), mono(Object.entries(p.auxiliary).map(([k, v]) => `${k}=${v}`).join(", "))]] as Array<[string, ReactNode]>)
                    : []),
                ]}
              />
            ))}
          </div>
        )}
      </Section>
      <Section title={t("netDetail.s.options")} count={Object.keys(d.options).length}>
        <KvList entries={Object.entries(d.options).sort(([a], [b]) => a.localeCompare(b))} empty={t("netDetail.noOptions")} />
      </Section>
      <Section title={t("netDetail.s.labels")} count={Object.keys(s.labels).length}>
        <KvList entries={Object.entries(s.labels).sort(([a], [b]) => a.localeCompare(b))} empty={t("detail.noLabels")} />
      </Section>
    </div>
  );
}

function Members({ d, onConnect, onDisconnect }: { d: NetworkDetails; onConnect: () => void; onDisconnect: (m: NetworkMember) => void }) {
  const { t } = useTranslation();
  const s = d.summary;
  const canConnect = !(s.predefined && s.driver !== "bridge");
  return (
    <div className="space-y-3">
      <div className="flex flex-wrap items-center gap-3">
        <p className="flex-1 text-[12.5px] text-ink-3">{t("netDetail.membersHint")}</p>
        {canConnect && (
          <Button size="sm" onClick={onConnect}>
            <Link2 size={14} />
            {t("netOps.connectShort")}
          </Button>
        )}
      </div>
      {s.containers.length === 0 ? (
        <p className="text-[13px] text-ink-3">{t("netDetail.noMembers")}</p>
      ) : (
        <div className="overflow-x-auto rounded-xl border border-line bg-surface">
          <table className="w-full min-w-[640px] table-fixed text-left text-[13px]">
            <thead className="border-b border-line bg-surface-2 text-[12px] text-ink-3">
              <tr>
                <th className="w-[28%] px-4 py-2 font-medium">{t("netDetail.c.name")}</th>
                <th className="w-[16%] px-3 py-2 font-medium">{t("netDetail.c.state")}</th>
                <th className="px-3 py-2 font-medium">{t("netDetail.c.ip")}</th>
                <th className="w-[20%] px-3 py-2 font-medium">{t("netDetail.c.mac")}</th>
                <th className="w-[52px] px-3 py-2">
                  <span className="sr-only">{t("networks.columns.actions")}</span>
                </th>
              </tr>
            </thead>
            <tbody className="divide-y divide-line">
              {s.containers.map((m) => (
                <tr key={m.id} className="align-top">
                  <td className="px-4 py-2.5">
                    <button
                      type="button"
                      className="font-medium hover:text-beacon-strong hover:underline"
                      onClick={() => {
                        useContainersNav.getState().showDetail(m.id, m.name);
                        useApp.getState().navigate("containers");
                      }}
                    >
                      {m.name}
                    </button>
                    <span className="block font-mono text-[11.5px] text-ink-3">{shortId(m.id)}</span>
                  </td>
                  <td className="px-3 py-2.5 text-[12.5px]">{t(`containers.state.${m.state}`, { defaultValue: m.state })}</td>
                  <td className="selectable px-3 py-2.5 font-mono text-[12px]">
                    {m.ipv4 || m.ipv6 ? (
                      <>
                        {m.ipv4 && <span className="block">{m.ipv4}</span>}
                        {m.ipv6 && <span className="block">{m.ipv6}</span>}
                      </>
                    ) : (
                      <span className="font-sans text-[12px] text-ink-3">{isActive(m.state) ? "—" : t("netDetail.ipWhenRunning")}</span>
                    )}
                  </td>
                  <td className="selectable px-3 py-2.5 font-mono text-[12px] text-ink-2">{m.mac ?? "—"}</td>
                  <td className="px-3 py-2">
                    <button
                      type="button"
                      onClick={() => onDisconnect(m)}
                      aria-label={t("netOps.disconnectAction", { container: m.name })}
                      title={t("netOps.disconnectAction", { container: m.name })}
                      className="grid h-7 w-7 place-items-center rounded-md text-ink-2 hover:bg-surface-sunk hover:text-err"
                    >
                      <Unlink size={15} />
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

export function NetworkDetail({ id, name, tab }: { id: string; name: string; tab: NetworkTab }) {
  const { t } = useTranslation();
  const qc = useQueryClient();
  const nav = useNetworksNav();
  const q = useQuery({ queryKey: ["network", id], queryFn: () => api.networkDetails(id, name) });
  const d = q.data;
  const [dialog, setDialog] = useState<{ kind: "remove" } | { kind: "connect" } | { kind: "disconnect"; member: NetworkMember } | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const title = d?.summary.name ?? name;
  const notFound = (q.error as { httpStatus?: number | null } | null)?.httpStatus === 404;
  const refresh = () => void qc.invalidateQueries({ queryKey: ["network", id] });
  const net = d ? { ...d.summary } : null;

  return (
    <div className="flex min-h-full flex-col">
      <div className="flex flex-wrap items-center gap-3 border-b border-line px-8 py-3">
        <Button variant="ghost" size="sm" onClick={nav.showList}>
          <ArrowLeft size={15} />
          {t("netDetail.back")}
        </Button>
        <h2 className="selectable min-w-0 truncate font-mono text-[14.5px] font-semibold">{title}</h2>
        <span className="font-mono text-[12px] text-ink-3">{shortId(id)}</span>
        {d && (
          <span className="flex flex-wrap gap-1">
            <NetworkBadges n={d.summary} />
          </span>
        )}
        <div className="ml-auto flex flex-wrap items-center gap-1.5">
          {d && !(d.summary.predefined && d.summary.driver !== "bridge") && (
            <Button size="sm" onClick={() => setDialog({ kind: "connect" })}>
              <Link2 size={14} />
              {t("netOps.connectShort")}
            </Button>
          )}
          {d && !d.summary.predefined && (
            <Button size="sm" onClick={() => setDialog({ kind: "remove" })} className="hover:border-err/50 hover:text-err">
              <Trash2 size={14} />
              {t("netOps.remove")}
            </Button>
          )}
          <Button size="sm" variant="ghost" onClick={refresh} aria-label={t("networks.refresh")} title={t("networks.refresh")} disabled={q.isFetching}>
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
          <Banner tone="error">{notFound ? t("netDetail.notFound", { name }) : t("netDetail.loadError", { message: (q.error as { message: string }).message })}</Banner>
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
                {t(`netDetail.tabs.${tb}`)}
                {tb === "containers" && d.summary.containers.length > 0 && <span className="ml-1.5 text-[12px] text-ink-3 tabular-nums">{d.summary.containers.length}</span>}
              </button>
            ))}
          </div>
          {tab === "summary" && <Summary d={d} />}
          {tab === "containers" && <Members d={d} onConnect={() => setDialog({ kind: "connect" })} onDisconnect={(m) => setDialog({ kind: "disconnect", member: m })} />}
          {tab === "inspect" && (
            <div className="flex min-h-0 flex-1 flex-col gap-2">
              <div className="flex items-center gap-2">
                <p className="flex-1 text-[12.5px] text-ink-3">{t("netDetail.inspectHint", { name: title })}</p>
                <CopyButton text={d.raw} />
              </div>
              <pre data-testid="network-json" className="selectable min-h-[400px] flex-1 overflow-auto rounded-lg border border-line bg-log p-3 font-mono text-[12px] leading-[1.5]">
                {d.raw}
              </pre>
            </div>
          )}
        </div>
      ) : null}

      {dialog?.kind === "connect" && net && <ConnectDialog network={net} onClose={() => setDialog(null)} onDone={setNotice} />}
      {dialog?.kind === "disconnect" && net && <DisconnectDialog network={net} member={dialog.member} onClose={() => setDialog(null)} onDone={setNotice} />}
      {dialog?.kind === "remove" && net && (
        <RemoveNetworkDialog
          network={net}
          onClose={() => setDialog(null)}
          onDone={() => {
            qc.removeQueries({ queryKey: ["network", id] });
            nav.showList();
          }}
        />
      )}
    </div>
  );
}
