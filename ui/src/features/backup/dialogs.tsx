import { useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { useApi } from "@/app/server";
import { api } from "@/shared/ipc/client";
import type { BackupEntry } from "@/shared/ipc/bindings/BackupEntry";
import type { BackupJob } from "@/shared/ipc/bindings/BackupJob";
import type { BackupJobView } from "@/shared/ipc/bindings/BackupJobView";
import type { BackupSchedule } from "@/shared/ipc/bindings/BackupSchedule";
import type { CommandError } from "@/shared/ipc/bindings/CommandError";
import type { RestoreOutcome } from "@/shared/ipc/bindings/RestoreOutcome";
import type { RestorePreview } from "@/shared/ipc/bindings/RestorePreview";
import { keys } from "@/shared/query";
import { Button } from "@/shared/ui/controls";
import { Banner, Checkbox, Dialog, Spinner } from "@/shared/ui/feedback";
import { allAccepted, HostRisks } from "@/shared/ui/HostRisks";
import { codeText, confirmWord, defaultTarget, errorText, itemKey, restoreItems } from "./model";

const field = "mt-1 h-8 w-full rounded-md border border-line bg-surface px-2 text-[13px] text-ink";

/** Confirmación con una acción que puede fallar (el error se muestra dentro). */
export function ConfirmDialog({ title, body, action, run, onDone, onClose }: { title: string; body: string; action: string; run: () => Promise<unknown>; onDone: () => void; onClose: () => void }) {
  const { t, i18n } = useTranslation();
  const m = useMutation({ mutationFn: run, onSuccess: onDone });
  return (
    <Dialog
      title={title}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="danger" loading={m.isPending} onClick={() => m.mutate()}>
            {action}
          </Button>
        </>
      }
    >
      <p className="text-[13px]">{body}</p>
      {m.error && <Banner tone="error">{errorText(t, (k) => i18n.exists(k), m.error as unknown as CommandError)}</Banner>}
    </Dialog>
  );
}

/** Copia «deteniendo»: qué contenedores se detienen y se vuelven a arrancar. */
export function StopConfirmDialog({ containers, onConfirm, onClose }: { containers: string[]; onConfirm: () => void; onClose: () => void }) {
  const { t } = useTranslation();
  return (
    <Dialog
      title={t("backup.stopTitle")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="danger" onClick={onConfirm}>
            {t("backup.stopAction")}
          </Button>
        </>
      }
    >
      <p className="mb-2 text-[13px]">{t("backup.stopBody")}</p>
      <ul className="list-disc pl-5 font-mono text-[12.5px]">
        {containers.map((c) => (
          <li key={c}>{c}</li>
        ))}
      </ul>
    </Dialog>
  );
}

const pad = (n: number) => String(n).padStart(2, "0");

