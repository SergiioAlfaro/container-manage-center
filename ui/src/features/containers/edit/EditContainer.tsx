/**
 * «Editar configuración» de un contenedor existente (0.9.2, D-070).
 *
 * - Lo que Docker cambia sobre el contenedor (nombre, reinicio, CPU y memoria)
 *   se aplica directamente; las redes se conectan y desconectan al momento.
 * - El resto (variables, puertos, montajes, descripción) exige recrearlo: el
 *   asistente compara, avisa de los riesgos y solo actúa con confirmación. El
 *   original se conserva como copia y, si el nuevo falla, se vuelve a él.
 */
import { useEffect, useMemo, useState } from "react";
import { useApi } from "@/app/server";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Archive, Link2, PencilLine, RotateCcw, Trash2, Unlink } from "lucide-react";
import { useTranslation } from "react-i18next";
import { ConnectDialog, DisconnectDialog } from "@/features/networks/dialogs";
import type { BackupRef } from "@/shared/ipc/bindings/BackupRef";
import type { EditableConfig } from "@/shared/ipc/bindings/EditableConfig";
import type { EditRequest } from "@/shared/ipc/bindings/EditRequest";
import type { RestartName } from "@/shared/ipc/bindings/RestartName";
import type { SpecIssue } from "@/shared/ipc/bindings/SpecIssue";
import { keys } from "@/shared/query";
import { formatAbsolute, formatBytes, shortId } from "@/shared/format";
import { Button, cx } from "@/shared/ui/controls";
import { Banner, Dialog, Spinner } from "@/shared/ui/feedback";
import { ICONS, STATE_DOT, allowedActions } from "../containerUi";
import { KvEditor, MemField, MountsEditor, PortsEditor } from "../create/editors";
import { Field, Group, IssueList, Select, TextInput } from "../create/fields";
import { isSecretEntry, issuesFor } from "../specModel";
import { useContainersNav } from "../store";
import type { AuditFix } from "@/shared/ipc/bindings/AuditFix";
import { applyAuditFixes, EDIT_TABS, pendingByTab, pendingChanges, tabOfIssue, textsOf, withTexts, type EditTab, type Texts } from "./model";
import { ReviewPanel } from "./ReviewPanel";
import { OutcomeBanner, editKey } from "./shared";

export { editKey };

const RESTARTS: RestartName[] = ["no", "always", "unlessStopped", "onFailure"];


export function EditContainer({ id, name, proposal }: { id: string; name: string; proposal?: AuditFix[] }) {
  const api = useApi();
  const { t } = useTranslation();
  const nav = useContainersNav();
  const q = useQuery({ queryKey: editKey(id), queryFn: () => api.containerEditConfig(id) });
  const cfg = q.data;
  // Tras recrear, este ID es el de la copia: la cabecera pasa a mostrar el contenedor nuevo.
  const [now, setNow] = useState<{ id: string; name: string; status: string } | null>(null);
  const shownName = now?.name ?? cfg?.current.name ?? name;
  const shownStatus = now?.status ?? cfg?.status;

  return (
    <div className="flex min-h-full flex-col">
      <div className="flex flex-wrap items-center gap-3 border-b border-line px-8 py-3">
        <Button variant="ghost" size="sm" onClick={() => nav.showDetail(now?.id ?? id, shownName)}>
          <ArrowLeft size={15} />
          {t("edit.back")}
        </Button>
        <div className="flex min-w-0 items-center gap-2">
          <PencilLine size={16} className="shrink-0 text-beacon-strong" aria-hidden />
          <h2 className="selectable truncate text-[15px] font-semibold">{t("edit.title", { name: shownName })}</h2>
          {shownStatus && (
            <span className="flex items-center gap-1.5 text-[13px] text-ink-2">
              <span aria-hidden className={cx("h-2 w-2 rounded-full", STATE_DOT[shownStatus] ?? "bg-off")} />
              {t(`containers.state.${shownStatus}`, { defaultValue: shownStatus })}
            </span>
          )}
        </div>
      </div>
      {q.isPending ? (
        <div className="flex items-center justify-center gap-2 py-20 text-[13px] text-ink-3">
          <Spinner /> {t("common.loading")}
        </div>
      ) : q.error || !cfg ? (
        <div className="px-8 py-6">
          <Banner tone="error">{t("edit.loadError", { message: (q.error as { message: string } | null)?.message ?? "" })}</Banner>
        </div>
      ) : (
        // Al recargar la configuración (p. ej. tras aplicar) se parte de nuevo de lo que dice Docker.
        <Editor key={`${cfg.id}-${q.dataUpdatedAt}`} cfg={cfg} onApplied={setNow} proposal={now ? undefined : proposal} />
      )}
    </div>
  );
}

