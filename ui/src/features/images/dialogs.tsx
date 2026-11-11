import { useState } from "react";
import { useApi } from "@/app/server";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Tag, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { formatBytes, shortId } from "@/shared/format";
import type { ImageSummary } from "@/shared/ipc/bindings/ImageSummary";
import type { PruneResult } from "@/shared/ipc/bindings/PruneResult";
import type { RemoveResult } from "@/shared/ipc/bindings/RemoveResult";
import { keys } from "@/shared/query";
import { Button } from "@/shared/ui/controls";
import { Banner, Checkbox, Dialog, Spinner } from "@/shared/ui/feedback";
import { Field, TextInput } from "@/features/containers/create/fields";
import { useServerPulls } from "./pull/pulls";
import { familiar, useRefCheck } from "./useRefCheck";

type Img = Pick<ImageSummary, "id" | "repoTags" | "usedBy" | "size">;

const errMsg = (e: unknown) => (e as { message: string }).message;

/** Recarga lo afectado. Tras eliminar no se recarga el detalle: la imagen puede ya no existir. */
function useInvalidate(details = true) {
  const qc = useQueryClient();
  return () => {
    void qc.invalidateQueries({ queryKey: keys.images });
    void qc.invalidateQueries({ queryKey: keys.overview });
    if (details) void qc.invalidateQueries({ queryKey: ["image"] });
  };
}

/** Nombre con el que se refiere a la imagen en las operaciones: su primera etiqueta o su ID. */
export function imageRef(i: Pick<ImageSummary, "id" | "repoTags">): string {
  return i.repoTags[0] ?? i.id;
}

export function TagDialog({ image, onClose, onDone }: { image: Img; onClose: () => void; onDone: (msg: string) => void }) {
  const api = useApi();
  const { t } = useTranslation();
  const invalidate = useInvalidate();
  const source = imageRef(image);
  const [repo, setRepo] = useState("");
  const [tag, setTag] = useState("latest");
  const check = useRefCheck(repo.trim() ? `${repo.trim()}:${tag.trim()}` : "");
  const m = useMutation({
    mutationFn: () => api.imageTag(source, repo.trim(), tag.trim()),
    onSuccess: (full) => {
      invalidate();
      onDone(t("imageOps.tagged", { target: full }));
      onClose();
    },
  });
  const ok = check.state === "ok" && !check.ref.digest && !m.isPending;
  return (
    <Dialog
      title={t("imageOps.tagTitle")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("containers.cancel")}</Button>
          <Button variant="primary" disabled={!ok} onClick={() => m.mutate()}>
            {m.isPending ? <Spinner size={14} /> : <Tag size={14} />}
            {t("imageOps.tag")}
          </Button>
        </>
      }
    >
      <p className="text-[13px]">
        {t("imageOps.tagSource")} <code className="selectable font-mono text-[12.5px]">{source}</code>
      </p>
      <div className="grid grid-cols-[minmax(0,1fr)_140px] gap-3">
        <Field label={t("imageOps.repository")} issues={check.state === "error" ? [{ severity: "error", field: "repo", code: check.code, params: {} }] : []} hint={t("imageOps.repositoryHint")}>
          {(id, inv) => <TextInput id={id} invalid={inv} value={repo} onChange={(e) => setRepo(e.target.value)} placeholder="miusuario/miapp" autoFocus />}
        </Field>
        <Field label={t("imageOps.tagName")}>{(id) => <TextInput id={id} value={tag} onChange={(e) => setTag(e.target.value)} placeholder="latest" />}</Field>
      </div>
      {check.state === "ok" && (
        <p className="text-[12px] text-ink-3">
          {t("pull.equivalent")} <code className="selectable font-mono text-ink-2">docker tag {source} {repo.trim()}:{tag.trim()}</code>
          <span className="ml-1">({familiar(check.ref)})</span>
        </p>
      )}
      {m.error && <Banner tone="error">{t("imageOps.failed", { message: errMsg(m.error) })}</Banner>}
    </Dialog>
  );
}

