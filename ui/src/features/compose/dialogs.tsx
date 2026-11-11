import { useEffect, useState } from "react";
import { useApi } from "@/app/server";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { FilePlus2, Play, Trash2, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { ComposeAction } from "@/shared/ipc/bindings/ComposeAction";
import type { DownImages } from "@/shared/ipc/bindings/DownImages";
import type { ProjectModel } from "@/shared/ipc/bindings/ProjectModel";
import type { ProjectRef } from "@/shared/ipc/bindings/ProjectRef";
import type { PullPolicy } from "@/shared/ipc/bindings/PullPolicy";
import { keys } from "@/shared/query";
import { Button, CopyButton, Segmented } from "@/shared/ui/controls";
import { Banner, Checkbox, Dialog, Spinner } from "@/shared/ui/feedback";
import { downAction, removableVolumes, upAction, validProjectName } from "./model";
import { useRunCompose } from "./runs";
import { ServerPathInput, useIsRemote } from "@/features/servers/paths";

/** Comando exacto que se ejecutará, calculado por el backend. */
function useCommand(project: ProjectRef, action: ComposeAction) {
  const api = useApi();
  const [cmd, setCmd] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const key = JSON.stringify(action);
  useEffect(() => {
    let live = true;
    api
      .composePreview(project, action)
      .then((p) => live && (setCmd(p.command), setErr(null)))
      .catch((e) => live && (setCmd(null), setErr((e as { message: string }).message)));
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [project.name, key]);
  return { cmd, err };
}

function CommandBox({ cmd, err }: { cmd: string | null; err: string | null }) {
  const { t } = useTranslation();
  if (err) return <Banner tone="error">{err}</Banner>;
  return (
    <div>
      <p className="mb-1 text-[12px] text-ink-3">{t("composeOps.command")}</p>
      <div className="flex items-start gap-2 rounded-md bg-code px-2.5 py-1.5">
        <code data-testid="compose-dialog-command" className="selectable flex-1 font-mono text-[11.5px] [overflow-wrap:anywhere]">
          {cmd ?? "…"}
        </code>
        {cmd && <CopyButton text={cmd} compact />}
      </div>
    </div>
  );
}

function Toggles<T extends string>({ items, selected, onChange, label }: { items: T[]; selected: T[]; onChange: (v: T[]) => void; label: string }) {
  return (
    <fieldset>
      <legend className="mb-1 text-[12.5px] font-medium text-ink-2">{label}</legend>
      <div className="flex flex-wrap gap-1.5">
        {items.map((it) => {
          const on = selected.includes(it);
          return (
            <button
              key={it}
              type="button"
              aria-pressed={on}
              onClick={() => onChange(on ? selected.filter((x) => x !== it) : [...selected, it])}
              className={`h-7 rounded-md border px-2 font-mono text-[12px] ${on ? "border-beacon bg-beacon/10 text-ink" : "border-line text-ink-2 hover:border-line-strong"}`}
            >
              {it}
            </button>
          );
        })}
      </div>
    </fieldset>
  );
}

/** Desplegar (`up --detach`) con sus opciones. */
export function UpDialog({ project, model, onClose, onStarted }: { project: ProjectRef; model: ProjectModel | null; onClose: () => void; onStarted: () => void }) {
  const { t } = useTranslation();
  const run = useRunCompose();
  const [services, setServices] = useState<string[]>([]);
  const [profiles, setProfiles] = useState<string[]>([]);
  const [build, setBuild] = useState(false);
  const [pull, setPull] = useState<PullPolicy>("missing");
  const [forceRecreate, setForceRecreate] = useState(false);
  const [removeOrphans, setRemoveOrphans] = useState(false);
  const action = upAction({ services, profiles, build, pull, forceRecreate, removeOrphans });
  const { cmd, err } = useCommand(project, action);
  const canBuild = model?.services.some((s) => s.build) ?? false;
  return (
    <Dialog
      title={t("composeOps.upTitle", { name: project.name })}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("containers.cancel")}</Button>
          <Button
            variant="primary"
            disabled={!cmd}
            onClick={() => {
              void run(project, action);
              onStarted();
              onClose();
            }}
          >
            <Play size={14} />
            {t("composeOps.up")}
          </Button>
        </>
      }
    >
      <p className="text-[13px] text-ink-2">{t("composeOps.upBody")}</p>
      {model && model.services.length > 0 && (
        <div>
          <Toggles label={t("composeOps.services")} items={model.services.map((s) => s.name)} selected={services} onChange={setServices} />
          <p className="mt-1 text-[12px] text-ink-3">{services.length === 0 ? t("composeOps.allServices") : t("composeOps.someServices")}</p>
        </div>
      )}
      {model && model.profiles.length > 0 && (
        <div>
          <Toggles label={t("composeOps.profiles")} items={model.profiles} selected={profiles} onChange={setProfiles} />
          <p className="mt-1 text-[12px] text-ink-3">{t("composeOps.profilesHint")}</p>
        </div>
      )}
      <div>
        <p className="mb-1 text-[12.5px] font-medium text-ink-2">{t("composeOps.pull")}</p>
        <Segmented<PullPolicy> label={t("composeOps.pull")} value={pull} onChange={setPull} options={(["missing", "always", "never"] as PullPolicy[]).map((p) => ({ value: p, label: t(`composeOps.pullPolicy.${p}`) }))} />
      </div>
      {canBuild && <Checkbox checked={build} onChange={setBuild} label={t("composeOps.build")} hint={t("composeOps.buildHint")} />}
      <Checkbox checked={forceRecreate} onChange={setForceRecreate} label={t("composeOps.forceRecreate")} />
      <Checkbox checked={removeOrphans} onChange={setRemoveOrphans} label={t("composeOps.removeOrphans")} hint={t("composeOps.removeOrphansHint")} />
      <CommandBox cmd={cmd} err={err} />
    </Dialog>
  );
}

