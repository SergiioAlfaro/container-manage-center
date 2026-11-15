// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArchiveRestore, CalendarClock, CheckCircle2, DatabaseBackup, FolderOpen, History, Play, ShieldCheck, Square, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApi, useServerId, useServerView } from "@/app/server";
import { RequireConnection } from "@/features/engine/RequireConnection";
import { api } from "@/shared/ipc/client";
import type { BackupEntry } from "@/shared/ipc/bindings/BackupEntry";
import type { BackupJob } from "@/shared/ipc/bindings/BackupJob";
import type { BackupJobView } from "@/shared/ipc/bindings/BackupJobView";
import type { BackupRequest } from "@/shared/ipc/bindings/BackupRequest";
import type { CommandError } from "@/shared/ipc/bindings/CommandError";
import type { IntegrityReport } from "@/shared/ipc/bindings/IntegrityReport";
import type { VolumeMode } from "@/shared/ipc/bindings/VolumeMode";
import { formatBytes, formatTimestamp } from "@/shared/format";
import { keys } from "@/shared/query";
import { Button, cx, Segmented } from "@/shared/ui/controls";
import { Banner, Checkbox, Spinner } from "@/shared/ui/feedback";
import { ConfirmDialog, JobDialog, RestoreDialog, StopConfirmDialog, TimerDialog } from "./dialogs";
import { backupDate, codeText, databaseContainers, EMPTY_REQUEST, errorText, isEmptyRequest, itemWarnings, newJob, prefillRequest, retentionText, scheduleText, timerText } from "./model";
import { useBackupNav, useBackups } from "./store";

type Tab = "list" | "new" | "jobs" | "history";

export function BackupView() {
  return (
    <RequireConnection>
      <Backups />
    </RequireConnection>
  );
}

function when(iso: string, lang: string): string {
  if (!iso) return "";
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : formatTimestamp(d, lang, { date: true });
}

function Backups() {
  const { t } = useTranslation();
  const server = useServerId();
  // E6: al llegar desde la auditoría se abre «Nueva copia» con la selección.
  const [tab, setTab] = useState<Tab>(() => (useBackupNav.getState().prefill?.server === server ? "new" : "list"));
  const settings = useQuery({ queryKey: ["backupSettings"], queryFn: api.backupSettings });
  const qc = useQueryClient();
  const pick = useMutation({
    mutationFn: () => api.backupPickFolder(true),
    onSuccess: (p) => {
      if (p) void qc.invalidateQueries({ queryKey: ["backupSettings"] });
      void qc.invalidateQueries({ queryKey: ["backupList"] });
    },
  });
  const dest = settings.data?.destination ?? null;
  return (
    <div className="px-8 py-6">
      <p className="mb-4 flex max-w-[860px] items-start gap-2 text-[12.5px] text-ink-2">
        <DatabaseBackup size={16} className="mt-px shrink-0 text-beacon-strong" aria-hidden />
        {t("backup.intro")}
      </p>
      <section aria-label={t("backup.destination")} className="mb-4 flex flex-wrap items-center gap-3 rounded-xl border border-line bg-surface px-4 py-3">
        <FolderOpen size={16} className="text-ink-3" aria-hidden />
        <div className="min-w-0 flex-1">
          <div className="text-[12px] text-ink-3">{t("backup.destination")}</div>
          <div className="selectable truncate font-mono text-[12.5px]" title={dest ?? ""}>
            {dest ?? t("backup.noDestination")}
          </div>
        </div>
        <Button onClick={() => pick.mutate()} loading={pick.isPending}>
          {dest ? t("backup.changeDestination") : t("backup.chooseDestination")}
        </Button>
      </section>
      {pick.error && <Banner tone="error">{errorText(t, (k) => t(k) !== k, pick.error as unknown as CommandError)}</Banner>}
      <div className="mb-4">
        <Segmented
          label={t("backup.tabs")}
          value={tab}
          onChange={setTab}
          options={[
            { value: "list", label: t("backup.tab.list") },
            { value: "new", label: t("backup.tab.new") },
            { value: "jobs", label: t("backup.tab.jobs") },
            { value: "history", label: t("backup.tab.history") },
          ]}
        />
      </div>
      {tab === "list" && <BackupList destination={dest} />}
      {tab === "new" && <NewBackup destination={dest} onSaved={() => setTab("jobs")} />}
      {tab === "jobs" && <Jobs views={settings.data?.jobs ?? []} systemd={settings.data?.systemd ?? false} loading={settings.isPending} />}
      {tab === "history" && <RunHistory />}
    </div>
  );
}