/** Crear o cambiar una tarea: nombre, cuándo y cuántas copias conservar. */
export function JobDialog({ job, onClose, onSaved }: { job: BackupJob; onClose: () => void; onSaved: () => void }) {
  const { t, i18n } = useTranslation();
  const [j, setJ] = useState<BackupJob>(job);
  const s = j.schedule;
  const time = `${pad(s.hour)}:${pad(s.minute)}`;
  const setSchedule = (next: BackupSchedule) => setJ({ ...j, schedule: next });
  const setTime = (v: string) => {
    const [h, m] = v.split(":").map((x) => Number.parseInt(x, 10));
    if (Number.isFinite(h) && Number.isFinite(m)) setSchedule({ ...s, hour: h!, minute: m! });
  };
  const save = useMutation({ mutationFn: () => api.backupJobSave(j), onSuccess: onSaved });
  const num = (k: "daily" | "weekly" | "monthly", label: string) => (
    <label className="block text-[12.5px] font-medium text-ink-2">
      {label}
      <input
        type="number"
        min={0}
        max={k === "daily" ? 366 : k === "weekly" ? 520 : 240}
        className={field}
        value={j.retention[k]}
        onChange={(e) => setJ({ ...j, retention: { ...j.retention, [k]: Math.max(0, Number.parseInt(e.target.value || "0", 10)) } })}
      />
    </label>
  );
  return (
    <Dialog
      title={job.id ? t("backup.jobEdit") : t("backup.jobNew")}
      onClose={onClose}
      wide
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" loading={save.isPending} disabled={!j.name.trim()} onClick={() => save.mutate()}>
            {t("backup.jobSave")}
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <label className="block text-[12.5px] font-medium text-ink-2">
          {t("backup.jobName")}
          <input className={field} value={j.name} maxLength={80} onChange={(e) => setJ({ ...j, name: e.target.value })} />
        </label>
        <div className="text-[12px] text-ink-3">
          {t("backup.destination")}: <span className="font-mono">{j.destination}</span>
        </div>
        <div className="grid gap-3 md:grid-cols-3">
          <label className="block text-[12.5px] font-medium text-ink-2">
            {t("backup.schedule.label")}
            <select
              className={field}
              value={s.kind}
              onChange={(e) => {
                const k = e.target.value as BackupSchedule["kind"];
                setSchedule(k === "daily" ? { kind: k, hour: s.hour, minute: s.minute } : k === "weekly" ? { kind: k, weekday: 1, hour: s.hour, minute: s.minute } : { kind: k, day: 1, hour: s.hour, minute: s.minute });
              }}
            >
              <option value="daily">{t("backup.schedule.daily")}</option>
              <option value="weekly">{t("backup.schedule.weekly")}</option>
              <option value="monthly">{t("backup.schedule.monthly")}</option>
            </select>
          </label>
          {s.kind === "weekly" && (
            <label className="block text-[12.5px] font-medium text-ink-2">
              {t("backup.schedule.weekday")}
              <select className={field} value={s.weekday} onChange={(e) => setSchedule({ ...s, weekday: Number(e.target.value) })}>
                {(["mon", "tue", "wed", "thu", "fri", "sat", "sun"] as const).map((d, i) => (
                  <option key={d} value={i + 1}>
                    {t(`backup.weekday.${d}`)}
                  </option>
                ))}
              </select>
            </label>
          )}
          {s.kind === "monthly" && (
            <label className="block text-[12.5px] font-medium text-ink-2">
              {t("backup.schedule.day")}
              <input type="number" min={1} max={28} className={field} value={s.day} onChange={(e) => setSchedule({ ...s, day: Math.min(28, Math.max(1, Number(e.target.value) || 1)) })} />
            </label>
          )}
          <label className="block text-[12.5px] font-medium text-ink-2">
            {t("backup.schedule.time")}
            <input type="time" className={field} value={time} onChange={(e) => setTime(e.target.value)} />
          </label>
        </div>
        <fieldset>
          <legend className="mb-1 text-[12.5px] font-medium text-ink-2">{t("backup.retention")}</legend>
          <p className="mb-2 text-[12px] text-ink-3">{t("backup.retentionHint")}</p>
          <div className="grid gap-3 md:grid-cols-3">
            {num("daily", t("backup.retentionDaily"))}
            {num("weekly", t("backup.retentionWeekly"))}
            {num("monthly", t("backup.retentionMonthly"))}
          </div>
        </fieldset>
        {job.id && <p className="text-[12px] text-ink-3">{t("backup.jobEditTimer")}</p>}
        {save.error && <Banner tone="error">{errorText(t, (k) => i18n.exists(k), save.error as unknown as CommandError)}</Banner>}
      </div>
    </Dialog>
  );
}

/** Autorización expresa: se muestran los archivos de systemd y las órdenes
 *  exactas antes de crearlos (decisión 2 del propietario). */
export function TimerDialog({ view, onClose, onDone }: { view: BackupJobView; onClose: () => void; onDone: () => void }) {
  const { t, i18n } = useTranslation();
  const pv = useQuery({ queryKey: ["backupTimerPreview", view.job.id, view.job.schedule], queryFn: () => api.backupTimerPreview(view.job.id) });
  const [ok, setOk] = useState(false);
  const install = useMutation({ mutationFn: () => api.backupTimerInstall(view.job.id), onSuccess: onDone });
  return (
    <Dialog
      title={t("backup.timer.authTitle", { name: view.job.name })}
      onClose={onClose}
      wide
      target={false}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" disabled={!ok || !pv.data} loading={install.isPending} onClick={() => install.mutate()}>
            {t("backup.timer.authAction")}
          </Button>
        </>
      }
    >
      <p className="mb-3 text-[13px]">{t("backup.timer.authBody")}</p>
      {pv.isPending && <Spinner />}
      {pv.data && (
        <div className="space-y-3">
          {[
            [pv.data.servicePath, pv.data.serviceText],
            [pv.data.timerPath, pv.data.timerText],
          ].map(([path, text]) => (
            <div key={path}>
              <div className="mb-1 font-mono text-[12px] text-ink-2">{path}</div>
              <pre className="selectable max-h-48 overflow-auto rounded-md border border-line bg-surface-sunk p-2 text-[11.5px]">{text}</pre>
            </div>
          ))}
          <div>
            <div className="mb-1 text-[12px] font-medium text-ink-2">{t("backup.timer.commands")}</div>
            <pre className="selectable rounded-md border border-line bg-surface-sunk p-2 text-[11.5px]">{pv.data.commands.join("\n")}</pre>
          </div>
          <Checkbox checked={ok} onChange={setOk} label={t("backup.timer.authCheck")} />
        </div>
      )}
      {(pv.error || install.error) && <Banner tone="error">{errorText(t, (k) => i18n.exists(k), (pv.error ?? install.error) as unknown as CommandError)}</Banner>}
    </Dialog>
  );
}

