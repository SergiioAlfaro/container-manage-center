import { useEffect, useState } from "react";
import { ConfirmServerWrite, useIsRemote } from "@/features/servers/paths";
import { useApi } from "@/app/server";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { CheckCircle2, RotateCcw, Save, ShieldCheck } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { ComposeConfig } from "@/shared/ipc/bindings/ComposeConfig";
import type { ComposeFile } from "@/shared/ipc/bindings/ComposeFile";
import type { ProjectRef } from "@/shared/ipc/bindings/ProjectRef";
import { Button, Segmented } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { baseName } from "./model";

/** Área de texto para YAML: Tab inserta dos espacios (no cambia el foco). */
export function YamlArea({ value, onChange, readOnly, label }: { value: string; onChange: (v: string) => void; readOnly?: boolean; label: string }) {
  return (
    <textarea
      aria-label={label}
      value={value}
      readOnly={readOnly}
      spellCheck={false}
      autoCapitalize="off"
      autoCorrect="off"
      onChange={(e) => onChange(e.target.value)}
      onKeyDown={(e) => {
        if (e.key !== "Tab" || readOnly || e.shiftKey || e.ctrlKey || e.altKey || e.metaKey) return;
        e.preventDefault();
        const el = e.currentTarget;
        const { selectionStart: a, selectionEnd: b } = el;
        const next = value.slice(0, a) + "  " + value.slice(b);
        onChange(next);
        requestAnimationFrame(() => el.setSelectionRange(a + 2, a + 2));
      }}
      className="selectable min-h-[420px] w-full flex-1 resize-y rounded-lg border border-line bg-log p-3 font-mono text-[12.5px] leading-[1.55] outline-none focus:border-beacon read-only:opacity-80"
    />
  );
}

export function ValidationResult({ cfg }: { cfg: ComposeConfig }) {
  const { t } = useTranslation();
  if (cfg.error) {
    return (
      <Banner tone="error">
        <p className="font-medium">{t("composeFile.invalid")}</p>
        <pre className="selectable mt-1 font-mono text-[12px] whitespace-pre-wrap [overflow-wrap:anywhere]">{cfg.error}</pre>
      </Banner>
    );
  }
  return (
    <Banner tone="info">
      <span className="inline-flex items-center gap-1.5">
        <CheckCircle2 size={14} className="text-ok" />
        {t("composeFile.valid", { count: cfg.model?.services.length ?? 0 })}
      </span>
      {cfg.warnings.length > 0 && (
        <ul className="mt-1 list-disc pl-5 text-[12px]">
          {cfg.warnings.map((w, i) => (
            <li key={i}>{w}</li>
          ))}
        </ul>
      )}
    </Banner>
  );
}

function errorCode(e: unknown): string | undefined {
  return (e as { code?: string } | null)?.code;
}

/** Edición de un archivo del proyecto. Valida con Compose (sin guardar) y no
 * sobrescribe un archivo que haya cambiado fuera de la aplicación. */