// ------------------------------------------------------------------ copias

function StateBadge({ e }: { e: BackupEntry }) {
  const { t } = useTranslation();
  const cls = e.state === "complete" ? "bg-ok/12 text-ok" : "bg-warn/14 text-warn";
  return <span className={cx("rounded px-1.5 py-0.5 text-[11.5px] font-medium", cls)}>{t(`backup.state.${e.state}`)}</span>;
}

function BackupList({ destination }: { destination: string | null }) {
  const { t, i18n } = useTranslation();
  const exists = (k: string) => i18n.exists(k);
  const bound = useApi();
  const server = useServerId();
  const qc = useQueryClient();
  const list = useQuery({ queryKey: ["backupList", server, destination], queryFn: () => bound.backupList(), enabled: !!destination });
  const [open, setOpen] = useState<string | null>(null);
  const [checks, setChecks] = useState<Record<string, IntegrityReport | CommandError>>({});
  const [restore, setRestore] = useState<BackupEntry | null>(null);
  const [remove, setRemove] = useState<BackupEntry | null>(null);
  const verify = useMutation({
    mutationFn: (id: string) => bound.backupVerify(id),
    onSuccess: (r, id) => setChecks((c) => ({ ...c, [id]: r })),
    onError: (e, id) => setChecks((c) => ({ ...c, [id]: e as unknown as CommandError })),
  });
  if (!destination) return <Banner tone="info">{t("backup.chooseFirst")}</Banner>;
  if (list.isPending) return <Spinner />;
  if (list.error) return <Banner tone="error">{errorText(t, exists, list.error as unknown as CommandError)}</Banner>;
  const entries = list.data ?? [];
  if (entries.length === 0) return <p className="text-[13px] text-ink-3">{t("backup.empty")}</p>;
  return (
    <>
      <ul className="divide-y divide-line rounded-xl border border-line bg-surface" aria-label={t("backup.tab.list")}>
        {entries.map((e) => {
          const check = checks[e.id];
          const warns = itemWarnings(e);
          return (
            <li key={e.id} className="px-4 py-3">
              <div className="flex flex-wrap items-center gap-3">
                <button type="button" className="min-w-0 flex-1 text-left" onClick={() => setOpen(open === e.id ? null : e.id)} aria-expanded={open === e.id}>
                  <div className="flex flex-wrap items-center gap-2 text-[13px] font-medium">
                    {when(backupDate(e), i18n.language)}
                    <StateBadge e={e} />
                    {e.manifest?.job ? <span className="text-[12px] text-ink-3">{t("backup.byJob")}</span> : <span className="text-[12px] text-ink-3">{t("backup.manual")}</span>}
                  </div>
                  <div className="text-[12px] text-ink-3">
                    {t("backup.summary", { count: e.manifest?.items.length ?? 0, size: formatBytes(e.size, i18n.language) })}
                    {warns > 0 && ` · ${t("backup.warnings", { count: warns })}`}
                  </div>
                </button>
                {e.state === "complete" && (
                  <>
                    <Button size="sm" onClick={() => verify.mutate(e.id)} loading={verify.isPending && verify.variables === e.id}>
                      <ShieldCheck size={13} />
                      {t("backup.verify")}
                    </Button>
                    <Button size="sm" onClick={() => setRestore(e)}>
                      <ArchiveRestore size={13} />
                      {t("backup.restore.open")}
                    </Button>
                  </>
                )}
                <Button size="sm" variant="ghost" onClick={() => setRemove(e)} aria-label={t("backup.delete", { id: e.id })} title={t("backup.delete", { id: e.id })}>
                  <Trash2 size={13} />
                </Button>
              </div>
              {check && "ok" in check && (
                <p role="status" className={cx("mt-2 text-[12.5px]", check.ok ? "text-ok" : "text-err")}>
                  {check.ok
                    ? t("backup.verifyOk", { count: check.checked })
                    : t("backup.verifyBad", { altered: check.altered.join(", ") || "—", missing: check.missing.join(", ") || "—", unexpected: check.unexpected.join(", ") || "—" })}
                </p>
              )}
              {check && "code" in check && <p className="mt-2 text-[12.5px] text-err">{errorText(t, exists, check)}</p>}
              {e.state === "incomplete" && <p className="mt-2 text-[12.5px] text-warn">{t("backup.incompleteHint")}</p>}
              {open === e.id && e.manifest && (
                <ul className="mt-3 space-y-1.5 border-t border-line pt-3">
                  {e.manifest.items.map((it) => (
                    <li key={`${it.kind}:${it.name}`} className="text-[12.5px]">
                      <span className="font-medium">{t(`backup.kind.${it.kind}`)}</span> · <span className="font-mono">{it.name}</span> ·{" "}
                      <span className={it.consistency === "hot" ? "text-warn" : "text-ink-3"}>{t(`backup.consistency.${it.consistency}`)}</span>
                      {it.warnings.map((w) => (
                        <div key={w} className="ml-4 text-warn">
                          {codeText(t, exists, w)}
                        </div>
                      ))}
                    </li>
                  ))}
                </ul>
              )}
            </li>
          );
        })}
      </ul>
      {restore && <RestoreDialog entry={restore} onClose={() => setRestore(null)} />}
      {remove && (
        <ConfirmDialog
          title={t("backup.deleteTitle")}
          body={t("backup.deleteBody", { id: remove.id })}
          action={t("backup.deleteAction")}
          onClose={() => setRemove(null)}
          run={() => bound.backupDelete(remove.id)}
          onDone={() => {
            setRemove(null);
            void qc.invalidateQueries({ queryKey: ["backupList"] });
          }}
        />
      )}
    </>
  );
}

