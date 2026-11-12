import { useEffect, useMemo, useRef, useState } from "react";
import { useRemoteContext } from "@/features/servers/paths";
import { useApi } from "@/app/server";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Play, Plus } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { CommandError } from "@/shared/ipc/bindings/CommandError";
import type { ContainerSpec } from "@/shared/ipc/bindings/ContainerSpec";
import type { PortSpec } from "@/shared/ipc/bindings/PortSpec";
import type { RestartName } from "@/shared/ipc/bindings/RestartName";
import type { SpecIssue } from "@/shared/ipc/bindings/SpecIssue";
import type { SpecPreview } from "@/shared/ipc/bindings/SpecPreview";
import { keys } from "@/shared/query";
import { Button, CopyButton, Segmented, cx } from "@/shared/ui/controls";
import { HostRisks, allAccepted, stillAccepted } from "@/shared/ui/HostRisks";
import { Banner, Checkbox, Spinner } from "@/shared/ui/feedback";
import { joinArgs, splitArgs } from "../shellwords";
import { WIZARD_TABS, bytesFrom, emptySpec, isSecretEntry, issuesFor, tabOf, toMemField, type MemUnit, type WizardTab } from "../specModel";
import { useContainersNav, type Notice } from "../store";
import { Field, Group, IssueList, Select, TextInput, useIssueText } from "./fields";
import { KvEditor, MemField, MountsEditor, PortsEditor } from "./editors";

const RESTARTS: RestartName[] = ["no", "always", "unlessStopped", "onFailure"];

/** `nginx` → `nginx:latest`, como hace Docker al resolver la referencia. */
export function normalizeRef(image: string): string {
  const r = image.trim();
  if (r === "" || r.includes("@") || r.startsWith("sha256:")) return r;
  const last = r.slice(r.lastIndexOf("/") + 1);
  return last.includes(":") ? r : `${r}:latest`;
}

interface Texts {
  command: string;
  entrypoint: string;
  cpus: string;
  mem: { value: string; unit: MemUnit };
  memRes: { value: string; unit: MemUnit };
  capAdd: string;
  capDrop: string;
}

function textsOf(spec: ContainerSpec): Texts {
  return {
    command: joinArgs(spec.command),
    entrypoint: joinArgs(spec.entrypoint),
    cpus: spec.cpus === null ? "" : String(spec.cpus),
    mem: toMemField(spec.memoryBytes),
    memRes: toMemField(spec.memoryReservationBytes),
    capAdd: spec.capAdd.join(", "),
    capDrop: spec.capDrop.join(", "),
  };
}

const list = (s: string) =>
  s
    .split(/[,\s]+/)
    .map((x) => x.trim())
    .filter(Boolean);

const local = (field: string, code: string): SpecIssue => ({ severity: "error", field, code, params: {} });

/** Combina los campos de texto con la especificación y detecta errores de formato locales. */
export function effectiveSpec(spec: ContainerSpec, t: Texts): { spec: ContainerSpec; issues: SpecIssue[] } {
  const issues: SpecIssue[] = [];
  const out = { ...spec };
  for (const f of ["command", "entrypoint"] as const) {
    const r = splitArgs(t[f]);
    if (r.ok) out[f] = r.args;
    else issues.push(local(f, `argv.${r.error}`));
  }
  const cpus = t.cpus.trim().replace(",", ".");
  if (cpus === "") out.cpus = null;
  else if (Number.isFinite(Number(cpus))) out.cpus = Number(cpus);
  else {
    out.cpus = null;
    issues.push(local("cpus", "cpus.range"));
  }
  for (const [field, txt] of [
    ["memoryBytes", t.mem],
    ["memoryReservationBytes", t.memRes],
  ] as const) {
    const b = bytesFrom(txt.value, txt.unit);
    if (b !== null && Number.isNaN(b)) {
      out[field] = null;
      issues.push(local(field, "memory.invalid"));
    } else out[field] = b;
  }
  out.capAdd = list(t.capAdd).map((c) => c.toUpperCase());
  out.capDrop = list(t.capDrop).map((c) => c.toUpperCase());
  return { spec: out, issues };
}


