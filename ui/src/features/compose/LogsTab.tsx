import { useEffect, useRef, useState } from "react";
import { useApi } from "@/app/server";
import { Play, Square, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { OutputLine } from "@/shared/ipc/bindings/OutputLine";
import type { ProjectRef } from "@/shared/ipc/bindings/ProjectRef";
import type { RunClosed } from "@/shared/ipc/bindings/RunClosed";
import { Button, Segmented } from "@/shared/ui/controls";
import { Banner, Checkbox, Spinner } from "@/shared/ui/feedback";

const MAX = 5000;
const TAILS = ["100", "500", "2000"] as const;

type Status = "idle" | "opening" | "open" | "ended";

/** `docker compose logs --follow` mientras la pestaña está abierta y siguiendo. */
export function LogsTab({ project, services, initial }: { project: ProjectRef; services: string[]; initial?: string[] }) {
  const api = useApi();
  const { t } = useTranslation();
  const [selected, setSelected] = useState<string[]>(initial ?? []);
  const [tail, setTail] = useState<(typeof TAILS)[number]>("100");
  const [timestamps, setTimestamps] = useState(false);
  const [following, setFollowing] = useState(true);
  const [lines, setLines] = useState<OutputLine[]>([]);
  const [status, setStatus] = useState<Status>("idle");
  const [closed, setClosed] = useState<RunClosed | null>(null);
  const [openError, setOpenError] = useState<string | null>(null);
  const box = useRef<HTMLPreElement>(null);
  const stick = useRef(true);
  const key = `${project.name}|${selected.join(",")}|${tail}|${timestamps}|${following}`;

  useEffect(() => {
    if (!following) {
      setStatus("idle");
      return;
    }
    let disposed = false;
    let sid: number | null = null;
    let ended = false;
    setLines([]);
    setClosed(null);
    setOpenError(null);
    setStatus("opening");
    api
      .composeLogsOpen(project, selected, Number(tail), timestamps, (e) => {
        if (disposed) return;
        if (e.event === "lines") {
          setLines((prev) => {
            const next = prev.concat(e.lines);
            return next.length > MAX ? next.slice(next.length - MAX) : next;
          });
        } else {
          ended = true;
          setClosed(e.closed);
          setStatus("ended");
        }
      })
      .then((id) => {
        if (disposed) {
          void api.streamClose(id).catch(() => {});
          return;
        }
        sid = id;
        if (!ended) setStatus("open");
      })
      .catch((e) => {
        if (disposed) return;
        setOpenError((e as { message: string }).message);
        setStatus("ended");
      });
    return () => {
      disposed = true;
      if (sid !== null) void api.streamClose(sid).catch(() => {});
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  useEffect(() => {
    const el = box.current;
    if (el && stick.current) el.scrollTop = el.scrollHeight;
  }, [lines.length]);

  const toggle = (s: string) => setSelected((cur) => (cur.includes(s) ? cur.filter((x) => x !== s) : [...cur, s]));
  const err = openError ?? closed?.error?.message ?? null;

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-3">
        <div className="flex flex-wrap items-center gap-1.5" role="group" aria-label={t("composeLogs.services")}>
          <button type="button" aria-pressed={selected.length === 0} onClick={() => setSelected([])} className={`h-7 rounded-md border px-2 text-[12px] ${selected.length === 0 ? "border-beacon bg-beacon/10" : "border-line text-ink-2"}`}>
            {t("composeLogs.all")}
          </button>
          {services.map((s) => (
            <button key={s} type="button" aria-pressed={selected.includes(s)} onClick={() => toggle(s)} className={`h-7 rounded-md border px-2 font-mono text-[12px] ${selected.includes(s) ? "border-beacon bg-beacon/10" : "border-line text-ink-2"}`}>
              {s}
            </button>
          ))}
        </div>
        <Segmented<(typeof TAILS)[number]> label={t("composeLogs.tail")} value={tail} onChange={setTail} options={TAILS.map((n) => ({ value: n, label: t("composeLogs.tailN", { count: Number(n) }) }))} />
        <Checkbox checked={timestamps} onChange={setTimestamps} label={t("composeLogs.timestamps")} />
        <div className="ml-auto flex items-center gap-1.5">
          {following ? (
            <Button size="sm" onClick={() => setFollowing(false)}>
              <Square size={13} />
              {t("composeLogs.pause")}
            </Button>
          ) : (
            <Button size="sm" onClick={() => setFollowing(true)}>
              <Play size={13} />
              {t("composeLogs.follow")}
            </Button>
          )}
          <Button size="sm" variant="ghost" onClick={() => setLines([])} aria-label={t("composeLogs.clear")} title={t("composeLogs.clear")}>
            <Trash2 size={13} />
          </Button>
        </div>
      </div>
      <p className="flex items-center gap-1.5 text-[12px] text-ink-3" data-testid="compose-logs-status">
        {status === "opening" && <Spinner size={12} />}
        {status === "open" ? t("composeLogs.live") : status === "opening" ? t("composeLogs.opening") : status === "idle" ? t("composeLogs.paused") : closed?.end?.end === "exited" ? t("composeLogs.ended") : t("composeLogs.closed")}
      </p>
      {err && <Banner tone="error">{err}</Banner>}
      <pre
        ref={box}
        data-testid="compose-logs"
        onScroll={(e) => {
          const el = e.currentTarget;
          stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
        }}
        className="selectable min-h-[360px] flex-1 overflow-auto rounded-lg border border-line bg-log p-3 font-mono text-[12px] leading-[1.5] whitespace-pre-wrap [overflow-wrap:anywhere]"
      >
        {lines.length === 0 ? <span className="text-ink-3">{t("composeLogs.empty")}</span> : lines.map((l, i) => (
          <span key={i} className={l.stream === "stderr" ? "block text-ink-2" : "block"}>
            {l.text}
          </span>
        ))}
      </pre>
    </div>
  );
}