// ------------------------------------------------------------------ nueva copia

function NewBackup({ destination, onSaved }: { destination: string | null; onSaved: () => void }) {
  const { t, i18n } = useTranslation();
  const exists = (k: string) => i18n.exists(k);
  const bound = useApi();
  const server = useServerId();
  const remote = !!useServerView()?.remote;
  const qc = useQueryClient();
  const run = useBackups((s) => s.runs[server]);
  const start = useBackups((s) => s.start);
  const cancel = useBackups((s) => s.cancel);
  const volumes = useQuery({ queryKey: keys.volumes, queryFn: bound.volumesList });
  const containers = useQuery({ queryKey: keys.containers, queryFn: bound.containersList });
  const projects = useQuery({ queryKey: keys.composeProjects, queryFn: bound.composeProjects, enabled: !remote });
  const [req, setReq] = useState<BackupRequest>(EMPTY_REQUEST);
  const [fromAudit, setFromAudit] = useState<{ name: string; found: boolean } | null>(null);
  // E6: selección preparada desde la auditoría, cuando ya se conocen los
  // contenedores y volúmenes del servidor.
  useEffect(() => {
    if (!containers.data || !volumes.data || useBackupNav.getState().prefill?.server !== server) return;
    const p = useBackupNav.getState().take(server);
    if (!p) return;
    const r = prefillRequest(p.id, p.name, containers.data, volumes.data);
    if (r) setReq(r);
    setFromAudit({ name: p.name, found: !!r });
  }, [containers.data, volumes.data, server]);
  const [stopAsk, setStopAsk] = useState(false);
  const [job, setJob] = useState<BackupJob | null>(null);
  const running = run?.status === "running";
  const dbs = useMemo(() => databaseContainers(containers.data ?? []), [containers.data]);

  const toggle = <K extends "databases" | "containers">(k: K, v: string, on: boolean) =>
    setReq((r) => ({ ...r, [k]: on ? [...r[k], v] : r[k].filter((x) => x !== v) }));
  const volMode = (name: string) => req.volumes.find((v) => v.name === name)?.mode;
  const setVol = (name: string, mode: VolumeMode | null) =>
    setReq((r) => ({ ...r, volumes: mode ? [...r.volumes.filter((v) => v.name !== name), { name, mode }] : r.volumes.filter((v) => v.name !== name) }));
  const stopped = req.volumes
    .filter((v) => v.mode === "stop")
    .flatMap((v) => (volumes.data ?? []).find((x) => x.name === v.name)?.containers.filter((c) => c.state === "running").map((c) => c.name) ?? []);

  const go = () =>
    void start(server, req, null, (ok) => {
      if (ok) void qc.invalidateQueries({ queryKey: ["backupList"] });
      void qc.invalidateQueries({ queryKey: ["backupHistory"] });
    });
  const submit = () => (stopped.length > 0 ? setStopAsk(true) : go());
  const p = run?.progress;
  return (
    <div className="space-y-4">
      {!destination && <Banner tone="info">{t("backup.chooseFirst")}</Banner>}
      {fromAudit && <Banner tone={fromAudit.found ? "info" : "warn"}>{t(fromAudit.found ? "backup.fromAudit" : "backup.fromAuditGone", { name: fromAudit.name })}</Banner>}
      <fieldset className="rounded-xl border border-line bg-surface p-4" disabled={running}>
        <legend className="px-1 text-[13.5px] font-semibold">{t("backup.section.volumes")}</legend>
        <p className="mb-2 text-[12px] text-ink-3">{t("backup.volumesHint")}</p>
        {(volumes.data ?? []).length === 0 && <p className="text-[12.5px] text-ink-3">{t("backup.none")}</p>}
        <ul className="space-y-1.5">
          {(volumes.data ?? []).map((v) => {
            const busy = v.containers.some((c) => c.state === "running");
            const mode = volMode(v.name);
            return (
              <li key={v.name} className="flex flex-wrap items-center gap-3">
                <Checkbox checked={!!mode} onChange={(on) => setVol(v.name, on ? "hot" : null)} label={v.name} />
                {mode && busy && (
                  <select
                    aria-label={t("backup.mode", { name: v.name })}
                    className="h-7 rounded-md border border-line bg-surface px-2 text-[12.5px]"
                    value={mode}
                    onChange={(e) => setVol(v.name, e.target.value as VolumeMode)}
                  >
                    <option value="hot">{t("backup.modeHot")}</option>
                    <option value="stop">{t("backup.modeStop")}</option>
                  </select>
                )}
                {mode === "hot" && busy && <span className="text-[12px] text-warn">{t("backup.hotWarning")}</span>}
              </li>
            );
          })}
        </ul>
      </fieldset>
      <fieldset className="rounded-xl border border-line bg-surface p-4" disabled={running}>
        <legend className="px-1 text-[13.5px] font-semibold">{t("backup.section.databases")}</legend>
        <p className="mb-2 text-[12px] text-ink-3">{t("backup.databasesHint")}</p>
        {dbs.length === 0 && <p className="text-[12.5px] text-ink-3">{t("backup.noDatabases")}</p>}
        <ul className="space-y-1.5">
          {dbs.map((c) => (
            <li key={c.id}>
              <Checkbox checked={req.databases.includes(c.name)} onChange={(on) => toggle("databases", c.name, on)} label={`${c.name} · ${t(`backup.engine.${c.engine}`)}`} />
            </li>
          ))}
        </ul>
      </fieldset>
      <fieldset className="rounded-xl border border-line bg-surface p-4" disabled={running}>
        <legend className="px-1 text-[13.5px] font-semibold">{t("backup.section.config")}</legend>
        <p className="mb-2 text-[12px] text-ink-3">{t("backup.configHint")}</p>
        <ul className="grid gap-1.5 md:grid-cols-2">
          {(containers.data ?? []).map((c) => (
            <li key={c.id}>
              <Checkbox checked={req.containers.includes(c.name)} onChange={(on) => toggle("containers", c.name, on)} label={c.name} />
            </li>
          ))}
        </ul>
        <div className="mt-3 border-t border-line pt-3">
          <h3 className="mb-1.5 text-[12.5px] font-medium">{t("backup.section.compose")}</h3>
          {remote && <p className="text-[12px] text-ink-3">{t("backup.composeRemote")}</p>}
          {!remote && (projects.data ?? []).length === 0 && <p className="text-[12.5px] text-ink-3">{t("backup.none")}</p>}
          {!remote &&
            (projects.data ?? []).map((p) => {
              const sel = req.compose.find((c) => c.project === p.name);
              return (
                <div key={p.name} className="mb-1.5 flex flex-wrap items-center gap-4">
                  <Checkbox
                    checked={!!sel}
                    onChange={(on) => setReq((r) => ({ ...r, compose: on ? [...r.compose, { project: p.name, includeEnv: false }] : r.compose.filter((c) => c.project !== p.name) }))}
                    label={p.name}
                  />
                  {sel && (
                    <Checkbox
                      checked={sel.includeEnv}
                      onChange={(v) => setReq((r) => ({ ...r, compose: r.compose.map((c) => (c.project === sel.project ? { ...c, includeEnv: v } : c)) }))}
                      label={t("backup.includeEnv")}
                      hint={t("backup.includeEnvHint")}
                    />
                  )}
                </div>
              );
            })}
        </div>
        <div className="mt-3 border-t border-line pt-3">
          <Checkbox checked={req.appConfig} onChange={(v) => setReq((r) => ({ ...r, appConfig: v }))} label={t("backup.appConfig")} hint={t("backup.appConfigHint")} />
        </div>
      </fieldset>

      <div className="flex flex-wrap items-center gap-2">
        {running ? (
          <Button onClick={() => cancel(server)}>
            <Square size={13} />
            {t("backup.cancel")}
          </Button>
        ) : (
          <Button variant="primary" disabled={!destination || isEmptyRequest(req)} onClick={submit}>
            <Play size={14} />
            {t("backup.runNow")}
          </Button>
        )}
        <Button disabled={!destination || isEmptyRequest(req) || running} onClick={() => setJob(newJob(server, destination ?? "", req))}>
          <CalendarClock size={14} />
          {t("backup.saveAsJob")}
        </Button>
        {running && p && (
          <span role="status" className="flex items-center gap-2 text-[12.5px] text-ink-2">
            <Spinner size={14} />
            {t("backup.progress", { done: p.done, total: p.total, name: p.name, size: formatBytes(p.bytes, i18n.language) })}
          </span>
        )}
      </div>
      {run?.status === "done" && run.entry && (
        <Banner tone="info">
          <CheckCircle2 size={14} className="mr-1 inline text-ok" aria-hidden />
          {t("backup.done", { id: run.entry.id, size: formatBytes(run.entry.size, i18n.language) })}
        </Banner>
      )}
      {run?.status === "failed" && <Banner tone="error">{errorText(t, exists, run.error)}</Banner>}
      {run?.status === "cancelled" && <Banner tone="warn">{t("backup.cancelled")}</Banner>}
      {stopAsk && (
        <StopConfirmDialog
          containers={stopped}
          onClose={() => setStopAsk(false)}
          onConfirm={() => {
            setStopAsk(false);
            go();
          }}
        />
      )}
      {job && (
        <JobDialog
          job={job}
          onClose={() => setJob(null)}
          onSaved={() => {
            setJob(null);
            void qc.invalidateQueries({ queryKey: ["backupSettings"] });
            onSaved();
          }}
        />
      )}
    </div>
  );
}

