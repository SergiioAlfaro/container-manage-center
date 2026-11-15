// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * «Crear imagen a partir de este contenedor» (0.9.2, D-069): `docker commit`
 * mediante la API. Antes de crear se explica qué entra en la imagen (la capa
 * de escritura del contenedor) y qué no (volúmenes y carpetas montadas).
 */
import { useEffect, useState } from "react";
import { useApi } from "@/app/server";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { HardDrive, Layers, PackagePlus } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useApp } from "@/app/store";
import { useImagesNav } from "@/features/images/store";
import { renderArgv } from "@/features/activity/logic";
import type { CommitRequest } from "@/shared/ipc/bindings/CommitRequest";
import type { CommitResult } from "@/shared/ipc/bindings/CommitResult";
import type { SpecIssue } from "@/shared/ipc/bindings/SpecIssue";
import { keys } from "@/shared/query";
import { shortId } from "@/shared/format";
import { Button } from "@/shared/ui/controls";
import { Banner, Checkbox, Dialog, Spinner } from "@/shared/ui/feedback";
import { Field, IssueList, TextInput } from "../create/fields";

const COMPONENT = /^[a-z0-9]+(?:(?:[._]|__|-+)[a-z0-9]+)*$/;
const HOST = /^(?:[a-zA-Z0-9-]+(?:\.[a-zA-Z0-9-]+)*|\[[0-9a-fA-F:]+\])(?::[0-9]+)?$/;
const TAG = /^[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$/;

/** Comprobación inmediata mientras se escribe; el backend vuelve a validar. */
export function localCommitIssues(r: CommitRequest): SpecIssue[] {
  const out: SpecIssue[] = [];
  const issue = (field: string, code: string, params: Record<string, string> = {}): SpecIssue => ({ severity: "error", field, code, params });
  const repo = r.repository.trim();
  if (repo === "") out.push(issue("repository", "commit.repositoryRequired"));
  else {
    const parts = repo.split("/");
    const first = parts[0] ?? "";
    const hasHost = parts.length > 1 && (first.includes(".") || first.includes(":") || first === "localhost");
    const path = hasHost ? parts.slice(1) : parts;
    const okHost = !hasHost || HOST.test(first);
    if (repo.length > 255 || !okHost || path.length === 0 || !path.every((c) => COMPONENT.test(c))) out.push(issue("repository", "commit.repositoryInvalid"));
  }
  const tag = r.tag.trim();
  if (tag !== "" && !TAG.test(tag)) out.push(issue("tag", "commit.tagInvalid"));
  for (const [field, value, max] of [
    ["author", r.author, 200],
    ["message", r.message, 500],
  ] as const) {
    if ([...value].length > max) out.push(issue(field, "commit.tooLong", { max: String(max) }));
    // Sin caracteres de control (saltos de línea incluidos): Docker los guarda tal cual.
    if (/\p{Cc}/u.test(value)) out.push(issue(field, "commit.controlChars"));
  }
  return out;
}

/** `docker commit …` como lo registrará el backend. */
export function commitCommand(r: CommitRequest, container: string, running: boolean, reference: string): string {
  const a: string[] = [];
  if (r.author.trim()) a.push("--author", r.author.trim());
  if (r.message.trim()) a.push("--message", r.message.trim());
  if (running && !r.pause) a.push("--pause=false");
  return renderArgv("docker", ["commit", ...a, container, reference]);
}

export function fullReference(r: CommitRequest): string {
  const repo = r.repository.trim();
  return repo === "" ? "" : `${repo}:${r.tag.trim() || "latest"}`;
}

export function CommitDialog({ id, name, onClose }: { id: string; name: string; onClose: () => void }) {
  const api = useApi();
  const { t } = useTranslation();
  const qc = useQueryClient();
  const [form, setForm] = useState<CommitRequest>({ repository: "", tag: "", author: "", message: "", pause: true, replaceExisting: false });
  const [touched, setTouched] = useState(false);
  const [done, setDone] = useState<CommitResult | null>(null);
  const [existing, setExisting] = useState<string | null>(null);

  // Una sola consulta al abrir: estado del contenedor y montajes que no entran.
  const info = useQuery({ queryKey: ["commitInfo", id], queryFn: () => api.containerCommitPreview(id, { ...form, repository: "" }), staleTime: Infinity, gcTime: 0 });
  const running = info.data?.status === "running";
  useEffect(() => {
    // El nombre que se comprobó ya no es el mismo: la confirmación deja de valer.
    setExisting(null);
    setForm((f) => (f.replaceExisting ? { ...f, replaceExisting: false } : f));
  }, [form.repository, form.tag]);

  const local = localCommitIssues(form);
  const issuesOf = (f: string) => (touched ? local.filter((i) => i.field === f) : []);
  const reference = fullReference(form);

  const create = useMutation({
    mutationFn: async () => {
      // Docker solo pausa un contenedor en ejecución; detenido no hay nada que pausar.
      const request = { ...form, pause: running ? form.pause : true };
      // Comprobación en el backend justo antes: nombre válido y si ya existe.
      const pre = await api.containerCommitPreview(id, request);
      if (pre.issues.some((i) => i.severity === "error")) return { issues: pre.issues } as const;
      if (pre.existingImageId && !request.replaceExisting) {
        setExisting(pre.existingImageId);
        return { existing: pre.existingImageId } as const;
      }
      return { result: await api.containerCommit(id, request) } as const;
    },
    onSuccess: (r) => {
      if ("result" in r && r.result) {
        setDone(r.result);
        void qc.invalidateQueries({ queryKey: keys.images });
        void qc.invalidateQueries({ queryKey: keys.overview });
      }
    },
  });
  const backendIssues = (create.data && "issues" in create.data ? create.data.issues : undefined) ?? [];

  const openImage = () => {
    if (!done) return;
    useImagesNav.getState().showDetail(done.imageId, done.reference);
    useApp.getState().navigate("images");
    onClose();
  };

  if (done) {
    return (
      <Dialog
        title={t("commit.doneTitle")}
        onClose={onClose}
        footer={
          <>
            <Button onClick={onClose}>{t("common.close")}</Button>
            <Button variant="primary" onClick={openImage}>
              <Layers size={14} />
              {t("commit.openImage")}
            </Button>
          </>
        }
      >
        <p>{t("commit.done", { reference: done.reference, id: shortId(done.imageId) })}</p>
        {done.replacedImageId && <Banner tone="info">{t("commit.replaced", { id: shortId(done.replacedImageId) })}</Banner>}
        <p className="text-[12.5px] text-ink-3">{t("commit.originalUntouched", { name })}</p>
      </Dialog>
    );
  }

  const canSubmit = local.length === 0 && !create.isPending && !info.isPending && (existing === null || form.replaceExisting);
  return (
    <Dialog
      wide
      title={t("commit.title", { name })}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("containers.cancel")}</Button>
          <Button
            variant="primary"
            loading={create.isPending}
            disabled={!canSubmit}
            title={local.length > 0 ? t("commit.fixFirst") : undefined}
            onClick={() => {
              setTouched(true);
              if (local.length === 0) create.mutate();
            }}
          >
            {!create.isPending && <PackagePlus size={14} />}
            {create.isPending ? t("commit.creating") : t("commit.create")}
          </Button>
        </>
      }
    >
      <p className="text-[13px] text-ink-2">{t("commit.intro")}</p>
      <div className="grid gap-3 md:grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)]">
        <Field label={t("commit.repository")} issues={issuesOf("repository")} hint={t("commit.repositoryHint")}>
          {(fid, inv) => <TextInput id={fid} invalid={inv} autoFocus value={form.repository} onChange={(e) => setForm({ ...form, repository: e.target.value })} onBlur={() => setTouched(true)} placeholder="mi-aplicacion" />}
        </Field>
        <Field label={t("commit.tag")} issues={issuesOf("tag")} hint={t("commit.tagHint")}>
          {(fid, inv) => <TextInput id={fid} invalid={inv} value={form.tag} onChange={(e) => setForm({ ...form, tag: e.target.value })} onBlur={() => setTouched(true)} placeholder="latest" />}
        </Field>
        <Field label={t("commit.author")} issues={issuesOf("author")}>
          {(fid, inv) => <TextInput id={fid} invalid={inv} className="font-sans" value={form.author} onChange={(e) => setForm({ ...form, author: e.target.value })} placeholder={t("commit.authorPh")} />}
        </Field>
        <Field label={t("commit.message")} issues={issuesOf("message")}>
          {(fid, inv) => <TextInput id={fid} invalid={inv} className="font-sans" value={form.message} onChange={(e) => setForm({ ...form, message: e.target.value })} placeholder={t("commit.messagePh")} />}
        </Field>
      </div>
      <div className="rounded-md border border-line bg-surface-2 px-3 py-2 text-[12.5px]" aria-live="polite">
        <span className="text-ink-3">{t("commit.preview")} </span>
        <code className="selectable font-mono text-ink">{reference || "—"}</code>
      </div>
      <IssueList issues={backendIssues} />

      {/* Qué se guarda y qué no: antes de crear, siempre a la vista. */}
      <Banner tone="warn">
        <p className="font-medium">{t("commit.notIncludedTitle")}</p>
        <p className="mt-0.5">{t("commit.notIncluded")}</p>
        {info.isPending ? (
          <p className="mt-1 flex items-center gap-1.5 text-ink-3">
            <Spinner size={13} /> {t("common.loading")}
          </p>
        ) : info.data && info.data.excludedMounts.length > 0 ? (
          <ul className="mt-1.5 space-y-0.5" aria-label={t("commit.excludedList")}>
            {info.data.excludedMounts.map((m) => (
              <li key={m.target} className="flex items-center gap-1.5 font-mono text-[12px]">
                <HardDrive size={12} className="shrink-0 text-ink-3" />
                {m.target} <span className="font-sans text-ink-3">({t(`commit.mountKind.${m.kind}`, { defaultValue: m.kind })}: {m.source})</span>
              </li>
            ))}
          </ul>
        ) : (
          info.data && <p className="mt-1 text-ink-3">{t("commit.noMounts")}</p>
        )}
        <p className="mt-2">{t("commit.envIncluded")}</p>
      </Banner>
      {info.error && <Banner tone="error">{t("commit.loadError", { message: (info.error as { message: string }).message })}</Banner>}

      {running && (
        <div className="space-y-2 rounded-md border border-line px-3 py-2.5">
          <p className="text-[13px]">{t("commit.running")}</p>
          <Checkbox checked={form.pause} onChange={(pause) => setForm({ ...form, pause })} label={t("commit.pause")} hint={form.pause ? t("commit.pauseHint") : t("commit.noPauseHint")} />
        </div>
      )}

      {existing && (
        <div className="space-y-2">
          <Banner tone="warn">{t("commit.exists", { reference, id: shortId(existing) })}</Banner>
          <Checkbox checked={form.replaceExisting} onChange={(replaceExisting) => setForm({ ...form, replaceExisting })} label={t("commit.replace")} hint={t("commit.replaceHint")} />
        </div>
      )}
      {create.error && <Banner tone="error">{t("commit.failed", { message: (create.error as { message: string }).message })}</Banner>}
      {info.data && (
        <p className="text-[12px] text-ink-3">
          {t("pull.equivalent")}{" "}
          <code className="selectable font-mono text-ink-2">{commitCommand(form, name, running, reference || t("commit.refPlaceholder"))}</code>
        </p>
      )}
    </Dialog>
  );
}
