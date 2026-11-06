import { useEffect, useMemo, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Box, CheckCircle2, CircleAlert, CircleSlash, FileCode2, FolderOpen, Hammer, Loader2, Square } from "lucide-react";
import { useTranslation } from "react-i18next";
import { formatBytes } from "@/shared/format";
import { api } from "@/shared/ipc/client";
import type { BuildPreview } from "@/shared/ipc/bindings/BuildPreview";
import type { BuildSpec } from "@/shared/ipc/bindings/BuildSpec";
import type { BuildStep } from "@/shared/ipc/bindings/BuildStep";
import type { Builder } from "@/shared/ipc/bindings/Builder";
import type { KeyValue } from "@/shared/ipc/bindings/KeyValue";
import { keys } from "@/shared/query";
import { Button, CopyButton, Segmented, Switch, cx } from "@/shared/ui/controls";
import { Banner, Checkbox, Spinner } from "@/shared/ui/feedback";
import { Field, Group, IssueList, RowList, TextInput, useIssueText } from "@/features/containers/create/fields";
import { isSecretKey, issuesFor } from "@/features/containers/specModel";
import { createContainerFrom } from "../createFrom";
import { useImagesNav } from "../store";
import { useBuild } from "./buildStore";

export function emptyBuildSpec(): BuildSpec {
  return { context: "", dockerfile: "Dockerfile", tags: [], buildArgs: [], labels: [], target: "", noCache: false, pull: false, builder: "buildKit" };
}

/** Ruta relativa al contexto, o `null` si el archivo está fuera. */
export function relativeTo(context: string, file: string): string | null {
  const base = context.replace(/\/+$/, "");
  if (!base || !file.startsWith(`${base}/`)) return null;
  return file.slice(base.length + 1);
}

function KvList({ rows, onChange, field, issues, addLabel, empty, keyPh, valuePh, secret }: { rows: KeyValue[]; onChange: (v: KeyValue[]) => void; field: string; issues: BuildPreview["issues"]; addLabel: string; empty: string; keyPh: string; valuePh: string; secret?: boolean }) {
  const { t } = useTranslation();
  return (
    <RowList<KeyValue>
      rows={rows}
      onChange={onChange}
      make={() => ({ key: "", value: "" })}
      addLabel={addLabel}
      removeLabel={(i) => t("create.remove.row", { n: i + 1 })}
      empty={empty}
      render={(kv, set, i) => (
        <div>
          <div className="grid grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)] gap-2">
            <TextInput aria-label={t("create.f.key", { n: i + 1 })} invalid={issuesFor(issues, `${field}.${i}.key`).some((x) => x.severity === "error")} value={kv.key} onChange={(e) => set({ ...kv, key: e.target.value })} placeholder={keyPh} />
            <TextInput aria-label={t("create.f.value", { n: i + 1 })} type={secret && isSecretKey(kv.key) ? "password" : "text"} value={kv.value} onChange={(e) => set({ ...kv, value: e.target.value })} placeholder={valuePh} />
          </div>
          <IssueList issues={[...issuesFor(issues, `${field}.${i}.key`), ...issuesFor(issues, `${field}.${i}.value`)]} />
        </div>
      )}
    />
  );
}