// ------------------------------------------------------------------ tareas

function Jobs({ views, systemd, loading }: { views: BackupJobView[]; systemd: boolean; loading: boolean }) {
  const { t, i18n } = useTranslation();
  const exists = (k: string) => i18n.exists(k);
  const server = useServerId();
  const qc = useQueryClient();
  const run = useBackups((s) => s.runs[server]);
  const start = useBackups((s) => s.start);
  const [edit, setEdit] = useState<BackupJob | null>(null);
  const [timer, setTimer] = useState<BackupJobView | null>(null);
  const [confirm, setConfirm] = useState<{ kind: "unschedule" | "delete"; view: BackupJobView } | null>(null);
  const [err, setErr] = useState<CommandError | null>(null);
  const refresh = () => void qc.invalidateQueries({ queryKey: ["backupSettings"] });
  const toggle = useMutation({
    mutationFn: (v: { id: string; enabled: boolean }) => api.backupTimerSetEnabled(v.id, v.enabled),
    onSuccess: refresh,
    onError: (e) => setErr(e as unknown as CommandError),
  });
  if (loading) return <Spinner />;
  const mine = views.filter((v) => v.job.server === server);
  return (
    <div className="space-y-3">
      {!systemd && <Banner tone="warn">{t("backup.noSystemd")}</Banner>}
      <p className="text-[12px] text-ink-3">{t("backup.jobsHint")}</p>
      {err && (
        <Banner tone="error" onDismiss={() => setErr(null)} dismissLabel={t("common.close")}>
          {errorText(t, exists, err)}
        </Banner>
      )}
      {mine.length === 0 && <p className="text-[13px] text-ink-3">{t("backup.noJobs")}</p>}
      <ul className="divide-y divide-line rounded-xl border border-line bg-surface">
        {mine.map((v) => {
          const st = timerText(t, v.timer);
          const enabled = v.timer.installed && v.timer.unitFileState !== "disabled";
          return (
            <li key={v.job.id} className="px-4 py-3">
              <div className="flex flex-wrap items-center gap-3">
                <div className="min-w-0 flex-1">
                  <div className="text-[13px] font-medium">{v.job.name}</div>
                  <div className="text-[12px] text-ink-3">
                    {scheduleText(t, v.job.schedule)} · {retentionText(t, v.job.retention)}
                  </div>
                  <div className={cx("text-[12px]", st.tone === "ok" ? "text-ok" : st.tone === "warn" ? "text-warn" : "text-ink-3")}>{st.text}</div>
                </div>
                <Button size="sm" disabled={run?.status === "running"} onClick={() => void start(server, EMPTY_REQUEST, v.job.id, () => void qc.invalidateQueries({ queryKey: ["backupList"] }))}>
                  <Play size={13} />
                  {t("backup.runJob")}
                </Button>
                {systemd && !v.timer.foreign && (!v.timer.installed || v.timer.outdated) && (
                  <Button size="sm" variant="primary" onClick={() => setTimer(v)}>
                    <CalendarClock size={13} />
                    {v.timer.installed ? t("backup.timer.update") : t("backup.timer.schedule")}
                  </Button>
                )}
                {v.timer.installed && (
                  <Button size="sm" onClick={() => toggle.mutate({ id: v.job.id, enabled: !enabled })}>
                    {enabled ? t("backup.timer.disable") : t("backup.timer.enable")}
                  </Button>
                )}
                {v.timer.installed && (
                  <Button size="sm" onClick={() => setConfirm({ kind: "unschedule", view: v })}>
                    {t("backup.timer.remove")}
                  </Button>
                )}
                <Button size="sm" variant="ghost" onClick={() => setEdit(v.job)}>
                  {t("backup.edit")}
                </Button>
                <Button size="sm" variant="ghost" disabled={v.timer.installed} onClick={() => setConfirm({ kind: "delete", view: v })} aria-label={t("backup.jobDelete", { name: v.job.name })} title={t("backup.jobDelete", { name: v.job.name })}>
                  <Trash2 size={13} />
                </Button>
              </div>
            </li>
          );
        })}
      </ul>
      {edit && (
        <JobDialog
          job={edit}
          onClose={() => setEdit(null)}
          onSaved={() => {
            setEdit(null);
            refresh();
          }}
        />
      )}
      {timer && (
        <TimerDialog
          view={timer}
          onClose={() => setTimer(null)}
          onDone={() => {
            setTimer(null);
            refresh();
          }}
        />
      )}
      {confirm && (
        <ConfirmDialog
          title={confirm.kind === "delete" ? t("backup.jobDeleteTitle") : t("backup.timer.removeTitle")}
          body={confirm.kind === "delete" ? t("backup.jobDeleteBody", { name: confirm.view.job.name }) : t("backup.timer.removeBody", { name: confirm.view.job.name })}
          action={confirm.kind === "delete" ? t("backup.deleteAction") : t("backup.timer.remove")}
          onClose={() => setConfirm(null)}
          run={() => (confirm.kind === "delete" ? api.backupJobDelete(confirm.view.job.id) : api.backupTimerRemove(confirm.view.job.id))}
          onDone={() => {
            setConfirm(null);
            refresh();
          }}
        />
      )}
    </div>
  );
}

