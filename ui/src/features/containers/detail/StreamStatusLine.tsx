import { useTranslation } from "react-i18next";
import type { StreamClosed } from "@/shared/ipc/bindings/StreamClosed";
import { cx } from "@/shared/ui/controls";
import { Spinner } from "@/shared/ui/feedback";
import type { StreamStatus } from "./useStream";

/** Estado de un flujo: abriendo, en directo o cómo terminó (texto del backend). */
export function StreamStatusLine({ status, closed, openError, live, extra }: { status: StreamStatus; closed: StreamClosed | null; openError: string | null; live: string; extra?: string }) {
  const { t } = useTranslation();
  let text: string;
  let tone: "ok" | "off" | "err" = "off";
  if (status === "opening") text = t("stream.opening");
  else if (status === "open") {
    text = live;
    tone = "ok";
  } else if (openError) {
    text = t("stream.openFailed", { message: openError });
    tone = "err";
  } else if (closed?.error) {
    text = t("stream.failed", { message: closed.error.message });
    tone = "err";
  } else if (closed?.end) text = closed.end.summary;
  else text = t("stream.idle");
  return (
    <p role="status" className={cx("flex items-center gap-2 text-[12.5px]", tone === "err" ? "text-err" : "text-ink-2")}>
      {status === "opening" ? <Spinner size={13} /> : <span aria-hidden className={cx("h-2 w-2 shrink-0 rounded-full", tone === "ok" ? "animate-pulse bg-ok" : tone === "err" ? "bg-err" : "bg-off")} />}
      <span className="selectable">{text}</span>
      {extra && <span className="ml-auto text-ink-3 tabular-nums">{extra}</span>}
    </p>
  );
}