/** Restauración completa o selectiva: elementos, destinos, vista previa,
 *  aviso de pérdida de datos y confirmación escrita. */
export function RestoreDialog({ entry, onClose }: { entry: BackupEntry; onClose: () => void }) {
  const { t, i18n } = useTranslation();
  const exists = (k: string) => i18n.exists(k);
  const bound = useApi();
  const items = entry.manifest?.items ?? [];
  const containers = useQuery({ queryKey: keys.containers, queryFn: bound.containersList });
  const [folder, setFolder] = useState<string | null>(null);
  const [selected, setSelected] = useState<Record<string, boolean>>(() => Object.fromEntries(items.map((i) => [itemKey(i), true])));
  const [targets, setTargets] = useState<Record<string, string>>(() => Object.fromEntries(items.map((i) => [itemKey(i), defaultTarget(i, null)])));
  const [preview, setPreview] = useState<RestorePreview | null>(null);
  const [accepted, setAccepted] = useState<string[]>([]);
  const [typed, setTyped] = useState("");
  const [outcome, setOutcome] = useState<RestoreOutcome | null>(null);
  const [running, setRunning] = useState(false);
  const [err, setErr] = useState<CommandError | null>(null);
  const reqItems = restoreItems(items, selected, targets);
  const needsFolder = items.some((i) => (i.kind === "compose" || i.kind === "appConfig") && selected[itemKey(i)]);
  const change = (f: () => void) => {
    f();
    setPreview(null);
    setTyped("");
  };
  const pick = useMutation({
    mutationFn: () => api.backupPickFolder(false),
    onSuccess: (p) => {
      if (!p) return;
      change(() => {
        setFolder(p);
        setTargets((ts) => ({ ...ts, ...Object.fromEntries(items.filter((i) => i.kind === "compose" || i.kind === "appConfig").map((i) => [itemKey(i), defaultTarget(i, p)])) }));
      });
    },
  });
  const doPreview = useMutation({
    mutationFn: () => bound.backupRestorePreview(entry.id, { items: reqItems, confirmed: false, acceptedRisks: [] }),
    onSuccess: setPreview,
    onError: (e) => setErr(e as unknown as CommandError),
  });
  const dataLoss = !!preview?.items.some((i) => i.dataLoss);
  const blocked = !!preview?.items.some((i) => i.blocked);
  const word = confirmWord(t);
  const ready = !!preview && preview.integrity.ok && !blocked && allAccepted(preview.risks, accepted) && (!dataLoss || typed.trim().toLowerCase() === word.toLowerCase());
  const go = async () => {
    setRunning(true);
    setErr(null);
    try {
      await bound.backupRestoreOpen(entry.id, { items: reqItems, confirmed: true, acceptedRisks: accepted }, (e) => {
        if (e.event !== "restoreEnd") return;
        setRunning(false);
        if (e.outcome) setOutcome(e.outcome);
        else setErr(e.error);
      });
    } catch (e) {
      setRunning(false);
      setErr(e as CommandError);
    }
  };
  const running_ = running;
  return (
    <Dialog
      title={t("backup.restore.title", { id: entry.id })}
      onClose={onClose}
      wide
      footer={
        outcome ? (
          <Button variant="primary" onClick={onClose}>
            {t("common.close")}
          </Button>
        ) : (
          <>
            <Button onClick={onClose}>{t("common.cancel")}</Button>
            {!preview ? (
              <Button variant="primary" disabled={reqItems.length === 0 || (needsFolder && !folder)} loading={doPreview.isPending} onClick={() => doPreview.mutate()}>
                {t("backup.restore.preview")}
              </Button>
            ) : (
              <Button variant={dataLoss ? "danger" : "primary"} disabled={!ready} loading={running_} onClick={() => void go()}>
                {t("backup.restore.run")}
              </Button>
            )}
          </>
        )
      }
    >
      {outcome ? (
        <ul className="space-y-1.5" aria-label={t("backup.restore.results")}>
          {outcome.items.map((r) => (
            <li key={itemKey(r)} className="text-[12.5px]">
              <span className={r.ok ? "text-ok" : "text-err"}>{r.ok ? "✓" : "✗"}</span> {t(`backup.kind.${r.kind}`)} <span className="font-mono">{r.name}</span> → <span className="font-mono">{r.target}</span>
              {r.error && <div className="ml-4 text-err">{codeText(t, exists, r.error)}</div>}
              {r.warnings.map((w) => (
                <div key={w} className="ml-4 text-warn">
                  {codeText(t, exists, w)}
                </div>
              ))}
            </li>
          ))}
        </ul>
      ) : (
        <div className="space-y-3">
          <p className="text-[12.5px] text-ink-2">{t("backup.restore.intro")}</p>
          <ul className="space-y-2">
            {items.map((i) => {
              const k = itemKey(i);
              const plan = preview?.items.find((p) => itemKey(p) === k);
              return (
                <li key={k} className="rounded-lg border border-line p-2.5">
                  <Checkbox checked={!!selected[k]} onChange={(v) => change(() => setSelected((s) => ({ ...s, [k]: v })))} label={`${t(`backup.kind.${i.kind}`)} · ${i.name}`} />
                  {selected[k] && (
                    <div className="mt-1.5 pl-6">
                      {i.kind === "database" ? (
                        <label className="block text-[12px] text-ink-2">
                          {t("backup.restore.targetDatabase")}
                          <select className={field} value={targets[k] ?? ""} onChange={(e) => change(() => setTargets((ts) => ({ ...ts, [k]: e.target.value })))}>
                            {(containers.data ?? [])
                              .filter((c) => c.state === "running")
                              .map((c) => (
                                <option key={c.id} value={c.name}>
                                  {c.name}
                                </option>
                              ))}
                          </select>
                        </label>
                      ) : i.kind === "compose" || i.kind === "appConfig" ? (
                        <div className="text-[12px] text-ink-2">
                          {t("backup.restore.targetFolder")}: <span className="font-mono">{targets[k] || "—"}</span>
                        </div>
                      ) : (
                        <label className="block text-[12px] text-ink-2">
                          {i.kind === "volume" ? t("backup.restore.targetVolume") : t("backup.restore.targetContainer")}
                          <input className={field} value={targets[k] ?? ""} onChange={(e) => change(() => setTargets((ts) => ({ ...ts, [k]: e.target.value })))} />
                        </label>
                      )}
                      {plan && (
                        <div className="mt-1 text-[12px]">
                          {plan.blocked ? <span className="text-err">{codeText(t, exists, plan.blocked)}</span> : <span className={plan.dataLoss ? "text-err" : "text-ok"}>{codeText(t, exists, plan.action)}</span>}
                          {plan.warnings.map((w) => (
                            <div key={w} className="text-warn">
                              {codeText(t, exists, w)}
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  )}
                </li>
              );
            })}
          </ul>
          {needsFolder && (
            <Button size="sm" onClick={() => pick.mutate()}>
              {folder ? t("backup.restore.changeFolder") : t("backup.restore.chooseFolder")}
            </Button>
          )}
          {preview && !preview.integrity.ok && <Banner tone="error">{t("backup.restore.integrityBad")}</Banner>}
          {preview && preview.integrity.ok && <p className="text-[12px] text-ok">{t("backup.restore.integrityOk", { count: preview.integrity.checked })}</p>}
          {preview && preview.risks.length > 0 && <HostRisks risks={preview.risks} accepted={accepted} onChange={setAccepted} />}
          {preview && dataLoss && (
            <div className="rounded-lg border border-err/30 bg-err/8 p-3">
              <p className="mb-2 text-[12.5px]">{t("backup.restore.dataLoss", { word })}</p>
              <input aria-label={t("backup.restore.typeToConfirm", { word })} className={field} value={typed} onChange={(e) => setTyped(e.target.value)} />
            </div>
          )}
          {err && <Banner tone="error">{errorText(t, exists, err)}</Banner>}
        </div>
      )}
    </Dialog>
  );
}
