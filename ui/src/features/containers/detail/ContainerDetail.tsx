import { Suspense, lazy, useState } from "react";
import { useApi } from "@/app/server";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Copy, PackagePlus, PencilLine, RefreshCw, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { ContainerAction } from "@/shared/ipc/bindings/ContainerAction";
import type { ContainerDetails } from "@/shared/ipc/bindings/ContainerDetails";
import { keys } from "@/shared/query";
import { Button, CopyButton, cx } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { ICONS, RemoveDialog, STATE_DOT, allowedActions } from "../containerUi";
import { useContainersNav, type DetailTab } from "../store";
import { CommitDialog } from "../edit/CommitDialog";
import { LogsPanel } from "./LogsPanel";
import { StatsPanel } from "./StatsPanel";
import { SummaryPanel } from "./SummaryPanel";

// xterm.js (unos 280 KB) solo se carga al abrir la pestaña Terminal (Fase 9).
const TerminalPanel = lazy(() => import("./TerminalPanel").then((m) => ({ default: m.TerminalPanel })));

const TABS: DetailTab[] = ["summary", "logs", "terminal", "stats", "inspect"];

export const detailKey = (id: string) => ["container", id] as const;

/** Estado tal como lo muestra la lista (`docker ps`). */
export function stateOf(d: ContainerDetails): string {
  return d.status;
}

