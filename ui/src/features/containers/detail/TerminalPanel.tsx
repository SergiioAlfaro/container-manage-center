import { useEffect, useRef, useState } from "react";
import { useApi } from "@/app/server";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import { terminalTheme, trackTerminal } from "@/shared/ui/terminalTheme";
import { terminalLinkHandler } from "@/shared/ui/terminalLinks";
import "@xterm/xterm/css/xterm.css";
import { Power, TerminalSquare } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { ShellChoice } from "@/shared/ipc/bindings/ShellChoice";
import type { TerminalEvent } from "@/shared/ipc/bindings/TerminalEvent";
import { Button, Segmented } from "@/shared/ui/controls";
import { Banner } from "@/shared/ui/feedback";
import { StreamStatusLine } from "./StreamStatusLine";
import { useStream } from "./useStream";

/** base64 → bytes (la salida de la terminal llega así para no romper UTF-8 a medias). */
export function decodeBase64(data: string): Uint8Array {
  const bin = atob(data);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}


/**
 * Terminal interactiva: `docker exec -it` real. La sesión se abre solo cuando
 * el usuario lo pide; al cerrarla se termina la shell (D-037).
 */
export function TerminalPanel({ id, name, running }: { id: string; name: string; running: boolean }) {
  const api = useApi();
  const { t } = useTranslation();
  const host = useRef<HTMLDivElement>(null);
  const term = useRef<Terminal | null>(null);
  const fit = useRef<FitAddon | null>(null);
  const [shell, setShell] = useState<ShellChoice>("auto");
  const [session, setSession] = useState(0);

  // xterm vive mientras el panel está montado; las sesiones van y vienen.
  useEffect(() => {
    const el = host.current;
    if (!el) return;
    const tm = new Terminal({ fontFamily: "JetBrains Mono, monospace", fontSize: 13, cursorBlink: true, theme: terminalTheme(), scrollback: 5000, convertEol: false, linkHandler: terminalLinkHandler });
    const untrack = trackTerminal(tm);
    const fa = new FitAddon();
    tm.loadAddon(fa);
    tm.open(el);
    try {
      fa.fit();
    } catch {
      /* el contenedor aún no tiene tamaño */
    }
    term.current = tm;
    fit.current = fa;
    const ro = new ResizeObserver(() => {
      try {
        fa.fit();
      } catch {
        /* oculto */
      }
    });
    ro.observe(el);
    return () => {
      ro.disconnect();
      untrack();
      tm.dispose();
      term.current = null;
      fit.current = null;
    };
  }, []);

  const stream = useStream<TerminalEvent>(
    session > 0
      ? (h) => {
          const tm = term.current;
          tm?.reset();
          return api.terminalOpen(id, name, shell, tm?.cols ?? 80, tm?.rows ?? 24, h);
        }
      : null,
    (e) => {
      const tm = term.current;
      if (!tm) return;
      if (e.event === "output") tm.write(decodeBase64(e.data));
      else tm.write(`\r\n\x1b[2m— ${e.closed.end?.summary ?? e.closed.error?.message ?? ""} —\x1b[0m\r\n`);
    },
    `${id}|${session}`,
  );

  // Entrada y cambios de tamaño hacia la sesión abierta.
  const sid = stream.streamId;
  useEffect(() => {
    const tm = term.current;
    if (!tm || stream.status !== "open") return;
    const d1 = tm.onData((data) => {
      if (sid.current !== null) void api.terminalInput(sid.current, data).catch(() => {});
    });
    const d2 = tm.onResize(({ cols, rows }) => {
      if (sid.current !== null) void api.terminalResize(sid.current, cols, rows).catch(() => {});
    });
    // El tamaño pudo cambiar entre la apertura y ahora.
    if (sid.current !== null) void api.terminalResize(sid.current, tm.cols, tm.rows).catch(() => {});
    tm.focus();
    return () => {
      d1.dispose();
      d2.dispose();
    };
  }, [stream.status, sid]);

  const active = stream.status === "open" || stream.status === "opening";

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-3">
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
        {active ? (
          <Button onClick={stream.close} disabled={stream.status !== "open"}>
            <Power size={14} />
            {t("terminal.close")}
          </Button>
        ) : (
          <Button variant="primary" disabled={!running} onClick={() => setSession((s) => s + 1)}>
            <TerminalSquare size={14} />
            {session === 0 ? t("terminal.open") : t("terminal.reopen")}
          </Button>
        )}
        <span className="text-[12px] text-ink-3">{t("terminal.hint")}</span>
      </div>
      {!running && <Banner tone="info">{t("terminal.notRunning")}</Banner>}
      {session > 0 && <StreamStatusLine status={stream.status} closed={stream.closed} openError={stream.openError} live={t("terminal.live")} />}
      <div className="h-[calc(100vh-300px)] min-h-[260px] overflow-hidden rounded-lg border border-line bg-[#0b1219] p-2">
        <div ref={host} data-testid="terminal" className="h-full w-full" />
      </div>
    </div>
  );
}
