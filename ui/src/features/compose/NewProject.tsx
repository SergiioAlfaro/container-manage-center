import { useState } from "react";
import { useApi } from "@/app/server";
import { useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, FilePlus2, FolderOpen, ShieldCheck } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { ComposeConfig } from "@/shared/ipc/bindings/ComposeConfig";
import { keys } from "@/shared/query";
import { Button } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { ValidationResult, YamlArea } from "./FileEditor";
import { nameFromDir, template, validProjectName } from "./model";
import { useComposeNav } from "./store";

/** Crea `compose.yaml` en una carpeta, lo valida con Compose y lo añade a la lista. */
export function NewProject() {
  const api = useApi();
  const { t } = useTranslation();
  const qc = useQueryClient();
  const nav = useComposeNav();
  const [dir, setDir] = useState<string | null>(null);
  const [existing, setExisting] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [text, setText] = useState(template(""));
  const [touched, setTouched] = useState(false);
  const [result, setResult] = useState<ComposeConfig | null>(null);
  const [busy, setBusy] = useState<"validate" | "create" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const trimmed = name.trim();
  const badName = trimmed !== "" && !validProjectName(trimmed);
  const effective = trimmed || (dir ? nameFromDir(dir) : "");

  const pickDir = async () => {
    const d = await api.pickFolder(t("composeNew.pickTitle"));
    if (!d) return;
    setDir(d);
    setResult(null);
    setError(null);
    setExisting(await api.composeFindFile(d).catch(() => null));
  };

  const onName = (v: string) => {
    setName(v);
    // Mientras no se edite el contenido, la plantilla refleja el nombre.
    if (!touched) setText(template(v.trim()));
  };

  const projectFor = (d: string) => ({ name: validProjectName(effective) ? effective : "nuevo", files: [`${d}/compose.yaml`], workingDir: null, envFiles: [] });

  const validate = async (): Promise<ComposeConfig | null> => {
    if (!dir) return null;
    setBusy("validate");
    setError(null);
    try {
      const r = await api.composeConfig(projectFor(dir), { index: 0, content: text });
      setResult(r);
      return r;
    } catch (e) {
      setError((e as { message: string }).message);
      return null;
    } finally {
      setBusy(null);
    }
  };

  const create = async () => {
    if (!dir) return;
    const r = await validate();
    if (!r || r.error) return;
    setBusy("create");
    try {
      const file = await api.composeFileCreate(dir, text);
      const reg = await api.composeRegister([file.path], trimmed || null);
      if (!reg.project) {
        setError(reg.error ?? "");
        return;
      }
      void qc.invalidateQueries({ queryKey: keys.composeProjects });
      nav.showDetail(reg.project.name);
    } catch (e) {
      setError((e as { message: string }).message);
    } finally {
      setBusy(null);
    }
  };

  const addExisting = async () => {
    if (!existing) return;
    setBusy("create");
    setError(null);
    try {
      const reg = await api.composeRegister([existing], trimmed || null);
      if (!reg.project) {
        setError(reg.error ?? "");
        return;
      }
      void qc.invalidateQueries({ queryKey: keys.composeProjects });
      nav.showDetail(reg.project.name);
    } catch (e) {
      setError((e as { message: string }).message);
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="flex min-h-full flex-col">
      <div className="flex flex-wrap items-center gap-3 border-b border-line px-8 py-3">
        <Button variant="ghost" size="sm" onClick={nav.showList}>
          <ArrowLeft size={15} />
          {t("composeDetail.back")}
        </Button>
        <h2 className="text-[14.5px] font-semibold">{t("composeNew.title")}</h2>
      </div>
      <div className="flex min-h-0 flex-1 flex-col gap-4 px-8 py-5">
        <div className="flex flex-wrap items-end gap-4">
          <div>
            <p className="mb-1 text-[12.5px] font-medium text-ink-2">{t("composeNew.folder")}</p>
            <div className="flex items-center gap-2">
              <Button onClick={() => void pickDir()}>
                <FolderOpen size={14} />
                {dir ? t("composeNew.changeFolder") : t("composeNew.pickFolder")}
              </Button>
              {dir && <span className="selectable font-mono text-[12.5px]">{dir}</span>}
            </div>
          </div>
          <label className="block w-[280px] text-[12.5px]">
            <span className="font-medium text-ink-2">{t("composeAdd.name")}</span>
            <input
              value={name}
              onChange={(e) => onName(e.target.value)}
              placeholder={dir ? nameFromDir(dir) : t("composeAdd.namePh")}
              aria-invalid={badName}
              className="mt-1 h-8 w-full rounded-md border border-line bg-surface px-2 font-mono text-[13px] outline-none hover:border-line-strong focus:border-beacon aria-[invalid=true]:border-err"
            />
          </label>
        </div>
        <p className={`-mt-2 text-[12px] ${badName ? "text-err" : "text-ink-3"}`}>{badName ? t("composeAdd.badName") : t("composeNew.nameHint")}</p>
        {existing && (
          <Banner tone="warn">
            <p>{t("composeNew.exists", { file: existing })}</p>
            <div className="mt-2">
              <Button size="sm" onClick={() => void addExisting()} disabled={busy !== null}>
                {t("composeNew.addExisting")}
              </Button>
            </div>
          </Banner>
        )}
        <div className="flex flex-wrap items-center gap-2">
          <p className="flex-1 text-[12px] text-ink-3">{t("composeNew.editorHint")}</p>
          <Button size="sm" onClick={() => void validate()} disabled={!dir || busy !== null}>
            {busy === "validate" ? <Spinner size={13} /> : <ShieldCheck size={13} />}
            {t("composeFile.validate")}
          </Button>
          <Button size="sm" variant="primary" onClick={() => void create()} disabled={!dir || !!existing || badName || busy !== null}>
            {busy === "create" ? <Spinner size={13} /> : <FilePlus2 size={13} />}
            {t("composeNew.create")}
          </Button>
        </div>
        {!dir && <p className="text-[12.5px] text-ink-3">{t("composeNew.needFolder")}</p>}
        {result && <ValidationResult cfg={result} />}
        {error && <Banner tone="error">{error}</Banner>}
        <YamlArea
          label={t("composeNew.editorLabel")}
          value={text}
          onChange={(v) => {
            setTouched(true);
            setText(v);
            setResult(null);
          }}
        />
      </div>
    </div>
  );
}