/** Eliminar el proyecto (`down`): contenedores y redes; volúmenes e imágenes opcionales. */
export function DownDialog({ project, model, containers, onClose, onStarted }: { project: ProjectRef; model: ProjectModel | null; containers: number; onClose: () => void; onStarted: () => void }) {
  const { t } = useTranslation();
  const run = useRunCompose();
  const [volumes, setVolumes] = useState(false);
  const [images, setImages] = useState<DownImages>("none");
  const [removeOrphans, setRemoveOrphans] = useState(false);
  const [sure, setSure] = useState(false);
  const action = downAction({ volumes, images, removeOrphans });
  const { cmd, err } = useCommand(project, action);
  const vols = removableVolumes(model);
  return (
    <Dialog
      title={t("composeOps.downTitle", { name: project.name })}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("containers.cancel")}</Button>
          <Button
            variant="danger"
            disabled={!sure || !cmd}
            onClick={() => {
              void run(project, action, true);
              onStarted();
              onClose();
            }}
          >
            <Trash2 size={14} />
            {t("composeOps.down")}
          </Button>
        </>
      }
    >
      <p className="text-[13px]">{t("composeOps.downBody", { count: containers })}</p>
      <Checkbox checked={volumes} onChange={setVolumes} label={t("composeOps.downVolumes")} hint={vols.length > 0 ? vols.join(", ") : t("composeOps.downVolumesUnknown")} />
      {volumes && <Banner tone="warn">{t("composeOps.downVolumesWarn")}</Banner>}
      <div>
        <p className="mb-1 text-[12.5px] font-medium text-ink-2">{t("composeOps.downImages")}</p>
        <Segmented<DownImages> label={t("composeOps.downImages")} value={images} onChange={setImages} options={(["none", "local", "all"] as DownImages[]).map((i) => ({ value: i, label: t(`composeOps.images.${i}`) }))} />
        {images !== "none" && <p className="mt-1 text-[12px] text-ink-3">{t(`composeOps.imagesHint.${images}`)}</p>}
      </div>
      <Checkbox checked={removeOrphans} onChange={setRemoveOrphans} label={t("composeOps.removeOrphans")} hint={t("composeOps.removeOrphansHint")} />
      <CommandBox cmd={cmd} err={err} />
      <Checkbox checked={sure} onChange={setSure} label={t("composeOps.downSure")} />
    </Dialog>
  );
}