function BuildForm({ initial }: { initial: BuildSpec }) {
  const { t, i18n } = useTranslation();
  const qc = useQueryClient();
  const nav = useImagesNav();
  const start = useBuild((s) => s.start);
  const issueText = useIssueText();
  const [spec, setSpec] = useState<BuildSpec>(initial);
  const [preview, setPreview] = useState<BuildPreview | null>(null);
  const [pickError, setPickError] = useState<string | null>(null);
  const set = <K extends keyof BuildSpec>(k: K, v: BuildSpec[K]) => setSpec((s) => ({ ...s, [k]: v }));

  const seq = useRef(0);
  useEffect(() => {
    const n = ++seq.current;
    const timer = window.setTimeout(() => {
      api
        .buildPreview(spec)
        .then((p) => n === seq.current && setPreview(p))
        .catch(() => {});
    }, 300);
    return () => window.clearTimeout(timer);
  }, [spec]);

  const issues = preview?.issues ?? [];
  const of = (f: string) => issuesFor(issues, f);
  const errors = issues.filter((i) => i.severity === "error");
  const ctx = preview?.context;

  const pickContext = async () => {
    const dir = await api.pickFolder(t("build.pickContext"));
    if (dir) set("context", dir);
  };
  const pickDockerfile = async () => {
    setPickError(null);
    const f = await api.pickFile(t("build.pickDockerfile"), spec.context || undefined);
    if (!f) return;
    const rel = relativeTo(spec.context, f);
    if (rel) set("dockerfile", rel);
    else setPickError(t("build.outsideContext"));
  };

  // Al empezar, la página pasa a mostrar el progreso (y los errores de arranque).
  const submit = () =>
    start({ ...spec, tags: spec.tags.map((x) => x.trim()).filter(Boolean) }, (ok) => {
      if (ok) {
        void qc.invalidateQueries({ queryKey: keys.images });
        void qc.invalidateQueries({ queryKey: keys.overview });
      }
    });

  return (
    <div className="grid flex-1 gap-6 px-8 py-5 xl:grid-cols-[minmax(0,1fr)_400px]">
      <div className="min-w-0 space-y-6">
        <Group title={t("build.g.context")} description={t("build.g.contextHint")}>
          <Field label={t("build.f.context")} issues={of("context")}>
            {(id, inv) => (
              <div className="flex gap-1.5">
                <TextInput id={id} invalid={inv} value={spec.context} onChange={(e) => set("context", e.target.value)} placeholder="/home/usuario/proyecto" />
                <Button size="sm" className="h-8 shrink-0" onClick={pickContext} aria-label={t("build.pickContext")} title={t("build.pickContext")}>
                  <FolderOpen size={14} />
                </Button>
              </div>
            )}
          </Field>
          <Field label={t("build.f.dockerfile")} issues={of("dockerfile")} hint={pickError ?? t("build.h.dockerfile")}>
            {(id, inv) => (
              <div className="flex gap-1.5">
                <TextInput id={id} invalid={inv} value={spec.dockerfile} onChange={(e) => set("dockerfile", e.target.value)} placeholder="Dockerfile" />
                <Button size="sm" className="h-8 shrink-0" onClick={pickDockerfile} disabled={!spec.context} aria-label={t("build.pickDockerfile")} title={t("build.pickDockerfile")}>
                  <FileCode2 size={14} />
                </Button>
              </div>
            )}
          </Field>
        </Group>

        <Group title={t("build.g.tags")} description={t("build.g.tagsHint")}>
          <IssueList issues={of("tags")} />
          <RowList<string>
            rows={spec.tags}
            onChange={(v) => set("tags", v)}
            make={() => ""}
            addLabel={t("build.addTag")}
            removeLabel={(i) => t("build.removeTag", { n: i + 1 })}
            empty={t("build.noTags")}
            render={(tag, setTag, i) => (
              <div>
                <TextInput aria-label={t("build.tagN", { n: i + 1 })} invalid={of(`tags.${i}`).some((x) => x.severity === "error")} value={tag} onChange={(e) => setTag(e.target.value)} placeholder="miapp:1.0" />
                <IssueList issues={of(`tags.${i}`)} />
              </div>
            )}
          />
        </Group>

        <Group title={t("build.g.args")} description={t("build.g.argsHint")}>
          <KvList rows={spec.buildArgs} onChange={(v) => set("buildArgs", v)} field="buildArgs" issues={issues} addLabel={t("build.addArg")} empty={t("build.noArgs")} keyPh="VERSION" valuePh="1.0" secret />
        </Group>
        <Group title={t("build.g.labels")}>
          <KvList rows={spec.labels} onChange={(v) => set("labels", v)} field="labels" issues={issues} addLabel={t("create.add.label")} empty={t("create.empty.labels")} keyPh="org.opencontainers.image.version" valuePh="1.0" />
        </Group>

        <Group title={t("build.g.options")}>
          <div className="grid gap-4 md:grid-cols-2">
            <Field label={t("build.f.target")} issues={of("target")} hint={t("build.h.target")}>
              {(id, inv) => <TextInput id={id} invalid={inv} value={spec.target} onChange={(e) => set("target", e.target.value)} placeholder={t("build.ph.target")} />}
            </Field>
            <div>
              <p className="mb-1 text-[12.5px] font-medium text-ink-2">{t("build.f.builder")}</p>
              <Segmented<Builder>
                label={t("build.f.builder")}
                value={spec.builder}
                onChange={(v) => set("builder", v)}
                options={[
                  { value: "buildKit", label: "BuildKit" },
                  { value: "classic", label: t("build.classic") },
                ]}
              />
              <p className="mt-1 text-[12px] text-ink-3">{t(`build.h.${spec.builder}`)}</p>
            </div>
          </div>
          <div className="grid gap-3 md:grid-cols-2">
            <Checkbox checked={spec.noCache} onChange={(v) => set("noCache", v)} label={t("build.f.noCache")} hint="--no-cache" />
            <Checkbox checked={spec.pull} onChange={(v) => set("pull", v)} label={t("build.f.pull")} hint="--pull" />
          </div>
        </Group>
      </div>

      <aside className="min-w-0 space-y-4 xl:sticky xl:top-5 xl:self-start">
        <div className="rounded-xl border border-line bg-surface">
          <div className="flex items-center gap-2 border-b border-line px-4 py-2.5">
            <h3 className="flex-1 text-[13px] font-semibold">{t("create.equivalent")}</h3>
            {preview && <CopyButton text={preview.command} />}
          </div>
          <div className="space-y-2 px-4 py-3">
            {preview ? (
              <>
                <pre data-testid="build-command" className="selectable max-h-[220px] overflow-auto rounded-md bg-code p-2.5 font-mono text-[11.5px] leading-relaxed whitespace-pre-wrap [overflow-wrap:anywhere]">
                  {preview.command}
                </pre>
                <p className="selectable font-mono text-[11px] break-all text-ink-3">{preview.request}</p>
                {preview.secretsHidden > 0 && <p className="text-[12px] text-ink-3">{t("build.secretsHidden", { count: preview.secretsHidden })}</p>}
              </>
            ) : (
              <div className="flex items-center gap-2 py-3 text-[12.5px] text-ink-3">
                <Spinner size={14} /> {t("create.previewing")}
              </div>
            )}
          </div>
        </div>

        <div className="rounded-xl border border-line bg-surface px-4 py-3">
          <h3 className="text-[13px] font-semibold">{t("build.context")}</h3>
          {preview?.contextError && <Banner tone="error">{preview.contextError}</Banner>}
          {ctx ? (
            <div className="mt-1.5 space-y-1.5 text-[12.5px]" data-testid="context-summary">
              <p>{t("build.contextSummary", { files: ctx.files, size: formatBytes(ctx.bytes, i18n.language), dirs: ctx.dirs })}</p>
              <p className="text-ink-2">{ctx.dockerignore ? t("build.excluded", { count: Number(ctx.excluded) }) : t("build.noDockerignore")}</p>
              {ctx.ignorePatterns.length > 0 && (
                <details>
                  <summary className="cursor-pointer text-ink-3">{t("build.patterns", { count: ctx.ignorePatterns.length })}</summary>
                  <ul className="mt-1 font-mono text-[11.5px] text-ink-2">
                    {ctx.ignorePatterns.map((p) => (
                      <li key={p}>{p}</li>
                    ))}
                  </ul>
                </details>
              )}
              {ctx.invalidIgnoreLines.length > 0 && <p className="text-warn">{t("build.badIgnore", { lines: ctx.invalidIgnoreLines.join(", ") })}</p>}
              {ctx.skippedSpecial > 0 && <p className="text-ink-3">{t("build.special", { count: Number(ctx.skippedSpecial) })}</p>}
              {ctx.unreadable.length > 0 && <p className="text-err">{t("build.unreadable", { files: ctx.unreadable.join(", ") })}</p>}
              {ctx.bytes > 500 * 1024 ** 2 && <p className="text-warn">{t("build.bigContext")}</p>}
              {ctx.largest.length > 0 && (
                <details>
                  <summary className="cursor-pointer text-ink-3">{t("build.largest")}</summary>
                  <ul className="mt-1 space-y-0.5 text-[11.5px]">
                    {ctx.largest.map((f) => (
                      <li key={f.path} className="flex gap-2">
                        <span className="min-w-0 flex-1 truncate font-mono">{f.path}</span>
                        <span className="text-ink-3 tabular-nums">{formatBytes(f.size, i18n.language)}</span>
                      </li>
                    ))}
                  </ul>
                </details>
              )}
            </div>
          ) : (
            !preview?.contextError && <p className="mt-1 text-[12.5px] text-ink-3">{t("build.contextPending")}</p>
          )}
        </div>

        {issues.length > 0 && (
          <div className="rounded-xl border border-line bg-surface px-4 py-3">
            <h3 className="text-[13px] font-semibold">{t("create.review")}</h3>
            <ul className="mt-1.5 space-y-1">
              {issues.map((i, n) => (
                <li key={`${i.field}-${i.code}-${n}`} className={cx("text-[12.5px]", i.severity === "error" ? "text-err" : "text-warn")}>
                  {issueText(i)}
                </li>
              ))}
            </ul>
          </div>
        )}
        <div className="flex justify-end gap-2">
          <Button onClick={nav.showList}>{t("containers.cancel")}</Button>
          <Button variant="primary" disabled={!preview || errors.length > 0} onClick={() => void submit()}>
            <Hammer size={14} />
            {t("build.start")}
          </Button>
        </div>
      </aside>
    </div>
  );
}