export function FileEditor({ project, onSaved }: { project: ProjectRef; onSaved: () => void }) {
  const api = useApi();
  const { t } = useTranslation();
  const qc = useQueryClient();
  const [index, setIndex] = useState(0);
  const path = project.files[index] ?? project.files[0] ?? "";
  const q = useQuery({ queryKey: ["composeFile", path], queryFn: () => api.composeFileRead(path), refetchOnMount: "always", refetchOnWindowFocus: false, staleTime: Infinity });
  const [loaded, setLoaded] = useState<ComposeFile | null>(null);
  const [text, setText] = useState("");
  const [result, setResult] = useState<ComposeConfig | null>(null);
  const [busy, setBusy] = useState<"validate" | "save" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [conflict, setConflict] = useState(false);
  const [offerForce, setOfferForce] = useState(false);
  const [savedAt, setSavedAt] = useState<number | null>(null);
  const [pendingIndex, setPendingIndex] = useState<number | null>(null);
  const switchTo = (i: number) => {
    setPendingIndex(null);
    setIndex(i);
    setResult(null);
    setLoaded(null);
    setSavedAt(null);
  };

  // Carga inicial y tras recargar: solo si no hay cambios sin guardar.
  useEffect(() => {
    if (q.data && (!loaded || loaded.path !== q.data.path || text === loaded.content)) {
      setLoaded(q.data);
      setText(q.data.content);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [q.data]);

  const dirty = loaded !== null && text !== loaded.content;
  const readOnly = loaded !== null && !loaded.writable;

  const validate = async (): Promise<ComposeConfig | null> => {
    setBusy("validate");
    setError(null);
    try {
      const r = await api.composeConfig(project, { index, content: text });
      setResult(r);
      return r;
    } catch (e) {
      setError((e as { message: string }).message);
      return null;
    } finally {
      setBusy(null);
    }
  };

  const remote = useIsRemote();
  const [confirmWrite, setConfirmWrite] = useState<boolean | null>(null);
  const save = async (force: boolean, confirmed = false) => {
    if (!loaded) return;
    setOfferForce(false);
    // En un servidor remoto se confirma el destino antes de escribir (§5).
    if (remote && !confirmed) {
      if (!force) {
        const r = await validate();
        if (!r) return;
        if (r.error) {
          setOfferForce(true);
          return;
        }
      }
      setConfirmWrite(force);
      return;
    }
    if (!force && !confirmed) {
      const r = await validate();
      if (!r) return;
      if (r.error) {
        setOfferForce(true);
        return;
      }
    }
    setBusy("save");
    setError(null);
    try {
      const saved = await api.composeFileSave(loaded.path, text, loaded.version);
      setLoaded(saved);
      setText(saved.content);
      setConflict(false);
      setSavedAt(Date.now());
      qc.setQueryData(["composeFile", path], saved);
      onSaved();
    } catch (e) {
      if (errorCode(e) === "engine.fileChanged") setConflict(true);
      else setError((e as { message: string }).message);
    } finally {
      setBusy(null);
    }
  };

  const reload = async () => {
    setConflict(false);
    setResult(null);
    const r = await q.refetch();
    if (r.data) {
      setLoaded(r.data);
      setText(r.data.content);
    }
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      {project.files.length > 1 && (
        <Segmented<string>
          label={t("composeFile.files")}
          value={String(index)}
          onChange={(v) => {
            if (dirty) setPendingIndex(Number(v));
            else switchTo(Number(v));
          }}
          options={project.files.map((f, i) => ({ value: String(i), label: baseName(f) }))}
        />
      )}
      {pendingIndex !== null && (
        <Banner tone="warn">
          <p>{t("composeFile.discard")}</p>
          <div className="mt-2 flex gap-2">
            <Button size="sm" onClick={() => switchTo(pendingIndex)}>
              {t("composeFile.discardYes")}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setPendingIndex(null)}>
              {t("composeFile.discardNo")}
            </Button>
          </div>
        </Banner>
      )}
      <div className="flex flex-wrap items-center gap-2">
        <p className="selectable min-w-0 flex-1 truncate font-mono text-[12px] text-ink-3" title={path}>
          {path}
          {dirty && <span className="ml-2 font-sans text-warn">{t("composeFile.unsaved")}</span>}
          {!dirty && savedAt && <span className="ml-2 font-sans text-ok">{t("composeFile.saved")}</span>}
        </p>
        <Button size="sm" onClick={() => void validate()} disabled={busy !== null || !loaded}>
          {busy === "validate" ? <Spinner size={13} /> : <ShieldCheck size={13} />}
          {t("composeFile.validate")}
        </Button>
        <Button size="sm" variant="primary" onClick={() => void save(false)} disabled={busy !== null || !dirty || readOnly}>
          {busy === "save" ? <Spinner size={13} /> : <Save size={13} />}
          {t("composeFile.save")}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            setText(loaded?.content ?? "");
            setResult(null);
            setOfferForce(false);
          }}
          disabled={!dirty || busy !== null}
        >
          <RotateCcw size={13} />
          {t("composeFile.revert")}
        </Button>
      </div>
      <p className="text-[12px] text-ink-3">{t("composeFile.hint")}</p>
      {q.error && <Banner tone="error">{t("composeFile.loadError", { message: (q.error as { message: string }).message })}</Banner>}
      {readOnly && <Banner tone="warn">{t("composeFile.readOnly")}</Banner>}
      {conflict && (
        <Banner tone="error">
          <p>{t("composeFile.conflict")}</p>
          <div className="mt-2 flex gap-2">
            <Button size="sm" onClick={() => void reload()}>
              {t("composeFile.reload")}
            </Button>
          </div>
        </Banner>
      )}
      {result && <ValidationResult cfg={result} />}
      {offerForce && (
        <Banner tone="warn">
          <p>{t("composeFile.saveInvalid")}</p>
          <div className="mt-2 flex gap-2">
            <Button size="sm" onClick={() => void save(true)}>
              {t("composeFile.saveAnyway")}
            </Button>
          </div>
        </Banner>
      )}
      {error && <Banner tone="error">{error}</Banner>}
      {q.isPending ? (
        <div className="flex items-center gap-2 py-10 text-[13px] text-ink-3">
          <Spinner /> {t("common.loading")}
        </div>
      ) : (
        loaded && (
          <YamlArea
            label={t("composeFile.editorLabel", { file: baseName(path) })}
            value={text}
            onChange={(v) => {
              setText(v);
              // Lo validado ya no corresponde al texto.
              setOfferForce(false);
              setSavedAt(null);
            }}
            readOnly={readOnly}
          />
        )
      )}
      {confirmWrite !== null && loaded && (
        <ConfirmServerWrite path={loaded.path} create={false} onConfirm={() => void save(confirmWrite, true)} onClose={() => setConfirmWrite(null)} />
      )}
    </div>
  );
}
