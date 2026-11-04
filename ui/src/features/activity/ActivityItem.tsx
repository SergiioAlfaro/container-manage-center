import { memo, useState, type ReactNode } from "react";
import { ChevronRight } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { CapturedText } from "@/shared/ipc/bindings/CapturedText";
import type { OpStatus } from "@/shared/ipc/bindings/OpStatus";
import type { OperationRecord } from "@/shared/ipc/bindings/OperationRecord";
import { CopyButton, cx } from "@/shared/ui/controls";
import { describeTransport, formatDateTime, formatDuration, formatTime, headlineCommand } from "./logic";

const STATUS_TEXT: Record<OpStatus, string> = {
  running: "text-run",
  succeeded: "text-ok",
  failed: "text-err",
  cancelled: "text-off",
};
const STATUS_BG: Record<OpStatus, string> = {
  running: "",
  succeeded: "bg-ok",
  failed: "bg-err",
  cancelled: "bg-off",
};

function Block({ label, children, action }: { label: string; children: ReactNode; action?: ReactNode }) {
  return (
    <section className="mt-3 first:mt-0">
      <div className="mb-1 flex items-center justify-between gap-2">
        <h4 className="text-[11.5px] font-medium text-ink-3">{label}</h4>
        {action}
      </div>
      {children}
    </section>
  );
}

function Code({ children, tone }: { children: ReactNode; tone?: "err" }) {
  return (
    <pre
      className={cx(
        "selectable max-h-60 overflow-auto rounded-md bg-code px-2.5 py-2 font-mono text-[12px] leading-[1.55] whitespace-pre-wrap [overflow-wrap:anywhere]",
        tone === "err" ? "text-err" : "text-ink",
      )}
    >
      {children}
    </pre>
  );
}

function Captured({ label, value, tone }: { label: string; value: CapturedText | null; tone?: "err" }) {
  const { t } = useTranslation();
  if (!value || value.text.length === 0) return null;
  return (
    <Block label={label} action={<CopyButton text={value.text} />}>
      <Code tone={tone}>{value.text}</Code>
      {value.truncated && (
        <p className="mt-1 text-[11.5px] text-warn">
          {t("activity.truncated", { shown: new TextEncoder().encode(value.text).length, total: value.originalBytes })}
        </p>
      )}
    </Block>
  );
}

