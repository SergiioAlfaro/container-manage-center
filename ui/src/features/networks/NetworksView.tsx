import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Link2, Plus, RefreshCw, Search, Sparkles, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { RequireConnection } from "@/features/engine/RequireConnection";
import { shortId } from "@/shared/format";
import { api } from "@/shared/ipc/client";
import type { NetworkSummary } from "@/shared/ipc/bindings/NetworkSummary";
import { keys } from "@/shared/query";
import { Button, Segmented } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { CreateNetwork } from "./CreateNetwork";
import { ConnectDialog, PruneNetworksDialog, RemoveNetworkDialog } from "./dialogs";
import { NetworkBadges } from "./badges";
import { activeMembers, matchesNetwork, type NetworkFilter } from "./model";
import { NetworkDetail } from "./NetworkDetail";
import { useNetworksNav } from "./store";

type Dialogs = { kind: "prune" } | { kind: "remove"; network: NetworkSummary } | { kind: "connect"; network: NetworkSummary } | null;

function IconAction({ label, onClick, children, danger = false, disabled = false }: { label: string; onClick: () => void; children: React.ReactNode; danger?: boolean; disabled?: boolean }) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={label}
      title={label}
      disabled={disabled}
      className={`grid h-7 w-7 place-items-center rounded-md text-ink-2 hover:bg-surface-sunk disabled:cursor-not-allowed disabled:opacity-35 ${danger ? "hover:text-err" : "hover:text-ink"}`}
    >
      {children}
    </button>
  );
}