function Editor({ cfg, onApplied, proposal }: { cfg: EditableConfig; onApplied: (now: { id: string; name: string; status: string }) => void; proposal?: AuditFix[] }) {
  const { t, i18n } = useTranslation();
  // E3: las correcciones propuestas por la auditoría se aplican al borrador;
  // nada se cambia hasta revisar y confirmar.
  const proposed = useMemo(() => (proposal && proposal.length > 0 ? applyAuditFixes(cfg.current, proposal) : null), [cfg, proposal]);
  const [tab, setTab] = useState<EditTab>(proposed?.tab ?? "general");
  const [draft, setDraft] = useState<EditRequest>(() => structuredClone(proposed?.request ?? cfg.current));
  const [texts, setTexts] = useState<Texts>(() => textsOf(cfg.current));
  const [reviewing, setReviewing] = useState(false);
  const { request, problems } = useMemo(() => withTexts(draft, texts), [draft, texts]);
  const pending = useMemo(() => pendingChanges(cfg.current, request), [cfg.current, request]);
  const byTab = pendingByTab(pending);
  const [serverIssues, setServerIssues] = useState<SpecIssue[]>([]);
  const set = <K extends keyof EditRequest>(k: K, v: EditRequest[K]) => setDraft((d) => ({ ...d, [k]: v }));
  const reset = () => {
    setDraft(structuredClone(cfg.current));
    setTexts(textsOf(cfg.current));
    setServerIssues([]);
  };
  const localIssues: SpecIssue[] = problems.map((p) => ({ severity: "error", field: p === "memory" ? "memoryBytes" : p === "memoryReservation" ? "memoryReservationBytes" : "cpus", code: "edit.numberInvalid", params: {} }));
  const issues = [...localIssues, ...serverIssues];
  const of = (f: string) => issuesFor(issues, f);
  const hasRecreate = pending.some((p) => p.kind === "recreate");

  if (reviewing) {
    return (
      <ReviewPanel
        cfg={cfg}
        request={request}
        onApplied={(out) => onApplied({ id: out.containerId, name: out.name, status: out.started ? "running" : out.containerId === cfg.id ? cfg.status : "created" })}
        onBack={(found) => {
          setReviewing(false);
          if (found && found.length > 0) {
            setServerIssues(found);
            setTab(tabOfIssue(found[0]!.field));
          }
        }}
      />
    );
  }

  return (
    <div className="flex flex-1 flex-col">
      <div role="tablist" aria-label={t("edit.sections")} className="flex flex-wrap gap-1 border-b border-line px-8">
        {EDIT_TABS.map((tb) => {
          const mark = byTab[tb];
          const hasIssue = issues.some((i) => i.severity === "error" && tabOfIssue(i.field) === tb);
          return (
            <button
              key={tb}
              type="button"
              role="tab"
              id={`edit-tab-${tb}`}
              aria-selected={tab === tb}
              aria-controls="edit-panel"
              onClick={() => setTab(tb)}
              className={cx("-mb-px flex h-9 items-center gap-1.5 border-b-2 px-3 text-[13px]", tab === tb ? "border-beacon font-medium text-ink" : "border-transparent text-ink-2 hover:text-ink")}
            >
              {t(`edit.tabs.${tb}`)}
              {hasIssue ? (
                <span className="h-1.5 w-1.5 rounded-full bg-err" title={t("edit.tabHasErrors")} aria-label={t("edit.tabHasErrors")} />
              ) : (
                mark && <span className={cx("h-1.5 w-1.5 rounded-full", mark === "recreate" ? "bg-warn" : "bg-beacon")} title={t(`edit.kind.${mark}`)} aria-label={t(`edit.kind.${mark}`)} />
              )}
            </button>
          );
        })}
      </div>

      <div id="edit-panel" role="tabpanel" aria-labelledby={`edit-tab-${tab}`} className="mx-auto w-full max-w-[920px] flex-1 space-y-6 px-8 py-6">
        {proposed && (
          <Banner tone="info">
            {t("audit.proposal.banner", { count: proposed.applied })}
            {proposed.resources && ` ${t("audit.proposal.resources")}`}
          </Banner>
        )}
        {tab === "general" && (
          <>
            <Group title={t("edit.g.identity")}>
              <div className="grid gap-4 md:grid-cols-2">
                <Field label={t("edit.f.name")} issues={of("name")} hint={<KindHint kind="direct" />}>
                  {(fid, inv) => <TextInput id={fid} invalid={inv} value={draft.name} onChange={(e) => set("name", e.target.value)} />}
                </Field>
                <Field label={t("edit.f.restart")} hint={<KindHint kind="direct" />}>
                  {(fid) => (
                    <Select id={fid} value={draft.restart.name} onChange={(e) => set("restart", { ...draft.restart, name: e.target.value as RestartName })}>
                      {RESTARTS.map((r) => (
                        <option key={r} value={r}>
                          {t(`restart.${r}`)}
                        </option>
                      ))}
                    </Select>
                  )}
                </Field>
                {draft.restart.name === "onFailure" && (
                  <Field label={t("create.f.maxRetries")} hint={t("create.h.maxRetries")}>
                    {(fid) => (
                      <TextInput
                        id={fid}
                        inputMode="numeric"
                        value={draft.restart.maxRetries ? String(draft.restart.maxRetries) : ""}
                        onChange={(e) => {
                          const n = Number(e.target.value);
                          set("restart", { ...draft.restart, maxRetries: Number.isInteger(n) && n >= 0 && n < 2 ** 31 ? n : draft.restart.maxRetries });
                        }}
                        placeholder="0"
                      />
                    )}
                  </Field>
                )}
              </div>
              <Field label={t("edit.f.description")} issues={of("description")} hint={<>{t("edit.h.description")} <KindHint kind="recreate" /></>}>
                {(fid, inv) => (
                  <textarea
                    id={fid}
                    rows={2}
                    maxLength={600}
                    value={draft.description}
                    onChange={(e) => set("description", e.target.value)}
                    className={cx("w-full rounded-md border bg-surface px-2.5 py-1.5 text-[13px] outline-none hover:border-line-strong focus:border-beacon", inv ? "border-err" : "border-line")}
                  />
                )}
              </Field>
            </Group>
            <Lifecycle cfg={cfg} />
            {cfg.backups.length > 0 && <Backups backups={cfg.backups} />}
          </>
        )}

        {tab === "network" && <NetworksTab cfg={cfg} />}

        {tab === "storage" && (
          <Group title={t("edit.g.mounts")} description={t("edit.h.mounts")}>
            <Banner tone="info">{t("edit.recreateNote.mounts")}</Banner>
            {cfg.anonymousVolumes.length > 0 && <p className="text-[12.5px] text-ink-3">{t("edit.anonymousVolumes", { names: cfg.anonymousVolumes.map(shortId).join(", ") })}</p>}
            <VolumesDatalist />
            <MountsEditor mounts={draft.mounts} onChange={(v) => set("mounts", v)} issues={issues} />
            {cfg.otherMounts.length > 0 && (
              <div className="rounded-md border border-line bg-surface-2 px-3 py-2 text-[12.5px]">
                <p className="text-ink-2">{t("edit.otherMounts")}</p>
                <ul className="selectable mt-1 font-mono text-[12px] text-ink-2">
                  {cfg.otherMounts.map((m) => (
                    <li key={m}>{m}</li>
                  ))}
                </ul>
              </div>
            )}
          </Group>
        )}

        {tab === "env" && (
          <Group title={t("edit.g.env")} description={t("edit.h.env")}>
            <Banner tone="warn">{t("edit.secretsWarning")}</Banner>
            <KvEditor rows={draft.env} onChange={(v) => set("env", v)} field="env" issues={issues} keyPh="MODO" valuePh="produccion" secret addLabel={t("create.add.env")} empty={t("create.empty.env")} />
            {cfg.imageEnv.length > 0 && (
              <details className="rounded-md border border-line bg-surface-2 px-3 py-2 text-[12.5px]">
                <summary className="cursor-pointer text-ink-2">{t("create.imageEnv", { count: cfg.imageEnv.length })}</summary>
                <p className="mt-1 text-ink-3">{t("edit.imageEnvHint")}</p>
                <ul className="selectable mt-2 space-y-0.5 font-mono text-[12px] text-ink-2">
                  {cfg.imageEnv.map((e) => (
                    <li key={e.key}>{isSecretEntry(e.key, e.value) ? `${e.key}=••••••` : `${e.key}=${e.value}`}</li>
                  ))}
                </ul>
              </details>
            )}
          </Group>
        )}

        {tab === "ports" && (
          <Group title={t("edit.g.ports")} description={t("edit.h.ports")}>
            {["host", "none"].includes(cfg.networkMode) || cfg.networkMode.startsWith("container:") ? (
              <Banner tone="info">{t("edit.portsNotApplicable", { mode: cfg.networkMode })}</Banner>
            ) : (
              <>
                <Banner tone="info">{t("edit.recreateNote.ports")}</Banner>
                <IssueList issues={of("ports")} />
                <PortsEditor ports={draft.ports} onChange={(v) => set("ports", v)} issues={issues} />
              </>
            )}
          </Group>
        )}

        {tab === "resources" && (
          <Group title={t("edit.g.resources")} description={t("edit.h.resources")}>
            <p className="text-[12.5px] text-ink-2">{t("edit.hostLimits", { cpus: cfg.hostCpus, memory: formatBytes(cfg.hostMemory, i18n.language) })}</p>
            <div className="grid gap-4 md:grid-cols-3">
              <Field label={t("create.f.cpus")} issues={of("cpus")} hint={t("edit.h.cpus", { max: cfg.hostCpus })}>
                {(fid, inv) => <TextInput id={fid} invalid={inv} inputMode="decimal" value={texts.cpus} onChange={(e) => setTexts({ ...texts, cpus: e.target.value })} placeholder={t("create.ph.unlimited")} />}
              </Field>
              <MemField label={t("create.f.memory")} value={texts.mem} onChange={(v) => setTexts({ ...texts, mem: v })} issues={of("memoryBytes")} />
              <MemField label={t("create.f.memoryReservation")} value={texts.memRes} onChange={(v) => setTexts({ ...texts, memRes: v })} issues={of("memoryReservationBytes")} />
            </div>
            <p className="text-[12.5px] text-ink-3">{t("edit.h.limitRemoval")}</p>
          </Group>
        )}

        {tab === "advanced" && <Advanced cfg={cfg} />}
      </div>

      {/* Barra de cambios pendientes: siempre visible al pie. */}
      <div className="sticky bottom-0 flex flex-wrap items-center gap-3 border-t border-line bg-surface px-8 py-3">
        <p className="min-w-0 flex-1 text-[13px] text-ink-2" aria-live="polite">
          {pending.length === 0
            ? t("edit.noChanges")
            : hasRecreate
              ? t("edit.pendingRecreate", { count: pending.length })
              : t("edit.pendingDirect", { count: pending.length })}
        </p>
        <Button onClick={reset} disabled={pending.length === 0}>
          <RotateCcw size={14} />
          {t("edit.discard")}
        </Button>
        <Button variant="primary" disabled={pending.length === 0 || problems.length > 0} title={problems.length > 0 ? t("edit.fixFirst") : undefined} onClick={() => setReviewing(true)}>
          {t("edit.review")}
        </Button>
      </div>
    </div>
  );
}

