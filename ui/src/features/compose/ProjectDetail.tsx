import { useState } from "react";
import { useApi } from "@/app/server";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Download, Eye, Hammer, ListPlus, ListX, Play, RefreshCw, RotateCw, ScrollText, Square, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApp } from "@/app/store";
import { useContainersNav } from "@/features/containers/store";
import { Badge } from "@/features/networks/badges";
import type { ComposeProject } from "@/shared/ipc/bindings/ComposeProject";
import type { ComposeConfig } from "@/shared/ipc/bindings/ComposeConfig";
import type { ProjectRef } from "@/shared/ipc/bindings/ProjectRef";
import { keys } from "@/shared/query";
import { Button, CopyButton, cx } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { DownDialog, UpDialog } from "./dialogs";
import { FileEditor } from "./FileEditor";
import { LogsTab } from "./LogsTab";
import { buildAction, dirOf, hasFiles, relativeTo, portLabel, refOf, serviceRows, simpleAction, upAction, type ServiceRow } from "./model";
import { RunPanel, RunStatusLine } from "./RunPanel";
import { StateBadge } from "./StateBadge";
import { useProjectRun, useRunCompose } from "./runs";
import { useComposeNav, type ProjectTab } from "./store";

const TABS: ProjectTab[] = ["services", "logs", "file", "config", "output"];

function openContainer(id: string, name: string) {
  useContainersNav.getState().showDetail(id, name);
  useApp.getState().navigate("containers");
}

function Icon({ label, onClick, disabled, children }: { label: string; onClick: () => void; disabled?: boolean; children: React.ReactNode }) {
  return (
    <button type="button" aria-label={label} title={label} disabled={disabled} onClick={onClick} className="grid h-7 w-7 place-items-center rounded-md text-ink-2 hover:bg-surface-sunk hover:text-ink disabled:opacity-40 disabled:hover:bg-transparent">
      {children}
    </button>
  );
}

