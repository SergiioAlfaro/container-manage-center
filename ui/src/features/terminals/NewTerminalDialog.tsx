// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useEffect, useId, useState } from "react";
import { useApi, useServerView } from "@/app/server";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import type { ShellChoice } from "@/shared/ipc/bindings/ShellChoice";
import { keys } from "@/shared/query";
import { Button, Segmented } from "@/shared/ui/controls";
import { Banner, Dialog, Spinner } from "@/shared/ui/feedback";
import { defaultName, MAX_NAME, nameProblem, openSession, type SessionKind } from "./sessions";

const fieldClass =
  "h-8 w-full rounded-md border border-line bg-surface px-2 text-[13px] text-ink outline-none transition-colors placeholder:text-ink-3 hover:border-line-strong focus:border-beacon aria-[invalid=true]:border-err";

/** Elegir contenedor en ejecución, shell y nombre de la sesión. */
export function NewTerminalDialog({ onClose }: { onClose: () => void }) {
  const api = useApi();
  const view = useServerView();
  const { t } = useTranslation();
  const ids = { container: useId(), name: useId(), nameError: useId() };
  const list = useQuery({ queryKey: keys.containers, queryFn: api.containersList });
  const running = (list.data ?? []).filter((c) => c.state === "running").sort((a, b) => a.name.localeCompare(b.name));
  const [container, setContainer] = useState("");
  const [shell, setShell] = useState<ShellChoice>("auto");
  const [name, setName] = useState("");
  // En un servidor remoto se puede abrir también la shell SSH del servidor (0.10.0).
  const remote = !!view?.remote;
  const [kind, setKind] = useState<SessionKind>("container");
  const serverShell = remote && kind === "server";
  useEffect(() => {
    if (!container && running[0]) setContainer(running[0].id);
  }, [container, running]);

  const chosen = running.find((c) => c.id === container) ?? null;
  const problem = name.trim() === "" ? null : nameProblem(name);
  const placeholder = serverShell ? defaultName(`SSH ${view?.name ?? ""}`) : chosen ? defaultName(chosen.name) : "";

  const submit = () => {
    if (problem) return;
    if (serverShell) {
      openSession({ kind: "server", server: api.server, serverName: view?.name ?? "", remote: true, containerId: "", containerName: "", shell: "auto", name: name.trim() || undefined });
      onClose();
      return;
    }
    if (!chosen) return;
    openSession({ server: api.server, serverName: view?.remote ? view.name : "", remote: !!view?.remote, containerId: chosen.id, containerName: chosen.name, shell, name: name.trim() || undefined });
    onClose();
  };

  return (
    <Dialog
      title={t("terminals.newTitle")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" type="submit" form="new-terminal" disabled={(!serverShell && !chosen) || problem !== null}>
            {t("terminals.openSession")}
          </Button>
        </>
      }
    >
      <form
        id="new-terminal"
        className="space-y-3"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        {remote && (
          <div className="space-y-1">
            <span className="block text-[12.5px] font-medium text-ink-2">{t("terminals.sessionKind")}</span>
            <Segmented<SessionKind>
              label={t("terminals.sessionKind")}
              value={kind}
              onChange={setKind}
              options={[
                { value: "container", label: t("terminals.kind.container") },
                { value: "server", label: t("terminals.kind.server") },
              ]}
            />
          </div>
        )}
        {serverShell && <Banner tone="warn">{t("terminals.serverShellHint", { user: view?.config?.user ?? "", server: view?.name ?? "" })}</Banner>}
        {!serverShell && list.isPending && (
          <p role="status" className="flex items-center gap-2 text-ink-3">
            <Spinner size={14} /> {t("common.loading")}
          </p>
        )}
        {!serverShell && list.error && <Banner tone="error">{(list.error as { message: string }).message}</Banner>}
        {!serverShell && list.isSuccess && running.length === 0 && <Banner tone="info">{t("terminals.noRunning")}</Banner>}
        {!serverShell && running.length > 0 && (
          <div className="space-y-1">
            <label htmlFor={ids.container} className="text-[12.5px] font-medium text-ink-2">
              {t("terminals.containerField")}
            </label>
            <select id={ids.container} value={container} onChange={(e) => setContainer(e.target.value)} className={fieldClass}>
              {running.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name} — {c.image}
                </option>
              ))}
            </select>
          </div>
        )}
        {!serverShell && <div className="space-y-1">
          <span className="block text-[12.5px] font-medium text-ink-2">{t("terminal.shell")}</span>
          <Segmented<ShellChoice>
            label={t("terminal.shell")}
            value={shell}
            onChange={setShell}
            options={[
              { value: "auto", label: t("terminal.auto") },
              { value: "bash", label: "bash" },
              { value: "sh", label: "sh" },
            ]}
          />
        </div>}
        <div className="space-y-1">
          <label htmlFor={ids.name} className="text-[12.5px] font-medium text-ink-2">
            {t("terminals.sessionName")}
          </label>
          <input
            id={ids.name}
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={placeholder}
            spellCheck={false}
            aria-invalid={problem !== null}
            aria-describedby={problem ? ids.nameError : undefined}
            className={fieldClass}
          />
          {problem ? (
            <p id={ids.nameError} className="text-[12px] text-err">
              {t(`terminals.nameProblem.${problem}`, { max: MAX_NAME })}
            </p>
          ) : (
            <p className="text-[12px] text-ink-3">{t("terminals.nameHint", { context: serverShell ? "server" : undefined })}</p>
          )}
        </div>
      </form>
    </Dialog>
  );
}
