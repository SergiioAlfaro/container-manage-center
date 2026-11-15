// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useTranslation } from "react-i18next";
import { cx } from "@/shared/ui/controls";
import { Spinner } from "@/shared/ui/feedback";
import { useEngine } from "./store";

/** Indicador de conexión de la barra superior; lleva al Dashboard (donde está el diagnóstico). */
export function ConnectionBadge({ onOpen }: { onOpen: () => void }) {
  const { t } = useTranslation();
  const state = useEngine((s) => s.state);

  let content;
  let title: string;
  if (state === null || state.state === "connecting") {
    title = t("conn.connecting");
    content = (
      <>
        <Spinner size={12} className="text-ink-3" />
        <span>{t("conn.connecting")}</span>
      </>
    );
  } else if (state.state === "connected") {
    title = t("conn.badgeTitle", { path: state.endpoint.socketPath });
    content = (
      <>
        <span aria-hidden className="h-2 w-2 rounded-full bg-ok shadow-[0_0_0_3px_color-mix(in_srgb,var(--signal-ok)_20%,transparent)]" />
        <span className="tabular-nums">{t("conn.connected", { version: state.engine.version })}</span>
        {state.engine.rootless && <span className="rounded bg-beacon-soft px-1 text-[10.5px] font-semibold text-beacon-strong">{t("conn.rootless")}</span>}
      </>
    );
  } else if (state.state === "idle") {
    title = t("conn.openDashboard");
    content = (
      <>
        <span aria-hidden className="h-2 w-2 rounded-full bg-ink-3/60" />
        <span>{t("servers.status.disconnected")}</span>
      </>
    );
  } else {
    title = t("conn.openDashboard");
    content = (
      <>
        <span aria-hidden className="h-2 w-2 rounded-full bg-err" />
        <span>{t("conn.disconnected")}</span>
      </>
    );
  }

  return (
    <button
      type="button"
      onClick={onOpen}
      title={title}
      aria-label={title}
      className={cx(
        "inline-flex h-8 shrink-0 items-center gap-2 rounded-md border px-2.5 text-[12.5px] whitespace-nowrap transition-colors",
        state?.state === "disconnected" ? "border-err/35 bg-err/8 text-err hover:bg-err/12" : "border-line bg-surface text-ink-2 hover:border-line-strong hover:text-ink",
      )}
    >
      {content}
    </button>
  );
}
