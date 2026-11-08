import { useCallback, useState, type ReactNode } from "react";
import { Plus, SquareTerminal, XCircle } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/shared/ui/controls";
import { Dialog } from "@/shared/ui/feedback";
import { layoutFor, pairs } from "./layout";
import { NewTerminalDialog } from "./NewTerminalDialog";
import { closeAllSessions, MAX_SESSIONS, setSplit, useTerminals, type Session } from "./sessions";
import { Splitter } from "./Splitter";
import { TerminalPane } from "./TerminalPane";

function Columns({ a, b }: { a: ReactNode; b: ReactNode }) {
  const { t } = useTranslation();
  const col = useTerminals((s) => s.split.col);
  return (
    <div className="flex min-h-0 min-w-0 flex-1">
      <div className="flex min-h-0 min-w-0 shrink-0 grow-0 flex-col" style={{ flexBasis: `calc(${col * 100}% - 4px)` }}>
        {a}
      </div>
      <Splitter orientation="vertical" value={col} onChange={(col) => setSplit({ col })} label={t("terminals.splitColumns")} />
      <div className="flex min-h-0 min-w-0 flex-1 flex-col">{b}</div>
    </div>
  );
}

function Rows({ a, b }: { a: ReactNode; b: ReactNode }) {
  const { t } = useTranslation();
  const row = useTerminals((s) => s.split.row);
  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      <div className="flex min-h-0 min-w-0 shrink-0 grow-0 flex-col" style={{ flexBasis: `calc(${row * 100}% - 4px)` }}>
        {a}
      </div>
      <Splitter orientation="horizontal" value={row} onChange={(row) => setSplit({ row })} label={t("terminals.splitRows")} />
      <div className="flex min-h-0 min-w-0 flex-1 flex-col">{b}</div>
    </div>
  );
}

export function TerminalsView() {
  const { t } = useTranslation();
  const sessions = useTerminals((s) => s.sessions);
  const active = useTerminals((s) => s.active);
  const maximized = useTerminals((s) => s.maximized);
  const [creating, setCreating] = useState(false);
  const [confirmAll, setConfirmAll] = useState(false);
  const closeCreate = useCallback(() => setCreating(false), []);
  const closeConfirm = useCallback(() => setConfirmAll(false), []);

  const visible = maximized !== null ? sessions.filter((s) => s.key === maximized) : sessions;
  const layout = layoutFor(visible.length);
  const live = sessions.filter((s) => s.status !== "ended").length;
  const full = sessions.length >= MAX_SESSIONS;

  const pane = (s: Session) => (
    <TerminalPane key={s.key} session={s} active={s.key === active} maximized={s.key === maximized} canMaximize={sessions.length > 1} />
  );
  const [a, b, c, d] = visible;

  let body: ReactNode;
  switch (layout) {
    case "empty":
      body = (
        <div className="m-auto max-w-[460px] rounded-xl border border-dashed border-line-strong bg-surface px-8 py-8 text-center">
          <SquareTerminal size={28} strokeWidth={1.5} className="mx-auto text-beacon-strong" aria-hidden />
          <p className="mt-3 text-[14px] font-medium text-ink">{t("terminals.emptyTitle")}</p>
          <p className="mt-1.5 text-[13px] text-ink-2">{t("terminals.emptyHint")}</p>
          <Button variant="primary" className="mt-4" onClick={() => setCreating(true)}>
            <Plus size={14} /> {t("terminals.new")}
          </Button>
        </div>
      );
      break;
    case "single":
      body = pane(a!);
      break;
    case "columns":
      body = <Columns a={pane(a!)} b={pane(b!)} />;
      break;
    case "three":
      body = <Columns a={pane(a!)} b={<Rows a={pane(b!)} b={pane(c!)} />} />;
      break;
    case "grid":
      body = <Rows a={<Columns a={pane(a!)} b={pane(b!)} />} b={<Columns a={pane(c!)} b={pane(d!)} />} />;
      break;
    case "scroll":
      body = (
        <div className="-mr-2 min-h-0 flex-1 space-y-2 overflow-y-auto pr-2" data-testid="terminals-scroll">
          {pairs(visible).map((row) => (
            // Cada fila ocupa media pantalla (mínimo 280 px); el contenedor desplaza.
            <div key={row.map((s) => s.key).join("-")} className="flex min-w-0" style={{ height: "max(280px, calc((100vh - 190px) / 2))" }} data-testid="terminals-row">
              {row.length === 2 ? <Columns a={pane(row[0]!)} b={pane(row[1]!)} /> : <div className="flex min-w-0 flex-1 flex-col">{pane(row[0]!)}</div>}
            </div>
          ))}
        </div>
      );
      break;
  }

  return (
    <div className="flex h-full min-h-[420px] flex-col gap-3 px-6 py-4">
      <div className="flex flex-wrap items-center gap-3">
        <Button variant="primary" onClick={() => setCreating(true)} disabled={full} title={full ? t("terminals.full", { max: MAX_SESSIONS }) : undefined}>
          <Plus size={14} /> {t("terminals.new")}
        </Button>
        {sessions.length > 0 && (
          <Button onClick={() => setConfirmAll(true)}>
            <XCircle size={14} /> {t("terminals.closeAll")}
          </Button>
        )}
        <span className="text-[12.5px] text-ink-3" aria-live="polite">
          {sessions.length > 0 ? t("terminals.count", { count: sessions.length, live }) : t("terminals.hint")}
        </span>
      </div>
      <div className="flex min-h-0 flex-1 flex-col">{body}</div>
      {creating && <NewTerminalDialog onClose={closeCreate} />}
      {confirmAll && (
        <Dialog
          title={t("terminals.closeAllTitle")}
          onClose={closeConfirm}
          footer={
            <>
              <Button onClick={closeConfirm}>{t("common.cancel")}</Button>
              <Button
                variant="danger"
                onClick={() => {
                  closeAllSessions();
                  setConfirmAll(false);
                }}
              >
                {t("terminals.closeAll")}
              </Button>
            </>
          }
        >
          <p>{t("terminals.closeAllBody", { count: sessions.length })}</p>
        </Dialog>
      )}
    </div>
  );
}
