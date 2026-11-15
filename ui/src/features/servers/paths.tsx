// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
/**
 * Rutas de un servidor remoto (0.10.0, §5): en un servidor remoto los
 * archivos y carpetas son los del servidor. No se usa el selector de archivos
 * de este equipo (mostraría archivos que allí no existen): se escribe la ruta
 * del servidor. Antes de escribir un archivo en el servidor se confirma el
 * destino.
 */
import { useId, useState } from "react";
import { Server } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useServerView } from "@/app/server";
import { Button } from "@/shared/ui/controls";
import { Dialog } from "@/shared/ui/feedback";

/** El servidor del ámbito es remoto. */
export function useIsRemote(): boolean {
  return !!useServerView()?.remote;
}

/** Contexto de i18next para los textos con variante de servidor («clave_remote»). */
export function useRemoteContext(): "remote" | undefined {
  return useIsRemote() ? "remote" : undefined;
}

/** Ruta absoluta válida del servidor. */
export function validServerPath(p: string): boolean {
  const v = p.trim();
  return v.startsWith("/") && !v.includes("\0") && v.length <= 4096;
}

/** Campo para escribir una ruta del servidor. */
export function ServerPathInput({ label, placeholder, submitLabel, onSubmit }: { label: string; placeholder: string; submitLabel: string; onSubmit: (path: string) => void }) {
  const { t } = useTranslation();
  const view = useServerView();
  const [value, setValue] = useState("");
  const [touched, setTouched] = useState(false);
  const id = useId();
  const ok = validServerPath(value);
  const submit = () => {
    setTouched(true);
    if (!ok) return;
    onSubmit(value.trim());
    setValue("");
    setTouched(false);
  };
  return (
    <div>
      <div className="block text-[12.5px]">
        <label htmlFor={id} className="font-medium text-ink-2">
          {label}
        </label>
        <div className="mt-1 flex gap-2">
          <input
            id={id}
            value={value}
            spellCheck={false}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                submit();
              }
            }}
            placeholder={placeholder}
            aria-invalid={touched && !ok}
            className="h-8 min-w-0 flex-1 rounded-md border border-line bg-surface px-2 font-mono text-[13px] outline-none hover:border-line-strong focus:border-beacon aria-[invalid=true]:border-err"
          />
          <Button size="sm" className="h-8" onClick={submit}>
            {submitLabel}
          </Button>
        </div>
      </div>
      <p className={`mt-1 flex items-center gap-1.5 text-[12px] ${touched && !ok ? "text-err" : "text-ink-3"}`}>
        <Server size={12} aria-hidden />
        {touched && !ok ? t("servers.paths.absolute") : t("servers.paths.hint", { name: view?.name ?? "" })}
      </p>
    </div>
  );
}

/** Confirmación antes de escribir un archivo en el servidor (muestra el destino). */
export function ConfirmServerWrite({ path, create, onConfirm, onClose }: { path: string; create: boolean; onConfirm: () => void; onClose: () => void }) {
  const { t } = useTranslation();
  return (
    <Dialog
      title={t(create ? "servers.write.titleCreate" : "servers.write.titleSave")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button
            variant="primary"
            onClick={() => {
              onClose();
              onConfirm();
            }}
          >
            {t(create ? "servers.write.confirmCreate" : "servers.write.confirmSave")}
          </Button>
        </>
      }
    >
      <p>{t("servers.write.body")}</p>
      <code data-testid="write-destination" className="selectable block rounded-md bg-code px-2.5 py-1.5 font-mono text-[12.5px] [overflow-wrap:anywhere]">
        {path}
      </code>
    </Dialog>
  );
}
