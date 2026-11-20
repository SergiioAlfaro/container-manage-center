// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * Comandos masivos (1.11.0): elegir contenedores, escribir el comando (un
 * ejecutable con argumentos o una secuencia para un intérprete elegido),
 * usuario, directorio, tiempo máximo y concurrencia, ver la vista previa que
 * calcula el backend, confirmar y seguir los resultados. La interfaz nunca
 * habla con Docker: todo pasa por `bulk_exec_preview` y `bulk_exec_open`.
 */
import { useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Play, Save, Search, Trash2, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApp } from "@/app/store";
import { shortId } from "@/shared/format";
import type { BulkEnvironment } from "@/shared/ipc/bindings/BulkEnvironment";
import type { BulkExecRequest } from "@/shared/ipc/bindings/BulkExecRequest";
import type { ExecPreview } from "@/shared/ipc/bindings/ExecPreview";
import { api } from "@/shared/ipc/client";
import { Button, cx, Segmented } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { STATE_DOT } from "@/features/containers/containerUi";
import { ExecConfirmDialog, RiskBadge } from "./ExecDialogs";
import { ExecPanel } from "./ExecPanel";
import {
  CUSTOM,
  EMPTY_FORM,
  EXAMPLES,
  INTERPRETERS,
  MAX_TIMEOUT_SECS,
  commandOf,
  formFromTemplate,
  formReady,
  newTemplateId,
  splitTargets,
  templateFromForm,
  type ExecForm,
} from "./execModel";
import { useBulkExec } from "./execStore";
import { envLabel, matches, EMPTY_FILTER, type BulkRow } from "./model";

const INPUT = "h-8 w-full rounded-md border border-line bg-surface px-2 text-[13px] outline-none placeholder:text-ink-3 hover:border-line-strong focus:border-beacon";
const MAX_PICKER = 100;