function KindHint({ kind }: { kind: "direct" | "recreate" }) {
  const { t } = useTranslation();
  return <span className={cx("rounded px-1 text-[11px] font-medium", kind === "direct" ? "bg-beacon-soft text-beacon-strong" : "bg-warn/15 text-warn")}>{t(`edit.kind.${kind}`)}</span>;
}

function VolumesDatalist() {
  const api = useApi();
  const volumes = useQuery({ queryKey: ["volumesBrief"], queryFn: api.volumesBrief });
  return (
    <datalist id="dcc-volumes">
      {(volumes.data ?? []).map((v) => (
        <option key={v.name} value={v.name} />
      ))}
    </datalist>
  );
}

function useRefreshContainer(id: string) {
  const qc = useQueryClient();
  return () => {
    void qc.invalidateQueries({ queryKey: editKey(id) });
    void qc.invalidateQueries({ queryKey: keys.containers });
    void qc.invalidateQueries({ queryKey: keys.overview });
    void qc.invalidateQueries({ queryKey: keys.networks });
  };
}

/** Estado y acciones de ciclo de vida: se aplican al momento. */
function Lifecycle({ cfg }: { cfg: EditableConfig }) {
  const api = useApi();
  const { t } = useTranslation();
  const refresh = useRefreshContainer(cfg.id);
  const m = useMutation({ mutationFn: (a: Parameters<typeof api.containerAction>[2]) => api.containerAction(cfg.id, cfg.current.name, a), onSettled: refresh });
  return (
    <Group title={t("edit.g.lifecycle")} description={t("edit.h.lifecycle")}>
      <div className="flex flex-wrap items-center gap-2">
        {allowedActions(cfg.status).map((a) => {
          const Icon = ICONS[a];
          return (
            <Button key={a} size="sm" loading={m.isPending && m.variables === a} disabled={m.isPending} onClick={() => m.mutate(a)}>
              {!(m.isPending && m.variables === a) && <Icon size={14} />}
              {t(`containers.action.${a}`)}
            </Button>
          );
        })}
      </div>
      {m.error && <Banner tone="error">{t("containers.failed", { action: t(`containers.action.${m.variables}`), name: cfg.current.name, message: (m.error as { message: string }).message })}</Banner>}
    </Group>
  );
}