export function ContainerDetail({ id, name, tab }: { id: string; name: string; tab: DetailTab }) {
  const api = useApi();
  const { t } = useTranslation();
  const qc = useQueryClient();
  const nav = useContainersNav();
  const notices = useContainersNav((s) => (s.page.kind === "detail" ? s.page.notices : []));
  // Las pestañas con flujos se mantienen montadas tras visitarlas, para no perder la sesión.
  const [visited, setVisited] = useState<Set<DetailTab>>(() => new Set([tab]));
  const [failure, setFailure] = useState<string | null>(null);
  const [removing, setRemoving] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [committing, setCommitting] = useState(false);

  const q = useQuery({ queryKey: detailKey(id), queryFn: () => api.containerInspect(id, name) });
  const d = q.data;
  const display = d?.name ?? name;

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: detailKey(id) });
    void qc.invalidateQueries({ queryKey: keys.containers });
    void qc.invalidateQueries({ queryKey: keys.overview });
  };

  const action = useMutation({
    mutationFn: (a: ContainerAction) => api.containerAction(id, display, a),
    onMutate: (a) => {
      setFailure(null);
      setBusy(t(`containers.action.${a}`));
    },
    onError: (e, a) => setFailure(t("containers.failed", { action: t(`containers.action.${a}`), name: display, message: (e as { message: string }).message })),
    onSettled: () => {
      setBusy(null);
      refresh();
    },
  });

  const remove = useMutation({
    mutationFn: ({ force, volumes }: { force: boolean; volumes: boolean }) => api.containerRemove(id, display, force, volumes),
    onMutate: () => {
      setFailure(null);
      setBusy(t("containers.action.remove"));
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.containers });
      void qc.invalidateQueries({ queryKey: keys.overview });
      qc.removeQueries({ queryKey: detailKey(id) });
      nav.showList();
    },
    onError: (e) => setFailure(t("containers.failed", { action: t("containers.action.remove"), name: display, message: (e as { message: string }).message })),
    onSettled: () => setBusy(null),
  });

  const duplicate = useMutation({
    mutationFn: () => api.containerDuplicateSpec(id, display),
    onMutate: () => setFailure(null),
    onSuccess: (r) => nav.showCreate({ spec: r.spec, notes: r.notes, sourceName: r.sourceName }),
    onError: (e) => setFailure(t("detail.duplicateFailed", { message: (e as { message: string }).message })),
  });

  const go = (tb: DetailTab) => {
    setVisited((v) => new Set(v).add(tb));
    nav.setTab(tb);
  };

  const status = d ? stateOf(d) : null;
  const running = d?.running === true && !d.paused;
  const notFound = (q.error as { httpStatus?: number | null } | null)?.httpStatus === 404;

  return (
    <div className="flex min-h-full flex-col">
      <div className="flex flex-wrap items-center gap-3 border-b border-line px-8 py-3">
        <Button variant="ghost" size="sm" onClick={nav.showList}>
          <ArrowLeft size={15} />
          {t("detail.back")}
        </Button>
        <div className="flex min-w-0 items-center gap-2">
          <span aria-hidden className={cx("h-2.5 w-2.5 shrink-0 rounded-full", STATE_DOT[status ?? ""] ?? "bg-off")} />
          <h2 className="selectable truncate text-[15px] font-semibold">{display}</h2>
          {status && <span className="text-[13px] text-ink-2">{t(`containers.state.${status}`, { defaultValue: status })}</span>}
        </div>
        <div className="ml-auto flex flex-wrap items-center gap-1.5">
          {busy ? (
            <span className="flex h-8 items-center gap-1.5 px-2 text-[12.5px] text-ink-3">
              <Spinner size={14} /> {busy}
            </span>
          ) : (
            d && (
              <>
                {allowedActions(status ?? "").map((a) => {
                  const Icon = ICONS[a];
                  return (
                    <Button key={a} size="sm" onClick={() => action.mutate(a)}>
                      <Icon size={14} />
                      {t(`containers.action.${a}`)}
                    </Button>
                  );
                })}
                <Button size="sm" onClick={() => nav.showEdit(id, display)}>
                  <PencilLine size={14} />
                  {t("edit.action")}
                </Button>
                <Button size="sm" onClick={() => setCommitting(true)} title={t("commit.action")}>
                  <PackagePlus size={14} />
                  {t("commit.short")}
                </Button>
                <Button size="sm" onClick={() => duplicate.mutate()} disabled={duplicate.isPending}>
                  {duplicate.isPending ? <Spinner size={14} /> : <Copy size={14} />}
                  {t("detail.duplicate")}
                </Button>
                <Button size="sm" onClick={() => setRemoving(true)} className="hover:border-err/50 hover:text-err">
                  <Trash2 size={14} />
                  {t("containers.action.remove")}
                </Button>
              </>
            )
          )}
          <Button size="sm" variant="ghost" onClick={refresh} aria-label={t("containers.refresh")} title={t("containers.refresh")} disabled={q.isFetching}>
            {q.isFetching ? <Spinner size={14} /> : <RefreshCw size={14} />}
          </Button>
        </div>
      </div>

      <div className="space-y-3 px-8 pt-4 empty:hidden">
        {notices.map((n, i) => (
          <Banner key={i} tone={n.tone} onDismiss={nav.dismissNotices} dismissLabel={t("common.dismiss")}>
            {n.text}
          </Banner>
        ))}
        {failure && (
          <Banner tone="error" onDismiss={() => setFailure(null)} dismissLabel={t("common.dismiss")}>
            {failure}
          </Banner>
        )}
      </div>

      {q.isPending ? (
        <div className="flex items-center justify-center gap-2 py-20 text-[13px] text-ink-3">
          <Spinner /> {t("common.loading")}
        </div>
      ) : q.error && !d ? (
        <div className="px-8 py-6">
          <Banner tone="error">{notFound ? t("detail.notFound", { name: display }) : t("detail.loadError", { message: (q.error as { message: string }).message })}</Banner>
        </div>
      ) : d ? (
        <div className="flex min-h-0 flex-1 flex-col px-8 py-4">
          <div role="tablist" aria-label={display} className="mb-4 flex flex-wrap gap-1 border-b border-line">
            {TABS.map((tb) => (
              <button
                key={tb}
                type="button"
                role="tab"
                aria-selected={tab === tb}
                onClick={() => go(tb)}
                className={cx("-mb-px h-9 border-b-2 px-3 text-[13px]", tab === tb ? "border-beacon font-medium text-ink" : "border-transparent text-ink-2 hover:text-ink")}
              >
                {t(`detail.tabs.${tb}`)}
              </button>
            ))}
          </div>
          {tab === "summary" && <SummaryPanel d={d} />}
          {(tab === "logs" || visited.has("logs")) && (
            <div className={cx("flex flex-col", tab !== "logs" && "hidden")}>
              <LogsPanel id={d.id} name={d.name} />
            </div>
          )}
          {(tab === "terminal" || visited.has("terminal")) && (
            <div className={cx("flex flex-col", tab !== "terminal" && "hidden")}>
              <Suspense fallback={<Spinner />}>
                <TerminalPanel id={d.id} name={d.name} running={running} />
              </Suspense>
            </div>
          )}
          {(tab === "stats" || visited.has("stats")) && (
            <div className={cx(tab !== "stats" && "hidden")}>
              <StatsPanel id={d.id} name={d.name} running={running} />
            </div>
          )}
          {tab === "inspect" && (
            <div className="flex min-h-0 flex-1 flex-col gap-2">
              <div className="flex items-center gap-2">
                <p className="flex-1 text-[12.5px] text-ink-3">{t("detail.inspectHint", { name: d.name })}</p>
                <CopyButton text={d.raw} />
              </div>
              <pre data-testid="inspect-json" className="selectable min-h-[400px] flex-1 overflow-auto rounded-lg border border-line bg-log p-3 font-mono text-[12px] leading-[1.5]">
                {d.raw}
              </pre>
            </div>
          )}
        </div>
      ) : null}

      {committing && <CommitDialog id={id} name={display} onClose={() => setCommitting(false)} />}
      {removing && d && (
        <RemoveDialog
          c={{ id: d.id, name: d.name, image: d.image, state: status ?? "" }}
          onClose={() => setRemoving(false)}
          onConfirm={(force, volumes) => {
            setRemoving(false);
            remove.mutate({ force, volumes });
          }}
        />
      )}
    </div>
  );
}
