import { useEffect, useRef } from "react";
import { CheckCircle2, CircleStop, OctagonX, Square, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { CopyButton } from "@/shared/ui/controls";
import { Button } from "@/shared/ui/controls";
import { Banner, Spinner } from "@/shared/ui/feedback";
import { runKey, useComposeRuns, type Run } from "./runs";
import { useServerId } from "@/app/server";

export function RunStatusLine({ run }: { run: Run }) {
  const { t } = useTranslation();
  const verb = t(`composeOps.verb.${run.action.action}`);
  switch (run.status) {
    case "running":
      return (
        <span className="inline-flex items-center gap-1.5 text-[12.5px] text-ink-2">
          <Spinner size={13} /> {t("composeOps.running", { verb })}
        </span>
      );
    case "succeeded":
      return (
        <span className="inline-flex items-center gap-1.5 text-[12.5px] text-ok">
          <CheckCircle2 size={14} /> {t(`composeOps.succeeded.${run.action.action}`)}
        </span>
      );
    case "stopped":
      return (
        <span className="inline-flex items-center gap-1.5 text-[12.5px] text-warn">
          <CircleStop size={14} /> {t(`composeOps.stopped.${run.action.action}`)}
        </span>
      );
    default:
      return (
        <span className="inline-flex items-center gap-1.5 text-[12.5px] text-err">
          <OctagonX size={14} /> {run.status === "failed" ? t("composeOps.failed", { verb, code: run.closed?.end?.end === "exited" ? run.closed.end.code : "?" }) : t("composeOps.error", { verb })}
        </span>
      );
  }
}

/** Salida en vivo de la última operación de un proyecto. */
export function RunPanel({ project }: { project: string }) {
  const { t } = useTranslation();
  const server = useServerId();
  const key = runKey(server, project);
  const run = useComposeRuns((s) => s.runs[key]);
  const stop = useComposeRuns((s) => s.stop);
  const dismiss = useComposeRuns((s) => s.dismiss);
  const box = useRef<HTMLPreElement>(null);
  const follow = useRef(true);
  const count = run?.lines.length ?? 0;
  useEffect(() => {
    const el = box.current;
    if (el && follow.current) el.scrollTop = el.scrollHeight;
  }, [count]);

  if (!run) return <p className="text-[13px] text-ink-3">{t("composeOps.none")}</p>;
  const err = run.error ?? run.closed?.error?.message ?? null;
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      <div className="flex flex-wrap items-center gap-2">
        <RunStatusLine run={run} />
        <div className="ml-auto flex items-center gap-1.5">
          {run.status === "running" && (
            <Button size="sm" onClick={() => stop(key)} disabled={run.stream === null}>
              <Square size={13} />
              {t("composeOps.stop")}
            </Button>
          )}
          {run.status !== "running" && (
            <Button size="sm" variant="ghost" onClick={() => dismiss(key)}>
              <X size={13} />
              {t("composeOps.dismiss")}
            </Button>
          )}
        </div>
      </div>
      {run.command && (
        <div className="flex items-start gap-2 rounded-md bg-code px-2.5 py-1.5">
          <code data-testid="compose-run-command" className="selectable flex-1 font-mono text-[11.5px] [overflow-wrap:anywhere]">
            {run.command}
          </code>
          <CopyButton text={run.command} compact />
        </div>
      )}
      {err && <Banner tone="error">{err}</Banner>}
      {run.status === "stopped" && <Banner tone="warn">{t("composeOps.stoppedNote")}</Banner>}
      {run.dropped > 0 && <p className="text-[12px] text-ink-3">{t("composeOps.dropped", { count: run.dropped })}</p>}
      <pre
        ref={box}
        data-testid="compose-run-output"
        onScroll={(e) => {
          const el = e.currentTarget;
          follow.current = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
        }}
        className="selectable min-h-[260px] flex-1 overflow-auto rounded-lg border border-line bg-log p-3 font-mono text-[12px] leading-[1.5] whitespace-pre-wrap [overflow-wrap:anywhere]"
      >
        {run.lines.length === 0 ? <span className="text-ink-3">{run.status === "running" ? t("composeOps.waiting") : t("composeOps.noOutput")}</span> : run.lines.map((l, i) => (
          <span key={i} className={l.stream === "stderr" ? "block text-ink-2" : "block"}>
            {l.text}
          </span>
        ))}
      </pre>
    </div>
  );
}