/** Copias que dejaron las recreaciones anteriores: restaurar o eliminar. */
function Backups({ backups }: { backups: BackupRef[] }) {
  const api = useApi();
  const { t, i18n } = useTranslation();
  const qc = useQueryClient();
  const nav = useContainersNav();
  const [restoring, setRestoring] = useState<BackupRef | null>(null);
  const [removing, setRemoving] = useState<BackupRef | null>(null);
  const restore = useMutation({
    mutationFn: (b: BackupRef) => api.containerRestoreBackup(b.id, true),
    onSuccess: (out) => {
      void qc.invalidateQueries({ queryKey: keys.containers });
      if (!out.error) {
        setRestoring(null);
        nav.showEdit(out.containerId, out.name);
      }
    },
  });
  const remove = useMutation({
    // Solo el contenedor de la copia; sus volúmenes, nunca (los comparte con el actual).
    mutationFn: (b: BackupRef) => api.containerRemove(b.id, b.name, false, false),
    onSuccess: () => {
      setRemoving(null);
      void qc.invalidateQueries({ queryKey: ["containerEdit"] });
      void qc.invalidateQueries({ queryKey: keys.containers });
    },
  });
  return (
    <Group title={t("edit.g.backups")} description={t("edit.h.backups")}>
      <ul className="divide-y divide-line rounded-lg border border-line">
        {backups.map((b) => (
          <li key={b.id} className="flex flex-wrap items-center gap-3 px-3 py-2 text-[13px]">
            <Archive size={15} className="shrink-0 text-ink-3" aria-hidden />
            <span className="selectable min-w-0 flex-1 truncate font-mono text-[12.5px]" title={b.name}>
              {b.name}
            </span>
            <span className="text-[12px] text-ink-3">
              {formatAbsolute(b.created, i18n.language)} · {t(`containers.state.${b.state}`, { defaultValue: b.state })}
            </span>
            <Button size="sm" onClick={() => setRestoring(b)}>
              <RotateCcw size={14} />
              {t("edit.restore")}
            </Button>
            <Button size="sm" onClick={() => setRemoving(b)} className="hover:border-err/50 hover:text-err">
              <Trash2 size={14} />
              {t("edit.removeBackup")}
            </Button>
          </li>
        ))}
      </ul>
      {restoring && (
        <Dialog
          title={t("edit.restoreTitle", { name: restoring.name })}
          onClose={() => setRestoring(null)}
          footer={
            <>
              <Button onClick={() => setRestoring(null)}>{t("containers.cancel")}</Button>
              <Button variant="primary" loading={restore.isPending} onClick={() => restore.mutate(restoring)}>
                {t("edit.restore")}
              </Button>
            </>
          }
        >
          <p>{t("edit.restoreBody", { original: restoring.originalName })}</p>
          <p className="text-[12.5px] text-ink-3">{t("edit.restoreSafe")}</p>
          {restore.data?.error && <OutcomeBanner error={restore.data.error} rolledBack={restore.data.rolledBack} rollbackFailed={restore.data.rollbackFailed} />}
          {restore.error && <Banner tone="error">{(restore.error as { message: string }).message}</Banner>}
        </Dialog>
      )}
      {removing && (
        <Dialog
          title={t("edit.removeBackupTitle", { name: removing.name })}
          onClose={() => setRemoving(null)}
          footer={
            <>
              <Button onClick={() => setRemoving(null)}>{t("containers.cancel")}</Button>
              <Button variant="danger" loading={remove.isPending} onClick={() => remove.mutate(removing)}>
                {t("edit.removeBackup")}
              </Button>
            </>
          }
        >
          <p>{t("edit.removeBackupBody")}</p>
          {remove.error && <Banner tone="error">{(remove.error as { message: string }).message}</Banner>}
        </Dialog>
      )}
    </Group>
  );
}