export function CreateWizard({ initial, notes, sourceName }: { initial: ContainerSpec | null; notes: string[]; sourceName: string | null }) {
  const remoteCtx = useRemoteContext();
  const api = useApi();
  const { t } = useTranslation();
  const qc = useQueryClient();
  const nav = useContainersNav();
  const issueText = useIssueText();
  const [spec, setSpec] = useState<ContainerSpec>(() => {
    const s = initial ?? emptySpec();
    return sourceName && !s.name ? { ...s, name: `${sourceName}-copia` } : s;
  });
  const [texts, setTexts] = useState<Texts>(() => textsOf(initial ?? emptySpec()));
  const [tab, setTab] = useState<WizardTab>("general");
  const [preview, setPreview] = useState<SpecPreview | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [serverIssues, setServerIssues] = useState<SpecIssue[] | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [showDetails, setShowDetails] = useState<"run" | "create" | "body">("run");
  // Riesgos para el equipo aceptados uno a uno (E1, D-086).
  const [acceptedRisks, setAcceptedRisks] = useState<string[]>([]);

  const set = <K extends keyof ContainerSpec>(k: K, v: ContainerSpec[K]) => setSpec((s) => ({ ...s, [k]: v }));
  const setText = <K extends keyof Texts>(k: K, v: Texts[K]) => setTexts((x) => ({ ...x, [k]: v }));

  const eff = useMemo(() => effectiveSpec(spec, texts), [spec, texts]);

  // Vista previa: validación y traducción en el backend, sin hablar con Docker.
  const seq = useRef(0);
  useEffect(() => {
    const n = ++seq.current;
    const timer = window.setTimeout(() => {
      api
        .containerPreview(eff.spec)
        .then((p) => {
          if (n !== seq.current) return;
          setPreview(p);
          setPreviewError(null);
          setServerIssues(null);
        })
        .catch((e) => n === seq.current && setPreviewError((e as { message: string }).message));
    }, 200);
    return () => window.clearTimeout(timer);
  }, [eff.spec]);

  const issues = useMemo(() => [...eff.issues, ...(serverIssues ?? preview?.issues ?? [])], [eff.issues, serverIssues, preview]);
  const risks = preview?.risks ?? [];
  const kept = stillAccepted(risks, acceptedRisks);
  const errors = issues.filter((i) => i.severity === "error");
  const of = (field: string) => issuesFor(issues, field);

  // Datos reales para ayudar a rellenar: imágenes locales, redes y volúmenes.
  const images = useQuery({ queryKey: keys.images, queryFn: api.imagesList });
  const networks = useQuery({ queryKey: ["networks"], queryFn: api.networksBrief });
  const volumes = useQuery({ queryKey: ["volumes"], queryFn: api.volumesBrief });
  const tags = useMemo(() => (images.data ?? []).flatMap((i) => i.repoTags), [images.data]);
  const ref = normalizeRef(spec.image);
  const localImage = ref !== "" && (images.data ?? []).some((i) => i.repoTags.includes(ref) || i.id === ref || (ref.startsWith("sha256:") && i.id.startsWith(ref)));
  const defaults = useQuery({
    queryKey: ["imageDefaults", ref],
    queryFn: () => api.imageDefaults(ref),
    enabled: localImage,
    staleTime: Infinity,
  });
  const d = localImage ? defaults.data : undefined;

  const create = useMutation({
    mutationFn: ({ start }: { start: boolean }) => api.containerCreate({ ...eff.spec, acceptedRisks: kept }, start),
    onMutate: () => setFailure(null),
    onSuccess: (r) => {
      void qc.invalidateQueries({ queryKey: keys.containers });
      void qc.invalidateQueries({ queryKey: keys.overview });
      const notices: Notice[] = r.warnings.map((w) => ({ tone: "warn", text: t("create.dockerWarning", { message: w }) }));
      if (r.startError) notices.unshift({ tone: "error", text: t("create.startFailed", { message: r.startError.message }) });
      nav.showDetail(r.id, r.name, "summary", notices);
    },
    onError: (e) => {
      const err = e as unknown as CommandError;
      if (err.code === "engine.invalidSpec" && err.issues) {
        setServerIssues(err.issues);
        const first = err.issues.find((i) => i.severity === "error");
        if (first) setTab(tabOf(first.field));
      } else if (err.code === "engine.risksNotAccepted") setFailure(t("hostRisk.notAccepted"));
      else setFailure(err.message);
    },
  });

  const counts = (tb: WizardTab) => {
    const own = issues.filter((i) => tabOf(i.field) === tb);
    return { errors: own.filter((i) => i.severity === "error").length, warnings: own.filter((i) => i.severity === "warning").length };
  };
  // Los riesgos para el equipo se muestran aparte, con su aceptación.
  const listed = issues.filter((i) => !i.code.startsWith("risk."));
  const canCreate = errors.length === 0 && preview !== null && !create.isPending && allAccepted(risks, kept);

  return (
    <div className="flex min-h-full flex-col">
      <div className="flex items-center gap-3 border-b border-line px-8 py-3">
        <Button variant="ghost" size="sm" onClick={nav.showList}>
          <ArrowLeft size={15} />
          {t("detail.back")}
        </Button>
        <h2 className="text-[15px] font-semibold">{sourceName ? t("create.titleDuplicate", { name: sourceName }) : t("create.title")}</h2>
      </div>

      <div className="grid flex-1 gap-6 px-8 py-5 xl:grid-cols-[minmax(0,1fr)_400px]">
        <div className="min-w-0 space-y-5">
          {sourceName && notes.length > 0 && (
            <Banner tone="info">
              <p className="font-medium">{t("create.notesTitle")}</p>
              <ul className="mt-1 list-disc space-y-0.5 pl-5">
                {notes.map((n) => (
                  <li key={n}>{t(`dupNotes.${n}`, { defaultValue: n })}</li>
                ))}
              </ul>
            </Banner>
          )}
          <div role="tablist" aria-label={t("create.title")} className="flex flex-wrap gap-1 border-b border-line">
            {WIZARD_TABS.map((tb) => {
              const c = counts(tb);
              return (
                <button
                  key={tb}
                  type="button"
                  role="tab"
                  aria-selected={tab === tb}
                  onClick={() => setTab(tb)}
                  className={cx(
                    "-mb-px flex h-9 items-center gap-1.5 border-b-2 px-3 text-[13px]",
                    tab === tb ? "border-beacon font-medium text-ink" : "border-transparent text-ink-2 hover:text-ink",
                  )}
                >
                  {t(`create.tabs.${tb}`)}
                  {c.errors > 0 && <span className="rounded-full bg-err px-1.5 text-[11px] leading-[17px] font-semibold text-white tabular-nums">{c.errors}</span>}
                  {c.errors === 0 && c.warnings > 0 && <span aria-hidden className="h-2 w-2 rounded-full bg-warn" />}
                </button>
              );
            })}
          </div>

          <div role="tabpanel" className="space-y-6">
            {tab === "general" && (
              <>
                <div className="grid gap-4 md:grid-cols-2">
                  <Field label={t("create.f.name")} hint={t("create.h.name")} issues={of("name")}>
                    {(id, inv) => <TextInput id={id} invalid={inv} value={spec.name} onChange={(e) => set("name", e.target.value)} placeholder={t("create.ph.name")} />}
                  </Field>
                  <Field
                    label={t("create.f.image")}
                    issues={of("image")}
                    hint={
                      spec.image.trim() === "" ? t("create.h.image", { context: remoteCtx }) : localImage ? (defaults.isFetching ? t("create.h.imageReading") : t("create.h.imageLocal", { ref })) : null
                    }
                  >
                    {(id, inv) => (
                      <>
                        <TextInput id={id} invalid={inv} list="dcc-images" value={spec.image} onChange={(e) => set("image", e.target.value)} placeholder="nginx:1.27" />
                        <datalist id="dcc-images">
                          {tags.map((tg) => (
                            <option key={tg} value={tg} />
                          ))}
                        </datalist>
                      </>
                    )}
                  </Field>
                </div>
                {spec.image.trim() !== "" && !localImage && images.isSuccess && of("image").length === 0 && (
                  <Banner tone="warn">{t("create.imageNotLocal", { ref, context: remoteCtx })}</Banner>
                )}
                {defaults.error && <Banner tone="warn">{t("create.imageDefaultsError", { message: (defaults.error as { message: string }).message })}</Banner>}
                <Group title={t("create.g.process")} description={t("create.g.processHint")}>
                  <ArgvField label={t("create.f.command")} field="command" value={texts.command} onChange={(v) => setText("command", v)} placeholder={d ? joinArgs(d.command) : ""} args={eff.spec.command} issues={of("command")} />
                  <ArgvField label={t("create.f.entrypoint")} field="entrypoint" value={texts.entrypoint} onChange={(v) => setText("entrypoint", v)} placeholder={d ? joinArgs(d.entrypoint) : ""} args={eff.spec.entrypoint} issues={of("entrypoint")} hint={eff.spec.entrypoint.length > 1 ? t("create.h.entrypointMulti") : undefined} />
                  <Field label={t("create.f.workingDir")} issues={of("workingDir")}>
                    {(id, inv) => <TextInput id={id} invalid={inv} value={spec.workingDir} onChange={(e) => set("workingDir", e.target.value)} placeholder={d?.workingDir || "/"} />}
                  </Field>
                </Group>
              </>
            )}

            {tab === "env" && (
              <>
                <Group title={t("create.g.env")} description={t("create.g.envHint")}>
                  <KvEditor rows={spec.env} onChange={(v) => set("env", v)} field="env" issues={issues} keyPh="MODO" valuePh="produccion" secret addLabel={t("create.add.env")} empty={t("create.empty.env")} />
                  {d && d.env.length > 0 && (
                    <details className="rounded-md border border-line bg-surface-2 px-3 py-2 text-[12.5px]">
                      <summary className="cursor-pointer text-ink-2">{t("create.imageEnv", { count: d.env.length })}</summary>
                      <ul className="selectable mt-2 space-y-0.5 font-mono text-[12px] text-ink-2">
                        {d.env.map((e) => {
                          const i = e.indexOf("=");
                          const k = i < 0 ? e : e.slice(0, i);
                          return <li key={e}>{isSecretEntry(k, i < 0 ? "" : e.slice(i + 1)) ? `${k}=••••••` : e}</li>;
                        })}
                      </ul>
                    </details>
                  )}
                </Group>
                <Group title={t("create.g.labels")}>
                  <KvEditor rows={spec.labels} onChange={(v) => set("labels", v)} field="labels" issues={issues} keyPh="equipo" valuePh="web" addLabel={t("create.add.label")} empty={t("create.empty.labels")} />
                </Group>
              </>
            )}

            {tab === "network" && (
              <>
                <div className="grid gap-4 md:grid-cols-2">
                  <Field label={t("create.f.network")} issues={of("network")} hint={networks.error ? t("create.listError", { message: (networks.error as { message: string }).message }) : t("create.h.network")}>
                    {(id, inv) => (
                      <Select id={id} invalid={inv} value={spec.network} onChange={(e) => set("network", e.target.value)}>
                        <option value="">{t("create.defaultNetwork")}</option>
                        {(networks.data ?? []).map((n) => (
                          <option key={n.name} value={n.name}>
                            {n.name} ({n.driver})
                          </option>
                        ))}
                        {spec.network && !(networks.data ?? []).some((n) => n.name === spec.network) && <option value={spec.network}>{spec.network}</option>}
                      </Select>
                    )}
                  </Field>
                  <Field label={t("create.f.hostname")} issues={of("hostname")}>
                    {(id, inv) => <TextInput id={id} invalid={inv} value={spec.hostname} onChange={(e) => set("hostname", e.target.value)} placeholder={t("create.ph.hostname")} />}
                  </Field>
                </div>
                <Group
                  title={t("create.g.ports")}
                  description={t("create.g.portsHint", { context: remoteCtx })}
                  action={
                    d && d.exposedPorts.length > 0 ? (
                      <Button
                        size="sm"
                        onClick={() =>
                          set("ports", [
                            ...spec.ports,
                            ...d.exposedPorts
                              .map((p) => {
                                const [num, proto] = p.split("/");
                                return { hostIp: "", hostPort: null, containerPort: Number(num), protocol: (proto ?? "tcp") as PortSpec["protocol"] };
                              })
                              .filter((p) => !spec.ports.some((x) => x.containerPort === p.containerPort && x.protocol === p.protocol)),
                          ])
                        }
                      >
                        <Plus size={14} />
                        {t("create.addExposed", { ports: d.exposedPorts.join(", ") })}
                      </Button>
                    ) : undefined
                  }
                >
                  <IssueList issues={of("ports")} />
                  <PortsEditor ports={spec.ports} onChange={(v) => set("ports", v)} issues={issues} />
                  <Checkbox checked={spec.publishAll} onChange={(v) => set("publishAll", v)} label={t("create.f.publishAll")} hint={t("create.h.publishAll")} />
                </Group>
                <Group title={t("create.g.hosts")} description={t("create.g.hostsHint")}>
                  <KvEditor rows={spec.extraHosts} onChange={(v) => set("extraHosts", v)} field="extraHosts" issues={issues} keyPh="api.local" valuePh="10.0.0.5" addLabel={t("create.add.host")} empty={t("create.empty.hosts")} />
                </Group>
              </>
            )}

            {tab === "storage" && (
              <Group title={t("create.g.mounts")} description={t("create.g.mountsHint", { context: remoteCtx })}>
                {d && d.volumes.length > 0 && <Banner tone="info">{t("create.imageVolumes", { paths: d.volumes.join(", ") })}</Banner>}
                {volumes.error && <Banner tone="warn">{t("create.listError", { message: (volumes.error as { message: string }).message })}</Banner>}
                <datalist id="dcc-volumes">
                  {(volumes.data ?? []).map((v) => (
                    <option key={v.name} value={v.name} />
                  ))}
                </datalist>
                <MountsEditor mounts={spec.mounts} onChange={(v) => set("mounts", v)} issues={issues} />
              </Group>
            )}

            {tab === "resources" && (
              <>
                <Group title={t("create.g.limits")} description={t("create.g.limitsHint")}>
                  <div className="grid gap-4 md:grid-cols-3">
                    <Field label={t("create.f.cpus")} issues={of("cpus")} hint={t("create.h.cpus")}>
                      {(id, inv) => <TextInput id={id} invalid={inv} inputMode="decimal" value={texts.cpus} onChange={(e) => setText("cpus", e.target.value)} placeholder={t("create.ph.unlimited")} />}
                    </Field>
                    <MemField label={t("create.f.memory")} value={texts.mem} onChange={(v) => setText("mem", v)} issues={of("memoryBytes")} />
                    <MemField label={t("create.f.memoryReservation")} value={texts.memRes} onChange={(v) => setText("memRes", v)} issues={of("memoryReservationBytes")} />
                  </div>
                </Group>
                <Group title={t("create.g.restart")} description={t("create.g.restartHint")}>
                  <div className="grid gap-4 md:grid-cols-3">
                    <Field label={t("create.f.restart")}>
                      {(id) => (
                        <Select id={id} value={spec.restart.name} onChange={(e) => set("restart", { ...spec.restart, name: e.target.value as RestartName })}>
                          {RESTARTS.map((r) => (
                            <option key={r} value={r}>
                              {t(`restart.${r}`)}
                            </option>
                          ))}
                        </Select>
                      )}
                    </Field>
                    {spec.restart.name === "onFailure" && (
                      <Field label={t("create.f.maxRetries")} hint={t("create.h.maxRetries")}>
                        {(id) => (
                          <TextInput
                            id={id}
                            inputMode="numeric"
                            value={spec.restart.maxRetries ? String(spec.restart.maxRetries) : ""}
                            onChange={(e) => {
                              const n = Number(e.target.value);
                              set("restart", { ...spec.restart, maxRetries: Number.isInteger(n) && n >= 0 && n < 2 ** 31 ? n : spec.restart.maxRetries });
                            }}
                            placeholder="0"
                          />
                        )}
                      </Field>
                    )}
                  </div>
                </Group>
              </>
            )}

            {tab === "advanced" && (
              <>
                <div className="grid gap-4 md:grid-cols-2">
                  <Field label={t("create.f.user")} issues={of("user")} hint={t("create.h.user")}>
                    {(id, inv) => <TextInput id={id} invalid={inv} value={spec.user} onChange={(e) => set("user", e.target.value)} placeholder={d?.user || "root"} />}
                  </Field>
                </div>
                <Group title={t("create.g.options")}>
                  <div className="grid gap-3 md:grid-cols-2">
                    <Checkbox checked={spec.tty} onChange={(v) => set("tty", v)} label={t("create.f.tty")} hint="--tty" />
                    <Checkbox checked={spec.interactive} onChange={(v) => set("interactive", v)} label={t("create.f.interactive")} hint="--interactive" />
                    <Checkbox checked={spec.init} onChange={(v) => set("init", v)} label={t("create.f.init")} hint="--init" />
                    <Checkbox checked={spec.readOnlyRootfs} onChange={(v) => set("readOnlyRootfs", v)} label={t("create.f.readOnlyRootfs")} hint="--read-only" />
                    <div>
                      <Checkbox checked={spec.autoRemove} onChange={(v) => set("autoRemove", v)} label={t("create.f.autoRemove")} hint="--rm" />
                      <IssueList issues={of("autoRemove")} />
                    </div>
                    <div>
                      <Checkbox checked={spec.privileged} onChange={(v) => set("privileged", v)} label={t("create.f.privileged")} hint="--privileged" />
                      <IssueList issues={of("privileged")} />
                    </div>
                  </div>
                </Group>
                <Group title={t("create.g.caps")} description={t("create.g.capsHint")}>
                  <div className="grid gap-4 md:grid-cols-2">
                    <Field label={t("create.f.capAdd")} issues={of("capAdd")}>
                      {(id, inv) => <TextInput id={id} invalid={inv} value={texts.capAdd} onChange={(e) => setText("capAdd", e.target.value)} placeholder="NET_ADMIN, SYS_TIME" />}
                    </Field>
                    <Field label={t("create.f.capDrop")} issues={of("capDrop")}>
                      {(id, inv) => <TextInput id={id} invalid={inv} value={texts.capDrop} onChange={(e) => setText("capDrop", e.target.value)} placeholder="ALL" />}
                    </Field>
                  </div>
                </Group>
              </>
            )}
          </div>
        </div>

        <aside className="min-w-0 space-y-4 xl:sticky xl:top-5 xl:self-start">
          <div className="rounded-xl border border-line bg-surface">
            <div className="flex items-center gap-2 border-b border-line px-4 py-2.5">
              <h3 className="flex-1 text-[13px] font-semibold">{t("create.equivalent")}</h3>
              {preview && <CopyButton text={showDetails === "run" ? preview.runCommand : showDetails === "create" ? preview.createCommand : preview.body} />}
            </div>
            <div className="space-y-2 px-4 py-3">
              <Segmented<"run" | "create" | "body">
                label={t("create.equivalent")}
                value={showDetails}
                onChange={setShowDetails}
                options={[
                  { value: "run", label: "docker run" },
                  { value: "create", label: "docker create" },
                  { value: "body", label: t("create.apiBody") },
                ]}
              />
              {previewError ? (
                <Banner tone="error">{previewError}</Banner>
              ) : preview ? (
                <pre data-testid="cli-preview" className="selectable max-h-[340px] overflow-auto rounded-md bg-code p-2.5 font-mono text-[11.5px] leading-relaxed whitespace-pre-wrap [overflow-wrap:anywhere]">
                  {showDetails === "run" ? preview.runCommand : showDetails === "create" ? preview.createCommand : preview.body}
                </pre>
              ) : (
                <div className="flex items-center gap-2 py-3 text-[12.5px] text-ink-3">
                  <Spinner size={14} /> {t("create.previewing")}
                </div>
              )}
              <p className="text-[12px] text-ink-3">{t(showDetails === "body" ? "create.bodyHint" : "create.equivalentHint")}</p>
              {preview && preview.secretsHidden > 0 && <p className="text-[12px] text-ink-3">{t("create.secretsHidden", { count: preview.secretsHidden })}</p>}
            </div>
          </div>

          <div className="rounded-xl border border-line bg-surface px-4 py-3">
            <h3 className="text-[13px] font-semibold">{t("create.review")}</h3>
            {listed.length === 0 ? (
              <p className="mt-1 text-[12.5px] text-ok">{preview ? (risks.length === 0 ? t("create.noIssues") : t("hostRisk.seeBelow")) : t("create.previewing")}</p>
            ) : (
              <ul className="mt-1.5 space-y-1">
                {listed.map((i, n) => (
                  <li key={`${i.field}-${i.code}-${n}`}>
                    <button
                      type="button"
                      onClick={() => setTab(tabOf(i.field))}
                      className={cx("text-left text-[12.5px] hover:underline", i.severity === "error" ? "text-err" : "text-warn")}
                    >
                      <span className="text-ink-3">{t(`create.tabs.${tabOf(i.field)}`)} · </span>
                      {issueText(i)}
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>

          <HostRisks risks={risks} accepted={kept} onChange={setAcceptedRisks} />

          {failure && (
            <Banner tone="error" onDismiss={() => setFailure(null)} dismissLabel={t("common.dismiss")}>
              {t("create.failed", { message: failure })}
            </Banner>
          )}

          <div className="flex flex-wrap justify-end gap-2">
            <Button onClick={nav.showList}>{t("containers.cancel")}</Button>
            <Button disabled={!canCreate} onClick={() => create.mutate({ start: false })}>
              {create.isPending && create.variables?.start === false && <Spinner size={14} />}
              {t("create.create")}
            </Button>
            <Button variant="primary" disabled={!canCreate} onClick={() => create.mutate({ start: true })}>
              {create.isPending && create.variables?.start ? <Spinner size={14} /> : <Play size={14} />}
              {t("create.createStart")}
            </Button>
          </div>
          {errors.length > 0 && <p className="text-right text-[12px] text-err">{t("create.blocked", { count: errors.length })}</p>}
        </aside>
      </div>
    </div>
  );
}

function ArgvField({ label, field, value, onChange, placeholder, args, issues, hint }: { label: string; field: string; value: string; onChange: (v: string) => void; placeholder: string; args: string[]; issues: SpecIssue[]; hint?: string }) {
  const { t } = useTranslation();
  return (
    <Field label={label} issues={issues} hint={hint ?? (placeholder ? t("create.h.fromImage", { value: placeholder }) : t("create.h.argv"))}>
      {(id, inv) => (
        <>
          <TextInput id={id} invalid={inv} value={value} onChange={(e) => onChange(e.target.value)} placeholder={placeholder} data-field={field} />
          {args.length > 0 && issues.length === 0 && (
            <div className="mt-1.5 flex flex-wrap gap-1" aria-label={t("create.argsParsed")}>
              {args.map((a, i) => (
                <code key={i} className="rounded bg-surface-sunk px-1.5 py-px font-mono text-[11.5px] whitespace-pre text-ink-2">
                  {a === "" ? "''" : a}
                </code>
              ))}
            </div>
          )}
        </>
      )}
    </Field>
  );
}