type RemoveMode = { kind: "tag"; tag: string } | { kind: "image" };

/**
 * Eliminar: con varias etiquetas se puede quitar solo una; eliminar la imagen
 * completa usa su ID. Avisa de los contenedores que la usan antes de pedir confirmación.
 */
export function RemoveImageDialog({ image, onClose, onDone }: { image: Img; onClose: () => void; onDone: (r: RemoveResult) => void }) {
  const api = useApi();
  const { t, i18n } = useTranslation();
  const invalidate = useInvalidate(false);
  const multi = image.repoTags.length > 1;
  const [mode, setMode] = useState<RemoveMode>(multi ? { kind: "tag", tag: image.repoTags[0]! } : { kind: "image" });
  const running = image.usedBy.filter((u) => u.state === "running" || u.state === "paused" || u.state === "restarting");
  const stopped = image.usedBy.filter((u) => !running.includes(u));
  const needsForce = mode.kind === "image" && (multi || image.usedBy.length > 0);
  const [force, setForce] = useState(false);
  const target = mode.kind === "tag" ? mode.tag : multi ? image.id : (image.repoTags[0] ?? image.id);
  const forceFlag = needsForce && force;
  const m = useMutation({
    mutationFn: () => api.imageRemove(target, forceFlag),
    onSuccess: (r) => {
      invalidate();
      onDone(r);
      onClose();
    },
  });
  const blocked = (needsForce && !force) || m.isPending;
  return (
    <Dialog
      title={t("imageOps.removeTitle", { name: image.repoTags[0] ?? shortId(image.id) })}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("containers.cancel")}</Button>
          <Button variant="danger" disabled={blocked} onClick={() => m.mutate()}>
            {m.isPending ? <Spinner size={14} /> : <Trash2 size={14} />}
            {mode.kind === "tag" ? t("imageOps.removeTag") : t("imageOps.removeImage")}
          </Button>
        </>
      }
    >
      <p className="selectable flex flex-wrap gap-3 font-mono text-[12px] text-ink-3">
        <span>{shortId(image.id)}</span>
        <span>{formatBytes(image.size, i18n.language)}</span>
      </p>
      {multi && (
        <fieldset className="space-y-1.5">
          <legend className="mb-1 text-[12.5px] font-medium text-ink-2">{t("imageOps.removeWhat")}</legend>
          {image.repoTags.map((tag) => (
            <label key={tag} className="flex items-center gap-2 text-[13px]">
              <input type="radio" name="rm" checked={mode.kind === "tag" && mode.tag === tag} onChange={() => setMode({ kind: "tag", tag })} />
              <span>
                {t("imageOps.onlyTag")} <code className="font-mono text-[12.5px]">{tag}</code>
              </span>
            </label>
          ))}
          <label className="flex items-center gap-2 text-[13px]">
            <input type="radio" name="rm" checked={mode.kind === "image"} onChange={() => setMode({ kind: "image" })} />
            <span>{t("imageOps.wholeImage", { count: image.repoTags.length })}</span>
          </label>
        </fieldset>
      )}
      {mode.kind === "tag" ? (
        <p className="text-[13px]">{t("imageOps.removeTagBody")}</p>
      ) : (
        <p className="text-[13px]">{t("imageOps.removeImageBody")}</p>
      )}
      {image.usedBy.length > 0 && (
        <Banner tone="warn">
          <p>{t("imageOps.inUse", { count: image.usedBy.length })}</p>
          <ul className="mt-1 list-disc pl-5 font-mono text-[12px]">
            {image.usedBy.map((u) => (
              <li key={u.id}>
                {u.name} <span className="font-sans text-ink-3">({t(`containers.state.${u.state}`, { defaultValue: u.state })})</span>
              </li>
            ))}
          </ul>
          {running.length > 0 && mode.kind === "image" && <p className="mt-1">{t("imageOps.runningRefuses")}</p>}
          {stopped.length > 0 && mode.kind === "image" && running.length === 0 && <p className="mt-1">{t("imageOps.stoppedNeedForce")}</p>}
        </Banner>
      )}
      {needsForce && <Checkbox checked={force} onChange={setForce} label={t("imageOps.force")} hint={multi ? t("imageOps.forceMulti") : t("imageOps.forceUsed")} />}
      <p className="text-[12px] text-ink-3">
        {t("pull.equivalent")} <code className="selectable font-mono text-ink-2">docker image rm {forceFlag ? "--force " : ""}{target}</code>
      </p>
      {m.error && <Banner tone="error">{t("imageOps.failed", { message: errMsg(m.error) })}</Banner>}
    </Dialog>
  );
}