/** Redes: conectar y desconectar se aplican al momento (operaciones directas). */
function NetworksTab({ cfg }: { cfg: EditableConfig }) {
  const api = useApi();
  const { t } = useTranslation();
  const refresh = useRefreshContainer(cfg.id);
  const nets = useQuery({ queryKey: keys.networks, queryFn: api.networksList });
  const [picked, setPicked] = useState("");
  const [connecting, setConnecting] = useState<string | null>(null);
  const [disconnecting, setDisconnecting] = useState<string | null>(null);
  const [done, setDone] = useState<string | null>(null);
  const special = ["host", "none"].includes(cfg.networkMode) || cfg.networkMode.startsWith("container:");
  const connected = new Set(cfg.networks.map((n) => n.name));
  const available = (nets.data ?? []).filter((n) => !connected.has(n.name) && !(n.predefined && (n.driver === "host" || n.driver === "null")));
  useEffect(() => {
    if (picked && !available.some((n) => n.id === picked)) setPicked("");
  }, [available, picked]);
  const connectNet = nets.data?.find((n) => n.id === connecting);
  const disconnectNet = cfg.networks.find((n) => n.name === disconnecting);

  return (
    <>
      <Group title={t("edit.g.networks")} description={t("edit.h.networks")}>
        {done && (
          <Banner tone="info" onDismiss={() => setDone(null)} dismissLabel={t("common.dismiss")}>
            {done}
          </Banner>
        )}
        {special && <Banner tone="info">{t("edit.networkModeSpecial", { mode: cfg.networkMode })}</Banner>}
        {cfg.networks.length === 0 ? (
          <p className="rounded-md border border-dashed border-line px-3 py-2.5 text-[12.5px] text-ink-3">{t("edit.noNetworks")}</p>
        ) : (
          <div className="overflow-x-auto rounded-lg border border-line">
            <table className="w-full min-w-[640px] text-left text-[12.5px]">
              <thead className="border-b border-line bg-surface-2 text-[12px] text-ink-3">
                <tr>
                  <th className="px-3 py-1.5 font-medium">{t("edit.net.name")}</th>
                  <th className="px-3 py-1.5 font-medium">{t("edit.net.ip")}</th>
                  <th className="px-3 py-1.5 font-medium">{t("edit.net.mac")}</th>
                  <th className="px-3 py-1.5 font-medium">{t("edit.net.aliases")}</th>
                  <th className="px-3 py-1.5" />
                </tr>
              </thead>
              <tbody className="divide-y divide-line">
                {cfg.networks.map((n) => (
                  <tr key={n.name}>
                    <td className="px-3 py-2 font-medium">{n.name}</td>
                    <td className="selectable px-3 py-2 font-mono text-[12px]">
                      {n.ipv4 || <span className="font-sans text-ink-3">{t("edit.net.noIp")}</span>}
                      {n.ipv6 && <div>{n.ipv6}</div>}
                      {n.fixedIpv4 && <div className="font-sans text-[11.5px] text-ink-3">{t("edit.net.fixed", { ip: n.fixedIpv4 })}</div>}
                    </td>
                    <td className="selectable px-3 py-2 font-mono text-[12px]">{n.mac || "—"}</td>
                    <td className="selectable px-3 py-2 font-mono text-[12px]">{n.aliases.filter((a) => a !== shortId(cfg.id)).join(", ") || "—"}</td>
                    <td className="px-3 py-2 text-right">
                      <Button size="sm" disabled={special} onClick={() => setDisconnecting(n.name)} className="hover:border-err/50 hover:text-err">
                        <Unlink size={14} />
                        {t("netOps.disconnect")}
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        <p className="text-[12px] text-ink-3">{t("edit.aliasesRestriction")}</p>
      </Group>
      {!special && (
        <Group title={t("edit.g.connect")} description={t("edit.h.connect")}>
          <div className="flex flex-wrap items-end gap-2">
            <div className="min-w-[240px] flex-1">
              <Field label={t("edit.f.network")} hint={nets.error ? t("create.listError", { message: (nets.error as { message: string }).message }) : undefined}>
                {(fid) => (
                  <Select id={fid} value={picked} onChange={(e) => setPicked(e.target.value)} disabled={nets.isPending}>
                    <option value="">{available.length ? t("edit.pickNetwork") : t("edit.noOtherNetworks")}</option>
                    {available.map((n) => (
                      <option key={n.id} value={n.id}>
                        {n.name} ({n.driver})
                      </option>
                    ))}
                  </Select>
                )}
              </Field>
            </div>
            <Button disabled={!picked} onClick={() => setConnecting(picked)}>
              <Link2 size={14} />
              {t("netOps.connect")}…
            </Button>
          </div>
        </Group>
      )}
      {connectNet && (
        <ConnectDialog
          network={connectNet}
          initialContainer={cfg.id}
          onClose={() => setConnecting(null)}
          onDone={(msg) => {
            setDone(msg);
            setPicked("");
            refresh();
          }}
        />
      )}
      {disconnectNet && (
        <DisconnectDialog
          network={{ id: disconnectNet.id || disconnectNet.name, name: disconnectNet.name }}
          member={{ id: cfg.id, name: cfg.current.name, state: cfg.status, ipv4: disconnectNet.ipv4 || null, ipv6: disconnectNet.ipv6 || null, mac: disconnectNet.mac || null }}
          lastNetwork={cfg.networks.length === 1}
          onClose={() => setDisconnecting(null)}
          onDone={(msg) => {
            setDone(msg);
            refresh();
          }}
        />
      )}
    </>
  );
}

/** Opciones que Docker no permite cambiar en un contenedor existente: solo lectura. */
function Advanced({ cfg }: { cfg: EditableConfig }) {
  const api = useApi();
  const { t, i18n } = useTranslation();
  const nav = useContainersNav();
  const yes = (b: boolean) => (b ? t("common.yes") : t("common.no"));
  const list = (v: string[]) => (v.length ? v.join(" ") : "—");
  const rows: Array<[string, string]> = [
    [t("edit.adv.image"), cfg.image],
    [t("edit.adv.imageId"), shortId(cfg.imageId)],
    [t("edit.adv.entrypoint"), list(cfg.entrypoint)],
    [t("edit.adv.command"), list(cfg.command)],
    [t("edit.adv.workingDir"), cfg.workingDir || "—"],
    [t("edit.adv.user"), cfg.user || "—"],
    [t("edit.adv.hostname"), cfg.hostname || "—"],
    [t("edit.adv.networkMode"), cfg.networkMode || "default"],
    [t("edit.adv.privileged"), yes(cfg.privileged)],
    [t("edit.adv.tty"), yes(cfg.tty)],
    [t("edit.adv.interactive"), yes(cfg.interactive)],
    [t("edit.adv.readOnlyRootfs"), yes(cfg.readOnlyRootfs)],
    [t("edit.adv.init"), yes(cfg.init)],
    [t("edit.adv.autoRemove"), yes(cfg.autoRemove)],
    [t("edit.adv.capAdd"), list(cfg.capAdd)],
    [t("edit.adv.capDrop"), list(cfg.capDrop)],
    [t("edit.adv.extraHosts"), list(cfg.extraHosts)],
  ];
  if (cfg.composeProject) rows.push([t("edit.adv.compose"), cfg.composeProject]);
  if (cfg.sizeRw !== null) rows.push([t("edit.adv.sizeRw"), formatBytes(cfg.sizeRw, i18n.language)]);
  return (
    <Group title={t("edit.g.advanced")} description={t("edit.h.advanced")}>
      <dl className="grid grid-cols-[minmax(160px,auto)_minmax(0,1fr)] gap-x-4 gap-y-1.5 rounded-lg border border-line px-4 py-3 text-[12.5px]">
        {rows.map(([k, v]) => (
          <div key={k} className="contents">
            <dt className="text-ink-3">{k}</dt>
            <dd className="selectable font-mono text-[12px] [overflow-wrap:anywhere]">{v}</dd>
          </div>
        ))}
      </dl>
      {cfg.labels.length > 0 && (
        <details className="rounded-md border border-line bg-surface-2 px-3 py-2 text-[12.5px]">
          <summary className="cursor-pointer text-ink-2">{t("edit.adv.labels", { count: cfg.labels.length })}</summary>
          <ul className="selectable mt-2 space-y-0.5 font-mono text-[12px] text-ink-2">
            {cfg.labels.map((l) => (
              <li key={l.key}>
                {l.key}={l.value}
              </li>
            ))}
          </ul>
        </details>
      )}
      <p className="text-[12.5px] text-ink-2">
        {t("edit.h.duplicateInstead")}{" "}
        <button
          type="button"
          className="font-medium text-beacon-strong hover:underline"
          onClick={async () => {
            const r = await api.containerDuplicateSpec(cfg.id, cfg.current.name);
            nav.showCreate({ spec: r.spec, notes: r.notes, sourceName: r.sourceName });
          }}
        >
          {t("detail.duplicate")}
        </button>
      </p>
    </Group>
  );
}
