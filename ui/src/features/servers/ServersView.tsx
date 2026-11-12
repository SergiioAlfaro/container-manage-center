/**
 * «Servidores» (0.10.0, §3, §7, §10): lista con el estado de cada servidor,
 * alta y edición, prueba y diagnóstico, conexión y desconexión, eliminación
 * del perfil y vista general de todos los servidores.
 */
import { useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { ArrowLeft, MoreHorizontal, Plug, Plus, RefreshCw, Stethoscope, Unplug } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApp } from "@/app/store";
import { LOCAL_SERVER, useServers } from "@/app/server";
import { api, toCommandError } from "@/shared/ipc/client";
import type { ServerView } from "@/shared/ipc/bindings/ServerView";
import type { ServerConfig } from "@/shared/ipc/bindings/ServerConfig";
import { formatBytes, formatTimestamp } from "@/shared/format";
import { Button, Segmented, cx } from "@/shared/ui/controls";
import { Banner, Dialog, Spinner } from "@/shared/ui/feedback";
import { Menu } from "@/shared/ui/Menu";
import { isIdentityProblem, IdentityReview } from "./IdentityReview";
import { useServersNav } from "./nav";
import { displayAddress, remoteProblemOf, statusOf } from "./remote";
import { ServerEditor } from "./ServerEditor";
import { CheckList, KindIcon, RemoteProblemView, StatusLabel } from "./ui";

export function ServersView() {
  const nav = useServersNav();
  if (nav.page.kind === "edit") return <ServerEditor key={nav.page.id ?? "new"} id={nav.page.id} />;
  if (nav.page.kind === "diagnose") return <Diagnose id={nav.page.id} />;
  return <ServersHome />;
}

function ServersHome() {
  const { t } = useTranslation();
  const nav = useServersNav();
  return (
    <div className="px-8 py-6">
      <div className="mb-5 flex flex-wrap items-center gap-3">
        <Segmented
          label={t("servers.tabs.label")}
          value={nav.tab}
          options={[
            { value: "servers", label: t("servers.tabs.servers") },
            { value: "overview", label: t("servers.tabs.overview") },
          ]}
          onChange={nav.setTab}
        />
        <div className="ml-auto">
          <Button variant="primary" onClick={() => nav.edit(null)}>
            <Plus size={15} />
            {t("servers.add")}
          </Button>
        </div>
      </div>
      {nav.tab === "servers" ? <ServerList /> : <Overview />}
    </div>
  );
}

/** Abre un servidor en el área principal. */
export function useOpenServer() {
  const select = useServers((s) => s.select);
  const navigate = useApp((s) => s.navigate);
  return (id: string) => {
    select(id);
    navigate("dashboard");
  };
}