export function CommandsTab({
  rows,
  envs,
  selected,
  sel,
  setSel,
  onFinish,
}: {
  rows: BulkRow[];
  envs: BulkEnvironment[];
  selected: BulkRow[];
  sel: Set<string>;
  setSel: (f: (s: Set<string>) => Set<string>) => void;
  onFinish: () => void;
}) {
  const { t } = useTranslation();
  const localName = t("servers.local");
  const templates = useApp((s) => s.config.exec.templates);
  const defaultConcurrency = useApp((s) => s.config.bulk.concurrency);
  const phase = useBulkExec((s) => s.phase);
  const runExec = useBulkExec((s) => s.run);
  const execError = useBulkExec((s) => s.error);
  const busy = phase === "starting" || phase === "running";

  const [form, setForm] = useState<ExecForm>(EMPTY_FORM);
  const [concurrency, setConcurrency] = useState(defaultConcurrency);
  const [confirm, setConfirm] = useState(false);
  const [saveOpen, setSaveOpen] = useState(false);
  const [saveName, setSaveName] = useState("");
  const [saveDesc, setSaveDesc] = useState("");
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [query, setQuery] = useState("");
  const [env, setEnv] = useState("all");
  const [onlyRunning, setOnlyRunning] = useState(true);
  const patch = (p: Partial<ExecForm>) => setForm((f) => ({ ...f, ...p }));

  const command = useMemo(() => commandOf(form), [form]);
  const ready = formReady(form);
  // Vista previa del backend (argumentos exactos, comando equivalente y riesgo).
  const preview = useQuery<ExecPreview>({
    queryKey: ["bulkExecPreview", command],
    queryFn: () => api.bulkExecPreview(command),
    enabled: ready,
    retry: false,
    staleTime: Infinity,
  });

  const { run, skip } = splitTargets(selected);
  const tooMany = selected.length > 1000;
  const canRun = ready && preview.isSuccess && run.length > 0 && !busy && !tooMany;

  // Selector: buscar y filtrar para añadir a la selección (la misma que en «Contenedores»).
  const picker = useMemo(
    () => rows.filter((r) => matches(r, { ...EMPTY_FILTER, query, env, state: onlyRunning ? "running" : "all" })),
    [rows, query, env, onlyRunning],
  );
  const toggle = (key: string) =>
    setSel((s) => {
      const next = new Set(s);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });

  useEffect(() => setSaveError(null), [form, saveName]);

  const loadTemplate = (value: string) => {
    if (value.startsWith("example:")) {
      const ex = EXAMPLES.find((e) => `example:${e.key}` === value);
      if (ex) setForm(formFromTemplate(ex.template));
    } else {
      const tp = templates.find((x) => x.id === value);
      if (tp) setForm(formFromTemplate(tp));
    }
  };

  const save = async () => {
    setSaving(true);
    setSaveError(null);
    try {
      const cfg = await api.execTemplatesSave([...templates, templateFromForm(form, newTemplateId(), saveName, saveDesc)]);
      useApp.setState({ config: cfg });
      setSaveOpen(false);
      setSaveName("");
      setSaveDesc("");
    } catch (e) {
      setSaveError((e as { message: string }).message);
    } finally {
      setSaving(false);
    }
  };

  const remove = async (id: string) => {
    try {
      const cfg = await api.execTemplatesSave(templates.filter((x) => x.id !== id));
      useApp.setState({ config: cfg });
    } catch (e) {
      setSaveError((e as { message: string }).message);
    }
  };

  const start = async (req: BulkExecRequest) => {
    setConfirm(false);
    await runExec(
      req,
      {
        commandText: preview.data?.equivalent ?? "",
        rows: run.map((r) => ({ envName: r.envName, image: r.c.image })),
      },
      onFinish,
    );
  };

  return (
    <div className="space-y-4" data-testid="bulk-commands">
      <p className="max-w-[820px] text-[13px] text-ink-2">{t("bulk.exec.intro")}</p>
      {execError && (
        <Banner tone="error" onDismiss={() => useBulkExec.setState({ error: null })} dismissLabel={t("containers.dismiss")}>
          {t("bulk.exec.runError", { message: execError })}
        </Banner>
      )}

      <div className="grid gap-4 xl:grid-cols-[minmax(0,5fr)_minmax(0,7fr)]">
        {/* Contenedores */}
        <section className="rounded-xl border border-line bg-surface p-4" aria-label={t("bulk.exec.targets")}>
          <h2 className="mb-2 text-[14px] font-semibold">{t("bulk.exec.targets")}</h2>
          <div className="mb-2 flex items-center gap-2 text-[12.5px]">
            <span data-testid="exec-selected-count">{t("bulk.exec.selectedCount", { count: selected.length, running: run.length })}</span>
            {selected.length > 0 && (
              <Button size="sm" variant="ghost" className="ml-auto" onClick={() => setSel(() => new Set())}>
                {t("bulk.bar.clear")}
              </Button>
            )}
          </div>
          {selected.length === 0 ? (
            <p className="rounded-md border border-dashed border-line px-3 py-4 text-center text-[12.5px] text-ink-3">{t("bulk.exec.noneSelected")}</p>
          ) : (
            <ul className="max-h-[220px] divide-y divide-line overflow-y-auto rounded-md border border-line" data-testid="exec-selected">
              {selected.map((r) => (
                <li key={r.key} className="flex items-center gap-2 px-3 py-1.5 text-[12.5px]">
                  <span aria-hidden className={cx("h-2 w-2 shrink-0 rounded-full", STATE_DOT[r.c.state] ?? "bg-off")} />
                  <div className="min-w-0 flex-1">
                    <div className="flex min-w-0 items-center gap-2">
                      <span className="min-w-0 truncate font-medium">{r.c.name}</span>
                      <span className="shrink-0 font-mono text-[11px] text-ink-3">{shortId(r.c.id)}</span>
                      <span className="shrink-0 rounded bg-surface-sunk px-1.5 text-[11px] text-ink-2">{r.envName}</span>
                    </div>
                    {r.c.state !== "running" && (
                      <div className="text-[11.5px] text-warn">{t("bulk.exec.willNotRun", { state: t(`containers.state.${r.c.state}`, { defaultValue: r.c.state }) })}</div>
                    )}
                  </div>
                  <button type="button" onClick={() => toggle(r.key)} className="shrink-0 text-ink-3 hover:text-ink" aria-label={t("bulk.exec.removeTarget", { name: r.c.name })} title={t("bulk.exec.removeTarget", { name: r.c.name })}>
                    <X size={14} />
                  </button>
                </li>
              ))}
            </ul>
          )}

          <h3 className="mt-4 mb-2 text-[12.5px] font-medium">{t("bulk.exec.addTargets")}</h3>
          <div className="mb-2 flex flex-wrap items-center gap-2">
            <label className="relative min-w-[160px] flex-1">
              <span className="sr-only">{t("bulk.exec.pickerSearch")}</span>
              <Search size={14} className="pointer-events-none absolute top-1/2 left-2 -translate-y-1/2 text-ink-3" />
              <input type="search" value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("bulk.search")} aria-label={t("bulk.exec.pickerSearch")} className={cx(INPUT, "pl-7")} />
            </label>
            <select aria-label={t("bulk.exec.pickerEnv")} value={env} onChange={(e) => setEnv(e.target.value)} className="h-8 rounded-md border border-line bg-surface px-2 text-[12.5px]">
              <option value="all">
                {t("bulk.filter.env")}: {t("bulk.filter.all")}
              </option>
              {envs.map((e) => (
                <option key={e.server} value={e.server}>
                  {envLabel(e, localName)}
                </option>
              ))}
            </select>
            <label className="flex items-center gap-1.5 text-[12.5px]">
              <input type="checkbox" checked={onlyRunning} onChange={(e) => setOnlyRunning(e.target.checked)} className="h-4 w-4 accent-[var(--beacon)]" />
              {t("bulk.exec.onlyRunning")}
            </label>
          </div>
          <ul className="max-h-[220px] divide-y divide-line overflow-y-auto rounded-md border border-line" data-testid="exec-picker" aria-label={t("bulk.exec.addTargets")}>
            {picker.slice(0, MAX_PICKER).map((r) => (
              <li key={r.key}>
                <label className="flex cursor-pointer items-center gap-2 px-3 py-1.5 text-[12.5px] hover:bg-surface-2">
                  <input type="checkbox" checked={sel.has(r.key)} onChange={() => toggle(r.key)} className="h-4 w-4 accent-[var(--beacon)]" aria-label={t("bulk.selectOne", { name: r.c.name, env: r.envName })} />
                  <span className="min-w-0 truncate">{r.c.name}</span>
                  <span className="min-w-0 truncate font-mono text-[11px] text-ink-3">{r.c.image}</span>
                  <span className="ml-auto shrink-0 rounded bg-surface-sunk px-1.5 text-[11px] text-ink-2">{r.envName}</span>
                </label>
              </li>
            ))}
            {picker.length === 0 && <li className="px-3 py-3 text-center text-[12.5px] text-ink-3">{t("bulk.noMatches")}</li>}
          </ul>
          {picker.length > MAX_PICKER && <p className="mt-1 text-[12px] text-ink-3">{t("bulk.exec.pickerMore", { count: picker.length - MAX_PICKER })}</p>}
        </section>

        {/* Comando */}
        <section className="space-y-3 rounded-xl border border-line bg-surface p-4" aria-label={t("bulk.exec.command")}>
          <div className="flex flex-wrap items-center gap-2">
            <h2 className="text-[14px] font-semibold">{t("bulk.exec.command")}</h2>
            <select
              aria-label={t("bulk.exec.templates")}
              value=""
              onChange={(e) => loadTemplate(e.target.value)}
              className="ml-auto h-8 max-w-[260px] rounded-md border border-line bg-surface px-2 text-[12.5px]"
            >
              <option value="">{t("bulk.exec.loadTemplate")}</option>
              {templates.length > 0 && (
                <optgroup label={t("bulk.exec.saved")}>
                  {templates.map((x) => (
                    <option key={x.id} value={x.id}>
                      {x.name}
                    </option>
                  ))}
                </optgroup>
              )}
              <optgroup label={t("bulk.exec.examples.title")}>
                {EXAMPLES.map((e) => (
                  <option key={e.key} value={`example:${e.key}`}>
                    {t(`bulk.exec.examples.${e.key}`)}
                  </option>
                ))}
              </optgroup>
            </select>
          </div>

          <Segmented<ExecForm["mode"]>
            label={t("bulk.exec.mode")}
            value={form.mode}
            onChange={(mode) => patch({ mode })}
            options={[
              { value: "direct", label: t("bulk.exec.modeDirect") },
              { value: "script", label: t("bulk.exec.modeScript") },
            ]}
          />
          <p className="text-[12px] text-ink-3">{t(form.mode === "direct" ? "bulk.exec.directHint" : "bulk.exec.scriptHint")}</p>

          {form.mode === "direct" ? (
            <div className="grid gap-3">
              <label className="block text-[12.5px]">
                {t("bulk.exec.program")}
                <input value={form.program} onChange={(e) => patch({ program: e.target.value })} placeholder="/usr/bin/df" className={cx(INPUT, "mt-1 font-mono")} spellCheck={false} />
              </label>
              <label className="block text-[12.5px]">
                {t("bulk.exec.args")}
                <textarea
                  value={form.argsText}
                  onChange={(e) => patch({ argsText: e.target.value })}
                  rows={3}
                  placeholder={"-h\n/var"}
                  spellCheck={false}
                  className="mt-1 w-full rounded-md border border-line bg-surface px-2 py-1.5 font-mono text-[12.5px] outline-none focus:border-beacon"
                />
                <span className="text-[11.5px] text-ink-3">{t("bulk.exec.argsHint")}</span>
              </label>
            </div>
          ) : (
            <div className="grid gap-3">
              <div className="flex flex-wrap items-end gap-2">
                <label className="block text-[12.5px]">
                  {t("bulk.exec.interpreter")}
                  <select value={form.interpreter} onChange={(e) => patch({ interpreter: e.target.value })} className="mt-1 block h-8 rounded-md border border-line bg-surface px-2 font-mono text-[12.5px]">
                    {INTERPRETERS.map((i) => (
                      <option key={i} value={i}>
                        {i}
                      </option>
                    ))}
                    <option value={CUSTOM}>{t("bulk.exec.otherInterpreter")}</option>
                  </select>
                </label>
                {form.interpreter === CUSTOM && (
                  <label className="block min-w-[180px] flex-1 text-[12.5px]">
                    {t("bulk.exec.customInterpreter")}
                    <input value={form.customInterpreter} onChange={(e) => patch({ customInterpreter: e.target.value })} placeholder="/usr/bin/perl" className={cx(INPUT, "mt-1 font-mono")} spellCheck={false} />
                  </label>
                )}
                <label className="block w-[90px] text-[12.5px]">
                  {t("bulk.exec.flag")}
                  <input value={form.flag} onChange={(e) => patch({ flag: e.target.value })} className={cx(INPUT, "mt-1 font-mono")} spellCheck={false} />
                </label>
              </div>
              <label className="block text-[12.5px]">
                {t("bulk.exec.script")}
                <textarea
                  value={form.script}
                  onChange={(e) => patch({ script: e.target.value })}
                  rows={6}
                  spellCheck={false}
                  placeholder={"set -e\ndf -h\ncat /etc/os-release"}
                  className="mt-1 w-full rounded-md border border-line bg-surface px-2 py-1.5 font-mono text-[12.5px] outline-none focus:border-beacon"
                />
              </label>
              <p className="text-[12px] text-ink-3">{t("bulk.exec.syntaxWarning")}</p>
            </div>
          )}

          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
            <label className="block text-[12.5px]">
              {t("bulk.exec.user")}
              <input value={form.user} onChange={(e) => patch({ user: e.target.value })} placeholder={t("bulk.exec.userPlaceholder")} className={cx(INPUT, "mt-1 font-mono")} spellCheck={false} />
            </label>
            <label className="block text-[12.5px]">
              {t("bulk.exec.workdir")}
              <input value={form.workdir} onChange={(e) => patch({ workdir: e.target.value })} placeholder={t("bulk.exec.workdirPlaceholder")} className={cx(INPUT, "mt-1 font-mono")} spellCheck={false} />
            </label>
            <label className="block text-[12.5px]">
              {t("bulk.exec.timeout")}
              <input
                type="number"
                min={1}
                max={MAX_TIMEOUT_SECS}
                value={form.timeoutSecs}
                onChange={(e) => patch({ timeoutSecs: Number(e.target.value) })}
                aria-invalid={!(form.timeoutSecs >= 1 && form.timeoutSecs <= MAX_TIMEOUT_SECS)}
                className={cx(INPUT, "mt-1 aria-[invalid=true]:border-err")}
              />
            </label>
            <label className="block text-[12.5px]">
              {t("bulk.exec.concurrency")}
              <select value={concurrency} onChange={(e) => setConcurrency(Number(e.target.value))} className="mt-1 h-8 w-full rounded-md border border-line bg-surface px-2 text-[12.5px]">
                {Array.from({ length: 16 }, (_, i) => i + 1).map((v) => (
                  <option key={v} value={v}>
                    {v}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <p className="text-[12px] text-ink-3">{t("bulk.exec.timeoutHint")}</p>

          {/* Vista previa */}
          <div className="rounded-lg border border-line bg-surface-2 p-3" data-testid="exec-preview" aria-live="polite">
            <h3 className="mb-1.5 text-[12.5px] font-medium">{t("bulk.exec.preview")}</h3>
            {!ready ? (
              <p className="text-[12.5px] text-ink-3">{t("bulk.exec.previewIncomplete")}</p>
            ) : preview.isPending ? (
              <p className="flex items-center gap-2 text-[12.5px] text-ink-3">
                <Spinner size={13} /> {t("common.loading")}
              </p>
            ) : preview.error ? (
              <p className="text-[12.5px] text-err">{(preview.error as unknown as { message: string }).message}</p>
            ) : preview.data ? (
              <PreviewBody p={preview.data} />
            ) : null}
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <Button variant="primary" disabled={!canRun} onClick={() => setConfirm(true)} data-testid="exec-start">
              <Play size={14} /> {t("bulk.exec.run", { count: run.length })}
            </Button>
            <Button onClick={() => setSaveOpen((v) => !v)} disabled={!ready || !preview.isSuccess}>
              <Save size={14} /> {t("bulk.exec.saveTemplate")}
            </Button>
            {busy && <span className="text-[12px] text-ink-3">{t("bulk.exec.busy")}</span>}
            {!busy && selected.length > 0 && run.length === 0 && <span className="text-[12px] text-warn">{t("bulk.exec.noRunning")}</span>}
          </div>

          {saveOpen && (
            <div className="space-y-2 rounded-lg border border-line p-3" data-testid="exec-save">
              <label className="block text-[12.5px]">
                {t("bulk.exec.templateName")}
                <input value={saveName} onChange={(e) => setSaveName(e.target.value)} maxLength={80} className={cx(INPUT, "mt-1")} />
              </label>
              <label className="block text-[12.5px]">
                {t("bulk.exec.templateDescription")}
                <input value={saveDesc} onChange={(e) => setSaveDesc(e.target.value)} maxLength={500} className={cx(INPUT, "mt-1")} />
              </label>
              <p className="text-[12px] text-ink-3">{t("bulk.exec.templateNoSecrets")}</p>
              {saveError && <p className="text-[12.5px] text-err">{saveError}</p>}
              <div className="flex gap-2">
                <Button size="sm" variant="primary" disabled={!saveName.trim()} loading={saving} onClick={() => void save()}>
                  {t("bulk.exec.saveConfirm")}
                </Button>
                <Button size="sm" onClick={() => setSaveOpen(false)}>
                  {t("common.cancel")}
                </Button>
              </div>
            </div>
          )}

          {templates.length > 0 && (
            <details className="text-[12.5px]">
              <summary className="cursor-pointer text-ink-2">{t("bulk.exec.manageSaved", { count: templates.length })}</summary>
              <ul className="mt-2 divide-y divide-line rounded-md border border-line" data-testid="exec-saved">
                {templates.map((x) => (
                  <li key={x.id} className="flex items-center gap-2 px-3 py-1.5">
                    <div className="min-w-0 flex-1">
                      <div className="truncate font-medium">{x.name}</div>
                      {x.description && <div className="truncate text-[11.5px] text-ink-3">{x.description}</div>}
                    </div>
                    <Button size="sm" variant="ghost" onClick={() => setForm(formFromTemplate(x))}>
                      {t("bulk.exec.use")}
                    </Button>
                    <button type="button" onClick={() => void remove(x.id)} className="text-ink-3 hover:text-err" aria-label={t("bulk.exec.deleteTemplate", { name: x.name })} title={t("bulk.exec.deleteTemplate", { name: x.name })}>
                      <Trash2 size={14} />
                    </button>
                  </li>
                ))}
              </ul>
              {saveError && !saveOpen && <p className="mt-1 text-err">{saveError}</p>}
            </details>
          )}
        </section>
      </div>

      <ExecPanel />

      {confirm && preview.data && (
        <ExecConfirmDialog
          preview={preview.data}
          command={command}
          run={run}
          skip={skip}
          concurrency={concurrency}
          onClose={() => setConfirm(false)}
          onConfirm={(req) => void start(req)}
        />
      )}
    </div>
  );
}

function PreviewBody({ p }: { p: ExecPreview }) {
  const { t } = useTranslation();
  return (
    <div className="space-y-2 text-[12.5px]">
      <div>
        <div className="text-ink-3">{t("bulk.exec.equivalent")}</div>
        <code className="selectable block font-mono text-[12px] [overflow-wrap:anywhere] whitespace-pre-wrap">{p.equivalent}</code>
      </div>
      <div>
        <div className="text-ink-3">{t("bulk.exec.argv")}</div>
        <ol className="selectable list-inside list-decimal font-mono text-[12px]" data-testid="exec-argv">
          {p.argv.map((a, i) => (
            <li key={i} className="[overflow-wrap:anywhere] whitespace-pre-wrap">
              {i === 0 ? <strong>{a}</strong> : a}
            </li>
          ))}
        </ol>
      </div>
      <RiskBadge p={p} />
    </div>
  );
}