export function PruneDialog({ images, onClose, onDone }: { images: ImageSummary[]; onClose: () => void; onDone: (r: PruneResult) => void }) {
  const api = useApi();
  const { t, i18n } = useTranslation();
  const invalidate = useInvalidate(false);
  const [all, setAll] = useState(false);
  const [sure, setSure] = useState(false);
  const affected = images.filter((i) => i.usedBy.length === 0 && (all || i.dangling));
  const bytes = affected.reduce((a, i) => a + i.size, 0);
  // Con el almacén de containerd, limpiar hace fallar las descargas en curso (D-050).
  const pulling = useServerPulls().filter((p) => p.status === "starting" || p.status === "running").length;
  const m = useMutation({
    mutationFn: () => api.imagesPrune(all),
    onSuccess: (r) => {
      invalidate();
      onDone(r);
      onClose();
    },
  });
  return (
    <Dialog
      title={t("imageOps.pruneTitle")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("containers.cancel")}</Button>
          <Button variant="danger" disabled={pulling > 0 || (all && !sure) || m.isPending} onClick={() => m.mutate()}>
            {m.isPending ? <Spinner size={14} /> : <Trash2 size={14} />}
            {t("imageOps.prune")}
          </Button>
        </>
      }
    >
      <fieldset className="space-y-2">
        <label className="flex items-start gap-2 text-[13px]">
          <input type="radio" name="prune" className="mt-1" checked={!all} onChange={() => setAll(false)} />
          <span>
            {t("imageOps.pruneDangling")}
            <span className="block text-[12px] text-ink-3">docker image prune --force</span>
          </span>
        </label>
        <label className="flex items-start gap-2 text-[13px]">
          <input type="radio" name="prune" className="mt-1" checked={all} onChange={() => setAll(true)} />
          <span>
            {t("imageOps.pruneAll")}
            <span className="block text-[12px] text-ink-3">docker image prune --force --all</span>
          </span>
        </label>
      </fieldset>
      <p className="text-[13px]">{t("imageOps.pruneEstimate", { count: affected.length, size: formatBytes(bytes, i18n.language) })}</p>
      {affected.length > 0 && (
        <ul className="max-h-[140px] overflow-auto rounded-md border border-line bg-surface-2 px-3 py-2 font-mono text-[12px]">
          {affected.map((i) => (
            <li key={i.id}>{i.repoTags.join(", ") || `${t("images.dangling")} ${shortId(i.id)}`}</li>
          ))}
        </ul>
      )}
      <p className="text-[12px] text-ink-3">{t("imageOps.pruneNote")}</p>
      {pulling > 0 && <Banner tone="warn">{t("imageOps.pruneWhilePulling", { count: pulling })}</Banner>}
      {all && <Checkbox checked={sure} onChange={setSure} label={t("imageOps.pruneSure")} />}
      {m.error && <Banner tone="error">{t("imageOps.failed", { message: errMsg(m.error) })}</Banner>}
    </Dialog>
  );
}