function ServerList() {
  const { t, i18n } = useTranslation();
  const servers = useServers((s) => s.servers);
  const states = useServers((s) => s.states);
  const busy = useServers((s) => s.busy);
  const error = useServers((s) => s.error);
  const notices = useServers((s) => s.keyringNotice);
  const { connect, disconnect, load } = useServers.getState();
  const nav = useServersNav();
  const open = useOpenServer();
  const [menu, setMenu] = useState<string | null>(null);
  const [removing, setRemoving] = useState<ServerView | null>(null);
  const [identity, setIdentity] = useState<string | null>(null);
  const remotes = servers.filter((s) => s.remote);

  return (
    <div className="space-y-4">
      {error && <Banner tone="error">{error}</Banner>}
      {Object.entries(notices).map(
        ([id, n]) =>
          n && (
            <Banner key={id} tone="warn">
              {t("servers.keyringNotice", {
                name: servers.find((s) => s.id === id)?.name ?? id,
                message: n,
              })}
            </Banner>
          ),
      )}
      {remotes.length === 0 && (
        <div className="rounded-xl border border-dashed border-line px-6 py-8 text-center">
          <p className="text-[14px] font-medium">{t("servers.empty.title")}</p>
          <p className="mx-auto mt-1 max-w-[520px] text-[13px] text-ink-2">{t("servers.empty.body")}</p>
        </div>
      )}
      <div className="overflow-x-auto rounded-xl border border-line bg-surface">
        <table className="w-full min-w-[980px] text-left text-[13px]">
          <thead className="border-b border-line bg-surface-2 text-[12px] text-ink-3">
            <tr>
              <th className="min-w-[160px] px-4 py-2 font-medium">{t("servers.columns.name")}</th>
              <th className="px-3 py-2 font-medium">{t("servers.columns.address")}</th>
              <th className="px-3 py-2 font-medium">{t("servers.columns.method")}</th>
              <th className="px-3 py-2 font-medium">{t("servers.columns.status")}</th>
              <th className="px-3 py-2 font-medium">{t("servers.columns.docker")}</th>
              <th className="px-3 py-2 font-medium">{t("servers.columns.os")}</th>
              <th className="px-3 py-2 font-medium">{t("servers.columns.lastSeen")}</th>
              <th className="px-3 py-2 text-right font-medium">{t("servers.columns.latency")}</th>
              <th className="w-[200px] px-3 py-2 text-right font-medium">
                <span className="sr-only">{t("servers.columns.actions")}</span>
              </th>
            </tr>
          </thead>
          <tbody className="divide-y divide-line">
            {servers.map((s) => {
              const st = states[s.id] ?? s.state;
              const status = statusOf(st);
              const engine = st.state === "connected" ? st.engine : null;
              const seen = s.config?.lastSeen ?? null;
              const isBusy = !!busy[s.id];
              const problem = remoteProblemOf(st);
              const connected = st.state === "connected";
              return (
                <tr key={s.id} data-testid={`server-row-${s.id}`} className="align-top">
                  <td className="px-4 py-2.5">
                    <div className="flex items-center gap-2 font-medium">
                      <KindIcon remote={s.remote} className="text-ink-3" />
                      <span className="[overflow-wrap:anywhere]">{s.remote ? s.name : t("servers.local")}</span>
                    </div>
                    {s.config?.description && <p className="mt-0.5 text-[12px] text-ink-3 [overflow-wrap:anywhere]">{s.config.description}</p>}
                  </td>
                  {/* Sin cortes a mitad de la dirección; si es muy larga se recorta (completa en el título y en los detalles). */}
                  <td className="selectable max-w-[260px] truncate whitespace-nowrap px-3 py-2.5 font-mono text-[12px]" title={s.config ? displayAddress(s.config) : s.address}>
                    {s.config ? displayAddress(s.config) : s.address || "—"}
                  </td>
                  <td className="px-3 py-2.5 text-[12.5px]">{s.config ? t(`servers.auth.${s.config.auth}`) : t("servers.localSocket")}</td>
                  <td className="px-3 py-2.5 text-[12.5px]">
                    {isBusy ? (
                      <span className="inline-flex items-center gap-1.5">
                        <Spinner size={11} /> {t("servers.status.connecting")}
                      </span>
                    ) : (
                      <StatusLabel status={status} />
                    )}
                    {problem && isIdentityProblem(problem) && s.config && (
                      <button type="button" className="mt-1 block text-[12px] font-medium text-beacon-strong hover:underline" onClick={() => setIdentity(s.id)}>
                        {t("servers.hostKey.review")}
                      </button>
                    )}
                  </td>
                  <td className="px-3 py-2.5 text-[12.5px] tabular-nums">
                    {engine ? (
                      `${engine.version} · API ${engine.apiVersion}`
                    ) : seen ? (
                      <span className="text-ink-3">{`${seen.dockerVersion} · API ${seen.apiVersion}`}</span>
                    ) : (
                      "—"
                    )}
                  </td>
                  <td className="px-3 py-2.5 text-[12.5px]">{engine?.os ?? seen?.os ?? "—"}</td>
                  <td className="px-3 py-2.5 text-[12.5px]">
                    {s.remote
                      ? seen
                        ? formatTimestamp(new Date(seen.at), i18n.language, {
                            date: true,
                          })
                        : t("servers.never")
                      : "—"}
                  </td>
                  <td className="px-3 py-2.5 text-right text-[12.5px] tabular-nums">
                    {st.state === "connected" && st.latencyMs != null ? `${st.latencyMs} ms` : "—"}
                  </td>
                  <td className="px-3 py-2">
                    <div className="flex items-center justify-end gap-1.5">
                      <Button size="sm" onClick={() => open(s.id)}>
                        {t("servers.open")}
                      </Button>
                      {s.remote &&
                        (connected ? (
                          <Button
                            size="sm"
                            onClick={() => void disconnect(s.id)}
                            aria-label={t("servers.disconnectNamed", {
                              name: s.name,
                            })}
                          >
                            <Unplug size={14} />
                            {t("servers.disconnect")}
                          </Button>
                        ) : (
                          <Button
                            size="sm"
                            variant="primary"
                            disabled={isBusy}
                            onClick={() => void connect(s.id)}
                            aria-label={t("servers.connectNamed", {
                              name: s.name,
                            })}
                          >
                            <Plug size={14} />
                            {t("servers.connect")}
                          </Button>
                        ))}
                      <Menu
                        label={t("servers.moreActions", {
                          name: s.remote ? s.name : t("servers.local"),
                        })}
                        trigger={<MoreHorizontal size={16} />}
                        open={menu === s.id}
                        onOpenChange={(o) => setMenu(o ? s.id : null)}
                        items={[
                          {
                            id: "diagnose",
                            label: t("servers.diagnose"),
                            icon: <Stethoscope size={14} />,
                            onSelect: () => nav.diagnose(s.id),
                          },
                          ...(s.remote
                            ? [
                                {
                                  id: "edit",
                                  label: t("servers.edit"),
                                  onSelect: () => nav.edit(s.id),
                                },
                                {
                                  id: "delete",
                                  label: t("servers.delete"),
                                  danger: true,
                                  onSelect: () => setRemoving(s),
                                },
                              ]
                            : [
                                {
                                  id: "reconnect",
                                  label: t("conn.reconnect"),
                                  onSelect: () => void connect(LOCAL_SERVER),
                                },
                              ]),
                        ]}
                      />
                    </div>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <p className="text-[12px] text-ink-3">{t("servers.listHint")}</p>
      {removing && removing.config && (
        <DeleteDialog server={removing} config={removing.config} onClose={() => setRemoving(null)} onDeleted={() => void load()} />
      )}
      {identity && <IdentityDialog id={identity} onClose={() => setIdentity(null)} />}
    </div>
  );
}

function IdentityDialog({ id, onClose }: { id: string; onClose: () => void }) {
  const { t } = useTranslation();
  const s = useServers((x) => x.servers.find((y) => y.id === id));
  const st = useServers((x) => x.states[id]);
  const connect = useServers((x) => x.connect);
  const problem = remoteProblemOf(st);
  if (!s?.config || !isIdentityProblem(problem)) return null;
  return (
    <Dialog target={false} title={t("servers.hostKey.reviewTitle", { name: s.name })} onClose={onClose} wide footer={<Button onClick={onClose}>{t("common.close")}</Button>}>
      <IdentityReview
        config={s.config}
        problem={problem}
        onTrusted={() => {
          onClose();
          void connect(id);
        }}
      />
    </Dialog>
  );
}

/** Eliminar el perfil: solo datos de la aplicación (§3.6). */
function DeleteDialog({ server, config, onClose, onDeleted }: { server: ServerView; config: ServerConfig; onClose: () => void; onDeleted: () => void }) {
  const { t } = useTranslation();
  const del = useMutation({
    mutationFn: () => api.serverDelete(server.id),
    onSuccess: () => {
      onDeleted();
      onClose();
    },
  });
  return (
    <Dialog
      target={false}
      title={t("servers.deleteDialog.title", { name: server.name })}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="danger" disabled={del.isPending} onClick={() => del.mutate()}>
            {del.isPending && <Spinner size={14} />}
            {t("servers.deleteDialog.confirm")}
          </Button>
        </>
      }
    >
      <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 rounded-lg border border-line bg-surface-2 px-3 py-2 text-[12.5px]">
        <dt className="text-ink-3">{t("servers.columns.name")}</dt>
        <dd className="font-medium">{server.name}</dd>
        <dt className="text-ink-3">{t("servers.columns.address")}</dt>
        <dd className="selectable font-mono">{displayAddress(config)}</dd>
      </dl>
      <p>{t("servers.deleteDialog.body")}</p>
      <Banner tone="info">{t("servers.deleteDialog.nothingRemote")}</Banner>
      {del.error && <Banner tone="error">{toCommandError(del.error).message}</Banner>}
    </Dialog>
  );
}

/** Vista general de todos los servidores (§7). Cada uno se consulta por separado. */
function Overview() {
  const { t, i18n } = useTranslation();
  const open = useOpenServer();
  const q = useQuery({
    queryKey: ["serversOverview"],
    queryFn: api.serversOverview,
    refetchOnWindowFocus: false,
  });
  return (
    <div className="space-y-3">
      <div className="flex items-center gap-3">
        <p className="flex-1 text-[13px] text-ink-2">{t("servers.overview.intro")}</p>
        <Button size="sm" onClick={() => void q.refetch()} disabled={q.isFetching}>
          {q.isFetching ? <Spinner size={13} /> : <RefreshCw size={13} />}
          {t("servers.refresh")}
        </Button>
      </div>
      {q.error && <Banner tone="error">{toCommandError(q.error).message}</Banner>}
      <div className="grid gap-3 [grid-template-columns:repeat(auto-fill,minmax(min(100%,300px),1fr))]">
        {(q.data ?? []).map((o) => {
          const s = o.server;
          const info = o.overview?.info;
          const status = statusOf(s.state);
          return (
            <section
              key={s.id}
              data-testid={`overview-${s.id}`}
              aria-label={s.remote ? s.name : t("servers.local")}
              className="flex flex-col rounded-xl border border-line bg-surface p-4"
            >
              <div className="flex items-start gap-2">
                <KindIcon remote={s.remote} className="mt-0.5 text-ink-3" />
                <div className="min-w-0 flex-1">
                  <h3 className="truncate text-[14px] font-semibold">{s.remote ? s.name : t("servers.local")}</h3>
                  <p className="selectable truncate font-mono text-[11.5px] text-ink-3">{s.config ? displayAddress(s.config) : s.address}</p>
                </div>
                <StatusLabel status={status} />
              </div>
              {info ? (
                <dl className="mt-3 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-[12.5px]">
                  <dt className="text-ink-3">{t("servers.overview.docker")}</dt>
                  <dd className="tabular-nums">{o.overview?.version.version ?? "—"}</dd>
                  <dt className="text-ink-3">{t("servers.overview.containers")}</dt>
                  <dd className="tabular-nums">
                    {t("servers.overview.containersValue", {
                      running: info.containersRunning ?? 0,
                      stopped: info.containersStopped ?? 0,
                      paused: info.containersPaused ?? 0,
                    })}
                  </dd>
                  <dt className="text-ink-3">{t("servers.overview.images")}</dt>
                  <dd className="tabular-nums">{info.images ?? "—"}</dd>
                  <dt className="text-ink-3">{t("servers.overview.resources")}</dt>
                  <dd className="tabular-nums">
                    {t("servers.overview.resourcesValue", {
                      cpus: info.ncpu ?? "?",
                      memory: info.memTotal ? formatBytes(info.memTotal, i18n.language) : "?",
                    })}
                  </dd>
                  <dt className="text-ink-3">{t("servers.overview.updated")}</dt>
                  <dd>
                    {formatTimestamp(new Date(o.updatedAt), i18n.language, {
                      date: true,
                    })}
                  </dd>
                </dl>
              ) : (
                <p className="mt-3 text-[12.5px] text-ink-3">{o.error ? o.error.message : t("servers.overview.notConnected")}</p>
              )}
              <div className="mt-auto pt-3">
                <Button size="sm" onClick={() => open(s.id)}>
                  {t("servers.open")}
                </Button>
              </div>
            </section>
          );
        })}
      </div>
    </div>
  );
}

/** Diagnóstico de un servidor (§10). */
function Diagnose({ id }: { id: string }) {
  const { t, i18n } = useTranslation();
  const nav = useServersNav();
  const q = useQuery({
    queryKey: ["serverDiagnose", id],
    queryFn: () => api.serverDiagnose(id),
    refetchOnWindowFocus: false,
    staleTime: Infinity,
  });
  const d = q.data;
  const name = d ? (d.server.remote ? d.server.name : t("servers.local")) : "";
  const r = d?.report ?? null;
  return (
    <div className="flex min-h-full flex-col">
      <div className="flex items-center gap-3 border-b border-line px-8 py-3">
        <Button variant="ghost" size="sm" onClick={nav.showList}>
          <ArrowLeft size={15} />
          {t("servers.back")}
        </Button>
        <h2 className="text-[15px] font-semibold">{t("servers.diagnosis.title", { name })}</h2>
        <div className="ml-auto">
          <Button size="sm" onClick={() => void q.refetch()} disabled={q.isFetching}>
            {q.isFetching ? <Spinner size={13} /> : <RefreshCw size={13} />}
            {t("servers.diagnosis.again")}
          </Button>
        </div>
      </div>
      <div className="mx-auto w-full max-w-[860px] space-y-5 px-8 py-6">
        {q.isLoading && (
          <p role="status" className="flex items-center gap-2 text-[13px] text-ink-3">
            <Spinner /> {t("servers.diagnosis.running")}
          </p>
        )}
        {q.error && <Banner tone="error">{toCommandError(q.error).message}</Banner>}
        {d && !d.server.remote && <LocalSummary state={d.server.state} />}
        {r && (
          <>
            <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1.5 rounded-lg border border-line bg-surface px-4 py-3 text-[13px]">
              <dt className="text-ink-3">{t("servers.columns.address")}</dt>
              <dd className="selectable font-mono text-[12.5px]">{d!.server.config ? displayAddress(d!.server.config) : ""}</dd>
              <dt className="text-ink-3">{t("servers.diagnosis.fingerprint")}</dt>
              <dd className="selectable font-mono text-[12.5px] break-all">{r.presented ? `${r.presented.keyType} ${r.presented.fingerprint}` : "—"}</dd>
              <dt className="text-ink-3">{t("servers.diagnosis.docker")}</dt>
              <dd className="tabular-nums">{r.engine ? `${r.engine.version} · API ${r.engine.apiVersion} · ${r.engine.os}` : "—"}</dd>
              <dt className="text-ink-3">{t("servers.columns.latency")}</dt>
              <dd className="tabular-nums">{r.latencyMs != null ? `${r.latencyMs} ms` : "—"}</dd>
            </dl>
            <CheckList steps={r.steps} />
            {r.problem &&
              d!.server.config &&
              (isIdentityProblem(r.problem) ? (
                <IdentityReview config={d!.server.config} problem={r.problem} onTrusted={() => void q.refetch()} />
              ) : (
                <RemoteProblemView problem={r.problem} config={d!.server.config} />
              ))}
          </>
        )}
        {d && (
          <section>
            <h3 className="mb-2 text-[13px] font-semibold text-ink-2">{t("servers.diagnosis.recentErrors")}</h3>
            {d.recentErrors.length === 0 ? (
              <p className="text-[12.5px] text-ink-3">{t("servers.diagnosis.noErrors")}</p>
            ) : (
              <ul className="divide-y divide-line rounded-lg border border-line">
                {d.recentErrors.map((e, i) => (
                  <li key={i} className="px-3 py-2 text-[12.5px]">
                    <div className="flex gap-3">
                      <span className="font-medium">{e.kind}</span>
                      {e.target && <span className="text-ink-2">{e.target}</span>}
                      <span className="ml-auto text-ink-3">
                        {formatTimestamp(new Date(e.at), i18n.language, {
                          date: true,
                        })}
                      </span>
                    </div>
                    {e.error && <p className="selectable mt-0.5 text-err [overflow-wrap:anywhere]">{e.error}</p>}
                  </li>
                ))}
              </ul>
            )}
          </section>
        )}
        <p className="text-[12px] text-ink-3">{t("servers.test.readOnly")}</p>
      </div>
    </div>
  );
}

function LocalSummary({ state }: { state: ServerView["state"] }) {
  const { t } = useTranslation();
  return (
    <div className={cx("rounded-lg border px-4 py-3 text-[13px]", state.state === "connected" ? "border-line" : "border-err/30")}>
      <StatusLabel status={statusOf(state)} />
      <p className="mt-1 text-ink-2">{t("servers.diagnosis.localHint")}</p>
    </div>
  );
}