function ServicesTable({ project, rows, busy, canUp }: { project: ProjectRef; rows: ServiceRow[]; busy: boolean; canUp: boolean }) {
  const dir = project.workingDir ?? (project.files[0] ? dirOf(project.files[0]) : null);
  const { t } = useTranslation();
  const run = useRunCompose();
  const nav = useComposeNav();
  if (rows.length === 0) return <p className="text-[13px] text-ink-3">{t("composeDetail.noServices")}</p>;
  return (
    <div className="overflow-x-auto rounded-xl border border-line bg-surface">
      <table className="w-full min-w-[860px] table-fixed text-left text-[13px]">
        <thead className="border-b border-line bg-surface-2 text-[12px] text-ink-3">
          <tr>
            <th className="w-[18%] px-4 py-2 font-medium">{t("composeDetail.c.service")}</th>
            <th className="w-[24%] px-3 py-2 font-medium">{t("composeDetail.c.image")}</th>
            <th className="w-[14%] px-3 py-2 font-medium">{t("composeDetail.c.state")}</th>
            <th className="px-3 py-2 font-medium">{t("composeDetail.c.containers")}</th>
            <th className="w-[16%] px-3 py-2 font-medium">{t("composeDetail.c.ports")}</th>
            <th className="w-[136px] px-3 py-2 text-right font-medium">
              <span className="sr-only">{t("composeDetail.c.actions")}</span>
            </th>
          </tr>
        </thead>
        <tbody className="divide-y divide-line">
          {rows.map((r) => (
            <tr key={r.name} className="align-top">
              <td className="px-4 py-2.5">
                <span className="font-mono text-[12.5px] font-medium">{r.name}</span>
                <div className="mt-1 flex flex-wrap gap-1 empty:hidden">
                  {r.model?.profiles.map((p) => (
                    <Badge key={p}>{t("composeDetail.profile", { name: p })}</Badge>
                  ))}
                  {r.orphan && <Badge>{t("composeDetail.orphan")}</Badge>}
                </div>
              </td>
              <td className="px-3 py-2.5 font-mono text-[12px] [overflow-wrap:anywhere]">
                {r.model?.image ?? (r.model?.build ? <span className="text-ink-2">{t("composeDetail.built", { context: relativeTo(r.model.build, dir) })}</span> : (r.containers[0]?.image ?? "—"))}
                {r.model?.build && r.model.image && <span className="block font-sans text-[11.5px] text-ink-3">{t("composeDetail.builtFrom", { context: relativeTo(r.model.build, dir) })}</span>}
              </td>
              <td className="px-3 py-2.5 text-[12.5px]">
                <span className={cx(r.state === "running" && "text-ok", r.state === "partial" && "text-warn", (r.state === "stopped" || r.state === "notCreated") && "text-ink-3")}>{t(`composeDetail.state.${r.state}`, { running: r.running, total: r.containers.length })}</span>
              </td>
              <td className="px-3 py-2.5 text-[12.5px]">
                {r.containers.length === 0 ? (
                  <span className="text-ink-3">—</span>
                ) : (
                  <ul className="space-y-0.5">
                    {r.containers.map((c) => (
                      <li key={c.id}>
                        <button type="button" className="font-mono text-[12px] hover:text-beacon-strong hover:underline" onClick={() => openContainer(c.id, c.name)}>
                          {c.name}
                        </button>
                        <span className="ml-1.5 text-[11.5px] text-ink-3">{t(`containers.state.${c.state}`, { defaultValue: c.state })}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </td>
              <td className="px-3 py-2.5 font-mono text-[12px]">{r.model && r.model.ports.length > 0 ? r.model.ports.map(portLabel).join(", ") : <span className="text-ink-3">—</span>}</td>
              <td className="px-3 py-2">
                <div className="flex justify-end gap-0.5">
                  {r.state === "notCreated" ? (
                    <Icon label={t("composeDetail.upService", { name: r.name })} disabled={busy || !canUp} onClick={() => void run(project, upAction({ services: [r.name] }))}>
                      <Play size={15} />
                    </Icon>
                  ) : r.running === r.containers.length ? (
                    <Icon label={t("composeDetail.stopService", { name: r.name })} disabled={busy} onClick={() => void run(project, simpleAction("stop", [r.name]))}>
                      <Square size={14} />
                    </Icon>
                  ) : (
                    <Icon label={t("composeDetail.startService", { name: r.name })} disabled={busy} onClick={() => void run(project, simpleAction("start", [r.name]))}>
                      <Play size={15} />
                    </Icon>
                  )}
                  <Icon label={t("composeDetail.restartService", { name: r.name })} disabled={busy || r.containers.length === 0} onClick={() => void run(project, simpleAction("restart", [r.name]))}>
                    <RotateCw size={14} />
                  </Icon>
                  <Icon label={t("composeDetail.logsService", { name: r.name })} disabled={r.containers.length === 0} onClick={() => nav.showLogs([r.name])}>
                    <ScrollText size={14} />
                  </Icon>
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function ConfigTab({ cfg }: { cfg: ComposeConfig | undefined }) {
  const { t } = useTranslation();
  const [show, setShow] = useState(false);
  if (!cfg) return <p className="text-[13px] text-ink-3">{t("composeDetail.noConfig")}</p>;
  if (cfg.error) return <Banner tone="error">{cfg.error}</Banner>;
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      <p className="text-[12.5px] text-ink-3">{t("composeDetail.configHint")}</p>
      <div className="flex items-start gap-2 rounded-md bg-code px-2.5 py-1.5">
        <code className="selectable flex-1 font-mono text-[11.5px] [overflow-wrap:anywhere]">{cfg.command}</code>
      </div>
      {cfg.warnings.length > 0 && (
        <Banner tone="warn">
          <ul className="list-disc pl-5">
            {cfg.warnings.map((w, i) => (
              <li key={i}>{w}</li>
            ))}
          </ul>
        </Banner>
      )}
      {show ? (
        <>
          <div className="flex justify-end">{cfg.resolved && <CopyButton text={cfg.resolved} />}</div>
          <pre data-testid="compose-resolved" className="selectable min-h-[360px] flex-1 overflow-auto rounded-lg border border-line bg-log p-3 font-mono text-[12px] leading-[1.5]">
            {cfg.resolved}
          </pre>
        </>
      ) : (
        <Banner tone="info">
          <p>{t("composeDetail.secretsWarn")}</p>
          <div className="mt-2">
            <Button size="sm" onClick={() => setShow(true)}>
              <Eye size={13} />
              {t("composeDetail.showResolved")}
            </Button>
          </div>
        </Banner>
      )}
    </div>
  );
}

type Dialog = "up" | "down" | null;

export function ProjectDetail({ name, tab, logServices }: { name: string; tab: ProjectTab; logServices?: string[] }) {
  const api = useApi();
  const { t } = useTranslation();
  const qc = useQueryClient();
  const nav = useComposeNav();
  const run = useRunCompose();
  const projects = useQuery({ queryKey: keys.composeProjects, queryFn: api.composeProjects });
  const p: ComposeProject | undefined = projects.data?.find((x) => x.name === name);
  const files = p ? hasFiles(p) : false;
  const ref = p ? refOf(p) : null;
  const cfg = useQuery({
    queryKey: ["composeConfig", name, p?.files ?? []],
    queryFn: () => api.composeConfig(ref!),
    enabled: !!ref && files,
    refetchOnWindowFocus: false,
    staleTime: 30_000,
  });
  const current = useProjectRun(name);
  const busy = current?.status === "running";
  const [dialog, setDialog] = useState<Dialog>(null);
  const model = cfg.data?.model ?? null;
  const rows = serviceRows(model, p?.containers ?? []);
  const canUp = files && !!model;

  const register = useMutation({
    mutationFn: () => api.composeRegister(p!.files, p!.name),
    onSuccess: () => void qc.invalidateQueries({ queryKey: keys.composeProjects }),
  });
  const unregister = useMutation({
    mutationFn: () => api.composeUnregister(name),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: keys.composeProjects });
      if (p && p.total === 0) nav.showList();
    },
  });

  const act = (a: Parameters<typeof run>[1]) => {
    if (!ref) return;
    void run(ref, a);
    nav.setTab("output");
  };

  if (projects.isPending) {
    return (
      <div className="flex items-center justify-center gap-2 py-20 text-[13px] text-ink-3">
        <Spinner /> {t("common.loading")}
      </div>
    );
  }

  return (
    <div className="flex min-h-full flex-col">
      <div className="flex flex-wrap items-center gap-3 border-b border-line px-8 py-3">
        <Button variant="ghost" size="sm" onClick={nav.showList}>
          <ArrowLeft size={15} />
          {t("composeDetail.back")}
        </Button>
        <h2 className="selectable min-w-0 truncate font-mono text-[14.5px] font-semibold">{name}</h2>
        {p && <StateBadge p={p} />}
        {current && (
          <button type="button" onClick={() => nav.setTab("output")} className="hover:underline">
            <RunStatusLine run={current} />
          </button>
        )}
        <div className="ml-auto flex flex-wrap items-center gap-1.5">
          {p && (
            <>
              <Button size="sm" variant="primary" disabled={busy || !canUp} onClick={() => setDialog("up")} title={!files ? t("composeDetail.needsFiles") : !model ? t("composeDetail.needsValid") : undefined}>
                <Play size={14} />
                {t("composeOps.up")}
              </Button>
              <Button size="sm" disabled={busy || p.total === 0 || p.running === p.total} onClick={() => act(simpleAction("start"))}>
                <Play size={14} />
                {t("composeOps.start")}
              </Button>
              <Button size="sm" disabled={busy || p.running === 0} onClick={() => act(simpleAction("stop"))}>
                <Square size={13} />
                {t("composeOps.stopAction")}
              </Button>
              <Button size="sm" disabled={busy || p.total === 0} onClick={() => act(simpleAction("restart"))}>
                <RotateCw size={13} />
                {t("composeOps.restart")}
              </Button>
              <Button size="sm" disabled={busy || !canUp} onClick={() => act(simpleAction("pull"))}>
                <Download size={13} />
                {t("composeOps.pullAction")}
              </Button>
              {model?.services.some((s) => s.build) && (
                <Button size="sm" disabled={busy || !canUp} onClick={() => act(buildAction())}>
                  <Hammer size={13} />
                  {t("composeOps.buildAction")}
                </Button>
              )}
              <Button size="sm" disabled={busy || p.total === 0} onClick={() => setDialog("down")} className="hover:border-err/50 hover:text-err">
                <Trash2 size={13} />
                {t("composeOps.down")}
              </Button>
              {p.registered ? (
                <Button size="sm" variant="ghost" onClick={() => unregister.mutate()} title={t("composeDetail.unregisterHint")}>
                  <ListX size={14} />
                  {t("composeDetail.unregister")}
                </Button>
              ) : (
                files && (
                  <Button size="sm" variant="ghost" onClick={() => register.mutate()} disabled={register.isPending} title={t("composeDetail.registerHint")}>
                    <ListPlus size={14} />
                    {t("composeDetail.register")}
                  </Button>
                )
              )}
            </>
          )}
          <Button
            size="sm"
            variant="ghost"
            aria-label={t("composeDetail.refresh")}
            title={t("composeDetail.refresh")}
            disabled={projects.isFetching || cfg.isFetching}
            onClick={() => {
              void qc.invalidateQueries({ queryKey: keys.composeProjects });
              void qc.invalidateQueries({ queryKey: ["composeConfig", name] });
            }}
          >
            {projects.isFetching || cfg.isFetching ? <Spinner size={14} /> : <RefreshCw size={14} />}
          </Button>
        </div>
      </div>

      {!p ? (
        <div className="px-8 py-6">
          <Banner tone="info">{t("composeDetail.gone", { name })}</Banner>
        </div>
      ) : (
        <div className="flex min-h-0 flex-1 flex-col px-8 py-4">
          <div className="mb-3 space-y-1 text-[12.5px] text-ink-3">
            {p.files.length > 0 && (
              <p className="selectable font-mono [overflow-wrap:anywhere]">
                {p.files.join(" · ")}
              </p>
            )}
            {p.createdBy && <p>{t("composeDetail.createdBy", { version: p.createdBy })}</p>}
          </div>
          {p.missingFiles.length > 0 && (
            <div className="mb-3">
              <Banner tone="warn">{t("composeDetail.missingFiles", { files: p.missingFiles.join(", ") })}</Banner>
            </div>
          )}
          {p.files.length === 0 && (
            <div className="mb-3">
              <Banner tone="warn">{t("composeDetail.noFiles")}</Banner>
            </div>
          )}
          {cfg.data?.error && tab !== "file" && (
            <div className="mb-3">
              <Banner tone="error">
                <p>{t("composeDetail.invalidConfig")}</p>
                <pre className="selectable mt-1 font-mono text-[12px] whitespace-pre-wrap">{cfg.data.error}</pre>
              </Banner>
            </div>
          )}
          {cfg.error && (
            <div className="mb-3">
              <Banner tone="error">{(cfg.error as { message: string }).message}</Banner>
            </div>
          )}
          {(register.error || unregister.error) && (
            <div className="mb-3">
              <Banner tone="error">{((register.error ?? unregister.error) as { message: string }).message}</Banner>
            </div>
          )}

          <div role="tablist" aria-label={name} className="mb-4 flex flex-wrap gap-1 border-b border-line">
            {TABS.filter((tb) => tb !== "file" || files).map((tb) => (
              <button
                key={tb}
                type="button"
                role="tab"
                aria-selected={tab === tb}
                onClick={() => nav.setTab(tb)}
                className={cx("-mb-px h-9 border-b-2 px-3 text-[13px]", tab === tb ? "border-beacon font-medium text-ink" : "border-transparent text-ink-2 hover:text-ink")}
              >
                {t(`composeDetail.tabs.${tb}`)}
                {tb === "services" && rows.length > 0 && <span className="ml-1.5 text-[12px] text-ink-3 tabular-nums">{rows.length}</span>}
                {tb === "output" && busy && <Spinner size={11} className="ml-1.5 inline" />}
              </button>
            ))}
          </div>

          {tab === "services" && (
            <>
              {cfg.isFetching && !cfg.data && (
                <p className="mb-2 flex items-center gap-2 text-[12.5px] text-ink-3">
                  <Spinner size={13} /> {t("composeDetail.loadingModel")}
                </p>
              )}
              <ServicesTable project={refOf(p)} rows={rows} busy={busy} canUp={canUp} />
            </>
          )}
          {tab === "logs" && <LogsTab key={(logServices ?? []).join(",")} project={refOf(p)} services={rows.filter((r) => r.containers.length > 0).map((r) => r.name)} initial={logServices} />}
          {tab === "file" && files && <FileEditor project={refOf(p)} onSaved={() => void qc.invalidateQueries({ queryKey: ["composeConfig", name] })} />}
          {tab === "config" && <ConfigTab cfg={cfg.data} />}
          {tab === "output" && <RunPanel project={name} />}
        </div>
      )}

      {dialog === "up" && ref && <UpDialog project={ref} model={model} onClose={() => setDialog(null)} onStarted={() => nav.setTab("output")} />}
      {dialog === "down" && ref && p && <DownDialog project={ref} model={model} containers={p.total} onClose={() => setDialog(null)} onStarted={() => nav.setTab("output")} />}
    </div>
  );
}