// ------------------------------------------------------------------ historial

function RunHistory() {
  const { t, i18n } = useTranslation();
  const bound = useApi();
  const server = useServerId();
  const h = useQuery({ queryKey: ["backupHistory", server], queryFn: bound.backupHistory });
  if (h.isPending) return <Spinner />;
  const runs = h.data ?? [];
  if (runs.length === 0) return <p className="text-[13px] text-ink-3">{t("backup.noHistory")}</p>;
  return (
    <table className="w-full rounded-xl border border-line bg-surface text-[12.5px]">
      <caption className="sr-only">{t("backup.tab.history")}</caption>
      <thead className="text-left text-ink-3">
        <tr>
          <th className="px-3 py-2 font-medium">{t("backup.col.when")}</th>
          <th className="px-3 py-2 font-medium">{t("backup.col.trigger")}</th>
          <th className="px-3 py-2 font-medium">{t("backup.col.result")}</th>
          <th className="px-3 py-2 font-medium">{t("backup.col.size")}</th>
        </tr>
      </thead>
      <tbody className="divide-y divide-line">
        {runs.map((r) => (
          <tr key={`${r.startedAt}-${r.backupId ?? ""}-${r.trigger}`}>
            <td className="px-3 py-2">
              <History size={12} className="mr-1 inline text-ink-3" aria-hidden />
              {when(r.startedAt, i18n.language)}
            </td>
            <td className="px-3 py-2">{r.jobName ? `${t(`backup.trigger.${r.trigger}`)} · ${r.jobName}` : t(`backup.trigger.${r.trigger}`)}</td>
            <td className={cx("px-3 py-2", r.ok ? "text-ok" : "text-err")}>
              {r.ok ? t("backup.runOk", { count: r.items }) : t("backup.runFailed")}
              {r.deleted.length > 0 && <span className="ml-1 text-ink-3">{t("backup.runDeleted", { count: r.deleted.length })}</span>}
              {r.error && <div className="text-[11.5px] text-ink-3 [overflow-wrap:anywhere]">{r.error}</div>}
            </td>
            <td className="px-3 py-2">{r.ok ? formatBytes(r.size, i18n.language) : ""}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

