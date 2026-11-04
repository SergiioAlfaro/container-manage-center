import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Pause, Play, RefreshCw, RotateCw, Search, Square, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { RequireConnection } from "@/features/engine/RequireConnection";
import { formatAbsolute, formatRelative, shortId, uniquePorts } from "@/shared/format";
import { api } from "@/shared/ipc/client";
import type { ContainerAction } from "@/shared/ipc/bindings/ContainerAction";
import type { ContainerSummary } from "@/shared/ipc/bindings/ContainerSummary";
import { keys } from "@/shared/query";
import { Button, Segmented, cx } from "@/shared/ui/controls";
import { Banner, Checkbox, Dialog, Spinner } from "@/shared/ui/feedback";

type Filter = "all" | "running" | "stopped";
type Op = ContainerAction | "remove";

const STATE_DOT: Record<string, string> = {
  running: "bg-ok",
  paused: "bg-warn",
  restarting: "bg-run",
  created: "bg-line-strong",
  exited: "bg-off",
  dead: "bg-err",
  removing: "bg-err",
};

/** Acciones válidas según el estado que informa Docker. */
export function allowedActions(state: string): ContainerAction[] {
  switch (state) {
    case "running":
      return ["stop", "restart", "pause"];
    case "paused":
      return ["unpause", "stop"];
    case "restarting":
      return ["stop"];
    case "created":
    case "exited":
    case "dead":
      return ["start"];
    default:
      return [];
  }
}

const ICONS: Record<ContainerAction, typeof Play> = { start: Play, stop: Square, restart: RotateCw, pause: Pause, unpause: Play };

export function matchesContainer(c: ContainerSummary, filter: Filter, query: string): boolean {
  if (filter === "running" && c.state !== "running") return false;
  if (filter === "stopped" && c.state === "running") return false;
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return [c.name, c.image, c.id, c.composeProject ?? ""].some((v) => v.toLowerCase().includes(q));
}

interface Failure {
  op: Op;
  name: string;
  message: string;
}

function RemoveDialog({ c, onClose, onConfirm }: { c: ContainerSummary; onClose: () => void; onConfirm: (force: boolean, volumes: boolean) => void }) {
  const { t } = useTranslation();
  const active = c.state === "running" || c.state === "paused" || c.state === "restarting";
  const [force, setForce] = useState(false);
  const [volumes, setVolumes] = useState(false);
  return (
    <Dialog
      title={t("containers.removeTitle", { name: c.name })}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("containers.cancel")}</Button>
          <Button variant="danger" disabled={active && !force} onClick={() => onConfirm(force, volumes)}>
            {t("containers.confirmRemove")}
          </Button>
        </>
      }
    >
      <p>{t("containers.removeBody")}</p>
      <p className="selectable flex gap-3 font-mono text-[12px] text-ink-3">
        <span>{c.image}</span>
        <span>{shortId(c.id)}</span>
      </p>
      {active && (
        <>
          <Banner tone="warn">{t("containers.removeRunning", { state: t(`containers.state.${c.state}`).toLowerCase() })}</Banner>
          <Checkbox checked={force} onChange={setForce} label={t("containers.removeForce")} />
        </>
      )}
      <Checkbox checked={volumes} onChange={setVolumes} label={t("containers.removeVolumes")} hint={t("containers.removeVolumesHint")} />
    </Dialog>
  );
}