function StepIcon({ s }: { s: BuildStep }) {
  if (s.state === "running") return <Loader2 size={14} className="animate-spin text-beacon" />;
  if (s.state === "error") return <CircleAlert size={14} className="text-err" />;
  if (s.state === "cached") return <CheckCircle2 size={14} className="text-ink-3" />;
  return <CheckCircle2 size={14} className="text-ok" />;
}

function BuildProgress() {
  const { t } = useTranslation();
  const nav = useImagesNav();
  const b = useBuild();
  const [follow, setFollow] = useState(true);
  const box = useRef<HTMLPreElement>(null);
  const text = useMemo(() => b.lines.join("\n"), [b.lines]);
  useEffect(() => {
    if (follow && box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [text, follow]);
  const active = b.status === "starting" || b.status === "running";
  const tags = b.result?.tags ?? b.spec?.tags ?? [];

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4 px-8 py-5">
      <div className="flex flex-wrap items-center gap-3">
        {active ? <Spinner size={16} /> : b.status === "done" ? <CheckCircle2 size={17} className="text-ok" /> : b.status === "cancelled" ? <CircleSlash size={17} className="text-ink-3" /> : <CircleAlert size={17} className="text-err" />}
        <p role="status" className="text-[13.5px] font-medium">
          {t(`build.status.${b.status}`)}
          {tags.length > 0 && <span className="ml-2 font-mono text-[12.5px] text-ink-2">{tags.join(", ")}</span>}
        </p>
        <div className="ml-auto flex flex-wrap gap-2">
          {active ? (
            <Button onClick={b.cancel} disabled={b.cancelRequested}>
              <Square size={13} />
              {t("build.cancel")}
            </Button>
          ) : (
            <>
              {b.status === "done" && b.result?.imageId && (
                <>
                  <Button variant="primary" onClick={() => nav.showDetail(b.result!.imageId!, tags[0] ?? b.result!.imageId!.slice(7, 19))}>
                    {t("build.viewImage")}
                  </Button>
                  {tags[0] && (
                    <Button onClick={() => createContainerFrom(tags[0]!)}>
                      <Box size={14} />
                      {t("imageDetail.createContainer")}
                    </Button>
                  )}
                </>
              )}
              <Button
                onClick={() => {
                  const spec = b.spec;
                  b.reset();
                  nav.showBuild(spec ?? undefined);
                }}
              >
                {t("build.edit")}
              </Button>
            </>
          )}
        </div>
      </div>
      {b.result?.end.summary && !active && <Banner tone={b.status === "done" ? "info" : "warn"}>{b.result.end.summary}</Banner>}
      {b.error && <Banner tone="error">{t("build.failed", { message: b.error.message })}</Banner>}

      <div className="grid min-h-0 flex-1 gap-4 lg:grid-cols-[300px_minmax(0,1fr)]">
        <ol className="max-h-[calc(100vh-280px)] space-y-1 overflow-auto rounded-xl border border-line bg-surface p-2" aria-label={t("build.steps")}>
          {b.steps.length === 0 && <li className="px-2 py-1.5 text-[12.5px] text-ink-3">{t("build.sending")}</li>}
          {b.steps.map((s) => (
            <li key={s.number} className={cx("flex items-start gap-2 rounded-md px-2 py-1.5 text-[12.5px]", s.state === "error" && "bg-err/8")}>
              <span className="mt-0.5">
                <StepIcon s={s} />
              </span>
              <span className="w-6 shrink-0 text-right font-mono text-[11.5px] text-ink-3">#{s.number}</span>
              <span className="min-w-0 flex-1 font-mono text-[11.5px] break-all">{s.name}</span>
              <span className="shrink-0 text-[11px] text-ink-3 tabular-nums">{s.state === "cached" ? t("build.cached") : s.durationMs !== null ? `${(s.durationMs / 1000).toFixed(1)} s` : ""}</span>
            </li>
          ))}
        </ol>
        <div className="flex min-h-0 flex-col gap-2">
          <div className="flex items-center gap-3">
            <span className="flex items-center gap-1.5 text-[12.5px] text-ink-2">
              <Switch checked={follow} onChange={setFollow} label={t("logs.follow")} />
              {t("logs.follow")}
            </span>
            {b.droppedLines > 0 && <span className="text-[12px] text-ink-3">{t("build.dropped", { count: b.droppedLines })}</span>}
            <span className="ml-auto">
              <CopyButton text={text} />
            </span>
          </div>
          <pre
            ref={box}
            data-testid="build-log"
            onScroll={(e) => {
              const el = e.currentTarget;
              if (el.scrollHeight - el.scrollTop - el.clientHeight > 24 && follow) setFollow(false);
            }}
            className="selectable h-[calc(100vh-320px)] min-h-[240px] overflow-auto rounded-lg border border-line bg-log p-3 font-mono text-[12px] leading-[1.5] whitespace-pre-wrap [overflow-wrap:anywhere]"
          >
            {text || t("build.waiting")}
          </pre>
        </div>
      </div>
    </div>
  );
}

/** Asistente de construcción y progreso de la construcción en curso. */
export function BuildPage({ prefill }: { prefill: BuildSpec | null }) {
  const { t } = useTranslation();
  const nav = useImagesNav();
  const status = useBuild((s) => s.status);
  const inProgress = status !== "idle";
  return (
    <div className="flex min-h-full flex-col">
      <div className="flex items-center gap-3 border-b border-line px-8 py-3">
        <Button variant="ghost" size="sm" onClick={nav.showList}>
          <ArrowLeft size={15} />
          {t("imageDetail.back")}
        </Button>
        <h2 className="text-[15px] font-semibold">{t("build.title")}</h2>
      </div>
      {inProgress ? <BuildProgress /> : <BuildForm initial={prefill ?? emptyBuildSpec()} />}
    </div>
  );
}
