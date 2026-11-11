import { useMemo, useState } from "react";
import { ServerPathInput, useIsRemote, useRemoteContext } from "@/features/servers/paths";
import { useApi } from "@/app/server";
import { useQuery } from "@tanstack/react-query";
import { Box, FolderPlus, RefreshCw, Search } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApp } from "@/app/store";
import { useContainersNav } from "@/features/containers/store";
import { RequireConnection } from "@/features/engine/RequireConnection";
import { Badge } from "@/features/networks/badges";
import { bindMount, createContainerWith, modeString } from "@/features/volumes/model";
import type { BindSummary } from "@/shared/ipc/bindings/BindSummary";
import { keys } from "@/shared/query";
import { Button, CopyButton, Segmented } from "@/shared/ui/controls";
import { Banner, Dialog, Spinner } from "@/shared/ui/feedback";

export type BindFilter = "all" | "writable" | "attention";

export function needsAttention(b: Pick<BindSummary, "risk" | "host">): boolean {
  return b.risk !== null || b.host.kind === "missing";
}

export function matchesBind(b: BindSummary, filter: BindFilter, query: string): boolean {
  if (filter === "writable" && !b.writable) return false;
  if (filter === "attention" && !needsAttention(b)) return false;
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return [b.source, ...b.containers.flatMap((c) => [c.name, c.destination])].some((x) => x.toLowerCase().includes(q));
}

function openContainer(id: string, name: string) {
  useContainersNav.getState().showDetail(id, name);
  useApp.getState().navigate("containers");
}

