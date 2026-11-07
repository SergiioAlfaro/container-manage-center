import { useTranslation } from "react-i18next";
import type { ComposeProject } from "@/shared/ipc/bindings/ComposeProject";
import { cx } from "@/shared/ui/controls";

export function StateBadge({ p }: { p: Pick<ComposeProject, "state" | "running" | "total"> }) {
  const { t } = useTranslation();
  const tone = { running: "bg-ok/12 text-ok", partial: "bg-warn/12 text-warn", stopped: "bg-surface-sunk text-ink-2", notDeployed: "bg-surface-sunk text-ink-3" }[p.state];
  return (
    <span className={cx("inline-flex h-5 items-center rounded px-1.5 text-[11.5px] font-medium whitespace-nowrap tabular-nums", tone)}>
      {t(`composeList.state.${p.state}`, { running: p.running, total: p.total })}
    </span>
  );
}
