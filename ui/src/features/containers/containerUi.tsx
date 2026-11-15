// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Sergio Alfaro Lorite
import { useState } from "react";
import { Pause, Play, RotateCw, Square } from "lucide-react";
import { useTranslation } from "react-i18next";
import { shortId } from "@/shared/format";
import type { ContainerAction } from "@/shared/ipc/bindings/ContainerAction";
import type { ContainerSummary } from "@/shared/ipc/bindings/ContainerSummary";
import { Button } from "@/shared/ui/controls";
import { Banner, Checkbox, Dialog } from "@/shared/ui/feedback";

export const STATE_DOT: Record<string, string> = {
  running: "bg-ok",
  paused: "bg-warn",
  restarting: "bg-run",
  created: "bg-line-strong",
  exited: "bg-off",
  dead: "bg-err",
  removing: "bg-err",
};

/** Acciones válidas según el estado que informa Docker. */
export function allowedActions(state: string): ContainerAction[] {
  switch (state) {
    case "running":
      return ["stop", "restart", "pause"];
    case "paused":
      return ["unpause", "stop"];
    case "restarting":
      return ["stop"];
    case "created":
    case "exited":
    case "dead":
      return ["start"];
    default:
      return [];
  }
}

export const ICONS: Record<ContainerAction, typeof Play> = { start: Play, stop: Square, restart: RotateCw, pause: Pause, unpause: Play };

export function RemoveDialog({ c, onClose, onConfirm }: { c: Pick<ContainerSummary, "id" | "name" | "image" | "state">; onClose: () => void; onConfirm: (force: boolean, volumes: boolean) => void }) {
  const { t } = useTranslation();
  const active = c.state === "running" || c.state === "paused" || c.state === "restarting";
  const [force, setForce] = useState(false);
  const [volumes, setVolumes] = useState(false);
  return (
    <Dialog
      title={t("containers.removeTitle", { name: c.name })}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("containers.cancel")}</Button>
          <Button variant="danger" disabled={active && !force} onClick={() => onConfirm(force, volumes)}>
            {t("containers.confirmRemove")}
          </Button>
        </>
      }
    >
      <p>{t("containers.removeBody")}</p>
      <p className="selectable flex gap-3 font-mono text-[12px] text-ink-3">
        <span>{c.image}</span>
        <span>{shortId(c.id)}</span>
      </p>
      {active && (
        <>
          <Banner tone="warn">{t("containers.removeRunning", { state: t(`containers.state.${c.state}`).toLowerCase() })}</Banner>
          <Checkbox checked={force} onChange={setForce} label={t("containers.removeForce")} />
        </>
      )}
      <Checkbox checked={volumes} onChange={setVolumes} label={t("containers.removeVolumes")} hint={t("containers.removeVolumesHint")} />
    </Dialog>
  );
}