function BindCard({ b }: { b: BindSummary }) {
  const remoteCtx = useRemoteContext();
  const { t } = useTranslation();
  const kind = b.host.kind;
  return (
    <li className="rounded-xl border border-line bg-surface">
      <div className="flex flex-wrap items-start gap-3 px-4 py-3">
        <div className="min-w-0 flex-1">
          <p className="selectable font-mono text-[13px] break-all">{b.source}</p>
          <div className="mt-1 flex flex-wrap items-center gap-1.5 text-[12px] text-ink-3">
            <Badge>{t(`binds.kind.${kind}`)}</Badge>
            {b.writable ? <Badge>{t("binds.writable")}</Badge> : <Badge>{t("binds.readOnly")}</Badge>}
            {b.host.mode !== null && (
              <span className="font-mono" title={t("binds.permissions")}>
                {modeString(b.host.mode)} · uid {b.host.uid}
              </span>
            )}
            {kind !== "missing" && kind !== "socket" && !b.host.readable && <span className="text-warn">{t("binds.notReadable")}</span>}
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <CopyButton text={b.source} compact />
          {kind === "directory" && (
            <Button size="sm" onClick={() => createContainerWith(bindMount(b.source))} title={t("binds.newContainerHint")}>
              <Box size={14} />
              {t("binds.newContainer")}
            </Button>
          )}
        </div>
      </div>
      {(b.risk || kind === "missing") && (
        <div className="px-4 pb-3">
          {b.risk && (
            <Banner tone={b.risk === "dockerSocket" || b.risk === "hostRoot" ? "error" : "warn"}>
              {t(`binds.risk.${b.risk}`)}
              {b.writable && b.risk !== "dockerSocket" && ` ${t("binds.riskWritable")}`}
            </Banner>
          )}
          {kind === "missing" && <Banner tone="warn">{t("binds.missing", { context: remoteCtx })}</Banner>}
        </div>
      )}
      <table className="w-full table-fixed border-t border-line text-left text-[12.5px]">
        <thead className="text-[11.5px] text-ink-3">
          <tr>
            <th className="w-[30%] px-4 py-1.5 font-medium">{t("netDetail.c.name")}</th>
            <th className="w-[16%] px-3 py-1.5 font-medium">{t("netDetail.c.state")}</th>
            <th className="px-3 py-1.5 font-medium">{t("volDetail.c.destination")}</th>
            <th className="w-[130px] px-3 py-1.5 font-medium">{t("volDetail.c.access")}</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-line">
          {b.containers.map((c) => (
            <tr key={`${c.id}-${c.destination}`}>
              <td className="px-4 py-1.5">
                <button type="button" className="font-medium hover:text-beacon-strong hover:underline" onClick={() => openContainer(c.id, c.name)}>
                  {c.name}
                </button>
              </td>
              <td className="px-3 py-1.5">{t(`containers.state.${c.state}`, { defaultValue: c.state })}</td>
              <td className="selectable px-3 py-1.5 font-mono text-[12px] break-all">{c.destination}</td>
              <td className="px-3 py-1.5">{c.rw ? t("volDetail.rw") : t("volDetail.ro")}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </li>
  );
}

function BindsList() {
  const remoteCtx = useRemoteContext();
  const api = useApi();
  const { t } = useTranslation();
  const q = useQuery({ queryKey: keys.binds, queryFn: api.bindsList });
  const [filter, setFilter] = useState<BindFilter>("all");
  const [query, setQuery] = useState("");
  const all = q.data ?? [];
  const visible = useMemo(() => all.filter((b) => matchesBind(b, filter, query)), [all, filter, query]);
  const attention = all.filter(needsAttention).length;

  const remote = useIsRemote();
  const [askPath, setAskPath] = useState(false);
  const pickAndCreate = async () => {
    // En un servidor remoto la carpeta es del servidor: se escribe su ruta.
    if (remote) {
      setAskPath(true);
      return;
    }
    const dir = await api.pickFolder(t("binds.pickTitle"));
    if (dir) createContainerWith(bindMount(dir));
  };

  return (
    <div className="px-8 py-6">
      <div className="mb-4 flex flex-wrap items-center gap-2">
        <Button variant="primary" onClick={() => void pickAndCreate()}>
          <FolderPlus size={15} />
          {t("binds.bindFolder")}
        </Button>
        <Button className="ml-auto" onClick={() => void q.refetch()} disabled={q.isFetching}>
          {q.isFetching ? <Spinner size={14} /> : <RefreshCw size={14} />}
          {t("volumes.refresh")}
        </Button>
      </div>
      <p className="mb-4 max-w-[760px] text-[12.5px] text-ink-3">{t("binds.intro", { context: remoteCtx })}</p>
      {askPath && (
        <Dialog title={t("binds.bindFolder")} onClose={() => setAskPath(false)} footer={<Button onClick={() => setAskPath(false)}>{t("common.cancel")}</Button>}>
          <ServerPathInput
            label={t("servers.paths.bindLabel")}
            placeholder="/srv/datos"
            submitLabel={t("servers.paths.use")}
            onSubmit={(dir) => {
              setAskPath(false);
              createContainerWith(bindMount(dir));
            }}
          />
        </Dialog>
      )}

      <div className="mb-4 flex flex-wrap items-center gap-3">
        <Segmented<BindFilter>
          label={t("volumes.filterLabel")}
          value={filter}
          onChange={setFilter}
          options={(["all", "writable", "attention"] as BindFilter[]).map((f) => ({ value: f, label: f === "attention" ? t("binds.filter.attention", { count: attention }) : t(`binds.filter.${f}`) }))}
        />
        <label className="relative max-w-[360px] min-w-[220px] flex-1">
          <span className="sr-only">{t("binds.search")}</span>
          <Search size={15} className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-ink-3" />
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("binds.search")}
            className="h-8 w-full rounded-md border border-line bg-surface pr-2 pl-8 text-[13px] outline-none placeholder:text-ink-3 hover:border-line-strong focus:border-beacon"
          />
        </label>
        <span className="text-[12.5px] text-ink-3 tabular-nums">{t("binds.count", { count: visible.length })}</span>
      </div>

      {q.error && (
        <div className="mb-4">
          <Banner tone="error">{t("binds.loadError", { message: (q.error as { message: string }).message })}</Banner>
        </div>
      )}
      {attention > 0 && filter !== "attention" && (
        <div className="mb-4">
          <Banner tone="warn">
            {t("binds.attentionBanner", { count: attention })}
          </Banner>
        </div>
      )}

      {q.isPending ? (
        <div className="flex items-center justify-center gap-2 py-20 text-[13px] text-ink-3">
          <Spinner /> {t("common.loading")}
        </div>
      ) : all.length === 0 && !q.error ? (
        <div className="py-20 text-center">
          <h2 className="text-[15px] font-semibold">{t("binds.empty")}</h2>
          <p className="mx-auto mt-1.5 max-w-[460px] text-[13px] text-ink-2">{t("binds.emptyBody", { context: remoteCtx })}</p>
        </div>
      ) : visible.length === 0 ? (
        <p className="py-10 text-center text-[13px] text-ink-2">{t("binds.noMatches")}</p>
      ) : (
        <ul className="space-y-3">
          {visible.map((b) => (
            <BindCard key={b.source} b={b} />
          ))}
        </ul>
      )}
    </div>
  );
}

export function BindsView() {
  return (
    <RequireConnection>
      <BindsList />
    </RequireConnection>
  );
}