export const ActivityItem = memo(function ActivityItem({ record: r }: { record: OperationRecord }) {
  const { t, i18n } = useTranslation();
  const [open, setOpen] = useState(r.status === "failed");
  const headline = headlineCommand(r);
  const isApi = r.transport.via === "api";
  const detailsId = `op-${r.id}`;

  return (
    <li className="relative border-b border-log-rule">
      <span aria-hidden className={cx("channel", r.status === "running" ? "channel-running" : STATUS_BG[r.status])} />
      <div
        role="button"
        tabIndex={0}
        aria-expanded={open}
        aria-controls={detailsId}
        onClick={() => setOpen((o) => !o)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setOpen((o) => !o);
          }
        }}
        className="block w-full cursor-default py-2.5 pr-3 pl-4 text-left hover:bg-surface/60"
      >
        <div className="flex items-center gap-2 text-[11.5px]">
          <ChevronRight size={13} className={cx("shrink-0 text-ink-3 transition-transform", open && "rotate-90")} />
          <time dateTime={r.startedAt} className="tabular-nums text-ink-3">
            {formatTime(r.startedAt, i18n.language)}
          </time>
          <span
            title={t(`activity.viaLong.${r.transport.via}`)}
            className={cx(
              "rounded-[4px] px-1.5 py-px text-[10.5px] font-semibold",
              isApi ? "bg-beacon-soft text-beacon-strong" : "bg-warn/15 text-warn",
            )}
          >
            {t(`activity.via.${r.transport.via}`)}
          </span>
          <span className="min-w-0 truncate text-ink-2">
            {t(`ops.${r.kind}`, { defaultValue: r.kind })}
            {r.target && <span className="text-ink-3"> {r.target}</span>}
          </span>
          <span className="ml-auto flex shrink-0 items-center gap-2">
            {r.durationMs !== null && <span className="tabular-nums text-ink-3">{formatDuration(r.durationMs)}</span>}
            <span className={cx("font-medium", STATUS_TEXT[r.status])}>{t(`activity.status.${r.status}`)}</span>
          </span>
        </div>

        <div className="mt-1.5 flex items-start gap-2 pl-[21px]">
          {headline ? (
            <code className="selectable min-w-0 flex-1 truncate font-mono text-[12.5px] text-ink" title={headline}>
              <span className="text-ink-3 select-none">$ </span>
              {headline}
            </code>
          ) : (
            <span className="flex-1 text-[12px] text-ink-3 italic">{t("activity.noEquivalent")}</span>
          )}
          {headline && <CopyButton text={headline} compact />}
        </div>
        {r.status === "failed" && r.error && !open && (
          <p className="mt-1 truncate pl-[21px] text-[12px] text-err">{r.error}</p>
        )}
      </div>

      {open && (
        <div id={detailsId} className="border-t border-log-rule bg-surface/50 px-4 py-3 pl-[37px] text-[12.5px]">
          <Block label={isApi ? t("activity.executed") : t("activity.executedCommand")} action={<CopyButton text={describeTransport(r.transport)} />}>
            <Code>{describeTransport(r.transport)}</Code>
            <p className="mt-1 text-[11.5px] text-ink-3">
              {t(`activity.viaLong.${r.transport.via}`)}
              {r.transport.via === "api" && (
                <>
                  {", "}
                  {t("activity.endpoint").toLowerCase()} <span className="font-mono">{r.transport.endpoint}</span>
                </>
              )}
            </p>
          </Block>

          {isApi && (
            <Block label={t("activity.equivalent")} action={r.equivalentCommand ? <CopyButton text={r.equivalentCommand} /> : undefined}>
              {r.equivalentCommand ? (
                <>
                  <Code>{r.equivalentCommand}</Code>
                  <p className="mt-1 text-[11.5px] text-ink-3">{t("activity.equivalentNote")}</p>
                </>
              ) : (
                <p className="text-ink-3">{t("activity.noEquivalent")}</p>
              )}
            </Block>
          )}

          <Captured label={t("activity.requestBody")} value={r.requestBody} />

          {r.error && (
            <Block label={t("activity.error")}>
              <Code tone="err">{r.error}</Code>
            </Block>
          )}
          {r.summary && (
            <Block label={t("activity.summary")}>
              <p className="selectable text-ink">{r.summary}</p>
            </Block>
          )}
          <Captured label={t("activity.stdout")} value={r.stdout} />
          <Captured label={t("activity.stderr")} value={r.stderr} tone="err" />

          <dl className="mt-3 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-[12px]">
            <dt className="text-ink-3">{t("activity.started")}</dt>
            <dd className="selectable tabular-nums">{formatDateTime(r.startedAt, i18n.language)}</dd>
            {r.finishedAt && (
              <>
                <dt className="text-ink-3">{t("activity.finished")}</dt>
                <dd className="selectable tabular-nums">{formatDateTime(r.finishedAt, i18n.language)}</dd>
              </>
            )}
            {r.durationMs !== null && (
              <>
                <dt className="text-ink-3">{t("activity.duration")}</dt>
                <dd className="tabular-nums">{formatDuration(r.durationMs)}</dd>
              </>
            )}
            {r.httpStatus !== null && (
              <>
                <dt className="text-ink-3">{t("activity.httpStatus")}</dt>
                <dd className="tabular-nums">{r.httpStatus}</dd>
              </>
            )}
            {r.exitCode !== null && (
              <>
                <dt className="text-ink-3">{t("activity.exitCode")}</dt>
                <dd className="tabular-nums">{r.exitCode}</dd>
              </>
            )}
          </dl>
        </div>
      )}
    </li>
  );
});