function ContainersTable() {
  const { t, i18n } = useTranslation();
  const qc = useQueryClient();
  const q = useQuery({ queryKey: keys.containers, queryFn: api.containersList });
  const [filter, setFilter] = useState<Filter>("all");
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState<Record<string, Op>>({});
  const [failure, setFailure] = useState<Failure | null>(null);
  const [removing, setRemoving] = useState<ContainerSummary | null>(null);

  const run = useMutation({
    mutationFn: async ({ c, op, force = false, volumes = false }: { c: ContainerSummary; op: Op; force?: boolean; volumes?: boolean }) =>
      op === "remove" ? api.containerRemove(c.id, c.name, force, volumes) : api.containerAction(c.id, c.name, op),
    onMutate: ({ c, op }) => {
      setFailure(null);
      setBusy((b) => ({ ...b, [c.id]: op }));
    },
    onError: (e, { c, op }) => setFailure({ op, name: c.name, message: (e as { message: string }).message }),
    // El estado confirmado por inspect se aplica de inmediato (D-031); el listado
    // que se pide después ya es coherente con él.
    onSuccess: (confirmed, { c, op }) => {
      if (op === "remove" || !confirmed) return;
      qc.setQueryData<ContainerSummary[]>(keys.containers, (list) =>
        list?.map((x) => (x.id === c.id ? { ...x, state: confirmed.status } : x)),
      );
    },
    onSettled: (_d, _e, { c }) => {
      setBusy(({ [c.id]: _drop, ...rest }) => rest);
      // Los datos se vuelven a pedir a Docker después de cada operación.
      void qc.invalidateQueries({ queryKey: keys.containers });
      void qc.invalidateQueries({ queryKey: keys.overview });
      void qc.invalidateQueries({ queryKey: keys.images });
    },
  });

  const all = q.data ?? [];
  const visible = useMemo(() => all.filter((c) => matchesContainer(c, filter, query)), [all, filter, query]);

  return (
    <div className="px-8 py-6">
      <div className="mb-4 flex flex-wrap items-center gap-3">
        <Segmented<Filter>
          label={t("containers.columns.status")}
          value={filter}
          onChange={setFilter}
          options={(["all", "running", "stopped"] as Filter[]).map((f) => ({ value: f, label: t(`containers.filter.${f}`) }))}
        />
        <label className="relative min-w-[220px] flex-1 max-w-[360px]">
          <span className="sr-only">{t("containers.search")}</span>
          <Search size={15} className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-ink-3" />
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("containers.search")}
            className="h-8 w-full rounded-md border border-line bg-surface pr-2 pl-8 text-[13px] outline-none placeholder:text-ink-3 hover:border-line-strong focus:border-beacon"
          />
        </label>
        <span className="text-[12.5px] text-ink-3 tabular-nums">{t("containers.count", { count: visible.length })}</span>
        <Button className="ml-auto" onClick={() => void q.refetch()} disabled={q.isFetching}>
          {q.isFetching ? <Spinner size={14} /> : <RefreshCw size={14} />}
          {t("containers.refresh")}
        </Button>
      </div>

      {failure && (
        <div className="mb-4">
          <Banner tone="error" onDismiss={() => setFailure(null)} dismissLabel={t("containers.dismiss")}>
            {t("containers.failed", { action: t(`containers.action.${failure.op}`), name: failure.name, message: failure.message })}
          </Banner>
        </div>
      )}
      {q.error && (
        <div className="mb-4">
          <Banner tone="error">{t("containers.loadError", { message: (q.error as { message: string }).message })}</Banner>
        </div>
      )}

      {q.isPending ? (
        <div className="flex items-center justify-center gap-2 py-20 text-[13px] text-ink-3">
          <Spinner /> {t("common.loading")}
        </div>
      ) : all.length === 0 && !q.error ? (
        <div className="py-20 text-center">
          <h2 className="text-[15px] font-semibold">{t("containers.empty")}</h2>
          <p className="mx-auto mt-1.5 max-w-[420px] text-[13px] text-ink-2">{t("containers.emptyBody")}</p>
        </div>
      ) : (
        <div className="overflow-x-auto rounded-xl border border-line bg-surface">
          <table className="w-full min-w-[860px] table-fixed text-left text-[13px]">
            <thead className="border-b border-line bg-surface-2 text-[12px] text-ink-3">
              <tr>
                <th className="w-[26%] px-4 py-2 font-medium">{t("containers.columns.name")}</th>
                <th className="w-[22%] px-3 py-2 font-medium">{t("containers.columns.image")}</th>
                <th className="w-[16%] px-3 py-2 font-medium">{t("containers.columns.status")}</th>
                <th className="w-[15%] px-3 py-2 font-medium">{t("containers.columns.ports")}</th>
                <th className="w-[120px] px-3 py-2 font-medium">{t("containers.columns.created")}</th>
                <th className="w-[148px] px-3 py-2 text-right font-medium">{t("containers.columns.actions")}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-line">
              {visible.map((c) => {
                const op = busy[c.id];
                const ports = uniquePorts(c.ports);
                return (
                  <tr key={c.id} className={cx("align-top", op && "bg-surface-2/60")}>
                    <td className="px-4 py-2.5">
                      <div className="flex items-center gap-2">
                        <span aria-hidden className={cx("h-2 w-2 shrink-0 rounded-full", STATE_DOT[c.state] ?? "bg-off")} />
                        <span className="selectable truncate font-medium" title={c.name}>
                          {c.name}
                        </span>
                      </div>
                      <div className="mt-0.5 flex items-center gap-2 pl-4 text-[11.5px] text-ink-3">
                        <span className="selectable font-mono">{shortId(c.id)}</span>
                        {c.composeProject && <span className="truncate rounded bg-surface-sunk px-1.5">compose: {c.composeProject}</span>}
                      </div>
                    </td>
                    <td className="selectable truncate px-3 py-2.5 font-mono text-[12px]" title={c.image}>
                      {c.image}
                    </td>
                    <td className="px-3 py-2.5">
                      <div>{t(`containers.state.${c.state}`, { defaultValue: c.state })}</div>
                      <div className="truncate text-[11.5px] text-ink-3" title={c.status}>
                        {c.status}
                      </div>
                    </td>
                    <td className="selectable px-3 py-2.5 font-mono text-[11.5px] text-ink-2">
                      {ports.length === 0 ? <span className="text-ink-3">—</span> : ports.map((p) => <div key={p}>{p}</div>)}
                    </td>
                    <td className="truncate px-3 py-2.5 text-[12px] whitespace-nowrap text-ink-2" title={formatAbsolute(c.created, i18n.language)}>
                      {formatRelative(c.created, i18n.language)}
                    </td>
                    <td className="px-3 py-2">
                      <div className="flex justify-end gap-0.5">
                        {op ? (
                          <span className="flex h-7 items-center gap-1.5 px-2 text-[12px] text-ink-3">
                            <Spinner size={13} /> {t(`containers.action.${op}`)}
                          </span>
                        ) : (
                          <>
                            {allowedActions(c.state).map((a) => {
                              const Icon = ICONS[a];
                              const label = t("containers.actionOn", { action: t(`containers.action.${a}`), name: c.name });
                              return (
                                <button
                                  key={a}
                                  type="button"
                                  onClick={() => run.mutate({ c, op: a })}
                                  aria-label={label}
                                  title={t(`containers.action.${a}`)}
                                  className="grid h-7 w-7 place-items-center rounded-md text-ink-2 hover:bg-surface-sunk hover:text-ink"
                                >
                                  <Icon size={15} strokeWidth={1.75} />
                                </button>
                              );
                            })}
                            <button
                              type="button"
                              onClick={() => setRemoving(c)}
                              aria-label={t("containers.actionOn", { action: t("containers.action.remove"), name: c.name })}
                              title={t("containers.action.remove")}
                              className="grid h-7 w-7 place-items-center rounded-md text-ink-2 hover:bg-err/10 hover:text-err"
                            >
                              <Trash2 size={15} strokeWidth={1.75} />
                            </button>
                          </>
                        )}
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          {visible.length === 0 && <p className="px-4 py-10 text-center text-[13px] text-ink-2">{t("containers.noMatches")}</p>}
        </div>
      )}
      <p className="mt-3 text-[12px] text-ink-3">{t("containers.later")}</p>

      {removing && (
        <RemoveDialog
          c={removing}
          onClose={() => setRemoving(null)}
          onConfirm={(force, volumes) => {
            const c = removing;
            setRemoving(null);
            run.mutate({ c, op: "remove", force, volumes });
          }}
        />
      )}
    </div>
  );
}

export function ContainersView() {
  return (
    <RequireConnection>
      <ContainersTable />
    </RequireConnection>
  );
}
