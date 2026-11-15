// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useEffect, useRef, useState } from "react";
import { Box, Maximize2, Minimize2, Pencil, RotateCcw, SquareTerminal, X, Server as ServerIcon } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cx } from "@/shared/ui/controls";
import { attachSession, closeSession, fitSession, focusSession, nameProblem, renameSession, reopenSession, setActive, toggleMaximize, MAX_NAME, type Session } from "./sessions";

/** Botón solo con icono: siempre con nombre accesible y descripción emergente. */
function IconButton({ label, onClick, children, disabled, pressed }: { label: string; onClick: () => void; children: React.ReactNode; disabled?: boolean; pressed?: boolean }) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      aria-pressed={pressed}
      disabled={disabled}
      onClick={onClick}
      className="grid h-7 w-7 place-items-center rounded-md text-[#9fb0c2] transition-colors duration-150 hover:bg-white/10 hover:text-white active:bg-white/15 disabled:pointer-events-none disabled:opacity-40"
    >
      {children}
    </button>
  );
}

function Indicator({ session }: { session: Session }) {
  const { t } = useTranslation();
  const failed = session.status === "ended" && session.error !== null;
  const state = session.status === "opening" ? "opening" : session.status === "open" ? "open" : failed ? "error" : "ended";
  return (
    <span className="inline-flex shrink-0 items-center gap-1.5 text-[11.5px] text-[#9fb0c2]" data-testid="session-state">
      <span
        aria-hidden
        className={cx(
          "h-2 w-2 rounded-full",
          state === "opening" && "bg-warn motion-safe:animate-pulse",
          state === "open" && "bg-run",
          state === "ended" && "bg-[#6f8399]",
          state === "error" && "bg-err",
        )}
      />
      {t(`terminals.state.${state}`)}
    </span>
  );
}

export function TerminalPane({ session, active, maximized, canMaximize }: { session: Session; active: boolean; maximized: boolean; canMaximize: boolean }) {
  const { t } = useTranslation();
  const host = useRef<HTMLDivElement>(null);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(session.name);
  const key = session.key;

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    const detach = attachSession(key, el);
    const ro = new ResizeObserver(() => fitSession(key));
    ro.observe(el);
    if (active) focusSession(key);
    return () => {
      ro.disconnect();
      detach();
    };
    // `active` solo importa al montar (enfocar la sesión nueva o la maximizada).
  }, [key]);

  const problem = editing ? nameProblem(draft) : null;
  const commit = () => {
    if (renameSession(key, draft)) setEditing(false);
  };
  const cancel = () => {
    setDraft(session.name);
    setEditing(false);
  };

  const after = session.after ? t(`terminals.after.${session.after}`) : null;

  return (
    <section
      aria-label={
        session.kind === "server"
          ? t("terminals.serverPaneLabel", { name: session.name, server: session.serverName })
          : t("terminals.paneLabel", { name: session.name, container: session.containerName, server: session.remote ? session.serverName : t("servers.local") })
      }
      data-testid="terminal-pane"
      onMouseDown={() => setActive(key)}
      onFocusCapture={() => setActive(key)}
      className={cx(
        "flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-lg border bg-[#0b1219] transition-[border-color,box-shadow] duration-150",
        active ? "border-beacon shadow-[0_0_0_1px_var(--color-beacon)]" : "border-line",
      )}
    >
      <header className="flex h-9 shrink-0 items-center gap-2 border-b border-white/10 px-2 text-[12.5px]">
        <Indicator session={session} />
        <span
          data-testid="terminal-server"
          className={cx(
            "flex max-w-[30%] shrink-0 items-center gap-1 rounded px-1.5 text-[11.5px] leading-[18px]",
            session.remote ? "bg-beacon/25 text-[#e9dcff]" : "bg-white/10 text-[#b8c6d4]",
          )}
          title={t("terminals.server", { name: session.remote ? session.serverName : t("servers.local") })}
        >
          <ServerIcon size={12} aria-hidden className="shrink-0" />
          <span className="truncate">{session.remote ? session.serverName : t("servers.local")}</span>
        </span>
        {session.kind === "server" ? (
          <span className="flex min-w-0 items-center gap-1 text-[12px] font-semibold text-[#ffd79a]" title={t("terminals.serverShell")}>
            <SquareTerminal size={13} aria-hidden className="shrink-0" />
            <span className="truncate">{t("terminals.serverShell")}</span>
          </span>
        ) : (
          <span className="flex min-w-0 items-center gap-1 font-mono text-[12px] text-[#d5dee7]" title={t("terminals.container", { name: session.containerName })}>
            <Box size={13} aria-hidden className="shrink-0 text-[#6f8399]" />
            <span className="truncate">{session.containerName}</span>
          </span>
        )}
        <span aria-hidden className="text-[#6f8399]">·</span>
        {editing ? (
          <form
            className="flex min-w-0 flex-1 items-center"
            onSubmit={(e) => {
              e.preventDefault();
              commit();
            }}
          >
            <input
              autoFocus
              value={draft}
              maxLength={MAX_NAME * 2}
              aria-label={t("terminals.sessionName")}
              aria-invalid={problem !== null}
              title={problem ? t(`terminals.nameProblem.${problem}`, { max: MAX_NAME }) : undefined}
              onChange={(e) => setDraft(e.target.value)}
              onBlur={() => (nameProblem(draft) === null ? commit() : cancel())}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  e.preventDefault();
                  e.stopPropagation();
                  cancel();
                }
              }}
              className={cx(
                "h-6 min-w-0 flex-1 rounded border bg-[#121c26] px-1.5 text-[12.5px] text-white outline-none",
                problem ? "border-err" : "border-white/20 focus:border-beacon",
              )}
            />
          </form>
        ) : (
          <span
            className="min-w-0 flex-1 truncate font-medium text-white"
            onDoubleClick={() => {
              setDraft(session.name);
              setEditing(true);
            }}
          >
            {session.name}
          </span>
        )}
        {after && <span className="shrink-0 rounded bg-warn/20 px-1.5 text-[11px] text-[#f5d38a]">{after}</span>}
        <div className="ml-auto flex shrink-0 items-center gap-0.5">
          <IconButton
            label={t("terminals.rename")}
            onClick={() => {
              setDraft(session.name);
              setEditing(true);
            }}
          >
            <Pencil size={13} />
          </IconButton>
          {session.status === "ended" && (
            <IconButton label={session.after === "removed" ? t("terminals.reopenGone") : t("terminals.reopen")} disabled={session.after === "removed"} onClick={() => reopenSession(key)}>
              <RotateCcw size={13} />
            </IconButton>
          )}
          {canMaximize && (
            <IconButton label={maximized ? t("terminals.restore") : t("terminals.maximize")} pressed={maximized} onClick={() => toggleMaximize(key)}>
              {maximized ? <Minimize2 size={13} /> : <Maximize2 size={13} />}
            </IconButton>
          )}
          <IconButton label={t("terminals.close")} onClick={() => closeSession(key)}>
            <X size={14} />
          </IconButton>
        </div>
      </header>
      {session.status === "ended" && (session.error || after) && (
        <p role="status" className="shrink-0 border-b border-white/10 px-3 py-1.5 text-[12px] text-[#f0b4ae] [overflow-wrap:anywhere]">
          {session.error ?? session.summary}
        </p>
      )}
      <div className="min-h-0 flex-1 p-1.5">
        <div ref={host} data-testid="terminal" className="h-full w-full" onClick={() => focusSession(key)} />
      </div>
    </section>
  );
}