function NetworksTable() {
  const { t } = useTranslation();
  const nav = useNetworksNav();
  const q = useQuery({ queryKey: keys.networks, queryFn: api.networksList });
  const [filter, setFilter] = useState<NetworkFilter>("all");
  const [query, setQuery] = useState("");
  const [dialog, setDialog] = useState<Dialogs>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const all = q.data ?? [];
  const visible = useMemo(() => all.filter((n) => matchesNetwork(n, filter, query)), [all, filter, query]);

  return (
    <div className="px-8 py-6">
      <div className="mb-4 flex flex-wrap items-center gap-2">
        <Button variant="primary" onClick={nav.showCreate}>
          <Plus size={15} />
          {t("networks.create")}
        </Button>
        <Button onClick={() => setDialog({ kind: "prune" })} disabled={all.length === 0}>
          <Sparkles size={15} />
          {t("networks.prune")}
        </Button>
        <Button className="ml-auto" onClick={() => void q.refetch()} disabled={q.isFetching}>
          {q.isFetching ? <Spinner size={14} /> : <RefreshCw size={14} />}
          {t("networks.refresh")}
        </Button>
      </div>

      {notice && (
        <div className="mb-4">
          <Banner tone="info" onDismiss={() => setNotice(null)} dismissLabel={t("common.dismiss")}>
            {notice}
          </Banner>
        </div>
      )}

      <div className="mb-4 flex flex-wrap items-center gap-3">
        <Segmented<NetworkFilter>
          label={t("networks.filterLabel")}
          value={filter}
          onChange={setFilter}
          options={(["all", "inUse", "unused", "custom"] as NetworkFilter[]).map((f) => ({ value: f, label: t(`networks.filter.${f}`) }))}
        />
        <label className="relative max-w-[360px] min-w-[220px] flex-1">
          <span className="sr-only">{t("networks.search")}</span>
          <Search size={15} className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-ink-3" />
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("networks.search")}
            className="h-8 w-full rounded-md border border-line bg-surface pr-2 pl-8 text-[13px] outline-none placeholder:text-ink-3 hover:border-line-strong focus:border-beacon"
          />
        </label>
        <span className="text-[12.5px] text-ink-3 tabular-nums">{t("networks.count", { count: visible.length })}</span>
      </div>

      {q.error && (
        <div className="mb-4">
          <Banner tone="error">{t("networks.loadError", { message: (q.error as { message: string }).message })}</Banner>
        </div>
      )}

      {q.isPending ? (
        <div className="flex items-center justify-center gap-2 py-20 text-[13px] text-ink-3">
          <Spinner /> {t("common.loading")}
        </div>
      ) : (
        <div className="overflow-x-auto rounded-xl border border-line bg-surface">
          <table className="w-full min-w-[860px] table-fixed text-left text-[13px]">
            <thead className="border-b border-line bg-surface-2 text-[12px] text-ink-3">
              <tr>
                <th className="w-[28%] px-4 py-2 font-medium">{t("networks.columns.name")}</th>
                <th className="w-[12%] px-3 py-2 font-medium">{t("networks.columns.id")}</th>
                <th className="w-[96px] px-3 py-2 font-medium">{t("networks.columns.driver")}</th>
                <th className="w-[18%] px-3 py-2 font-medium">{t("networks.columns.subnet")}</th>
                <th className="px-3 py-2 font-medium">{t("networks.columns.containers")}</th>
                <th className="w-[84px] px-3 py-2 text-right font-medium">
                  <span className="sr-only">{t("networks.columns.actions")}</span>
                </th>
              </tr>
            </thead>
            <tbody className="divide-y divide-line">
              {visible.map((n) => {
                const running = activeMembers(n).length;
                return (
                  <tr key={n.id} className="align-top">
                    <td className="px-4 py-2.5">
                      <button type="button" onClick={() => nav.showDetail(n.id, n.name)} className="block max-w-full truncate text-left font-mono text-[12.5px] hover:text-beacon-strong hover:underline" title={t("networks.open", { name: n.name })}>
                        {n.name}
                      </button>
                      <div className="mt-1 flex flex-wrap gap-1 empty:hidden">
                        <NetworkBadges n={n} />
                      </div>
                    </td>
                    <td className="selectable px-3 py-2.5 font-mono text-[12px] text-ink-2" title={n.id}>
                      {shortId(n.id)}
                    </td>
                    <td className="px-3 py-2.5 font-mono text-[12px]">
                      {n.driver}
                      {n.scope && n.scope !== "local" && <span className="block text-[11px] text-ink-3">{n.scope}</span>}
                    </td>
                    <td className="selectable px-3 py-2.5 font-mono text-[12px]">
                      {n.subnets.length ? n.subnets.map((s) => <span key={s} className="block truncate">{s}</span>) : <span className="text-ink-3">—</span>}
                    </td>
                    <td className="px-3 py-2.5 text-[12.5px]">
                      {n.containers.length === 0 ? (
                        <span className="text-ink-3">{t("networks.noContainers")}</span>
                      ) : (
                        <span title={n.containers.map((c) => c.name).join(", ")}>
                          {t("networks.members", { count: n.containers.length, running })}
                          <span className="block truncate text-[11.5px] text-ink-3">{n.containers.map((c) => c.name).join(", ")}</span>
                        </span>
                      )}
                    </td>
                    <td className="px-3 py-2">
                      <div className="flex justify-end gap-0.5">
                        <IconAction label={t("networks.connectAction", { name: n.name })} onClick={() => setDialog({ kind: "connect", network: n })} disabled={n.predefined && n.driver !== "bridge"}>
                          <Link2 size={15} />
                        </IconAction>
                        <IconAction danger label={t("networks.removeAction", { name: n.name })} onClick={() => setDialog({ kind: "remove", network: n })} disabled={n.predefined}>
                          <Trash2 size={15} />
                        </IconAction>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          {visible.length === 0 && <p className="px-4 py-10 text-center text-[13px] text-ink-2">{all.length === 0 ? t("networks.empty") : t("networks.noMatches")}</p>}
        </div>
      )}

      {dialog?.kind === "prune" && <PruneNetworksDialog onClose={() => setDialog(null)} onDone={(r) => setNotice(t("netOps.pruned", { count: r.length, names: r.join(", ") }))} />}
      {dialog?.kind === "remove" && <RemoveNetworkDialog network={dialog.network} onClose={() => setDialog(null)} onDone={() => setNotice(t("netOps.removed", { name: dialog.network.name }))} />}
      {dialog?.kind === "connect" && <ConnectDialog network={dialog.network} onClose={() => setDialog(null)} onDone={setNotice} />}
    </div>
  );
}

function NetworksPages() {
  const page = useNetworksNav((s) => s.page);
  switch (page.kind) {
    case "detail":
      return <NetworkDetail key={page.id} id={page.id} name={page.name} tab={page.tab} />;
    case "create":
      return <CreateNetwork />;
    default:
      return <NetworksTable />;
  }
}

export function NetworksView() {
  return (
    <RequireConnection>
      <NetworksPages />
    </RequireConnection>
  );
}