/** Añadir a la lista un proyecto que ya existe en el equipo (uno o varios archivos). */
export function AddProjectDialog({ onClose, onAdded }: { onClose: () => void; onAdded: (name: string) => void }) {
  const api = useApi();
  const { t } = useTranslation();
  const qc = useQueryClient();
  const [files, setFiles] = useState<string[]>([]);
  const [name, setName] = useState("");
  const [composeError, setComposeError] = useState<string | null>(null);
  const trimmed = name.trim();
  const badName = trimmed !== "" && !validProjectName(trimmed);
  const m = useMutation({
    mutationFn: () => api.composeRegister(files, trimmed || null),
    onMutate: () => setComposeError(null),
    onSuccess: (r) => {
      if (r.project) {
        void qc.invalidateQueries({ queryKey: keys.composeProjects });
        onAdded(r.project.name);
        onClose();
      } else {
        setComposeError(r.error ?? "");
      }
    },
  });
  const remote = useIsRemote();
  const pick = async () => {
    const picked = await api.pickComposeFiles(t("composeAdd.pickTitle"), t("composeAdd.filter"));
    if (picked.length > 0) setFiles((f) => [...f, ...picked.filter((p) => !f.includes(p))]);
  };
  return (
    <Dialog
      title={t("composeAdd.title")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("containers.cancel")}</Button>
          <Button variant="primary" disabled={files.length === 0 || badName || m.isPending} onClick={() => m.mutate()}>
            {m.isPending ? <Spinner size={14} /> : <FilePlus2 size={14} />}
            {t("composeAdd.submit")}
          </Button>
        </>
      }
    >
      <p className="text-[13px] text-ink-2">{t("composeAdd.body")}</p>
      <div>
        {remote ? (
          <ServerPathInput
            label={t("servers.paths.composeFile")}
            placeholder="/srv/app/compose.yaml"
            submitLabel={t("servers.paths.add")}
            onSubmit={(p) => setFiles((f) => (f.includes(p) ? f : [...f, p]))}
          />
        ) : (
          <Button onClick={() => void pick()}>
            <FilePlus2 size={14} />
            {files.length === 0 ? t("composeAdd.pick") : t("composeAdd.pickMore")}
          </Button>
        )}
        {files.length > 0 && (
          <ol className="mt-2 space-y-1">
            {files.map((f, i) => (
              <li key={f} className="flex items-center gap-2 font-mono text-[12px]">
                <span className="text-ink-3 tabular-nums">{i + 1}.</span>
                <span className="selectable min-w-0 flex-1 [overflow-wrap:anywhere]">{f}</span>
                <button type="button" aria-label={t("composeAdd.removeFile", { file: f })} className="text-ink-3 hover:text-err" onClick={() => setFiles(files.filter((x) => x !== f))}>
                  <X size={14} />
                </button>
              </li>
            ))}
          </ol>
        )}
        {files.length > 1 && <p className="mt-1 text-[12px] text-ink-3">{t("composeAdd.order")}</p>}
      </div>
      <label className="block text-[12.5px]">
        <span className="font-medium text-ink-2">{t("composeAdd.name")}</span>
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder={t("composeAdd.namePh")}
          aria-invalid={badName}
          className="mt-1 h-8 w-full rounded-md border border-line bg-surface px-2 font-mono text-[13px] outline-none hover:border-line-strong focus:border-beacon aria-[invalid=true]:border-err"
        />
        <span className={`mt-1 block text-[12px] ${badName ? "text-err" : "text-ink-3"}`}>{badName ? t("composeAdd.badName") : t("composeAdd.nameHint")}</span>
      </label>
      {composeError !== null && <Banner tone="error">{t("composeAdd.invalid", { message: composeError })}</Banner>}
      {m.error && <Banner tone="error">{(m.error as { message: string }).message}</Banner>}
    </Dialog>
  );
}
